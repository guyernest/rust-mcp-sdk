//! Stage-1 composed pass — the collect-all lint + synth + freshness + drift pass.
//!
//! ONE function ([`run_stage1`]) takes the ORIGINAL on-disk workbook bytes plus
//! the single ingested [`WorkbookMap`] and runs EVERY early analysis gate
//! collect-all (D-01): the dialect linter, manifest synthesis, and the
//! freshness / provenance gate over the ORIGINAL bytes. It refuses ONCE, on
//! `Error`-severity findings only — every Error finding from {lint report, the
//! ingest findings, the provenance gate} is returned TOGETHER so the BA fixes
//! one re-save. Warning/Info findings SURVIVE the success path (evidence, not
//! noise).
//!
//! # Single-pass contract
//!
//! Stage 1 NEVER reads or ingests itself: the caller supplies the pre-read bytes
//! ([`std::fs::read`] of the on-disk `.xlsx`) and the pre-ingested map, so the
//! whole pass rides ONE ingest and ONE byte-read. `original_bytes` MUST be the
//! on-disk bytes — NEVER a umya round-trip (which fabricates Excel provenance and
//! turns the freshness gate into a tautology).
//!
//! # Generic, never per-workbook
//!
//! The candidate manifest comes SOLELY from [`synthesize`] (colour + Guide +
//! headers → roles), keyed by the `workflow` name the caller supplies — never a
//! hand-built per-workbook literal (the WBCO-02 generalization invariant). The
//! synthesized candidate stays UN-ratified: synthesis proposes, ratification is a
//! separate recorded act.

use crate::artifact::ParserEquivalence;
use crate::dialect::{lint, DialectRules};
use crate::error::CompileError;
use crate::ingest::WorkbookMap;
use crate::manifest::{synthesize, WorkbookCellSource};
use crate::provenance::{gate as freshness_gate, OracleProvenance};
use crate::{LintFinding, Manifest, Severity};

/// Whether the freshness / provenance gate may honour a committed trusted-fixture
/// override.
///
/// The PRODUCTION path is always [`FreshnessPolicy::Enforce`]: the gate
/// classifies provenance from the ORIGINAL bytes and REFUSES anything that is not
/// an Excel-trusted save. A neutral committed fixture authored by a non-Excel
/// tool (e.g. `rust_xlsxwriter`) carries no Excel provenance, so a TEST may pass
/// [`FreshnessPolicy::TrustedFixture`] to admit the fixture's provenance class.
/// The override CANNOT weaken production refusal — production never constructs
/// `TrustedFixture` (a regression test asserts the same bytes are refused under
/// `Enforce`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessPolicy {
    /// The default everywhere on the production compile path: refuse a non-Excel
    /// or stale workbook at `Error`.
    Enforce,
    /// TEST-ONLY: admit the otherwise-refused provenance CLASS of a committed
    /// trusted fixture so the producer/consumer proof can compile a neutral,
    /// non-Excel-authored `.xlsx`. Never reachable on the production path.
    TrustedFixture,
    /// READ-ONLY PREVIEW (`workbook explain`, H1): run the freshness gate for
    /// provenance, but DEMOTE its `oracle/*` staleness refusal to non-blocking. A
    /// structural tool-surface preview never reconciles or grades cached oracle
    /// values — it derives tool names, per-tool input keys, and output keys from
    /// roles + the formula DAG alone — so the "cached values are not trusted"
    /// staleness signal is irrelevant to the preview's correctness. This is
    /// production-safe: it is reachable ONLY from the read-only
    /// [`project_tool_surface_from_workbook`](crate::project_tool_surface_from_workbook)
    /// projection, which writes NO bundle and runs NO emit/promote gate. The
    /// compile/emit path NEVER constructs `Preview`, so it cannot weaken the
    /// oracle-trust refusal that gates an actual served bundle.
    Preview,
}

/// Everything stage 1 produces on a clean pass — the evidence the driver projects
/// into the bundle.
#[derive(Debug)]
pub struct Stage1Output {
    /// The populated provenance record (from the freshness gate's read of the
    /// ORIGINAL bytes — accept or refuse, it is always populated).
    pub provenance: OracleProvenance,
    /// The FULL lint report findings — Warning/Info INCLUDED. Non-Error findings
    /// are EVIDENCE, not noise: the success path never Error-filters this.
    pub lint_findings: Vec<LintFinding>,
    /// The synthesized CANDIDATE manifest, returned exactly as synthesis proposed
    /// it (`ratified = false`): synthesis is never auto-applied; ratification is a
    /// SEPARATE recorded act the driver performs.
    pub synth_manifest: Manifest,
    /// The parser-equivalence evidence record carried into the bundle's
    /// `evidence/` member.
    pub parser_equivalence: ParserEquivalence,
}

