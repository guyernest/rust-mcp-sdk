//! `pmcp-workbook-compiler` — the OFFLINE Excel→MCP compiler.
//!
//! This crate runs the build-time pipeline that turns a governed Excel workbook
//! into a tested, versioned, deterministic served bundle:
//! ingest → lint → manifest synth → formula parse → DAG compile →
//! penny-reconcile → artifact emit → promote-time gate.
//!
//! # The purity boundary (the milestone's #1 trap)
//!
//! This is the ONE crate in the workspace where the Excel reader
//! (`umya-spreadsheet`, plus its transitive `quick-xml`/`zip`) is allowed. The
//! reader is confined to the [`ingest`] and [`provenance`] modules; no umya type
//! leaks across the crate boundary, and the served-tree crates
//! (`pmcp-workbook-runtime`, `pmcp-workbook-dialect`, `pmcp-server-toolkit`)
//! NEVER link it. The Makefile `purity-check` gate POSITIVELY asserts umya IS
//! here and is ABSENT everywhere served.
//!
//! # Re-export, don't re-declare (the keystone)
//!
//! Every shared model/IR/hash/changelog/finding/rounding type is re-exported
//! from [`pmcp_workbook_runtime`] (and the dialect contract from
//! [`pmcp_workbook_dialect`]) — NEVER re-declared. A second copy of
//! `Manifest`/`ChangeClass`/`WHITELIST` would make the served loader and the
//! `diff_version` tool read a DIFFERENT definition than the compiler emits.

// Compiler/clippy-enforced panic-freedom on the library value path (copied
// verbatim from pmcp-workbook-runtime). Test code constructs fixtures freely.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::collections::HashMap;
use std::path::Path;

// ---- Pipeline modules (each a compilable Wave 1 stub; downstream plans fill bodies) ----

/// The compiler's typed error surface ([`CompileError`]).
pub mod error;

/// Ingest stage — the umya-isolated `.xlsx` reader (Plan 02).
pub mod ingest;

/// Dialect linter stage — runs against the SDK dialect contract (Plan 03).
pub mod dialect;

/// Manifest synthesis stage — colour/Guide/header heuristics → roles (Plan 04).
pub mod manifest;

/// Formula parse stage — tokenize + parse to the runtime's owned `Expr` (Plan 05).
pub mod formula;

/// DAG compile stage — build the dependency graph + toposort (Plan 05).
pub mod dag;

/// Sheet-IR eval bridge — drives the runtime's SERVE-time executor (Plan 05).
pub mod sheet_ir;

/// Reconcile stage — grade computed outputs against the cached oracle (Plan 06).
pub mod reconcile;

/// Provenance stage — quarantined raw-parts identity reader (Plan 02).
pub mod provenance;

/// Artifact emission stage — write the served bundle (Plan 07).
pub mod artifact;

/// Change-classification stage — diff a candidate vs the prior baseline (Plan 07).
pub mod change_class;

/// Promote-time governance gate — the build-time approval boundary (Plan 07).
pub mod gate;

/// Stage-1 composed pass — collect-all lint + synth + freshness + drift (Plan 06).
pub mod stage1;

/// Workbook-declared version accessor — the read-only `read_workbook_version`
/// surface the thin-shell CLI reads instead of a `--version` flag (D-02/D-11).
pub mod version;

// Producer/consumer golden proof (WBCO-05). In-crate `#[cfg(test)]` so it can
// reach the `#[cfg(test)]`-only `compile_workbook_with_fixture_override`
// (CR-01: the override is unreachable from any publishable feature; an external
// integration test could not see it). Runs via plain `cargo test`.
#[cfg(test)]
mod reemit_golden;

pub use error::CompileError;

// ---- Curated re-export surface (re-export the runtime/dialect shared types) ----
//
// These mirror the names `pmcp-workbook-runtime` exports so downstream consumers
// (and Plan 07's driver wiring) get one definition of every shared type. NEVER
// re-declare any of these as a fresh struct/enum here.

