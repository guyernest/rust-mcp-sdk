//! The trace-driven mismatch classifier (WBCO-04, operand-anchored, D-03).
//!
//! [`classify`] runs on EVERY cell where `computed != target`. It assigns exactly
//! one of the named [`MismatchClass`] variants from EVIDENCE — the formula [`Expr`]
//! AST, the operand [`CellValue`]s, the manifest [`Role`], and the per-cell
//! [`EvalTrace`] the executor emitted — or refuses (`Unclassified` = HARD FAIL).
//!
//! # Two non-negotiable invariants
//!
//! - **Operand-anchored, no magnitude inference (the milestone's named trap):**
//!   `RoundingBoundary` fires ONLY when the deciding cell's `Expr` actually contains
//!   a `ROUND`/`ROUNDUP`/`CEILING` call AND the operand sits within
//!   [`BOUNDARY_EPSILON`] of the rounding boundary (checked via the runtime rounding
//!   helpers), AND the divergence is ≤ ONE operand-derived rounding step. A naïve
//!   blanket abs-of-the-delta tolerance is FORBIDDEN — a grep gate asserts that
//!   literal pattern never appears in this file (the bound is always the
//!   operand-anchored rounding step, never the gap magnitude itself).
//! - **Non-numeric cached cells never panic (Codex MEDIUM):** a cached cell holding
//!   TEXT / BOOL / BLANK / an Excel error (`#REF!`, `#DIV/0!`) is compared
//!   structurally (exact for text/bool/blank; an error side routes to
//!   `ErrorPropagation`), never coerced through a numeric path that could panic.

use serde::Serialize;

// The IR / manifest-model / executor types live in `pmcp-workbook-runtime`; the
// rounding helpers live under its `sheet_ir::rounding`. Re-use, never re-declare.
use pmcp_workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup};
use pmcp_workbook_runtime::{CellValue, EvalTrace, ExcelError, Expr, Manifest, Role};

/// The relative epsilon within which an operand is judged to sit "on" a rounding
/// boundary — the same scale the deterministic rounding helpers use to undo
/// binary-`f64` representation error.
pub const BOUNDARY_EPSILON: f64 = 1e-6;

/// The class a `computed != target` divergence belongs to. The named classes plus
/// the `Unclassified` HARD-FAIL arm.
///
/// There is deliberately NO `LogicDivergence` variant — the classifier runs ONLY on
/// `computed != target`, but a logic divergence is `computed == target` (gap 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MismatchClass {
    /// The deciding cell rounds (`ROUND`/`ROUNDUP`/`CEILING`) and its operand sits
    /// within epsilon of the rounding boundary AND the gap is ≤ one rounding step —
    /// the legitimate decimal-boundary divergence. NEVER inferred from gap magnitude.
    RoundingBoundary,
    /// The computed and target agree to ~15 significant digits and diverge only in
    /// trailing binary-`f64` noise (Excel's 15-digit precision).
    Precision15Digit,
    /// An empty-cell-as-0 coercion (evidenced by the executor trace) drove it.
    EmptyCell,
    /// A lookup (`VLOOKUP`/`INDEX`/`MATCH`, evidenced by the dispatched fn) drove it.
    LookupMatchMode,
    /// A date treated as its serial number drove the divergence.
    DateSerial,
    /// An Excel error short-circuited the cell, OR a cached/computed cell holds an
    /// error value, OR an operand carried an error (the non-numeric error path).
    ErrorPropagation,
    /// A non-numeric cached cell (TEXT / BOOL / BLANK) whose structural value
    /// differs from the computed value — a real content mismatch, never a numeric
    /// tolerance case (handled exactly; Codex MEDIUM).
    NonNumericMismatch,
    /// A BA-governed constant (`Role::Constant` AND key in the governed-data table)
    /// — the ONLY constant-change route (D-03).
    GovernedData,
    /// No rule matched. A HARD FAIL — a real logic bug must never be silently fudged
    /// into another class.
    Unclassified,
}