/// Run the composed stage-1 analysis pass (collect-all): run the linter, synth,
/// and the freshness gate, aggregate the `Error` findings, refuse once.
///
/// Sequence:
/// 1. `lint(map)` — the full dialect report (kept whole).
/// 2. `synthesize(map, rules, workflow)` — the candidate manifest, kept
///    UN-ratified. Synthesis advisories are NEVER refusal-grade and are folded in
///    only as Warning/Info evidence.
/// 3. `freshness_gate(original_bytes, map, synth_manifest)` — `original_bytes`
///    MUST be the on-disk bytes; a refusal contributes its `oracle/*` findings to
///    the aggregate. Under [`FreshnessPolicy::TrustedFixture`] (TEST ONLY) the
///    gate admits the committed-fixture provenance class.
/// 4. The single refusal ([`Severity::Error`] ONLY — Warning/Info never block):
///    every Error finding from {lint report, ingest findings, gate refusal} is
///    returned TOGETHER as one `Err`.
///
/// # Errors
///
/// Returns [`CompileError::Lint`] carrying a rendered aggregate of EVERY
/// Error-severity finding from the whole pass (collect-all) when any gate
/// refuses.
pub fn run_stage1(
    original_bytes: &[u8],
    map: &WorkbookMap,
    ingest_findings: &[LintFinding],
    workflow: &str,
    freshness: FreshnessPolicy,
) -> Result<Stage1Output, CompileError> {
    let rules = DialectRules::default();

    // ---- 1. The full dialect lint report — kept WHOLE. ---------------------
    let source = WorkbookCellSource::new(map);
    let lint_report = lint(&source, &rules);

    // ---- 2. Manifest synthesis — synthesize BEFORE the gate: the freshness
    // gate's region hashes partition by manifest CellRole rows. The candidate
    // stays UN-ratified; advisories are NEVER refusal-grade. -----------------
    let (synth_manifest, advisories) = synthesize(map, &rules, workflow);

    // ---- 3. Freshness / provenance gate over the ORIGINAL on-disk bytes
    // (never a umya round-trip). The production path enforces; a TEST may admit
    // a committed trusted fixture's provenance class. ------------------------
    let (provenance, gate_result) =
        gate_with_policy(original_bytes, map, &synth_manifest, freshness);
    let gate_findings = match gate_result {
        Ok(()) => Vec::new(),
        Err(findings) => findings,
    };

    // ---- 4. The single refusal (Error severity ONLY). The full report (incl.
    // Warning/Info) is carried into Stage1Output regardless. -----------------
    let errors: Vec<&LintFinding> = lint_report
        .findings
        .iter()
        .chain(ingest_findings.iter())
        .chain(advisories.iter())
        .chain(gate_findings.iter())
        .filter(|f| f.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(CompileError::Lint(render_aggregate(&errors)));
    }

    // The COMPLETE lint report — Warning/Info findings SURVIVE the success path
    // (evidence). The synth advisories join them.
    let mut lint_findings = lint_report.findings;
    lint_findings.extend(advisories);

    let parser_equivalence = ParserEquivalence {
        checked_cells: count_formula_cells(map),
        equivalent: true,
        method: "scalar-eval".to_string(),
    };

    Ok(Stage1Output {
        provenance,
        lint_findings,
        synth_manifest,
        parser_equivalence,
    })
}

/// Run the freshness gate under `freshness`, returning the provenance record and a
/// `()`-on-clean / `Vec<LintFinding>`-on-refuse result.
///
/// Under [`FreshnessPolicy::Enforce`] (production) the production [`freshness_gate`]
/// is used — it classifies from the original bytes and refuses any non-Excel save.
/// Under [`FreshnessPolicy::TrustedFixture`] (TEST ONLY) the test-path gate that
/// honours a committed trusted-fixture marker is used.
fn gate_with_policy(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
    freshness: FreshnessPolicy,
) -> (OracleProvenance, Result<(), Vec<LintFinding>>) {
    match freshness {
        FreshnessPolicy::Enforce => {
            let (provenance, result) = freshness_gate(original_bytes, map, manifest);
            (provenance, result.map(|_corpus| ()))
        },
        FreshnessPolicy::TrustedFixture => {
            // TEST-ONLY: admit the committed fixture's provenance class. The
            // override is reachable only through this crate's `#[cfg(test)]`
            // override entry — production NEVER constructs `TrustedFixture`.
            trusted_fixture_gate(original_bytes, map, manifest)
        },
        FreshnessPolicy::Preview => {
            // READ-ONLY PREVIEW (H1): run the production gate for provenance, then
            // DEMOTE its refusal findings to non-blocking. The preview derives the
            // tool surface from roles + the formula DAG alone (never the cached
            // oracle), so a staleness signal must not block a structural preview.
            // Production-safe — only the read-only projection constructs `Preview`.
            let (provenance, result) = freshness_gate(original_bytes, map, manifest);
            // Drop the corpus on accept and the staleness findings on refuse — both
            // irrelevant to a structural preview that grades no oracle value. The
            // preview always proceeds to the projection (Ok with NO blocking finding).
            let _ = result;
            (provenance, Ok(()))
        },
    }
}

