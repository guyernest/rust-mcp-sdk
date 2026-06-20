//! Candidate manifest SYNTHESIS (WBCO-02/06, the heart of "stay-in-Excel").
//!
//! [`synthesize`] PROPOSES a candidate [`Manifest`] from colour + the `0_Guide`
//! legend + block headers; it marks the result `ratified = false` (D-04 — never
//! auto-applied; ratification is [`super::ratify`]). Colour only PROPOSES, it never
//! decides — the ratified manifest is canonical.
//!
//! # The WBCO-02 generalization fix (the §5 gap this plan closes)
//!
//! The prior reference implementation hardcoded a single workflow name and a
//! per-workbook reference manifest. THIS synthesis is fully workbook-driven: the
//! `workflow` name is a PARAMETER (the CLI/driver passes it; no hardcoded
//! reference-manifest builder, no customer-specific literal), and roles come solely
//! from colour/Guide/header evidence over the real [`WorkbookMap`].
//!
//! # BA-string info-flow boundary (T-93-04-INJ)
//!
//! Every BA-authored `meaning`/`unit`/enum-label string is sanitized + length-capped
//! ([`super::projections`]) BEFORE it enters the manifest, because those strings
//! reach the agent LLM through the served tool schema. An overflow emits a WARNING,
//! never a hard block (D-04).
//!
//! # DV → enum fork (WBCO-06/D-06)
//!
//! An inline DV literal list (≤10 distinct) freezes to a closed enum; a
//! range/named-range/governed/formula source falls back to a DYNAMIC input with a
//! precise reason-code WARNING and a schema-valid default that does not conflict
//! with WR-01 enum-tiering.
//!
//! # Unclassifiable cells (D-05)
//!
//! A cell the compiler cannot confidently classify stays in the computation but is
//! NOT exposed as an input/output. A warning fires ONLY when the cell LOOKS
//! exposable (a bare hardcoded number with no role signal) — obvious internal
//! helpers stay silent.

use std::collections::HashMap;

use crate::dialect::{CandidateRole, DialectRules};
use crate::ingest::{
    cell_key, CellRecord, DataValidationRecord, DefinedNameRecord, DefinedNameScope, RangeRef,
    WorkbookMap,
};
use crate::{LintFinding, Severity};

use super::model::{CellRole, Dtype, Manifest, Role};
use super::projections::{
    is_inline_literal, resolve_inline_list, sanitize_opt, MAX_INPUT_COUNT, MAX_MEANING_LEN,
    MAX_OUTPUT_COUNT,
};

/// The current manifest schema version synthesis stamps.
const SCHEMA_VERSION: u32 = 1;
/// The provenance label for a colour+Guide-classified role.
const SOURCE_COLOUR_GUIDE: &str = "colour+guide";
/// The provenance label for a yellow-assumption-classified role.
const SOURCE_YELLOW_ASSUMPTION: &str = "yellow-assumption";
/// The Guide legend sheet name (the spec-driven-from-data ARGB→role source).
const GUIDE_SHEET: &str = "0_Guide";

// ── The DV static→enum fork reason codes (D-06; the BA-actionable disqualifier) ──

/// The DV is not a `list` validation.
const REASON_NON_LIST: &str = "non_list";
/// The list source is a range / NAMED RANGE / formula reference (not an inline
/// quoted literal). Named/range-backed resolution is a documented DEFERRED seam.
const REASON_NOT_INLINE_LITERAL: &str = "not_inline_literal";
/// The inline literal carries more than 10 DISTINCT values (after trim+dedup).
const REASON_TOO_MANY_VALUES: &str = "too_many_values";
/// The list source range contains formula (computed) cells.
const REASON_FORMULA_SOURCE: &str = "formula_source";
/// The covered input cell's inferred dtype is NOT `Text` (a numeric/bool input
/// would advertise a string-enum no client value can satisfy — fail-open).
const REASON_NON_TEXT_DTYPE: &str = "non_text_dtype";