// Formula AST + Excel error value set + DAG container + executor value type.
pub use pmcp_workbook_runtime::{toposort, BinOp, CellValue, Dag, ExcelError, Expr, UnOp};

// The version-changelog module (reach `changelog::Severity` via its module path
// to preserve the runtime's `changelog::Severity` vs `finding::Severity`
// collision rule — the bare `Severity` re-exported below stays the lint tier).
pub use pmcp_workbook_runtime::changelog;
pub use pmcp_workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};

// The logical manifest projection model the compiler EMITS (lives in the
// runtime; re-export, never re-declare).
pub use pmcp_workbook_runtime::{AnnotationDecl, CellRole, Dtype, InputTier, Manifest, Role};

// The collect-all located lint findings the linter emits (bare `Severity` here
// is the lint-finding tier — the changelog tier stays module-path-only above).
pub use pmcp_workbook_runtime::{LintFinding, LintReport, Severity};

// The bundle artifact model + hashing helpers (NEVER hand-roll the combined
// hash — the served loader recomputes with these).
pub use pmcp_workbook_runtime::{build_bundle_lock, fold_evidence_hash, sha256_hex, BundleLock};

// The Excel rounding helpers the reconcile classifier anchors on.
pub use pmcp_workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup};

// The dialect contract the linter runs against (re-export, never re-declare — a
// second WHITELIST copy would defeat the dialect crate's spec-binding test).
pub use pmcp_workbook_dialect::{CandidateRole, DialectRules, WHITELIST};

// The SERVE-time executor surface the compiler-side reconcile drives (the runtime's
// pure-Rust executor — NO SWC/JS oracle; re-export, never re-declare). The O-1
// parity proof depends on the compiler and the server reconciling through ONE path.
pub use pmcp_workbook_runtime::{run_executor, CellEnv, EvalTrace, RunResult};

// The manifest→CellSource wiring seam (93-02 ⋈ 93-03): the real WorkbookMap drives
// the linter/parser/DAG through this adapter.
pub use manifest::WorkbookCellSource;

// The penny-reconcile surface (WBCO-04): the operand-anchored classifier, the D-03
// named-output/helper severity split, and the collect-all driver.
pub use reconcile::classifier::{MismatchClass, MismatchEvidence, BOUNDARY_EPSILON};
pub use reconcile::drift::{is_named_output, mismatch_severity};
pub use reconcile::{
    reconcile as reconcile_oracle, ComparisonMap, GradedMismatch, ReconcileReport,
};

// The bundle-emit surface (WBCO-05/WBGV-07): the seven-member emitter (deterministic
// serialization + bundle_id BUNDLE.lock via the runtime hash helpers + the WR-01
// enum-tier skip) and its evidence/parser-equivalence record. The runtime hash
// helpers (build_bundle_lock/fold_evidence_hash/sha256_hex/BundleLock) are already
// re-exported above from the runtime — NOT re-routed through `artifact` here, so
// there is exactly ONE definition of each.
pub use artifact::{
    build_cell_map, build_layout_descriptor, emit_bundle, parser_equivalence_json, CellEntry,
    CellMap, EmitError, EvidenceInputs, LayoutDescriptor, ParserEquivalence,
};

// The change-class surface (WBGV-01/02/03): the symmetric demotion-aware classifier,
// the strictest-policy reducer, the per-class routing policy + block message, the
// canonical IR sub-DAG identity hash, and the output redefinition diff. `ChangeClass`
// / `OutputDelta` / `VersionChangelog` are re-exported above from the runtime (the
// served `diff_version` tool reads the SAME enum) — NEVER re-declared here.
pub use change_class::{
    block_message, classify, diff_outputs, effective_policy, ir_subdag_hash, policy, GatePolicy,
};

// The composed stage-1 pass + its freshness policy (the production driver passes
// `Enforce`; a TEST may pass `TrustedFixture` to admit a committed neutral fixture).
pub use stage1::{run_stage1, FreshnessPolicy, Stage1Output};

