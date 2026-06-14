//! `cargo pmcp workbook compile` — compile a governed workbook into a gated bundle.
//!
//! The BA's primary verb (WBCL-01). It orchestrates the Phase-93
//! `pmcp-workbook-compiler` library through TWO lanes — and reimplements NO
//! compiler logic (thin-shell invariant): every gate / corpus / promotion verb is
//! a library call.
//!
//! ## The two lanes (D-12 / D-07)
//!
//! - **Seed lane** (FIRST version, no prior accepted baseline): run
//!   [`compile_workbook`] — ingest → lint → synth → compile → reconcile → write the
//!   seven-member bundle into `{workflow}@{version}/`. The gate is N/A on the seed
//!   lane (there is nothing to regress against).
//! - **Gated-update lane** (a prior baseline EXISTS): build the candidate WITHOUT
//!   writing via [`prepare_candidate`], derive the prior-baseline corpus golden via
//!   [`derive_corpus`], grade each case via [`gate`]. On a BLOCK, print
//!   [`GateBlock::render`] verbatim and signal the DISTINCT gate-block exit code via
//!   [`super::WorkbookExit::gate_block`] — writing NOTHING (gate before write). On a
//!   pass, [`promote`] the new version. The `--accept` flow (D-07) records a
//!   fingerprint-bound approval via [`accept`] then promotes.
//!
//! ## Version + approver provenance (D-02 / D-06 / D-11)
//!
//! The bundle version comes SOLELY from the workbook via [`read_workbook_version`]
//! (there is NO `--version` flag, never from `pmcp.toml`). The approver comes SOLELY
//! from the MANDATORY `--approver` flag (there is no git-identity fallback).
//!
//! ## Targets (D-03 / D-05 / WBCL-04)
//!
//! A bare PATH compiles that one workbook (its `--workflow` is required); a
//! bundle-id resolves path/out_dir/workflow from `pmcp.toml`; NO argument compiles
//! ALL declared workbooks (compile-all, continue-on-error, worst-status-wins).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Args;

use pmcp_workbook_compiler::change_class::classify;
use pmcp_workbook_compiler::gate::accept::{accept, promote, EmitLane, PromoteInputs};
use pmcp_workbook_compiler::gate::corpus::{derive_corpus, ApprovalCase};
use pmcp_workbook_compiler::gate::governed_artifact::read_approvals;
use pmcp_workbook_compiler::gate::{gate, GateDecision};
use pmcp_workbook_compiler::sheet_ir::{build_dag, Cell};
use pmcp_workbook_compiler::{
    compile_workbook, prepare_candidate, read_workbook_version, BundleLock, Candidate, ChangeClass,
    Dag, Manifest, VersionChangelog,
};

use super::targets::{resolve_targets, run_lint_phase, Target};
use super::{GlobalFlags, EXIT_ERROR, EXIT_GATE_BLOCK, EXIT_OK};
use crate::commands::configure::workspace::find_workspace_root;

/// Arguments for `cargo pmcp workbook compile`.
#[derive(Debug, Args)]
pub struct CompileArgs {
    /// A bare workbook PATH, a `pmcp.toml` bundle-id, or NOTHING (compile-all).
    ///
    /// - `Some(path-to-a-file)` → compile that workbook (`--workflow` required).
    /// - `Some(bundle-id)` → resolve path/out_dir/workflow from `pmcp.toml`.
    /// - `None` → compile every workbook declared in `pmcp.toml` (D-05).
    pub bundle_id_or_path: Option<String>,

    /// The workflow / bundle name (REQUIRED for a bare PATH; a bundle-id supplies
    /// its own workflow from `pmcp.toml`). NEVER a hardcoded literal (WBCO-02).
    #[arg(long)]
    pub workflow: Option<String>,

    /// The human approver recorded in the manifest sign-off / `ApprovalRecord`.
    ///
    /// MANDATORY (D-06): there is NO git-identity fallback — `--accept` cannot
    /// record an approval without an explicit approver.
    #[arg(long, required = true)]
    pub approver: String,