/// Whether an A1 `addr` falls inside a [`RangeRef`] (inclusive). Conservatively
/// `false` on any unparseable endpoint (a named range never matches a cell here).
/// Local to synth — the dialect linter does not export a range-membership helper.
pub(crate) fn addr_in_range(addr: &str, range: &RangeRef) -> bool {
    let Some((ac, ar)) = split_a1(addr) else {
        return false;
    };
    let Some((sc, sr)) = split_a1(&range.start) else {
        return false;
    };
    let Some((ec, er)) = split_a1(&range.end) else {
        return false;
    };
    let (lo_c, hi_c) = (sc.min(ec), sc.max(ec));
    let (lo_r, hi_r) = (sr.min(er), sr.max(er));
    ac >= lo_c && ac <= hi_c && ar >= lo_r && ar <= hi_r
}

/// Split an A1 address (`"E6"`, `"$E$6"`) into `(column index, row number)`,
/// 1-based. `None` on a malformed address (e.g. a named range).
fn split_a1(addr: &str) -> Option<(u32, u32)> {
    let cleaned = addr.replace('$', "");
    let split = cleaned.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = cleaned.split_at(split);
    let mut col_idx: u32 = 0;
    for c in col.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        col_idx = col_idx * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }
    let row_num: u32 = row.parse().ok()?;
    (col_idx > 0 && row_num > 0).then_some((col_idx, row_num))
}

/// Build a CANDIDATE [`Manifest`] for `workflow` from colour + the `0_Guide`
/// legend + headers over the real [`WorkbookMap`], plus the collect-all findings.
///
/// `workflow` is the WORKFLOW NAME the CLI/driver supplies — the WBCO-02
/// generalization fix: it is NEVER hardcoded and never a per-workbook literal.
///
/// Returns `(candidate_manifest, findings)`. The findings carry: the A5
/// Guide-missing advisory, each DV-dynamic reason code (D-06), each BA-string
/// truncation warning (T-93-04-INJ), each unclassifiable-but-exposable warning
/// (D-05), and the input/output count-cap warnings.
pub fn synthesize(
    wb: &WorkbookMap,
    rules: &DialectRules,
    workflow: &str,
) -> (Manifest, Vec<LintFinding>) {
    let mut findings = Vec::new();

    // Step 1: confirm the Guide legend (absence ⇒ fallback palette + A5 advisory).
    let guide_present = wb.sheets.iter().any(|s| s.name == GUIDE_SHEET);
    if !guide_present {
        findings.push(LintFinding::new(
            Severity::Warning,
            "manifest/guide-missing",
            GUIDE_SHEET,
            None,
            format!(
                "the `{GUIDE_SHEET}` legend sheet is absent; classifying roles from the \
                 hardcoded fallback colour palette (A5)"
            ),
            format!(
                "add a `{GUIDE_SHEET}` legend mapping each colour ARGB to its role so \
                 synthesis is spec-driven-from-data"
            ),
        ));
    }

    // Steps 2–4: classify every coloured / formula cell into a CellRole, applying
    // the DV→enum fork on inputs and the D-05 unclassifiable-but-exposable warning.
    let mut cells: Vec<CellRole> = Vec::new();
    for sheet in &wb.sheets {
        if sheet.name == GUIDE_SHEET {
            continue; // the legend is documentation, not role-bearing data
        }
        for cell in &sheet.cells {
            match classify_cell(&sheet.name, cell, rules, &mut findings) {
                Some(mut cell_role) => {
                    if cell_role.role == Role::Input {
                        apply_dv_fork(&mut cell_role, sheet, cell, wb, &mut findings);
                    }
                    cells.push(cell_role);
                },
                None => {
                    // D-05: an UNclassified cell stays internal. Warn ONLY when it
                    // LOOKS exposable (a bare hardcoded number with no colour role).
                    if looks_exposable(cell) {
                        findings.push(unclassifiable_finding(&sheet.name, &cell.addr));
                    }
                },
            }
        }
    }

    // DETERMINISM: sort by the fully-qualified cell key so the artifact is stable.
    cells.sort_by(|a, b| a.cell.cmp(&b.cell));

    // Count caps (the info-flow boundary on exposed surface — MAX_INPUT/OUTPUT).
    enforce_count_caps(&cells, &mut findings);

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        // WBCO-02: the workflow name comes from the caller, NEVER a hardcoded literal.
        workflow: workflow.to_string(),
        workbook_hash: None,
        ratified: false,
        ratified_by: None,
        ratified_at: None,
        cells,
        loop_block: None,
        governed_data: Vec::new(),
        changelog: vec![],
        capability_calls: vec![],
        // The in-repo struct field the lighthouse lacked: synthesized candidates
        // carry no Guide annotations, so an empty Vec (D-18 additive contract).
        annotations: vec![],
    };

    (manifest, findings)
}