// The workbook-declared-version accessor (WBCL-01 / D-02 / D-11): the version the
// bundle is stamped with comes FROM the workbook, never a CLI flag or `pmcp.toml`.
// The thin-shell CLI reads it through this re-export.
pub use version::read_workbook_version;

use pmcp_workbook_runtime::sheet_ir::{Cell, CellExpr};

/// Compile a governed Excel workbook into a served bundle.
///
/// This is the GENERIC driver that replaces the lighthouse's hardcoded
/// reference-manifest builder (the one surviving §5 gap — WBCO-02): the manifest
/// comes SOLELY from [`manifest::synthesize`] → [`manifest::ratify`], never from a
/// hand-built customer-specific literal, and there is NO hardcoded
/// reference-workbook-path / workflow-name const — `workflow` is a parameter.
///
/// The wired pipeline: read ORIGINAL bytes (provenance anchor) → [`ingest::ingest`]
/// (umya) → [`stage1::run_stage1`] (lint + synth + freshness, collect-all) →
/// [`manifest::ratify`] → parse formulas + [`dag::build_dag`] → run the SHARED
/// runtime executor → [`reconcile::reconcile`] (named-out = ERROR, helper =
/// WARNING) → emit the seven-member bundle into `{workflow}@{version}/`.
///
/// # Arguments
/// * `workbook_path` — the source `.xlsx` to compile.
/// * `out_root` — the bundle output root (one `{workflow}@{version}/` dir).
/// * `workflow` — the workflow/bundle name (NEVER a hardcoded literal — WBCO-02).
/// * `version` — the workbook-declared version (`BUNDLE.lock` version == changelog
///   `to_version`).
/// * `approver` — the human approver recorded in the manifest sign-off.
///
/// # Errors
/// Returns the per-stage [`CompileError`] variant on failure:
/// [`Ingest`](CompileError::Ingest), [`Lint`](CompileError::Lint),
/// [`Reconcile`](CompileError::Reconcile), [`Emit`](CompileError::Emit), or
/// [`Gate`](CompileError::Gate).
pub fn compile_workbook(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
) -> Result<BundleLock, CompileError> {
    compile_workbook_inner(
        workbook_path,
        out_root,
        workflow,
        version,
        approver,
        FreshnessPolicy::Enforce,
    )
}