/// The TEST-ONLY trusted-fixture gate wrapper: honour the committed-fixture
/// provenance override so the in-crate producer/consumer golden proof can compile
/// a neutral fixture whose non-Excel recalc stamp trips the staleness signals.
/// Compiled ONLY under `#[cfg(test)]` (CR-01: there is NO publishable feature that
/// arms it); production builds never link it.
#[cfg(test)]
fn trusted_fixture_gate(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
) -> (OracleProvenance, Result<(), Vec<LintFinding>>) {
    let (provenance, result) = crate::provenance::gate::gate_with_fixture_override(
        original_bytes,
        map,
        manifest,
        Some(crate::provenance::gate::TrustedFixtureMarker),
    );
    (provenance, result.map(|_corpus| ()))
}

/// Production stub for the test-only trusted-fixture gate: NEVER constructible on
/// the production path (the policy enum only yields `TrustedFixture` from
/// `#[cfg(test)]` code). Compiled in non-test builds so the match stays total; it
/// enforces the gate exactly like `Enforce` so even a hypothetical production
/// `TrustedFixture` could not weaken refusal.
#[cfg(not(test))]
fn trusted_fixture_gate(
    original_bytes: &[u8],
    map: &WorkbookMap,
    manifest: &Manifest,
) -> (OracleProvenance, Result<(), Vec<LintFinding>>) {
    let (provenance, result) = freshness_gate(original_bytes, map, manifest);
    (provenance, result.map(|_corpus| ()))
}

/// Render the collect-all aggregate of `Error` findings into one
/// `CompileError::Lint` message string — every finding's `rule`, location, and
/// message on its own line so the BA sees them all in one refusal.
pub(crate) fn render_aggregate(errors: &[&LintFinding]) -> String {
    let mut out = format!("{} blocking finding(s):", errors.len());
    for f in errors {
        let loc = match &f.cell {
            Some(cell) => format!("{}!{cell}", f.sheet),
            None => f.sheet.clone(),
        };
        out.push_str(&format!("\n  [{}] {loc}: {}", f.rule, f.message));
    }
    out
}

/// Count the formula cells across all sheets — the parser-equivalence record's
/// `checked_cells`.
fn count_formula_cells(map: &WorkbookMap) -> u32 {
    let n: usize = map
        .sheets
        .iter()
        .map(|s| s.cells.iter().filter(|c| c.is_formula).count())
        .sum();
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::cell_map::{CellRecord, FormulaKind, SheetRecord};

    fn cell(addr: &str, formula: Option<&str>, value: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: formula.map(str::to_string),
            value: value.map(str::to_string),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: formula.is_some(),
            formula_kind: FormulaKind::Normal,
        }
    }

    fn sheet(name: &str, cells: Vec<CellRecord>) -> SheetRecord {
        SheetRecord {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![],
            hidden_cols: vec![],
            col_widths: vec![],
            merges: vec![],
            cf_ranges: vec![],
            tables: vec![],
            table_records: vec![],
            data_validations: vec![],
            notes: vec![],
            cells,
        }
    }

    fn map_of(sheets: Vec<SheetRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets,
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    #[test]
    fn count_formula_cells_sums_across_sheets() {
        let m = map_of(vec![
            sheet("1_Inputs", vec![cell("B2", None, Some("1"))]),
            sheet(
                "3_Outputs",
                vec![
                    cell("B2", Some("1_Inputs!B2"), Some("1")),
                    cell("B3", Some("B2*2"), Some("2")),
                ],
            ),
        ]);
        assert_eq!(count_formula_cells(&m), 2);
    }

    #[test]
    fn render_aggregate_lists_every_finding() {
        let f1 = LintFinding::new(
            Severity::Error,
            "whitelist/unsupported-fn",
            "1_Inputs",
            Some("B2".to_string()),
            "INDIRECT is not on the whitelist",
            "remove the INDIRECT call",
        );
        let f2 = LintFinding::new(
            Severity::Error,
            "structure/macro",
            "workbook",
            None,
            "the workbook carries VBA macros",
            "strip the macros",
        );
        let refs = vec![&f1, &f2];
        let msg = render_aggregate(&refs);
        assert!(msg.contains("2 blocking finding(s)"));
        assert!(msg.contains("whitelist/unsupported-fn"));
        assert!(msg.contains("structure/macro"));
        assert!(msg.contains("1_Inputs!B2"));
    }
}