/// Apply the DV → enum fork to an INPUT cell role (D-06): freeze an eligible inline
/// literal list to `allowed_values`; otherwise leave it DYNAMIC and emit a precise
/// reason-coded WARNING. Never blocks — a dynamic input is schema-valid (WR-01-safe).
fn apply_dv_fork(
    cell_role: &mut CellRole,
    sheet: &crate::ingest::SheetRecord,
    cell: &CellRecord,
    wb: &WorkbookMap,
    findings: &mut Vec<LintFinding>,
) {
    let Some(dv) = sheet
        .data_validations
        .iter()
        .find(|dv| addr_in_range(&cell.addr, &dv.target))
    else {
        return;
    };
    match freeze_or_reason(dv, &sheet.name, wb, cell_role.dtype) {
        Ok(values) => cell_role.allowed_values = Some(values),
        Err(reason) => findings.push(dv_dynamic_finding(&sheet.name, &cell.addr, reason)),
    }
}

/// The D-06 disqualifier predicate: FREEZE (inline-literal token set) vs DYNAMIC
/// (precise reason code) for one covering DV. Check order fixes reason precedence:
/// `non_list` → `non_text_dtype` → inline-literal resolution (`too_many_values`) →
/// formula source (`formula_source`) → `not_inline_literal` (range/named-range).
fn freeze_or_reason(
    dv: &DataValidationRecord,
    dv_sheet: &str,
    wb: &WorkbookMap,
    cell_dtype: Dtype,
) -> Result<Vec<String>, &'static str> {
    if dv.dv_type != "list" {
        return Err(REASON_NON_LIST);
    }
    if cell_dtype != Dtype::Text {
        return Err(REASON_NON_TEXT_DTYPE);
    }
    let Some(formula1) = dv.formula1.as_deref() else {
        return Err(REASON_NOT_INLINE_LITERAL);
    };

    if is_inline_literal(formula1) {
        return match resolve_inline_list(formula1) {
            Some(values) => Ok(values),
            None => Err(REASON_TOO_MANY_VALUES),
        };
    }

    // A range / NAMED RANGE / formula-backed source — static resolution DEFERRED.
    let source = range_source(formula1, dv_sheet);
    if source_range_has_formula(&source, wb) {
        return Err(REASON_FORMULA_SOURCE);
    }
    Err(REASON_NOT_INLINE_LITERAL)
}

/// Parse a non-literal `formula1` source reference into a best-effort [`RangeRef`]
/// for the DISQUALIFIER checks only (never value resolution).
fn range_source(formula1: &str, default_sheet: &str) -> RangeRef {
    let s = formula1.trim().trim_start_matches('=').trim();
    let (sheet, addrs) = match s.rsplit_once('!') {
        Some((sheet, rest)) => (sheet.trim().trim_matches('\'').to_string(), rest),
        None => (default_sheet.to_string(), s),
    };
    let (start, end) = match addrs.split_once(':') {
        Some((start, end)) => (start, end),
        None => (addrs, addrs),
    };
    RangeRef {
        sheet,
        start: start.trim().replace('$', ""),
        end: end.trim().replace('$', ""),
    }
}

/// Whether any cell inside the source range is a formula (computed) cell.
fn source_range_has_formula(source: &RangeRef, wb: &WorkbookMap) -> bool {
    wb.sheets
        .iter()
        .find(|s| s.name == source.sheet)
        .is_some_and(|s| {
            s.cells
                .iter()
                .any(|c| c.is_formula && addr_in_range(&c.addr, source))
        })
}