/// The evidence a [`classify`] decision produces.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct MismatchEvidence {
    /// The fully-qualified cell key (`sheet!addr`).
    pub cell: String,
    /// The serialized deciding formula (a debug render of the `Expr`).
    pub formula: Option<String>,
    /// The compiled value.
    pub computed: CellValue,
    /// The reconciliation target (cached oracle) value.
    pub target: CellValue,
    /// The assigned class (one of the named, or `Unclassified`).
    pub class: MismatchClass,
    /// The stable slash-namespaced rule id that decided the class.
    pub deciding_rule: String,
    /// The cell's manifest role, when known.
    pub role: Option<Role>,
}

/// Classify a single `computed != target` divergence from EVIDENCE.
///
/// A small exhaustive early-return cascade: the FIRST matching rule wins; no match →
/// [`MismatchClass::Unclassified`] (HARD FAIL). The `trace` carries the executor's
/// operand/coercion/dispatch evidence several classes require.
pub fn classify(
    cell_key: &str,
    expr: &Expr,
    computed: &CellValue,
    target: &CellValue,
    trace: &EvalTrace,
    manifest: &Manifest,
) -> MismatchEvidence {
    let role = role_of(cell_key, manifest);

    let (class, deciding_rule) = if is_error_propagation(computed, target, trace) {
        (
            MismatchClass::ErrorPropagation,
            "reconcile/error-propagation",
        )
    } else if is_governed_data(role, cell_key, manifest) {
        (MismatchClass::GovernedData, "reconcile/governed-data")
    } else if is_rounding_boundary(expr, computed, target, trace) {
        (
            MismatchClass::RoundingBoundary,
            "reconcile/rounding-boundary",
        )
    } else if is_lookup_match_mode(trace) {
        (
            MismatchClass::LookupMatchMode,
            "reconcile/lookup-match-mode",
        )
    } else if is_empty_cell(trace) {
        (MismatchClass::EmptyCell, "reconcile/empty-cell")
    } else if is_date_serial(trace) {
        (MismatchClass::DateSerial, "reconcile/date-serial")
    } else if is_precision_15(computed, target) {
        (
            MismatchClass::Precision15Digit,
            "reconcile/precision-15-digit",
        )
    } else if is_non_numeric_mismatch(computed, target) {
        // A cached TEXT/BOOL/BLANK cell that differs structurally — a real content
        // mismatch (never a numeric tolerance case; Codex MEDIUM). It still routes
        // away from Unclassified ONLY for an audit-friendly class; the reconcile
        // driver treats it like any other mismatch (named-out=ERROR, helper=WARN).
        (MismatchClass::NonNumericMismatch, "reconcile/non-numeric")
    } else {
        (MismatchClass::Unclassified, "reconcile/unclassified")
    };

    MismatchEvidence {
        cell: cell_key.to_string(),
        formula: Some(format!("{expr:?}")),
        computed: computed.clone(),
        target: target.clone(),
        class,
        deciding_rule: deciding_rule.to_string(),
        role,
    }
}

/// Classify a comparison cell the executor NEVER computed (absent from the
/// `computed` map) — a real structural gap. A missing required output is NOT benign
/// error propagation: it is `Unclassified` (HARD FAIL). The `computed` field carries
/// an explicit `#REF!` for the audit trail, but the CLASS is `Unclassified`.
pub fn classify_absent(
    cell_key: &str,
    expr: &Expr,
    target: &CellValue,
    manifest: &Manifest,
) -> MismatchEvidence {
    MismatchEvidence {
        cell: cell_key.to_string(),
        formula: Some(format!("{expr:?}")),
        computed: CellValue::Error(ExcelError::Ref),
        target: target.clone(),
        class: MismatchClass::Unclassified,
        deciding_rule: "reconcile/absent-output".to_string(),
        role: role_of(cell_key, manifest),
    }
}

// ---------------------------------------------------------------------------
// Rule evidence sources. Each reads a SPECIFIC evidence channel — never the
// gap magnitude as a blanket tolerance.
// ---------------------------------------------------------------------------

/// The cell's manifest role, if the manifest carries a row for `cell_key`.
fn role_of(cell_key: &str, manifest: &Manifest) -> Option<Role> {
    manifest
        .cells
        .iter()
        .find(|c| c.cell == cell_key)
        .map(|c| c.role)
}