/// The shared driver body. `freshness` is [`FreshnessPolicy::Enforce`] on the
/// production path ([`compile_workbook`]); only the TEST-ONLY
/// [`compile_workbook_with_fixture_override`] passes
/// [`FreshnessPolicy::TrustedFixture`].
fn compile_workbook_inner(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
    freshness: FreshnessPolicy,
) -> Result<BundleLock, CompileError> {
    // (1) ORIGINAL on-disk bytes (the provenance anchor — NEVER a umya round-trip).
    let bytes = std::fs::read(workbook_path)?;

    // (2) umya ingest → owned WorkbookMap + collect-all ingest findings.
    let (map, ingest_findings) =
        ingest::ingest(workbook_path).map_err(|e| CompileError::Ingest(e.to_string()))?;

    // (3) Composed stage-1 pass (lint + synth + freshness), collect-all refuse.
    let stage1 = stage1::run_stage1(&bytes, &map, &ingest_findings, workflow, freshness)?;

    // (3a) Promote the workbook's `out_*` named-range targets to `Role::Output`.
    // Synthesis classifies Input/Constant/Formula from COLOUR alone (it never
    // emits `Role::Output`); the OUTPUT convention is a named-range act the
    // workbook authors (`out_taxable_income → 3_Outputs!B2`). The driver applies
    // it so the manifest carries the declared outputs a served `calculate`
    // requires — without this every emit would fail loud in `build_cell_map`
    // (zero-output manifest). This stays out of `synth` (colour-only) and is the
    // driver's naming-convention responsibility.
    let mut manifest = stage1.synth_manifest;
    promote_named_outputs(&mut manifest, &map);

    // (4) RATIFY the candidate manifest (a recorded sign-off). The sidecar lives
    // beside the output root so the audit trail is co-located with the bundle.
    let workbook_hash = sha256_hex(&bytes);
    let sidecar = out_root.join(format!("{workflow}.ratifications.jsonl"));
    manifest::ratify(&mut manifest, &workbook_hash, approver, &sidecar)
        .map_err(|e| CompileError::Emit(e.to_string()))?;

    // (5) Build the IR + DAG from the parsed formulas (whitelist-at-parse, D-06).
    // The literal cells emitted are the GOVERNED CONSTANTS only (the bracket
    // table) — inputs are seeded at run-time via the cell-map, and decorative
    // text labels never enter the served IR.
    let (ir, dag) = build_ir_and_dag(&map, &manifest)?;

    // (6) Run the SHARED runtime executor over the IR with the input cells seeded
    // from their cached values — the SAME pure-Rust path the served binary uses.
    let seed = seed_from_inputs(&map, &manifest);
    let run = sheet_ir::eval(&ir, &dag, &seed)
        .map_err(|finding| CompileError::Reconcile(finding.message.clone()))?;

    // (7) Reconcile the computed outputs against the cached oracle (named-output =
    // ERROR, helper = WARNING). A named-output mismatch blocks the emit.
    let comparison = comparison_from_outputs(&map, &manifest);
    let report = reconcile::reconcile(&run.computed, &run.traces, &ir, &comparison, &manifest);
    if report.has_errors() || report.is_hard_fail() {
        return Err(CompileError::Reconcile(format!(
            "{} named-output mismatch(es) against the cached oracle",
            report
                .mismatches
                .iter()
                .filter(|m| m.severity == Severity::Error)
                .count()
        )));
    }

    // (8) Emit the seven-member bundle through the SEED lane (first version: no
    // prior baseline). The manifest came SOLELY from synth→ratify (no hand-built
    // per-workbook reference manifest on this path — the WBCO-02 generalization).
    let layout = artifact::build_layout_descriptor(&map, &workbook_hash);
    let changelog = VersionChangelog {
        from_version: String::new(),
        to_version: version.to_string(),
        deltas: vec![],
        summary: format!("seed {version}"),
    };
    let inputs = gate::accept::PromoteInputs {
        bundle_id: workflow,
        version,
        ir: &ir,
        manifest: &manifest,
        layout: &layout,
        changelog: &changelog,
        parser_equivalence: &stage1.parser_equivalence,
        workbook_hash,
    };
    let (lock, _dir) = gate::accept::promote(&gate::accept::EmitLane::Seed, out_root, &inputs)
        .map_err(|e| CompileError::Emit(e.to_string()))?;
    Ok(lock)
}

/// TEST-ONLY: compile a committed neutral fixture, honouring its trusted-fixture
/// provenance override so the in-crate producer/consumer golden proof can run
/// against a workbook authored WITHOUT a genuine Excel save (its `fullCalcOnLoad`
/// staleness signal is demoted to a Warning). Reachable ONLY under `cfg(test)`
/// (CR-01: there is NO publishable feature that arms this — a default or
/// `--all-features` build of the crate as a dependency neither compiles nor
/// exposes this symbol). The production [`compile_workbook`] always passes
/// [`FreshnessPolicy::Enforce`], so the same bytes are REFUSED on the production
/// path (the override cannot weaken production refusal).
#[cfg(test)]
fn compile_workbook_with_fixture_override(
    workbook_path: &Path,
    out_root: &Path,
    workflow: &str,
    version: &str,
    approver: &str,
) -> Result<BundleLock, CompileError> {
    compile_workbook_inner(
        workbook_path,
        out_root,
        workflow,
        version,
        approver,
        FreshnessPolicy::TrustedFixture,
    )
}