/// The collect-all advisory for a covering DV list that stays DYNAMIC, CARRYING the
/// precise reason code (BA-actionable; never a block).
fn dv_dynamic_finding(sheet: &str, addr: &str, reason: &'static str) -> LintFinding {
    LintFinding::new(
        Severity::Warning,
        "manifest/dv-dynamic",
        sheet,
        Some(addr.to_string()),
        format!(
            "the data-validation list covering {sheet}!{addr} does not freeze to a \
             closed enum (reason: `{reason}`); `allowed_values` stays unset and the \
             input remains DYNAMIC"
        ),
        format!(
            "no action needed for a deliberately dynamic list (reason `{reason}`); to \
             freeze, make the source an inline quoted literal of ≤10 distinct text \
             values (D-06)"
        ),
    )
}

/// Classify ONE cell into a candidate [`CellRole`], or `None` when the cell carries
/// no colour/formula role signal. BA-string metadata is sanitized + capped BEFORE
/// it enters the role (T-93-04-INJ); a truncation emits a WARNING.
fn classify_cell(
    sheet: &str,
    cell: &CellRecord,
    rules: &DialectRules,
    findings: &mut Vec<LintFinding>,
) -> Option<CellRole> {
    let candidate = rules.candidate_role(
        cell.fill_argb.as_deref(),
        cell.font_argb.as_deref(),
        cell.is_formula,
    )?;

    let (role, source) = match candidate {
        CandidateRole::Input => (Role::Input, SOURCE_COLOUR_GUIDE),
        CandidateRole::Constant => (Role::Constant, SOURCE_COLOUR_GUIDE),
        CandidateRole::Assumption => (Role::Constant, SOURCE_YELLOW_ASSUMPTION),
        CandidateRole::Formula => (Role::Formula, SOURCE_COLOUR_GUIDE),
    };

    let colour_evidence = match candidate {
        CandidateRole::Input => cell.font_argb.clone(),
        CandidateRole::Constant | CandidateRole::Assumption => cell.fill_argb.clone(),
        CandidateRole::Formula => None,
    };

    // Sanitize + cap the BA-authored `meaning` (the cell's own text) BEFORE it
    // enters the manifest — it reaches the agent LLM through the served schema.
    let mut meaning = cell.value.clone();
    if sanitize_opt(&mut meaning, MAX_MEANING_LEN) {
        findings.push(metadata_truncated_finding(sheet, &cell.addr, "meaning"));
    }

    Some(CellRole {
        cell: cell_key(sheet, &cell.addr),
        role,
        name: None,
        unit: None,
        meaning,
        dtype: infer_dtype(cell),
        colour_evidence,
        source: source.to_string(),
        notes: None,
        tier: None,
        allowed_values: None,
    })
}

/// The WARNING emitted when a BA-authored metadata string was truncated at its cap
/// (T-93-04-INJ — never a hard block; keep the BA in Excel).
fn metadata_truncated_finding(sheet: &str, addr: &str, field: &str) -> LintFinding {
    LintFinding::new(
        Severity::Warning,
        "manifest/metadata-truncated",
        sheet,
        Some(addr.to_string()),
        format!(
            "the BA-authored `{field}` at {sheet}!{addr} exceeded its length cap and was \
             truncated before entering the manifest (the info-flow boundary for strings \
             reaching the agent)"
        ),
        format!("shorten the `{field}` text in the cell so it fits within the documented cap"),
    )
}

/// D-05: a warning for a cell that LOOKS exposable but could not be confidently
/// classified — it stays internal (never an input/output), but the BA is told.
fn unclassifiable_finding(sheet: &str, addr: &str) -> LintFinding {
    LintFinding::new(
        Severity::Warning,
        "manifest/unclassifiable-cell",
        sheet,
        Some(addr.to_string()),
        format!(
            "{sheet}!{addr} looks like it should be exposed (a bare hardcoded value) but \
             carries no colour/role signal; it stays an INTERNAL helper and is NOT exposed"
        ),
        "if this cell should be an input or constant, colour it per the `0_Guide` legend so \
         synthesis can classify it",
    )
}