/// Error-propagation: the executor short-circuited on an Excel error, OR either
/// reconciled value is an error, OR an operand carried an error. This is the
/// non-numeric ERROR path — a cached `#REF!`/`#DIV/0!` never reaches a numeric
/// branch (Codex MEDIUM — no panic on an error operand).
fn is_error_propagation(computed: &CellValue, target: &CellValue, trace: &EvalTrace) -> bool {
    trace.short_circuited.is_some()
        || matches!(computed, CellValue::Error(_))
        || matches!(target, CellValue::Error(_))
        || trace
            .operand_values
            .iter()
            .any(|v| matches!(v, CellValue::Error(_)))
}

/// Governed-data: the ONLY constant-change route (D-03). Fires ONLY when the cell's
/// manifest role is `Constant` AND its key is present in the governed-data table.
fn is_governed_data(role: Option<Role>, cell_key: &str, manifest: &Manifest) -> bool {
    role == Some(Role::Constant) && manifest.governed_data.iter().any(|g| g.key == cell_key)
}

/// Rounding-boundary: fires ONLY when the deciding cell's `Expr` contains a
/// `ROUND`/`ROUNDUP`/`CEILING` call, that call's operand sits within epsilon of its
/// rounding boundary, AND the `computed`-vs-`target` divergence is no larger than
/// ONE operand-derived rounding step of that call.
///
/// The step is EVIDENCE-DERIVED — the call's own significance (CEILING) or
/// `10^-digits` (ROUND/ROUNDUP), read from the operands the executor recorded, NOT
/// inferred from the divergence magnitude. A genuinely-wrong, large divergence in a
/// rounding cell stays out of this class and falls through to `Unclassified`. This
/// is NOT a blanket abs-tolerance: the bound is the operand-anchored rounding step.
fn is_rounding_boundary(
    expr: &Expr,
    computed: &CellValue,
    target: &CellValue,
    trace: &EvalTrace,
) -> bool {
    let Some(call) = find_rounding_call(expr) else {
        return false;
    };
    if !operand_on_boundary(call, trace) {
        return false;
    }
    match (rounding_step(call, trace), computed, target) {
        (Some(step), CellValue::Number(c), CellValue::Number(t))
            if c.is_finite() && t.is_finite() =>
        {
            // The gap is bounded by ONE operand-derived step (+ a representation
            // epsilon). `step` is the call's own rounding granularity — never a
            // tuned constant on the gap itself.
            let gap = (c - t).abs();
            gap <= step.abs() + BOUNDARY_EPSILON * step.abs().max(1.0)
        },
        // No usable step, or a non-numeric side: cannot prove a one-step rounding
        // difference → do NOT absorb it as a rounding boundary.
        _ => false,
    }
}

/// The finite numeric operands the executor recorded for this cell.
fn numeric_operands(trace: &EvalTrace) -> Vec<f64> {
    trace
        .operand_values
        .iter()
        .filter_map(|v| match v {
            CellValue::Number(n) if n.is_finite() => Some(*n),
            _ => None,
        })
        .collect()
}

/// True iff the executor dispatched a function on this cell whose (uppercased) name
/// is in `names`.
fn dispatched_is(trace: &EvalTrace, names: &[&str]) -> bool {
    trace
        .dispatched_fn
        .as_deref()
        .map(|f| names.contains(&f.to_ascii_uppercase().as_str()))
        .unwrap_or(false)
}

/// The rounding STEP (granularity) of a `ROUND`/`ROUNDUP`/`CEILING` call, derived
/// from the call's own operands: the significance for `CEILING`, or `10^-digits` for
/// `ROUND`/`ROUNDUP`. `None` when the operand evidence is absent. The operand-anchored
/// bound — it never reads the computed-vs-target divergence.
fn rounding_step(call: &Expr, trace: &EvalTrace) -> Option<f64> {
    let Expr::Call { name, .. } = call else {
        return None;
    };
    let nums = numeric_operands(trace);
    match name.to_ascii_uppercase().as_str() {
        "CEILING" => {
            let significance = nums.get(1).copied().unwrap_or(1.0);
            (significance != 0.0 && significance.is_finite()).then_some(significance.abs())
        },
        "ROUND" | "ROUNDUP" => {
            let digits = nums.get(1).copied().unwrap_or(0.0);
            let step = 10f64.powi(-(digits as i32));
            step.is_finite().then_some(step)
        },
        _ => None,
    }
}