/// Build the executable IR (`{cell_key -> Cell}`) and its dependency [`Dag`] from
/// the ingested [`ingest::WorkbookMap`].
///
/// Every formula cell parses to a [`CellExpr::Formula`] whose `Expr::Ref` nodes are
/// rebased to fully-qualified `cell_key`s (so the executor resolves cross-sheet
/// references). For NON-formula cells, ONLY the manifest's `Role::Constant`
/// (governed) cells enter the IR as a [`CellExpr::Literal`]: inputs are seeded at
/// run-time via the cell-map and decorative text labels never enter the served IR
/// (matching the frozen golden's IR shape — formula cells + governed constants).
/// The DAG is reconstructed SOLELY from the parsed references (never
/// `calcChain.xml`).
fn build_ir_and_dag(
    map: &ingest::WorkbookMap,
    manifest: &Manifest,
) -> Result<(HashMap<String, Cell>, Dag), CompileError> {
    use pmcp_workbook_runtime::range_ref::cell_key;
    let mut ir: HashMap<String, Cell> = HashMap::new();
    let mut parsed: Vec<dag::ParsedCell> = Vec::new();

    // The governed-constant cells that may enter the IR as literals.
    let governed: std::collections::HashSet<&str> = manifest
        .cells
        .iter()
        .filter(|c| matches!(c.role, Role::Constant))
        .map(|c| c.cell.as_str())
        .collect();

    for sheet in &map.sheets {
        for cell in &sheet.cells {
            let key = cell_key(&sheet.name, &cell.addr);
            if let Some(formula) = &cell.formula {
                let expr = formula::parse(formula, &sheet.name, &cell.addr)
                    .map_err(|e| CompileError::Lint(format!("parse {key}: {e}")))?;
                parsed.push(dag::ParsedCell {
                    sheet: sheet.name.clone(),
                    addr: cell.addr.clone(),
                    expr: expr.clone(),
                });
                let rebased = rebase_refs(&expr, &sheet.name);
                ir.insert(
                    key.clone(),
                    Cell {
                        key,
                        expr: CellExpr::Formula(rebased),
                    },
                );
            } else if governed.contains(key.as_str()) {
                if let Some(value) = &cell.value {
                    let lit = parse_cell_value(value);
                    ir.insert(
                        key.clone(),
                        Cell {
                            key,
                            expr: CellExpr::Literal(lit),
                        },
                    );
                }
            }
        }
    }

    // No synthetic defined names on the generic path (D-07: names resolve only
    // when the workbook authors them; the neutral fixture uses cell refs).
    let names: Vec<dialect::DefinedName> = Vec::new();
    let (dag, _order) =
        dag::build_dag(&parsed, &names).map_err(|e| CompileError::Lint(e.to_string()))?;
    Ok((ir, dag))
}

/// Rebase every [`Expr::Ref`] in `expr` to a fully-qualified `cell_key` against
/// `current_sheet` so the executor resolves a bare `B2` as `{sheet}!B2` and keeps
/// a cross-sheet `1_Inputs!B2` unchanged (the IR refs match the DAG node keys).
fn rebase_refs(expr: &Expr, current_sheet: &str) -> Expr {
    use pmcp_workbook_runtime::resolve::split_ref;
    match expr {
        Expr::Ref(reference) => {
            let (sheet, addr) = split_ref(reference, current_sheet);
            Expr::Ref(pmcp_workbook_runtime::range_ref::cell_key(&sheet, &addr))
        },
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(rebase_refs(left, current_sheet)),
            op: *op,
            right: Box::new(rebase_refs(right, current_sheet)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(rebase_refs(operand, current_sheet)),
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| rebase_refs(a, current_sheet)).collect(),
        },
        // Ranges/names/literals pass through unchanged (the neutral fixture uses
        // only scalar cell refs; range rebasing is the DAG's concern).
        other => other.clone(),
    }
}