/// Whether a role-less cell LOOKS exposable (D-05): a bare hardcoded NUMBER value
/// with no formula and no colour signal. A formula cell or a label/text cell is an
/// obvious internal helper and stays silent (avoid warning noise).
fn looks_exposable(cell: &CellRecord) -> bool {
    if cell.is_formula || cell.fill_argb.is_some() || cell.font_argb.is_some() {
        return false;
    }
    cell.value
        .as_deref()
        .is_some_and(|v| v.trim().parse::<f64>().is_ok())
}

/// Enforce the exposed input/output COUNT caps (the info-flow boundary on exposed
/// surface). Over-cap is a WARNING, never a block — the manifest still synthesizes.
fn enforce_count_caps(cells: &[CellRole], findings: &mut Vec<LintFinding>) {
    let inputs = cells.iter().filter(|c| c.role == Role::Input).count();
    if inputs > MAX_INPUT_COUNT {
        findings.push(LintFinding::new(
            Severity::Warning,
            "manifest/too-many-inputs",
            String::new(),
            None,
            format!("the manifest exposes {inputs} inputs, over the cap of {MAX_INPUT_COUNT}"),
            "reduce the number of exposed input cells",
        ));
    }
    let outputs = cells.iter().filter(|c| c.role == Role::Output).count();
    if outputs > MAX_OUTPUT_COUNT {
        findings.push(LintFinding::new(
            Severity::Warning,
            "manifest/too-many-outputs",
            String::new(),
            None,
            format!("the manifest exposes {outputs} outputs, over the cap of {MAX_OUTPUT_COUNT}"),
            "reduce the number of exposed output cells",
        ));
    }
}

/// Infer the declared [`Dtype`] from a cell's value/formula text. A formula cell or
/// a numeric-parseable value is `Number`; otherwise `Text` (conservative — the
/// manifest can be refined by the BA).
fn infer_dtype(cell: &CellRecord) -> Dtype {
    if cell.is_formula {
        return Dtype::Number;
    }
    match &cell.value {
        Some(v) if v.trim().parse::<f64>().is_ok() => Dtype::Number,
        _ => Dtype::Text,
    }
}

/// The D-04 two-layer overlap consistency check: a named-range NAME prefix
/// (`in_`/`const_`/`out_`) implies a role that MUST equal the manifest role at the
/// range's target cell; a mismatch is a `Severity::Error` `manifest/role-conflict`
/// finding that names both roles + the range (never silently resolved).
pub fn check_overlap(manifest: &Manifest, named_ranges: &[DefinedNameRecord]) -> Vec<LintFinding> {
    let mut findings = Vec::new();

    let manifest_roles: HashMap<&str, Role> = manifest
        .cells
        .iter()
        .map(|c| (c.cell.as_str(), c.role))
        .collect();

    for dn in named_ranges {
        let Some(implied) = Role::from_name_prefix(&dn.name) else {
            continue;
        };
        let cell_key = cell_key(&dn.target.sheet, &dn.target.start);
        let scope_label = scope_label(&dn.scope);

        match manifest_roles.get(cell_key.as_str()).copied() {
            Some(actual) if actual == implied => {},
            Some(actual) => {
                findings.push(LintFinding::new(
                    Severity::Error,
                    "manifest/role-conflict",
                    dn.target.sheet.clone(),
                    Some(dn.target.start.clone()),
                    format!(
                        "named range `{}` ({scope_label}) implies role `{:?}` by its prefix, \
                         but the manifest assigns role `{:?}` at {cell_key}; the two layers disagree",
                        dn.name, implied, actual
                    ),
                    format!(
                        "reconcile the named range `{}` and the manifest role at {cell_key} so \
                         both layers agree (D-04)",
                        dn.name
                    ),
                ));
            },
            None => {
                findings.push(LintFinding::new(
                    Severity::Error,
                    "manifest/role-conflict",
                    dn.target.sheet.clone(),
                    Some(dn.target.start.clone()),
                    format!(
                        "named range `{}` ({scope_label}) implies role `{:?}` by its prefix, \
                         but the manifest has no role for {cell_key}; the two layers disagree",
                        dn.name, implied
                    ),
                    format!(
                        "add a manifest role for {cell_key} (or remove the named range `{}`) so \
                         both layers agree (D-04)",
                        dn.name
                    ),
                ));
            },
        }
    }

    findings
}