/// A `ROUND`/`ROUNDUP`/`CEILING` call found anywhere in the `Expr` tree.
fn find_rounding_call(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Call { name, .. } if is_rounding_fn(name) => Some(expr),
        Expr::Call { args, .. } => args.iter().find_map(find_rounding_call),
        Expr::BinaryOp { left, right, .. } => {
            find_rounding_call(left).or_else(|| find_rounding_call(right))
        },
        Expr::UnaryOp { operand, .. } => find_rounding_call(operand),
        _ => None,
    }
}

/// Is `name` one of the three boundary-rounding functions (case-insensitive)?
fn is_rounding_fn(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "ROUND" | "ROUNDUP" | "CEILING"
    )
}

/// Does the rounding call's first numeric operand sit within epsilon of the rounding
/// boundary it would cross? Reads the trace's materialized operand values + the
/// deterministic rounding helpers. False when the operands are absent (no fallback).
fn operand_on_boundary(call: &Expr, trace: &EvalTrace) -> bool {
    let Expr::Call { name, .. } = call else {
        return false;
    };
    let nums = numeric_operands(trace);
    let Some(&x) = nums.first() else {
        return false;
    };
    let rounded = match name.to_ascii_uppercase().as_str() {
        "CEILING" => {
            let significance = nums.get(1).copied().unwrap_or(1.0);
            excel_ceiling(x, significance)
        },
        "ROUND" => excel_round(x, nums.get(1).copied().unwrap_or(0.0) as i32),
        "ROUNDUP" => excel_roundup(x, nums.get(1).copied().unwrap_or(0.0) as i32),
        _ => return false,
    };
    if !rounded.is_finite() {
        return false;
    }
    // "On the boundary" = the operand differs from its rounded image by a
    // representation-scale epsilon — it compares the OPERAND to its rounded image,
    // never the computed-vs-target gap.
    let scale = x.abs().max(1.0);
    (x - rounded).abs() <= BOUNDARY_EPSILON * scale
}

/// Lookup-match-mode: the executor dispatched a lookup function.
fn is_lookup_match_mode(trace: &EvalTrace) -> bool {
    dispatched_is(trace, &["VLOOKUP", "INDEX", "MATCH"])
}

/// Empty-cell: the executor recorded an empty-cell-as-0 coercion.
fn is_empty_cell(trace: &EvalTrace) -> bool {
    trace
        .coercions
        .iter()
        .any(|c| c.to_ascii_lowercase().contains("empty"))
}

/// Date-serial: a date treated as its serial number drove the divergence. Fires ONLY
/// when the trace carries POSITIVE date evidence (a date-producing function was
/// dispatched); a serial-looking operand is corroborating, never the trigger (so a
/// real logic bug whose operand happens to look like a serial still hard-fails).
fn is_date_serial(trace: &EvalTrace) -> bool {
    if !dispatched_date_fn(trace) {
        return false;
    }
    trace.operand_values.iter().any(|v| match v {
        CellValue::Number(n) => {
            n.is_finite() && *n >= 59.0 && *n <= 2_958_465.0 && n.fract() == 0.0
        },
        _ => false,
    })
}

/// True iff the executor dispatched a date-producing/parsing function on this cell.
fn dispatched_date_fn(trace: &EvalTrace) -> bool {
    dispatched_is(trace, &["DATE", "DATEVALUE", "DATEDIF", "EDATE", "EOMONTH"])
}

/// Precision-15-digit: computed and target agree to ~15 significant digits and
/// differ only in trailing binary-`f64` noise. A value-equality test at Excel
/// precision, NOT a gap-magnitude rounding inference.
fn is_precision_15(computed: &CellValue, target: &CellValue) -> bool {
    match (computed, target) {
        (CellValue::Number(a), CellValue::Number(b)) if a.is_finite() && b.is_finite() => {
            let scale = a.abs().max(b.abs()).max(1.0);
            (a - b).abs() <= scale * 1e-15
        },
        _ => false,
    }
}