/// Parse a cached cell-value string into a [`CellValue`]: a parseable number is
/// [`CellValue::Number`], `TRUE`/`FALSE` is [`CellValue::Bool`], anything else is
/// [`CellValue::Text`]. An empty string is [`CellValue::Empty`].
fn parse_cell_value(raw: &str) -> CellValue {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return CellValue::Empty;
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        return CellValue::Number(n);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "TRUE" => CellValue::Bool(true),
        "FALSE" => CellValue::Bool(false),
        _ => CellValue::Text(trimmed.to_string()),
    }
}

/// Seed the executor [`CellEnv`] from the manifest's `Role::Input` cells, taking
/// each input's cached value from the workbook map. Inputs are the leaves the
/// executor needs pre-loaded before it walks the formula DAG.
fn seed_from_inputs(map: &ingest::WorkbookMap, manifest: &Manifest) -> CellEnv {
    let mut value_by_key: HashMap<String, CellValue> = HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if cell.is_formula {
                continue;
            }
            if let Some(value) = &cell.value {
                let key = pmcp_workbook_runtime::range_ref::cell_key(&sheet.name, &cell.addr);
                value_by_key.insert(key, parse_cell_value(value));
            }
        }
    }

    let mut env = CellEnv::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Input | Role::Constant) {
            if let Some(value) = value_by_key.get(&role.cell) {
                env = env.seed_cell(&role.cell, value);
            }
        }
    }
    // ALSO seed any non-formula literal cell the executor depends on but the
    // manifest did not role (e.g. a governed bracket-table constant) so the DAG's
    // leaf cells resolve.
    for (key, value) in &value_by_key {
        env = env.seed_cell(key, value);
    }
    env
}

/// Build the reconcile [`reconcile::ComparisonMap`] from the manifest's
/// `Role::Output` cells, taking each output's cached value (the oracle the
/// reconcile stage grades the computed output against).
fn comparison_from_outputs(
    map: &ingest::WorkbookMap,
    manifest: &Manifest,
) -> reconcile::ComparisonMap {
    let mut value_by_key: HashMap<String, CellValue> = HashMap::new();
    for sheet in &map.sheets {
        for cell in &sheet.cells {
            if let Some(value) = &cell.value {
                let key = pmcp_workbook_runtime::range_ref::cell_key(&sheet.name, &cell.addr);
                value_by_key.insert(key, parse_cell_value(value));
            }
        }
    }

    let mut comparison = reconcile::ComparisonMap::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Output) {
            if let Some(value) = value_by_key.get(&role.cell) {
                comparison = comparison.with_value(&role.cell, value.clone());
            }
        }
    }
    comparison
}

/// Promote every `out_*` named-range target cell in `manifest` to [`Role::Output`].
///
/// Synthesis classifies cells from COLOUR alone and never emits [`Role::Output`];
/// the OUTPUT convention is a named-range the WORKBOOK authors (`out_<name>`
/// targeting a single result cell). For each workbook defined name whose name
/// starts with `out_` and whose target is a single cell, this re-roles the matching
/// manifest cell to [`Role::Output`] and records the named-range `name` (the
/// cell-map `json_key` source). A `Role::Output` cell with no synthesized
/// counterpart (e.g. an output formula cell synthesis classified as
/// [`Role::Formula`]) is re-roled in place; an unmatched name is ignored (a
/// defined name pointing nowhere is a linter concern, not a hard failure here).
fn promote_named_outputs(manifest: &mut Manifest, map: &ingest::WorkbookMap) {
    use pmcp_workbook_runtime::range_ref::cell_key;
    for dn in &map.defined_names {
        if !dn.name.starts_with("out_") {
            continue;
        }
        // Single-cell target only: start == end (a range output is not a scalar
        // named output and is left for the linter to flag).
        if dn.target.start != dn.target.end {
            continue;
        }
        let key = cell_key(&dn.target.sheet, &dn.target.start);
        if let Some(role) = manifest.cells.iter_mut().find(|c| c.cell == key) {
            role.role = Role::Output;
            role.name = Some(dn.name.clone());
        }
    }
}