/// A short human label for a [`DefinedNameScope`].
fn scope_label(scope: &DefinedNameScope) -> String {
    match scope {
        DefinedNameScope::Workbook => "workbook-scoped".to_string(),
        DefinedNameScope::Worksheet(sheet) => format!("worksheet-scoped on `{sheet}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{FormulaKind, SheetRecord};

    fn cell(
        addr: &str,
        fill: Option<&str>,
        font: Option<&str>,
        is_formula: bool,
        value: Option<&str>,
    ) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: if is_formula {
                Some("SUM(A1:A2)".to_string())
            } else {
                None
            },
            value: value.map(|v| v.to_string()),
            fill_argb: fill.map(|c| c.to_string()),
            font_argb: font.map(|c| c.to_string()),
            number_format: None,
            is_formula,
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

    fn wb(sheets: Vec<SheetRecord>, defined_names: Vec<DefinedNameRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets,
            defined_names,
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn input_cell(addr: &str, value: Option<&str>) -> CellRecord {
        cell(addr, None, Some("FF0000FF"), false, value)
    }

    fn dv(start: &str, end: &str, dv_type: &str, formula1: Option<&str>) -> DataValidationRecord {
        DataValidationRecord {
            target: RangeRef {
                sheet: "1_Inputs".to_string(),
                start: start.to_string(),
                end: end.to_string(),
            },
            dv_type: dv_type.to_string(),
            formula1: formula1.map(|f| f.to_string()),
        }
    }

    fn wb_with_dv(dv: DataValidationRecord, value: Option<&str>) -> WorkbookMap {
        let mut s = sheet("1_Inputs", vec![input_cell("C6", value)]);
        s.data_validations = vec![dv];
        wb(vec![s], vec![])
    }

    fn c6_allowed(manifest: &Manifest) -> Option<Vec<String>> {
        manifest
            .cells
            .iter()
            .find(|c| c.cell == "1_Inputs!C6")
            .expect("C6 is classified as a role cell")
            .allowed_values
            .clone()
    }

    fn has_reason(findings: &[LintFinding], reason: &str) -> bool {
        findings
            .iter()
            .any(|f| f.rule == "manifest/dv-dynamic" && f.message.contains(&format!("`{reason}`")))
    }

    // ── WBCO-02: workflow is a parameter, never hardcoded ────────────────────

    #[test]
    fn synth_workflow_name_comes_from_the_parameter_not_a_literal() {
        let map = wb(
            vec![sheet(
                "1_Inputs",
                vec![cell("E6", None, Some("FF0000FF"), false, Some("42"))],
            )],
            vec![],
        );
        let (m1, _) = synthesize(&map, &DialectRules::default(), "tax-calc");
        assert_eq!(m1.workflow, "tax-calc");
        let (m2, _) = synthesize(&map, &DialectRules::default(), "vat-return");
        assert_eq!(m2.workflow, "vat-return", "no hardcoded workflow literal");
    }

    #[test]
    fn manifest_has_annotations_field_and_ratification_stamps() {
        let map = wb(
            vec![sheet(
                "1_Inputs",
                vec![cell("E6", None, Some("FF0000FF"), false, Some("42"))],
            )],
            vec![],
        );
        let (m, _) = synthesize(&map, &DialectRules::default(), "wf");
        // The in-repo field the lighthouse lacked: present + empty for a candidate.
        assert!(
            m.annotations.is_empty(),
            "candidate carries empty annotations"
        );
        assert!(!m.ratified, "a synthesized manifest is a CANDIDATE (D-04)");
        assert_eq!(m.ratified_by, None);
        assert_eq!(m.ratified_at, None);
        // Round-trips through serde with the annotations field handled.
        let json = serde_json::to_string(&m).expect("serialize");
        let back: Manifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, back);
    }

    #[test]
    fn synthesizes_input_and_constant_cells() {
        let map = wb(
            vec![sheet(
                "1_Inputs",
                vec![
                    cell("E6", None, Some("FF0000FF"), false, Some("42")),
                    cell("B2", Some("FFE2EFDA"), None, false, Some("0.37")),
                ],
            )],
            vec![],
        );
        let (m, _) = synthesize(&map, &DialectRules::default(), "wf");
        assert!(m.cells.iter().any(|c| c.role == Role::Input));
        assert!(m.cells.iter().any(|c| c.role == Role::Constant));
    }

    #[test]
    fn yellow_fill_is_a_constant_with_assumption_source() {
        let map = wb(
            vec![sheet(
                "2_Constants",
                vec![cell("B2", Some("FFFFFF00"), None, false, Some("12.5"))],
            )],
            vec![],
        );
        let (m, _) = synthesize(&map, &DialectRules::default(), "wf");
        let yellow = m
            .cells
            .iter()
            .find(|c| c.cell == "2_Constants!B2")
            .expect("classified");
        assert_eq!(yellow.role, Role::Constant);
        assert_eq!(yellow.source, "yellow-assumption");
    }

    // ── WBCO-06 / D-06: inline DV → enum, range/named DV → dynamic warning ────

    #[test]
    fn synth_inline_dv_becomes_enum() {
        let map = wb_with_dv(
            dv("C6", "C6", "list", Some("\"single,married\"")),
            Some("single"),
        );
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        assert_eq!(
            c6_allowed(&m),
            Some(vec!["single".to_string(), "married".to_string()]),
            "an inline DV literal ≤10 freezes to a closed enum"
        );
        assert!(!findings.iter().any(|f| f.rule == "manifest/dv-dynamic"));
    }

    #[test]
    fn synth_range_dv_falls_back_to_dynamic_with_reason_and_safe_default() {
        // A named-range source never freezes — DYNAMIC input + a precise warning.
        let map = wb_with_dv(dv("C6", "C6", "list", Some("=SomeName")), Some("alpha"));
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        assert_eq!(c6_allowed(&m), None, "range/named DV stays DYNAMIC (D-06)");
        assert!(has_reason(&findings, "not_inline_literal"));
        // The warning never blocks: it is Warning severity, not Error.
        let f = findings
            .iter()
            .find(|f| f.rule == "manifest/dv-dynamic")
            .expect("a dv-dynamic finding");
        assert_eq!(f.severity, Severity::Warning, "a range DV is never a block");
    }

    #[test]
    fn synth_too_many_values_stays_dynamic() {
        let map = wb_with_dv(
            dv(
                "C6",
                "C6",
                "list",
                Some("\"v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v11\""),
            ),
            Some("v1"),
        );
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        assert_eq!(c6_allowed(&m), None);
        assert!(has_reason(&findings, "too_many_values"));
    }

    #[test]
    fn synth_non_text_input_stays_dynamic() {
        // A numeric input must NOT freeze a string enum (fail-open guard).
        let mut s = sheet("1_Inputs", vec![input_cell("C6", Some("42"))]);
        s.data_validations = vec![dv("C6", "C6", "list", Some("\"1,2,3\""))];
        let map = wb(vec![s], vec![]);
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        assert_eq!(c6_allowed(&m), None);
        assert!(has_reason(&findings, "non_text_dtype"));
    }

    // ── D-05: unclassifiable-but-exposable warning ───────────────────────────

    #[test]
    fn synth_unclassifiable_internal_warns_only_when_exposable() {
        // A bare hardcoded NUMBER with no colour signal LOOKS exposable → warns,
        // and is NOT exposed (no role cell synthesized for it).
        let map = wb(
            vec![sheet(
                "1_Calc",
                vec![
                    cell("A1", None, None, false, Some("99")), // bare number → warn
                    cell("A2", None, None, false, Some("a label")), // text → silent
                    cell("A3", None, None, true, Some("5")),   // formula → silent
                ],
            )],
            vec![],
        );
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        // The formula cell A3 is a Formula role (stays in the computation, not an
        // exposed input/output); the bare-number A1 and the text label A2 carry no
        // colour signal, so NEITHER is exposed as an input/constant/output (D-05).
        assert!(
            !m.cells
                .iter()
                .any(|c| matches!(c.role, Role::Input | Role::Constant | Role::Output)),
            "unclassifiable cells are never exposed as input/constant/output (D-05)"
        );
        assert!(
            !m.cells.iter().any(|c| c.cell.ends_with("!A1")),
            "the bare-number cell stays internal, not a role cell"
        );
        let warns: Vec<_> = findings
            .iter()
            .filter(|f| f.rule == "manifest/unclassifiable-cell")
            .collect();
        assert_eq!(
            warns.len(),
            1,
            "only the bare-number cell warns; the label + formula stay silent"
        );
        assert_eq!(warns[0].cell.as_deref(), Some("A1"));
    }

    // ── T-93-04-INJ: BA-string caps applied before entering the manifest ─────

    #[test]
    fn ba_string_metadata_capped() {
        // An over-long `meaning` (the cell's own text) is truncated to the cap +
        // a WARNING fires; control chars are stripped — never a hard block.
        let long = format!("Total\u{0007} area {}", "x".repeat(400));
        let map = wb(
            vec![sheet(
                "1_Inputs",
                vec![cell("E6", None, Some("FF0000FF"), false, Some(&long))],
            )],
            vec![],
        );
        let (m, findings) = synthesize(&map, &DialectRules::default(), "wf");
        let role = m
            .cells
            .iter()
            .find(|c| c.cell == "1_Inputs!E6")
            .expect("classified");
        let meaning = role.meaning.as_deref().expect("a meaning");
        assert_eq!(meaning.chars().count(), MAX_MEANING_LEN, "capped to MAX");
        assert!(!meaning.contains('\u{0007}'), "control char stripped");
        assert!(findings
            .iter()
            .any(|f| f.rule == "manifest/metadata-truncated"));
    }

    // ── D-04 overlap check ───────────────────────────────────────────────────

    #[test]
    fn overlap_conflict_is_one_located_error_naming_the_range() {
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            workflow: "wf".to_string(),
            workbook_hash: None,
            ratified: false,
            ratified_by: None,
            ratified_at: None,
            cells: vec![CellRole {
                cell: "1_Inputs!E6".to_string(),
                role: Role::Constant,
                name: None,
                unit: None,
                meaning: None,
                dtype: Dtype::Number,
                colour_evidence: None,
                source: SOURCE_COLOUR_GUIDE.to_string(),
                notes: None,
                tier: None,
                allowed_values: None,
            }],
            loop_block: None,
            governed_data: Vec::new(),
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        };
        let named = vec![DefinedNameRecord {
            name: "in_total_area".to_string(),
            target: RangeRef {
                sheet: "1_Inputs".to_string(),
                start: "E6".to_string(),
                end: "E6".to_string(),
            },
            scope: DefinedNameScope::Workbook,
        }];
        let findings = check_overlap(&manifest, &named);
        let conflicts: Vec<_> = findings
            .iter()
            .filter(|f| f.rule == "manifest/role-conflict")
            .collect();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].severity, Severity::Error);
        assert!(conflicts[0].message.contains("in_total_area"));
    }

    #[test]
    fn addr_in_range_is_inclusive_and_false_on_named_ranges() {
        let r = RangeRef {
            sheet: "S".to_string(),
            start: "A1".to_string(),
            end: "C3".to_string(),
        };
        assert!(addr_in_range("B2", &r));
        assert!(addr_in_range("A1", &r));
        assert!(addr_in_range("C3", &r));
        assert!(!addr_in_range("D4", &r));
        // A named range as an endpoint never matches a cell.
        let named = RangeRef {
            sheet: "S".to_string(),
            start: "SomeName".to_string(),
            end: "SomeName".to_string(),
        };
        assert!(!addr_in_range("A1", &named));
    }
}
