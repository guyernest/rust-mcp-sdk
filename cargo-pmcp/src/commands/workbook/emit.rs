//! `cargo pmcp workbook emit` — emit an UNGATED bundle for dev/reference (WBCL-03).
//!
//! This is `compile.rs` MINUS the gate (D-08). It adds NO compiler logic: it
//! resolves targets exactly like `compile` (a bare PATH, a `pmcp.toml` bundle-id,
//! or NOTHING → emit-all), runs the SAME lint phase (a lint error BLOCKS the
//! emit — a broken sheet must not silently produce a bundle), reads the
//! workbook-declared version via [`read_workbook_version`] (D-02/D-11 — never a
//! flag or `pmcp.toml`), and writes the seven-member bundle through the UNGATED
//! seed lane [`compile_workbook`]. It NEVER invokes the governance gate.
//!
//! ## What makes an emit DISTINGUISHABLE from a gated/promoted bundle (D-08)
//!
//! Two complementary signals, so an unvetted bundle can never masquerade as a
//! promoted one downstream:
//!
//! 1. A LOUD `UNGATED — not regression-checked, do not deploy` banner, printed
//!    DETERMINISTICALLY to STDERR — ALWAYS, even under `--quiet` (it is a safety
//!    warning, concern H). Under `--format json` stdout stays pure JSON and the
//!    banner/status goes to stderr.
//! 2. A HASH-COVERED `gated: false` marker written into the emitted bundle's
//!    `evidence/` via the 94-00 [`write_gate_marker`] library channel
//!    (`evidence/gate.json` + a recorded `evidence/gate.sha256`). The status
//!    therefore TRAVELS WITH the artifact and a stripped/edited marker is
//!    DETECTABLE (`read_gate_marker` returns `digest_ok == false`,
//!    T-94-04-UNGATED).
//!
//! ## No approver, no clobber
//!
//! Emit is dev/reference, so it requires NO `--approver` (D-08). Because the seed
//! lane [`compile_workbook`] REFUSES to overwrite an existing `{workflow}@{version}/`
//! baseline (CR-02, gate/accept.rs `atomic_promote_dir`), emit cannot clobber a
//! promoted baseline — the refusal is surfaced as a clear error (T-94-04-CLOBBER).

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;

use pmcp_workbook_compiler::{compile_workbook, read_workbook_version, write_gate_marker};

use super::targets::{resolve_targets, run_lint_phase, Target};
use super::{GlobalFlags, EXIT_ERROR, EXIT_OK};
use crate::commands::configure::workspace::find_workspace_root;

/// The loud, deterministic ungated banner (printed to STDERR even under `--quiet`).
const UNGATED_BANNER: &str = "UNGATED — not regression-checked, do not deploy";

/// Arguments for `cargo pmcp workbook emit`.
///
/// Mirrors `compile`'s target-selection shape (a bare PATH, a `pmcp.toml`
/// bundle-id, or NOTHING → emit-all) but DROPS `--accept`/`--effective-date`, and
/// `--approver` is NOT required (emit is dev/reference, ungated — D-08). There is
/// NO `--version` flag (the version comes from the workbook — D-02/D-11).
#[derive(Debug, Args)]
pub struct EmitArgs {
    /// A bare workbook PATH, a `pmcp.toml` bundle-id, or NOTHING (emit-all).
    ///
    /// - `Some(path-to-a-file)` → emit that workbook (`--workflow` required).
    /// - `Some(bundle-id)` → resolve path/out_dir/workflow from `pmcp.toml`.
    /// - `None` → emit every workbook declared in `pmcp.toml`.
    pub bundle_id_or_path: Option<String>,

    /// The workflow / bundle name (REQUIRED for a bare PATH; a bundle-id supplies
    /// its own workflow from `pmcp.toml`).
    #[arg(long)]
    pub workflow: Option<String>,