    /// Record a fingerprint-bound approval that re-baselines an over-tolerance
    /// gated-update delta, then promote (D-07). REQUIRES `--effective-date`.
    #[arg(long)]
    pub accept: bool,

    /// The effective date (`YYYY-MM-DD`) the `--accept` re-baseline takes effect
    /// (D-07). Required whenever `--accept` is set.
    #[arg(long)]
    pub effective_date: Option<String>,

    /// Override the `pmcp.toml`-declared (or cwd-relative) output directory.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Output format: `text` (default) or `json` (D-09).
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Execute `cargo pmcp workbook compile`.
///
/// Resolves the target set (bare path / bundle-id / compile-all), compiles each
/// (continue-on-error), and reduces to the WORST per-workbook status
/// (`EXIT_GATE_BLOCK` > `EXIT_ERROR` > `EXIT_OK`), surfacing a gate block via the
/// DISTINCT [`super::WorkbookExit`] transport (D-10).
///
/// # Errors
/// - `EXIT_ERROR` (`anyhow::bail!`) when the worst per-workbook status is a
///   compile/lint error.
/// - A [`super::WorkbookExit`]-carrying error (downcast by `main.rs` to
///   `EXIT_GATE_BLOCK`) when the worst status is a governance gate block.
/// - A configuration / resolution error (missing `pmcp.toml`, unknown bundle-id, a
///   bare path with no `--workflow`, a `--out`/`--accept` flag misuse).
pub fn execute(args: CompileArgs, gf: &GlobalFlags) -> Result<()> {
    // `--accept` REQUIRES `--effective-date` (D-07 pairing) — enforced before any
    // filesystem work so a misuse fails loud and writes nothing.
    if args.accept && args.effective_date.is_none() {
        bail!("--accept requires --effective-date <YYYY-MM-DD>");
    }

    let project_root = find_workspace_root().unwrap_or_else(|_| PathBuf::from("."));
    let targets = resolve_targets(
        args.bundle_id_or_path.as_deref(),
        args.workflow.as_deref(),
        args.out.as_deref(),
        &project_root,
    )?;
    if targets.is_empty() {
        bail!("no workbook to compile: pass a path/bundle-id or declare workbooks in pmcp.toml");
    }

    let not_quiet = gf.should_output() && std::env::var("PMCP_QUIET").is_err();

    // Compile-all is CONTINUE-ON-ERROR (concern I): one workbook's failure never
    // aborts the rest; each per-workbook outcome reduces to worst-status-wins.
    let mut worst = EXIT_OK;
    for target in &targets {
        let code = match compile_one(target, &args, not_quiet) {
            Ok(code) => code,
            Err(e) => {
                eprintln!(
                    "error: {} ({}): {e:#}",
                    target.workflow,
                    target.path.display()
                );
                EXIT_ERROR
            },
        };
        worst = worst_status(worst, code);
    }

    surface_worst(worst)
}

/// Reduce the running worst status with one more per-workbook `code`:
/// `EXIT_GATE_BLOCK` > `EXIT_ERROR` > `EXIT_OK`.
fn worst_status(running: i32, code: i32) -> i32 {
    fn rank(code: i32) -> u8 {
        match code {
            EXIT_GATE_BLOCK => 2,
            EXIT_ERROR => 1,
            _ => 0,
        }
    }
    if rank(code) > rank(running) {
        code
    } else {
        running
    }
}

/// Surface the reduced worst status to the shell: a gate block uses the DISTINCT
/// [`super::WorkbookExit`] transport (so `main.rs` exits `2`), an error `bail!`s
/// (anyhow's default `1`), and `EXIT_OK` is `Ok(())`.
fn surface_worst(worst: i32) -> Result<()> {
    match worst {
        EXIT_GATE_BLOCK => Err(anyhow::Error::new(super::WorkbookExit::gate_block(
            "workbook compile blocked by the governance gate",
        ))),
        EXIT_ERROR => bail!("workbook compile failed"),
        _ => Ok(()),
    }
}

/// Compile ONE target, returning its per-workbook exit code. Runs the lint phase
/// first (an error short-circuits before any compile), reads the workbook-declared
/// version, then selects the lane: SEED (no prior baseline) or GATED UPDATE (a
/// prior baseline exists).
fn compile_one(target: &Target, args: &CompileArgs, not_quiet: bool) -> Result<i32> {
    if let Some(code) = run_lint_phase(target, &args.format, not_quiet)? {
        return Ok(code);
    }

    let version = read_workbook_version(&target.path).with_context(|| {
        format!(
            "reading the declared version from {}",
            target.path.display()
        )
    })?;

    match find_prior_baseline(&target.out_root, &target.workflow)? {
        None => run_seed_lane(target, &version, &args.approver, not_quiet),
        Some(prior) => run_gated_lane(target, &version, &prior, args, not_quiet),
    }
}

/// SEED LANE (first version, NO gate — D-12): [`compile_workbook`] writes the
/// seven-member bundle into `{workflow}@{version}/`, with the version read from the
/// workbook. Returns `EXIT_OK` on success.
fn run_seed_lane(target: &Target, version: &str, approver: &str, not_quiet: bool) -> Result<i32> {
    let lock = compile_workbook(
        &target.path,
        &target.out_root,
        &target.workflow,
        version,
        approver,
    )
    .map_err(|e| anyhow::anyhow!("seed compile of {} failed: {e}", target.workflow))?;
    if not_quiet {
        eprintln!(
            "ok: seeded {}@{} (no prior baseline — gate N/A)",
            lock.bundle_id, lock.version
        );
    }
    Ok(EXIT_OK)
}

/// A loaded prior accepted baseline: its compiled IR/DAG (for corpus derivation +
/// change-classification), its manifest (for change-classification), its combined
/// bundle hash (the gate's `prev_bundle_hash`), and its version (the
/// `GatedUpdate` lane's `prior_version`).
struct PriorBaseline {
    ir: HashMap<String, Cell>,
    dag: Dag,
    manifest: Manifest,
    prev_bundle_hash: String,
    version: String,
}

/// Probe `out_root` for the most-recent prior `{workflow}@{version}/` baseline. A
/// missing out-root or no matching dir means NO prior baseline (the seed lane).
///
/// Reads the prior bundle's `BUNDLE.lock` (the `combined` hash anchors the gate) +
/// `executable.ir.json` + `manifest.json` as plain JSON — no served bundle loader is
/// needed for the build-time gate inputs.
fn find_prior_baseline(out_root: &Path, workflow: &str) -> Result<Option<PriorBaseline>> {
    let prefix = format!("{workflow}@");
    let read = match std::fs::read_dir(out_root) {
        Ok(rd) => rd,
        Err(_) => return Ok(None),
    };
    let mut versions: Vec<(String, PathBuf)> = Vec::new();
    for entry in read.filter_map(std::result::Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(ver) = name.strip_prefix(&prefix) {
            if entry.path().is_dir() {
                versions.push((ver.to_string(), entry.path()));
            }
        }
    }
    // Lexical max picks the highest version dir deterministically (sufficient for
    // the single-prior-baseline transition the gate grades against).
    versions.sort_by(|a, b| a.0.cmp(&b.0));
    let Some((version, dir)) = versions.pop() else {
        return Ok(None);
    };
    Ok(Some(read_prior_bundle(&dir, version)?))
}

/// Read a prior baseline bundle's gate inputs from its on-disk members.
fn read_prior_bundle(dir: &Path, version: String) -> Result<PriorBaseline> {
    let lock: BundleLock = read_bundle_member(dir, "BUNDLE.lock")?;
    let ir: HashMap<String, Cell> = read_bundle_member(dir, "executable.ir.json")?;
    let manifest: Manifest = read_bundle_member(dir, "manifest.json")?;
    let dag = build_dag(&ir);
    Ok(PriorBaseline {
        ir,
        dag,
        manifest,
        prev_bundle_hash: lock.combined,
        version,
    })
}

/// Deserialize one JSON bundle member (`dir/member`) into `T`.
fn read_bundle_member<T: serde::de::DeserializeOwned>(dir: &Path, member: &str) -> Result<T> {
    let path = dir.join(member);
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading prior baseline member {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing prior baseline member {}", path.display()))
}

/// GATED UPDATE LANE (a prior baseline exists): build the candidate WITHOUT writing
/// ([`prepare_candidate`]), derive the prior corpus golden, grade every case via
/// [`gate`], and either BLOCK (print [`GateBlock::render`] verbatim + signal
/// `EXIT_GATE_BLOCK`, write nothing) or PROMOTE the new version. The `--accept` flow
/// records a fingerprint-bound approval first.
fn run_gated_lane(
    target: &Target,
    version: &str,
    prior: &PriorBaseline,
    args: &CompileArgs,
    not_quiet: bool,
) -> Result<i32> {
    let candidate = prepare_candidate(&target.path, &target.workflow)
        .map_err(|e| anyhow::anyhow!("preparing candidate for {} failed: {e}", target.workflow))?;

    let cases = derive_corpus(&candidate.manifest, &prior.ir, &prior.dag)
        .map_err(|e| anyhow::anyhow!("deriving the prior corpus failed: {e}"))?;
    let change_classes = derive_change_classes(prior, &candidate);
    let approvals = read_approvals(&target.out_root, &target.workflow)
        .map_err(|e| anyhow::anyhow!("reading approvals failed: {e}"))?;

    // Grade EVERY case; the first block surfaces the BA-actionable detail. The gate
    // runs against the candidate computed for that case (collect-all per region).
    for case in &cases {
        let computed = replay_case(&case, &candidate)?;
        let decision = gate(
            &case,
            &computed,
            &candidate.candidate_workbook_hash,
            &prior.prev_bundle_hash,
            &change_classes,
            &approvals,
        );
        if let GateDecision::Blocked(block) = decision {
            if !args.accept {
                emit_block(&block, &args.format)?;
                // gate-before-write: signal the DISTINCT exit code, write NOTHING.
                return Err(anyhow::Error::new(super::WorkbookExit::gate_block(
                    block.render(),
                )));
            }
            return accept_and_promote(
                target,
                version,
                prior,
                &candidate,
                &case,
                &computed,
                &change_classes,
                args,
                not_quiet,
            );
        }
    }

    // Every case reconciled within tolerance (or a covering approval already
    // existed) — promote the new version.
    promote_candidate(target, version, prior, &candidate, not_quiet)
}

/// Derive the auto-classified change classes (prior vs candidate manifest + IR),
/// stripped to the `Vec<ChangeClass>` the gate consumes (the region detail is
/// surfaced by the block render itself).
fn derive_change_classes(prior: &PriorBaseline, candidate: &Candidate) -> Vec<ChangeClass> {
    classify(
        &prior.manifest,
        &candidate.manifest,
        &prior.ir,
        &candidate.ir,
    )
    .into_iter()
    .map(|(class, _region)| class)
    .collect()
}

/// Replay one corpus case through the candidate IR/DAG to get its computed
/// named-output map (the value the gate grades against the prior golden).
fn replay_case(case: &ApprovalCase, candidate: &Candidate) -> Result<BTreeMap<String, f64>> {
    pmcp_workbook_compiler::gate::corpus::replay_candidate(
        case,
        &candidate.manifest,
        &candidate.ir,
        &candidate.dag,
    )
    .map_err(|e| anyhow::anyhow!("replaying case `{}` failed: {e}", case.case_id))
}

/// Print a gate block: the library's [`GateBlock::render`] VERBATIM in text mode
/// (the deltas + change classes + the copy-pasteable accept command are formatted by
/// the library — never re-formatted here), or the serialized block in JSON (D-09).
fn emit_block(block: &pmcp_workbook_compiler::gate::GateBlock, format: &str) -> Result<()> {
    match format {
        "json" => {
            let json = serde_json::json!({
                "status": "blocked",
                "case_id": block.case_id,
                "fingerprint": block.fingerprint,
                "accept_command": block.accept_command,
                "change_classes": format!("{:?}", block.change_classes),
                "render": block.render(),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        },
        _ => println!("{}", block.render()),
    }
    Ok(())
}

/// The `--accept` flow (D-07): record a fingerprint-bound [`ApprovalRecord`] for the
/// blocked transition via [`accept`], then [`promote`] the new version.
#[allow(clippy::too_many_arguments)]
fn accept_and_promote(
    target: &Target,
    version: &str,
    prior: &PriorBaseline,
    candidate: &Candidate,
    case: &ApprovalCase,
    computed: &BTreeMap<String, f64>,
    change_classes: &[ChangeClass],
    args: &CompileArgs,
    not_quiet: bool,
) -> Result<i32> {
    let effective_date = args
        .effective_date
        .as_deref()
        .context("--accept requires --effective-date")?;
    accept(
        case,
        &target.out_root,
        &target.workflow,
        computed,
        &candidate.candidate_workbook_hash,
        &prior.prev_bundle_hash,
        change_classes.to_vec(),
        &args.approver,
        effective_date,
    )
    .map_err(|e| anyhow::anyhow!("recording the approval failed: {e}"))?;
    if not_quiet {
        eprintln!(
            "ok: recorded approval for case `{}` (approver {})",
            case.case_id, args.approver
        );
    }
    promote_candidate(target, version, prior, candidate, not_quiet)
}

/// Promote the candidate over the prior baseline ([`promote`] on the
/// [`EmitLane::GatedUpdate`] lane), writing the new `{workflow}@{version}/` dir.
fn promote_candidate(
    target: &Target,
    version: &str,
    prior: &PriorBaseline,
    candidate: &Candidate,
    not_quiet: bool,
) -> Result<i32> {
    let changelog = VersionChangelog {
        from_version: prior.version.clone(),
        to_version: version.to_string(),
        deltas: vec![],
        summary: format!("{} -> {version}", prior.version),
    };
    let inputs = PromoteInputs {
        bundle_id: &target.workflow,
        version,
        ir: &candidate.ir,
        manifest: &candidate.manifest,
        layout: &candidate.layout,
        changelog: &changelog,
        parser_equivalence: &candidate.parser_equivalence,
        workbook_hash: candidate.candidate_workbook_hash.clone(),
    };
    let lane = EmitLane::GatedUpdate {
        prior_version: prior.version.clone(),
    };
    let (lock, _dir) = promote(&lane, &target.out_root, &inputs)
        .map_err(|e| anyhow::anyhow!("promoting {}@{version} failed: {e}", target.workflow))?;
    if not_quiet {
        eprintln!(
            "ok: promoted {}@{} (from {})",
            lock.bundle_id, lock.version, prior.version
        );
    }
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_compiler::gate::{GateBlock, GateDelta};

    #[test]
    fn worst_status_reduces_gate_block_above_error_above_ok() {
        assert_eq!(worst_status(EXIT_OK, EXIT_OK), EXIT_OK);
        assert_eq!(worst_status(EXIT_OK, EXIT_ERROR), EXIT_ERROR);
        assert_eq!(worst_status(EXIT_ERROR, EXIT_GATE_BLOCK), EXIT_GATE_BLOCK);
        // A gate block is never demoted by a later plain error.
        assert_eq!(worst_status(EXIT_GATE_BLOCK, EXIT_ERROR), EXIT_GATE_BLOCK);
        // An OK never overrides a recorded worse status.
        assert_eq!(worst_status(EXIT_ERROR, EXIT_OK), EXIT_ERROR);
    }

    #[test]
    fn surface_worst_ok_is_ok() {
        assert!(surface_worst(EXIT_OK).is_ok());
    }

    #[test]
    fn surface_worst_error_bails() {
        let err = surface_worst(EXIT_ERROR).expect_err("error must bail");
        // A plain error has no WorkbookExit (maps to anyhow's default exit 1).
        assert!(err.downcast_ref::<super::super::WorkbookExit>().is_none());
    }

    #[test]
    fn surface_worst_gate_block_carries_distinct_exit_code() {
        // D-10: the worst-status gate block surfaces via WorkbookExit so main.rs
        // exits with the DISTINCT EXIT_GATE_BLOCK code, not anyhow's default 1.
        let err = surface_worst(EXIT_GATE_BLOCK).expect_err("gate block must error");
        let wx = err
            .downcast_ref::<super::super::WorkbookExit>()
            .expect("a gate block carries a WorkbookExit");
        assert_eq!(wx.code, EXIT_GATE_BLOCK);
    }

    fn sample_block() -> GateBlock {
        GateBlock {
            case_id: "default".to_string(),
            deltas: vec![GateDelta {
                region: "3_Out!B2".to_string(),
                expected: 20.0,
                computed: Some(31.0),
            }],
            change_classes: vec![ChangeClass::FormulaLogic],
            fingerprint: "fp".to_string(),
            accept_command:
                "compile-workbook --accept --case default --approver <YOU> --effective-date <YYYY-MM-DD>"
                    .to_string(),
        }
    }

    #[test]
    fn block_render_carries_the_copy_pasteable_accept_line() {
        // The rendered block (printed verbatim) carries the BA's exact next action:
        // the --accept / --approver / --effective-date copy-pasteable command (D-07).
        let rendered = sample_block().render();
        assert!(rendered.contains("--accept"), "names --accept: {rendered}");
        assert!(
            rendered.contains("--approver"),
            "names --approver: {rendered}"
        );
        assert!(
            rendered.contains("--effective-date"),
            "names --effective-date: {rendered}"
        );
    }

    #[test]
    fn emit_block_json_is_serializable() {
        // JSON mode (D-09) serializes the block surface without a parallel DTO panic.
        let block = sample_block();
        // Construct the same JSON value emit_block builds and confirm it serializes.
        let json = serde_json::json!({
            "status": "blocked",
            "case_id": block.case_id,
            "fingerprint": block.fingerprint,
            "accept_command": block.accept_command,
            "change_classes": format!("{:?}", block.change_classes),
            "render": block.render(),
        });
        let s = serde_json::to_string_pretty(&json).expect("serialize block json");
        assert!(s.contains("blocked"));
        assert!(s.contains("--accept"));
    }

    #[test]
    fn find_prior_baseline_is_none_for_a_missing_out_root() {
        let missing = Path::new("/this/path/does/not/exist/at/all");
        let prior = find_prior_baseline(missing, "quote").expect("probe a missing root");
        assert!(
            prior.is_none(),
            "a missing out-root means no prior baseline (seed lane)"
        );
    }

    #[test]
    fn find_prior_baseline_is_none_for_an_empty_out_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prior = find_prior_baseline(tmp.path(), "quote").expect("probe an empty root");
        assert!(prior.is_none(), "no @version dir means no prior baseline");
    }

    #[test]
    fn execute_accept_without_effective_date_bails() {
        // D-07 pairing: --accept REQUIRES --effective-date.
        let args = CompileArgs {
            bundle_id_or_path: Some("x.xlsx".to_string()),
            workflow: Some("q".to_string()),
            approver: "alice".to_string(),
            accept: true,
            effective_date: None,
            out: None,
            format: "text".to_string(),
        };
        let gf = GlobalFlags {
            verbose: false,
            no_color: true,
            quiet: true,
        };
        let err = execute(args, &gf).expect_err("accept without effective-date must bail");
        assert!(
            err.to_string().contains("--effective-date"),
            "names the required flag: {err}"
        );
    }
}