/// Non-numeric mismatch: BOTH sides are non-error, non-numeric (TEXT / BOOL / BLANK)
/// and structurally differ — a real cached-content mismatch the numeric path can
/// never decide (Codex MEDIUM: never coerce a cached text/bool through a numeric
/// branch). A numeric side here is decided by the numeric rules above; an error side
/// is decided by `is_error_propagation` above — so this only catches text/bool/blank.
fn is_non_numeric_mismatch(computed: &CellValue, target: &CellValue) -> bool {
    let non_numeric = |v: &CellValue| {
        matches!(
            v,
            CellValue::Text(_) | CellValue::Bool(_) | CellValue::Empty
        )
    };
    non_numeric(computed) && non_numeric(target) && computed != target
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::{CellRole, Dtype, GovernedDatum};

    fn empty_manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "wf".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![],
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn manifest_with(cell: &str, role: Role, governed_key: Option<&str>) -> Manifest {
        let mut m = empty_manifest();
        m.cells.push(CellRole {
            cell: cell.to_string(),
            role,
            name: None,
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        });
        if let Some(k) = governed_key {
            m.governed_data.push(GovernedDatum {
                key: k.to_string(),
                value: CellValue::Number(100.0),
                effective_date: None,
                approved_by: None,
                provenance: None,
            });
        }
        m
    }

    fn trace() -> EvalTrace {
        EvalTrace::default()
    }

    #[test]
    fn reconcile_rounding_boundary_ok() {
        // CEILING with the operand ON a 50-multiple within ε, gap ≤ one step (50).
        let expr = Expr::Call {
            name: "CEILING".to_string(),
            args: vec![Expr::Ref("A1".to_string()), Expr::Number(50.0)],
        };
        let mut t = trace();
        t.operand_values = vec![CellValue::Number(700.0), CellValue::Number(50.0)];
        let ev = classify(
            "S!A1",
            &expr,
            &CellValue::Number(700.0),
            &CellValue::Number(750.0),
            &t,
            &empty_manifest(),
        );
        assert_eq!(ev.class, MismatchClass::RoundingBoundary);
    }

    #[test]
    fn no_blanket_tolerance() {
        // A divergence LARGER than one rounding step (200 ≫ 50) is NOT a benign
        // rounding difference — it falls through to Unclassified (hard fail), never
        // forgiven by a blanket epsilon.
        let expr = Expr::Call {
            name: "CEILING".to_string(),
            args: vec![Expr::Ref("A1".to_string()), Expr::Number(50.0)],
        };
        let mut t = trace();
        t.operand_values = vec![CellValue::Number(700.0), CellValue::Number(50.0)];
        let ev = classify(
            "S!A1",
            &expr,
            &CellValue::Number(700.0),
            &CellValue::Number(500.0),
            &t,
            &empty_manifest(),
        );
        assert_eq!(
            ev.class,
            MismatchClass::Unclassified,
            "a gap larger than one rounding step is never forgiven by a blanket epsilon"
        );
    }

    #[test]
    fn small_gap_with_no_rounding_call_is_unclassified() {
        // A small gap (0.005) on a formula with NO rounding Call is Unclassified —
        // NOT rounding-boundary. The small gap must never be inferred into a class.
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("A1".to_string())),
            op: pmcp_workbook_runtime::BinOp::Mul,
            right: Box::new(Expr::Number(1.05)),
        };
        let ev = classify(
            "S!A2",
            &expr,
            &CellValue::Number(699.300),
            &CellValue::Number(699.305),
            &trace(),
            &empty_manifest(),
        );
        assert_eq!(ev.class, MismatchClass::Unclassified);
    }

    #[test]
    fn reconcile_non_numeric_cached() {
        // Cached cells holding TEXT, BOOL, BLANK, and an Excel error are compared
        // WITHOUT panicking on a non-numeric value (Codex MEDIUM).
        let expr = Expr::Number(0.0);
        let m = empty_manifest();

        // TEXT mismatch → NonNumericMismatch (exact structural compare).
        let ev = classify(
            "S!A1",
            &expr,
            &CellValue::Text("compiled".to_string()),
            &CellValue::Text("cached".to_string()),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::NonNumericMismatch);

        // BOOL mismatch → NonNumericMismatch.
        let ev = classify(
            "S!A2",
            &expr,
            &CellValue::Bool(true),
            &CellValue::Bool(false),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::NonNumericMismatch);

        // BLANK vs TEXT → NonNumericMismatch (Empty is a non-numeric value).
        let ev = classify(
            "S!A3",
            &expr,
            &CellValue::Empty,
            &CellValue::Text("x".to_string()),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::NonNumericMismatch);

        // An Excel-error cached cell (#REF!) → ErrorPropagation, never a numeric panic.
        let ev = classify(
            "S!A4",
            &expr,
            &CellValue::Number(5.0),
            &CellValue::Error(ExcelError::Ref),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::ErrorPropagation);

        // A #DIV/0! cached cell → ErrorPropagation (the non-numeric error path).
        let ev = classify(
            "S!A5",
            &expr,
            &CellValue::Number(5.0),
            &CellValue::Error(ExcelError::DivZero),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::ErrorPropagation);
    }

    #[test]
    fn error_propagation_fires_on_a_short_circuited_trace() {
        let expr = Expr::Call {
            name: "SUM".to_string(),
            args: vec![],
        };
        let mut t = trace();
        t.short_circuited = Some(ExcelError::DivZero);
        let ev = classify(
            "S!A1",
            &expr,
            &CellValue::Error(ExcelError::DivZero),
            &CellValue::Number(3.0),
            &t,
            &empty_manifest(),
        );
        assert_eq!(ev.class, MismatchClass::ErrorPropagation);
    }

    #[test]
    fn governed_data_fires_for_a_constant_key_in_the_table() {
        let expr = Expr::Number(105.0);
        let m = manifest_with("S!C17", Role::Constant, Some("S!C17"));
        let ev = classify(
            "S!C17",
            &expr,
            &CellValue::Number(105.0),
            &CellValue::Number(100.0),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::GovernedData);
    }

    #[test]
    fn lookup_empty_date_precision_fire_on_their_evidence() {
        let m = empty_manifest();
        // Lookup.
        let mut t = trace();
        t.dispatched_fn = Some("VLOOKUP".to_string());
        let ev = classify(
            "S!A1",
            &Expr::Call {
                name: "VLOOKUP".to_string(),
                args: vec![],
            },
            &CellValue::Number(10.0),
            &CellValue::Number(12.0),
            &t,
            &m,
        );
        assert_eq!(ev.class, MismatchClass::LookupMatchMode);

        // Empty-cell coercion.
        let mut t = trace();
        t.coercions = vec!["empty-cell->0".to_string()];
        let ev = classify(
            "S!A2",
            &Expr::Number(0.0),
            &CellValue::Number(5.0),
            &CellValue::Number(7.0),
            &t,
            &m,
        );
        assert_eq!(ev.class, MismatchClass::EmptyCell);

        // Precision-15.
        let ev = classify(
            "S!A3",
            &Expr::Number(1.0),
            &CellValue::Number(1594.93),
            &CellValue::Number(1594.93 + 1e-13),
            &trace(),
            &m,
        );
        assert_eq!(ev.class, MismatchClass::Precision15Digit);
    }

    #[test]
    fn date_serial_requires_positive_date_evidence() {
        let m = empty_manifest();
        let expr = Expr::Call {
            name: "DATE".to_string(),
            args: vec![],
        };
        let mut t = trace();
        t.operand_values = vec![CellValue::Number(45000.0)];
        // A serial-looking operand WITHOUT a dispatched date fn is Unclassified.
        let bare = classify(
            "S!A1",
            &expr,
            &CellValue::Number(45000.0),
            &CellValue::Number(45001.0),
            &t,
            &m,
        );
        assert_eq!(bare.class, MismatchClass::Unclassified);
        // With the dispatched DATE evidence: DateSerial.
        t.dispatched_fn = Some("DATE".to_string());
        let ev = classify(
            "S!A1",
            &expr,
            &CellValue::Number(45000.0),
            &CellValue::Number(45001.0),
            &t,
            &m,
        );
        assert_eq!(ev.class, MismatchClass::DateSerial);
    }

    #[test]
    fn evidence_is_serde_and_schemars_clean() {
        let ev = classify(
            "S!A1",
            &Expr::Number(1.0),
            &CellValue::Number(1.0),
            &CellValue::Number(2.0),
            &trace(),
            &empty_manifest(),
        );
        let j = serde_json::to_value(&ev).expect("serialize");
        assert_eq!(j["class"], "unclassified");
        let _ = schemars::schema_for!(MismatchEvidence);
        let sc = serde_json::to_value(schemars::schema_for!(MismatchClass)).expect("schema");
        assert_eq!(sc["title"], "MismatchClass");
    }
}