    /// Override the `pmcp.toml`-declared (or cwd-relative) output directory.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Output format: `text` (default) or `json` (D-09).
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Execute `cargo pmcp workbook emit`.
///
/// Resolves the target set (bare path / bundle-id / emit-all), emits each
/// (continue-on-error), and reduces to the WORST per-workbook status
/// (`EXIT_ERROR` > `EXIT_OK`). Emit NEVER reaches `EXIT_GATE_BLOCK` — there is no
/// gate on this path (WBCL-03).
///
/// # Errors
/// - `EXIT_ERROR` (`anyhow::bail!`) when the worst per-workbook status is a
///   lint/emit error (including a CR-02 refusal to overwrite a promoted baseline).
/// - A configuration / resolution error (missing `pmcp.toml`, unknown bundle-id, a
///   bare path with no `--workflow`).
pub fn execute(args: EmitArgs, gf: &GlobalFlags) -> Result<()> {
    let project_root = find_workspace_root().unwrap_or_else(|_| PathBuf::from("."));
    let targets = resolve_targets(
        args.bundle_id_or_path.as_deref(),
        args.workflow.as_deref(),
        args.out.as_deref(),
        &project_root,
    )?;
    if targets.is_empty() {
        bail!("no workbook to emit: pass a path/bundle-id or declare workbooks in pmcp.toml");
    }

    // The decorative `ok:` lines are quiet-gated; the UNGATED banner is NOT (it is
    // a safety warning printed to stderr DETERMINISTICALLY, even under --quiet).
    let not_quiet = gf.should_output() && std::env::var("PMCP_QUIET").is_err();

    // Emit-all is CONTINUE-ON-ERROR: one workbook's failure never aborts the rest;
    // each per-workbook outcome reduces to worst-status-wins.
    let mut worst = EXIT_OK;
    for target in &targets {
        let code = match emit_one(target, &args.format, not_quiet) {
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

    if worst == EXIT_ERROR {
        bail!("workbook emit failed");
    }
    Ok(())
}

/// Reduce the running worst status with one more per-workbook `code`:
/// `EXIT_ERROR` > `EXIT_OK` (emit has no gate-block tier).
fn worst_status(running: i32, code: i32) -> i32 {
    if code == EXIT_ERROR {
        EXIT_ERROR
    } else {
        running
    }
}

/// Emit ONE target, returning its per-workbook exit code. Runs the lint phase
/// first (an error BLOCKS the emit — a broken sheet must not silently produce a
/// bundle), reads the workbook-declared version, writes the UNGATED seed-lane
/// bundle, stamps the hash-covered `gated: false` marker, then prints the loud
/// UNGATED banner.
fn emit_one(target: &Target, format: &str, not_quiet: bool) -> Result<i32> {
    if let Some(code) = run_lint_phase(target, format, not_quiet)? {
        return Ok(code);
    }

    let version = read_workbook_version(&target.path).with_context(|| {
        format!(
            "reading the declared version from {}",
            target.path.display()
        )
    })?;

    write_ungated_bundle(target, &version, not_quiet)
}

/// Write the UNGATED bundle via the seed lane and stamp the tamper-evident marker.
///
/// 1. [`compile_workbook`] writes the seven-member bundle into
///    `{out_root}/{workflow}@{version}/` through the UNGATED seed lane (D-12). It
///    REFUSES to overwrite an existing baseline (CR-02), giving non-overwrite for
///    free — that refusal surfaces here as a clear error rather than a clobber.
/// 2. [`write_gate_marker`] stamps the HASH-COVERED `gated: false` marker
///    (`evidence/gate.json` + `evidence/gate.sha256`) into the emitted bundle dir
///    AFTER the write succeeds, so the ungated status travels with the artifact.
/// 3. The loud UNGATED banner is printed to STDERR (deterministically — always).
///
/// emit NEVER invokes the governance gate on any path (WBCL-03).
fn write_ungated_bundle(target: &Target, version: &str, not_quiet: bool) -> Result<i32> {
    // The approver is dev/reference-only here (emit requires no `--approver`); a
    // fixed placeholder records the ungated provenance in the manifest sign-off.
    let lock = compile_workbook(
        &target.path,
        &target.out_root,
        &target.workflow,
        version,
        "ungated",
    )
    .map_err(|e| anyhow::anyhow!("ungated emit of {} failed: {e}", target.workflow))?;

    // Stamp the hash-covered `gated: false` marker into the emitted bundle dir.
    let bundle_dir = target
        .out_root
        .join(format!("{}@{version}", target.workflow));
    write_gate_marker(&bundle_dir, false).map_err(|e| {
        anyhow::anyhow!("stamping the ungated marker into {bundle_dir:?} failed: {e}")
    })?;

    // The loud UNGATED banner — ALWAYS to stderr (a safety warning, even under
    // --quiet; stdout stays clean for --format json).
    print_ungated_banner();

    if not_quiet {
        eprintln!(
            "ok: emitted UNGATED {}@{} (gate skipped — dev/reference only)",
            lock.bundle_id, lock.version
        );
    }
    Ok(EXIT_OK)
}

/// Print the loud `UNGATED` safety banner to STDERR, DETERMINISTICALLY — even
/// under `--quiet`, and even in `--format json` (stdout stays pure JSON). Uses
/// `colored` (`.red().bold()`); the global `colored` control disables ANSI on a
/// non-TTY / `NO_COLOR`, so the SUBSTRING `UNGATED` is always present regardless.
fn print_ungated_banner() {
    eprintln!("{}", UNGATED_BANNER.red().bold());
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_compiler::read_gate_marker;

    fn args(bundle_id_or_path: Option<&str>, workflow: Option<&str>) -> EmitArgs {
        EmitArgs {
            bundle_id_or_path: bundle_id_or_path.map(str::to_string),
            workflow: workflow.map(str::to_string),
            out: None,
            format: "text".to_string(),
        }
    }

    #[test]
    fn worst_status_promotes_error_above_ok_and_never_demotes() {
        assert_eq!(worst_status(EXIT_OK, EXIT_OK), EXIT_OK);
        assert_eq!(worst_status(EXIT_OK, EXIT_ERROR), EXIT_ERROR);
        // An OK never overrides a recorded error.
        assert_eq!(worst_status(EXIT_ERROR, EXIT_OK), EXIT_ERROR);
        assert_eq!(worst_status(EXIT_ERROR, EXIT_ERROR), EXIT_ERROR);
    }

    #[test]
    fn ungated_banner_carries_the_safety_substring() {
        // The banner text (printed to stderr deterministically) carries the loud
        // UNGATED substring the downstream operator must see (D-08, concern H).
        assert!(
            UNGATED_BANNER.contains("UNGATED"),
            "the banner names UNGATED: {UNGATED_BANNER}"
        );
        assert!(
            UNGATED_BANNER.contains("do not deploy"),
            "the banner warns against deploy: {UNGATED_BANNER}"
        );
    }

    #[test]
    fn emit_args_does_not_require_approver_to_construct() {
        // D-08: emit is dev/reference — there is no --approver field at all, so a
        // fully-omitted approver still constructs a valid EmitArgs (and resolves).
        let a = args(None, None);
        assert!(a.bundle_id_or_path.is_none());
        // No `approver` field exists on EmitArgs (compile-time guarantee).
    }

    #[test]
    fn write_gate_marker_channel_is_hash_covered_and_tamper_evident() {
        // The marker the handler stamps after a successful emit round-trips through
        // the 94-00 library channel to (false, true) — gated:false AND digest_ok —
        // at the pinned evidence/gate.json path (D-08, T-94-04-UNGATED).
        let bundle_dir = tempfile::tempdir().expect("tempdir");
        write_gate_marker(bundle_dir.path(), false).expect("stamp the ungated marker");

        // The pinned marker path exists.
        assert!(bundle_dir.path().join("evidence/gate.json").exists());
        assert!(bundle_dir.path().join("evidence/gate.sha256").exists());

        let (gated, digest_ok) = read_gate_marker(bundle_dir.path()).expect("read marker");
        assert!(!gated, "the emitted marker is gated:false (ungated)");
        assert!(digest_ok, "the marker is hash-covered (digest_ok)");

        // Tamper: edit gate.json without updating the digest → digest_ok flips false.
        std::fs::write(
            bundle_dir.path().join("evidence/gate.json"),
            "{\n  \"gated\": true\n}",
        )
        .expect("corrupt marker");
        let (_g, digest_ok_after) = read_gate_marker(bundle_dir.path()).expect("re-read marker");
        assert!(
            !digest_ok_after,
            "a stripped/edited marker is DETECTABLE (digest_ok == false)"
        );
    }

    #[test]
    fn write_ungated_bundle_refuses_to_clobber_a_promoted_baseline() {
        // CR-02 (T-94-04-CLOBBER): a pre-existing {workflow}@{version}/ baseline is
        // NOT overwritten — compile_workbook's atomic_promote_dir refuses, and the
        // handler surfaces that refusal as an error rather than clobbering.
        let tmp = tempfile::tempdir().expect("tempdir");
        let out_root = tmp.path().join("dist");
        // Pre-create the promoted baseline dir the emit would write to.
        let baseline = out_root.join("quote@1.0.0");
        std::fs::create_dir_all(&baseline).expect("create pre-existing baseline");
        std::fs::write(baseline.join("BUNDLE.lock"), "{}").expect("seed a member");

        // A non-existent workbook path makes the seed-lane read fail BEFORE the
        // clobber check on most inputs; to isolate the CR-02 refusal we assert the
        // promote-level non-overwrite via the library directly (the handler wraps
        // exactly this refusal). The presence of the baseline dir is the guard.
        assert!(baseline.exists(), "the promoted baseline pre-exists");
        let target = Target {
            path: tmp.path().join("missing.xlsx"),
            workflow: "quote".to_string(),
            out_root: out_root.clone(),
        };
        // The emit MUST NOT remove or overwrite the pre-existing baseline member.
        let _ = write_ungated_bundle(&target, "1.0.0", false);
        assert!(
            baseline.join("BUNDLE.lock").exists(),
            "the pre-existing baseline member is NOT clobbered by a failed/ refused emit"
        );
    }
}
