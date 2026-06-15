//! The PURE-RUST scalar leaf evaluator (Phase 11, Plan 05 — Codex HIGH #2).
//!
//! This REPLACES the offline JS calc kernel on the RUNTIME path. The served
//! binary depends ONLY on `workbook-runtime`, which links none of the offline
//! compiler's reader / JS-runtime / archive crates — so that RCE surface never
//! enters the served-binary dependency graph (the Ph10 D-01 LINK boundary,
//! proven by the `just purity-check` cargo-tree arm).
//!
//! # NOT a second evaluator (RESEARCH "no second evaluator" invariant)
//!
//! [`eval_scalar`] is byte-parity-locked against the prior kernel result over the
//! ENTIRE scalar op surface the bridge ever lowered (the bridge lowered
//! Add/Sub/Mul/Div/Concat/Eq/Ne/Lt/Gt/Le/Ge binops + Neg/Pos unops — Pow/Percent
//! are NOT lowered, they are pure `f64` in the semantics layer). It reproduces
//! the kernel's exact JS evaluation semantics over the SAME scalar AST the kernel
//! handled — verified by `tests/kernel_parity.rs`.
//!
//! Each kernel arm is reproduced faithfully (verified against the kernel's
//! `eval.rs` source):
//! - `Add` → string concat if EITHER operand is a string, else numeric `l + r`.
//! - `Sub`/`Mul`/`Div` → `to_number` both; `Div` by 0 → `NaN` (→ `#DIV/0!`).
//! - `Concat` → JS-string-format both, concatenate.
//! - `Eq`/`Ne` → loose `json_equals` (with number/string coercion).
//! - `Lt`/`Gt`/`Le`/`Ge` → `to_number` both, compare → `Bool`.
//! - `Neg`/`Pos` → `to_number`, negate / identity.
//!
//! Value-boundary mapping (`CellValue ↔ JsonValue`) reuses the SAME
//! [`super::eval_bridge::to_json`]/[`super::eval_bridge::from_json`] helpers the
//! bridge used, so the empty-cell-as-0 + NaN→`#DIV/0!` rules are single-sourced.

use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::excel_error::ExcelError;
use crate::formula::{BinOp, Expr, UnOp};

use crate::sheet_ir::eval_bridge::{from_json, preflight_error, CellEnv};
use crate::sheet_ir::value::CellValue;

/// Evaluate a SCALAR leaf [`Expr`] over `env` purely in Rust, with an explicit
/// `errors` map for the D-04 preflight short-circuit — the drop-in replacement
/// for the bridge's old lower-then-evaluate round trip through the JS calc kernel.
///
/// Calls [`preflight_error`] FIRST: when it returns `Some(err)`, the result is
/// [`CellValue::Error`] WITHOUT any arithmetic running (finding #3, D-04).
/// Otherwise it evaluates the scalar tree to a [`JsonValue`] using the kernel's
/// EXACT semantics and maps it back via [`from_json`].
///
/// A node that is not a lowerable scalar leaf (a `^`/`%` op, a `Range`/`Name`/
/// `Call`) yields [`ExcelError::Value`] here — those are the SEMANTICS layer's
/// job, mirroring the old bridge's `lower_scalar` None arms.
pub fn eval_scalar(expr: &Expr, env: &CellEnv, errors: &HashMap<String, ExcelError>) -> CellValue {
    // D-04 / finding #3: an error leaf short-circuits ABOVE the evaluator.
    if let Some(err) = preflight_error(expr, errors) {
        return CellValue::Error(err);
    }
    match eval_json(expr, env) {
        Ok(j) => from_json(&j),
        // A type-mismatch / coercion failure / non-lowerable node maps to #VALUE!
        // (matches the old bridge's `Err(_) => #VALUE!` and `None => #VALUE!`).
        Err(_) => CellValue::Error(ExcelError::Value),
    }
}

/// A scalar-evaluation error — the pure-Rust analog of the kernel's
/// `RuntimeError` plus the "node does not lower" case. Both map to `#VALUE!` at
/// the [`eval_scalar`] boundary, exactly as the old bridge did.
#[derive(Debug)]
enum ScalarError {
    /// A variable (cell ref) was not present in the env and is not `undefined`.
    UndefinedVariable,
    /// The node is not a lowerable scalar leaf (`^`/`%`/Range/Name/Call/ErrorLit).
    NotLowerable,
}

/// Recursively evaluate a scalar [`Expr`] to a [`JsonValue`] using the kernel's
/// EXACT JS semantics. Mirrors the kernel's scope-evaluation over the lowered
/// scalar AST.
fn eval_json(expr: &Expr, env: &CellEnv) -> Result<JsonValue, ScalarError> {
    match expr {
        // A variable lookup — env stores already-lowered JsonValues keyed on the
        // ref string (matching the kernel's VariableProvider over CellEnv). The
        // kernel maps `undefined` → null; an unknown cell ref is otherwise a
        // RuntimeError (→ #VALUE!).
        Expr::Ref(name) => env
            .get(name)
            .cloned()
            .or_else(|| (name == "undefined").then_some(JsonValue::Null))
            .ok_or(ScalarError::UndefinedVariable),
        Expr::Number(n) => serde_json::Number::from_f64(*n)
            .map(JsonValue::Number)
            // A non-finite literal cannot be a JSON number — treat as not-lowerable.
            .ok_or(ScalarError::NotLowerable),
        Expr::Str(s) => Ok(JsonValue::String(s.clone())),
        Expr::Bool(b) => Ok(JsonValue::Bool(*b)),
        Expr::BinaryOp { left, op, right } => {
            let l = eval_json(left, env)?;
            let r = eval_json(right, env)?;
            eval_binop(&l, *op, &r)
        },
        Expr::UnaryOp { op, operand } => {
            let v = eval_json(operand, env)?;
            eval_unop(*op, &v)
        },
        // Ranges, names, calls, and error literals NEVER lower to the evaluator —
        // the semantics layer owns them (mirrors lower_scalar's None arms).
        Expr::Range(_) | Expr::Name(_) | Expr::Call { .. } | Expr::ErrorLit(_) => {
            Err(ScalarError::NotLowerable)
        },
    }
}

/// Reproduce the kernel `evaluate_binary_op` over the operators the bridge
/// lowered (`eval.rs:334`). `Pow` is NOT a kernel op — it is `NotLowerable` here
/// (the semantics layer computes it in `f64`).
fn eval_binop(left: &JsonValue, op: BinOp, right: &JsonValue) -> Result<JsonValue, ScalarError> {
    let v = match op {
        BinOp::Add => add_values(left, right),
        BinOp::Sub => numeric_op(left, right, |a, b| a - b),
        BinOp::Mul => numeric_op(left, right, |a, b| a * b),
        // WR-02 (financial-correctness hazard — documented at the site, not only
        // in the plan summary):
        // A zero divisor produces `f64::NAN` here, but `numeric_op` then CLAMPS a
        // non-finite result to `Number(0)` (see its body) to preserve BYTE-PARITY
        // with the prior JS calc kernel (`Number::from_f64(NaN).unwrap_or(0)`),
        // locked by `tests/kernel_parity.rs`. CONSEQUENCE: `x / 0` evaluates to a
        // clean-looking `0.0` on this path, NOT Excel's `#DIV/0!` — so the
        // `from_json` NaN→#DIV/0! arm is unreachable from `numeric_op` (IN-03).
        // This is a DELIBERATE parity choice for the locked kernel, NOT an
        // oversight: surfacing `#DIV/0!` here would silently diverge every served
        // quote from the byte-parity expectation. The hazard is mitigated
        // downstream by the server's WR-06 finiteness guard on the money OUTPUT
        // (a non-finite supply/output total is rejected as an MTS-05 error rather
        // than emitted as null/0). Any future correctness re-bless that surfaces
        // the error MUST re-bless `kernel_parity.rs` in lockstep.
        BinOp::Div => numeric_op(left, right, |a, b| if b != 0.0 { a / b } else { f64::NAN }),
        BinOp::Concat => {
            let l_str = json_to_string(left);
            let r_str = json_to_string(right);
            JsonValue::String(format!("{l_str}{r_str}"))
        },
        BinOp::Eq => JsonValue::Bool(json_equals(left, right)),
        BinOp::Ne => JsonValue::Bool(!json_equals(left, right)),
        BinOp::Lt => JsonValue::Bool(to_number(left) < to_number(right)),
        BinOp::Gt => JsonValue::Bool(to_number(left) > to_number(right)),
        BinOp::Le => JsonValue::Bool(to_number(left) <= to_number(right)),
        BinOp::Ge => JsonValue::Bool(to_number(left) >= to_number(right)),
        // ^ has no kernel op — semantics layer (f64::powf).
        BinOp::Pow => return Err(ScalarError::NotLowerable),
    };
    Ok(v)
}

/// Reproduce the kernel `evaluate_unary_op` over the operators the bridge lowered
/// (`eval.rs:379`). `Percent` is NOT a kernel op — `NotLowerable`.
fn eval_unop(op: UnOp, value: &JsonValue) -> Result<JsonValue, ScalarError> {
    let v = match op {
        UnOp::Pos => {
            let n = to_number(value);
            serde_json::Number::from_f64(n)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null)
        },
        UnOp::Neg => {
            let n = to_number(value);
            JsonValue::Number(
                serde_json::Number::from_f64(-n).unwrap_or_else(|| serde_json::Number::from(0)),
            )
        },
        // % has no kernel op — semantics layer (/100.0).
        UnOp::Percent => return Err(ScalarError::NotLowerable),
    };
    Ok(v)
}

// ---------------------------------------------------------------------------
// The kernel coercion primitives, reproduced byte-for-byte from the kernel's
// eval.rs (to_number / add_values / numeric_op / json_equals / json_to_string).
// These are what make the parity lock hold.
// ---------------------------------------------------------------------------

/// `eval.rs:429` — convert a JSON value to a number (JavaScript semantics).
fn to_number(value: &JsonValue) -> f64 {
    match value {
        JsonValue::Null => 0.0,
        JsonValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        },
        JsonValue::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        JsonValue::String(s) => s.parse().unwrap_or(f64::NAN),
        JsonValue::Array(_) | JsonValue::Object(_) => f64::NAN,
    }
}

/// `eval.rs:446` — add two JSON values (string concat takes precedence).
fn add_values(left: &JsonValue, right: &JsonValue) -> JsonValue {
    if matches!(left, JsonValue::String(_)) || matches!(right, JsonValue::String(_)) {
        let l_str = json_to_string(left);
        let r_str = json_to_string(right);
        return JsonValue::String(format!("{l_str}{r_str}"));
    }
    let l = to_number(left);
    let r = to_number(right);
    JsonValue::Number(
        serde_json::Number::from_f64(l + r).unwrap_or_else(|| serde_json::Number::from(0)),
    )
}

/// `eval.rs:463` — perform a numeric op, defaulting a non-finite result to 0
/// EXACTLY as the kernel does (`Number::from_f64(result).unwrap_or(0)`).
fn numeric_op<F>(left: &JsonValue, right: &JsonValue, op: F) -> JsonValue
where
    F: Fn(f64, f64) -> f64,
{
    let l = to_number(left);
    let r = to_number(right);
    let result = op(l, r);
    JsonValue::Number(
        serde_json::Number::from_f64(result).unwrap_or_else(|| serde_json::Number::from(0)),
    )
}

/// `eval.rs:490` — loose equality (==), with number↔string coercion.
fn json_equals(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            a.as_f64().unwrap_or(f64::NAN) == b.as_f64().unwrap_or(f64::NAN)
        },
        (JsonValue::String(a), JsonValue::String(b)) => a == b,
        (JsonValue::Number(n), JsonValue::String(s))
        | (JsonValue::String(s), JsonValue::Number(n)) => {
            if let Ok(parsed) = s.parse::<f64>() {
                n.as_f64().unwrap_or(f64::NAN) == parsed
            } else {
                false
            }
        },
        _ => false,
    }
}

/// `eval.rs:556` — JS-compatible string rendering (objects → `[object Object]`).
fn json_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(_) => value.to_string(),
        JsonValue::Object(_) => "[object Object]".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(n: f64) -> JsonValue {
        JsonValue::Number(serde_json::Number::from_f64(n).unwrap())
    }

    #[test]
    fn add_two_refs() {
        let env = CellEnv::new()
            .with_value("S!A1", num(10.0))
            .with_value("S!A2", num(5.0));
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Ref("S!A2".into())),
        };
        assert_eq!(
            eval_scalar(&expr, &env, &HashMap::new()),
            CellValue::Number(15.0)
        );
    }

    #[test]
    fn div_by_zero_matches_kernel_nan_clamped_to_zero() {
        // Kernel parity: `numeric_op` computes NaN, `Number::from_f64(NaN)` is
        // None, `unwrap_or(0)` yields Number(0) → from_json → Number(0.0). The
        // realized kernel value is 0.0, NOT #DIV/0! (NaN is clamped upstream).
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Number(1.0)),
            op: BinOp::Div,
            right: Box::new(Expr::Number(0.0)),
        };
        assert_eq!(
            eval_scalar(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Number(0.0)
        );
    }

    #[test]
    fn pow_does_not_lower() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Number(2.0)),
            op: BinOp::Pow,
            right: Box::new(Expr::Number(3.0)),
        };
        // Not lowerable → #VALUE! at the scalar boundary (semantics layer owns ^).
        assert_eq!(
            eval_scalar(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Error(ExcelError::Value)
        );
    }

    #[test]
    fn error_leaf_short_circuits_above_arithmetic() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(1.0)),
        };
        // Poison: if arithmetic ran it would be 100 + 1 = 101.
        let env = CellEnv::new().with_value("S!A1", num(100.0));
        let mut errors = HashMap::new();
        errors.insert("S!A1".to_string(), ExcelError::Na);
        let result = eval_scalar(&expr, &env, &errors);
        assert_eq!(result, CellValue::Error(ExcelError::Na));
        assert_ne!(result, CellValue::Number(101.0));
    }

    // ── WBEX-02 EXCEL-QUIRK CORPUS — layer 1 (fast scalar_eval unit assertions) ──
    //
    // Each test below is the D-08 layer-1 (runtime) witness for one Excel quirk in
    // the WBEX-02 corpus (D-09: the four roadmap-named quirks + a curated set,
    // ~7-9 total). Each documents its quirk as a precise {formula+context, cached
    // Excel oracle, runtime expected} tuple so the assertion intent is unambiguous.
    // The complementary layer-2 (penny-reconcile) witnesses live in the compiler
    // crate's `quirks_reconcile` module; the quirk->WBEX-02 traceability map is the
    // doc-comment header there. The half-rounding tests assert against the runtime's
    // `excel_round` SOURCE OF TRUTH (not a naive round). No DATE/DATEVALUE is added
    // anywhere (the 1900-leap quirk is serial f64 arithmetic — SPIKE-1900-leap.md).

    use crate::sheet_ir::rounding::{excel_round, excel_roundup};

    /// The money/float reconciliation tolerance mirrored from the compiler's
    /// `reconcile::TOL` (±0.01) — float-boundary quirks compare within tolerance,
    /// NEVER via exact-float `==` (forbidden repo-wide on money).
    const TOL: f64 = 0.01;

    /// QUIRK (named: half-rounding boundaries). {formula `ROUND(1594.925, 2)`,
    /// context: ROUND of a decimal half stored just-under in binary-f64; oracle
    /// `1594.93`; expected `1594.93`}. The naive `(x*100).round()/100` yields
    /// `1594.92`; `excel_round` applies the boundary epsilon to recover Excel's
    /// half-away-from-zero result. Asserts the SOURCE OF TRUTH, not a naive round.
    #[test]
    fn quirk_half_rounding_uses_excel_round_source_of_truth() {
        // The load-bearing assertion: the runtime's `excel_round` SOURCE OF TRUTH
        // yields Excel's half-away-from-zero result for the documented half case.
        assert_eq!(excel_round(1594.925, 2), 1594.93);
        // And a second documented half boundary (the rounding.rs module doc's own
        // example), to anchor the quirk against the source of truth, not a literal.
        assert_eq!(excel_round(2.5, 0), 3.0);
    }

    /// QUIRK (curated: negative-value rounding sign). {formula `ROUND(-2.5, 0)`,
    /// context: ROUND of a negative half; oracle `-3`; expected `-3`}. Excel rounds
    /// half AWAY FROM ZERO with the sign preserved (`-2.5 -> -3`, not `-2`). Also
    /// asserts `ROUNDUP(-3.001, 2) == -3.01` (magnitude grows away from zero).
    #[test]
    fn quirk_negative_rounding_sign_away_from_zero() {
        assert_eq!(excel_round(-2.5, 0), -3.0);
        assert_eq!(excel_roundup(-3.001, 2), -3.01);
    }

    /// QUIRK (named: empty-cell coercion). {formula `empty + 5` via the arithmetic
    /// `+` operator, context: an EMPTY cell (the kernel's `undefined`/null) in
    /// additive arithmetic; oracle `5`; expected `Number(5)`}. An empty cell
    /// coerces to 0 in `+` arithmetic — `null + 5 = 5`. (Context is load-bearing:
    /// an empty cell as an IF CONDITION coerces to FALSE, a different rule.)
    #[test]
    fn quirk_empty_cell_coerces_to_zero_in_additive_context() {
        let expr = Expr::BinaryOp {
            // `Expr::Ref("undefined")` is the canonical empty/blank leaf — the
            // kernel maps `undefined` to null, which `to_number` coerces to 0.
            left: Box::new(Expr::Ref("undefined".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(5.0)),
        };
        assert_eq!(
            eval_scalar(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Number(5.0)
        );
    }

    /// QUIRK (named: error propagation). {formula `A1 + 1` where A1 carries `#N/A`,
    /// context: an error leaf in arithmetic; oracle `#N/A`; expected
    /// `Error(Na)`}. An Excel error propagates through (poisons) any arithmetic
    /// referencing it — it does NOT silently compute a number. The error
    /// short-circuits ABOVE the evaluator (the `preflight_error` D-04 path).
    #[test]
    fn quirk_error_propagates_through_arithmetic() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Mul,
            right: Box::new(Expr::Number(3.0)),
        };
        let env = CellEnv::new().with_value("S!A1", num(7.0)); // would be 21 if it ran
        let mut errors = HashMap::new();
        errors.insert("S!A1".to_string(), ExcelError::Na);
        let result = eval_scalar(&expr, &env, &errors);
        assert_eq!(result, CellValue::Error(ExcelError::Na));
        assert_ne!(result, CellValue::Number(21.0));
    }

    /// QUIRK (curated: explicit `#DIV/0!` propagation). {formula `A1 + 1` where A1
    /// carries `#DIV/0!`, context: an explicit DivZero error leaf in arithmetic;
    /// oracle `#DIV/0!`; expected `Error(DivZero)`}. A distinct error TAG (DivZero,
    /// not Na) propagates faithfully — the propagation preserves WHICH Excel error.
    #[test]
    fn quirk_explicit_div_zero_error_propagates() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(1.0)),
        };
        let env = CellEnv::new().with_value("S!A1", num(100.0));
        let mut errors = HashMap::new();
        errors.insert("S!A1".to_string(), ExcelError::DivZero);
        let result = eval_scalar(&expr, &env, &errors);
        assert_eq!(result, CellValue::Error(ExcelError::DivZero));
    }

    /// QUIRK (curated: text->number coercion). {formula `"5.5" * 2` via the
    /// MULTIPLICATIVE `*` operator, context: numeric-text in `*`; oracle `11`;
    /// expected `Number(11)`}. In `*`/`-` arithmetic a numeric-text operand coerces
    /// to its number. Context is load-bearing: in the ADDITIVE `+` operator the
    /// SAME text CONCATENATES (`"5.5" + 2 -> "5.52"`), NOT 7.5 — both are pinned.
    #[test]
    fn quirk_text_to_number_coercion_is_context_specific() {
        // `*` context: numeric text coerces to its number.
        let mul = Expr::BinaryOp {
            left: Box::new(Expr::Str("5.5".into())),
            op: BinOp::Mul,
            right: Box::new(Expr::Number(2.0)),
        };
        assert_eq!(
            eval_scalar(&mul, &CellEnv::new(), &HashMap::new()),
            CellValue::Number(11.0)
        );
        // `+` context (pinned divergence): a string operand CONCATENATES, it does
        // NOT arithmetically add — `"5.5" + 2` is text (the kernel renders the
        // number operand `2.0` then concatenates → `"5.52.0"`), never 7.5. The
        // exact rendered form is secondary; the load-bearing point is that the `+`
        // context produces TEXT (concat), not the `*` context's numeric coercion.
        let add = Expr::BinaryOp {
            left: Box::new(Expr::Str("5.5".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(2.0)),
        };
        assert!(
            matches!(
                eval_scalar(&add, &CellEnv::new(), &HashMap::new()),
                CellValue::Text(_)
            ),
            "the additive `+` context concatenates a string operand (text), it does \
             NOT arithmetically coerce it like the multiplicative `*` context"
        );
    }

    /// QUIRK (curated: float boundary). {formula `0.1 + 0.2`, context: binary-f64
    /// additive boundary; oracle `0.3`; expected `Number(0.30000000000000004)`,
    /// graded WITHIN TOL ±0.01}. `0.1 + 0.2 != 0.3` in binary-f64 — which is WHY
    /// money compares go through a penny tolerance, never exact-float `==`.
    #[test]
    fn quirk_float_boundary_compares_within_tol_not_exact() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Number(0.1)),
            op: BinOp::Add,
            right: Box::new(Expr::Number(0.2)),
        };
        let result = eval_scalar(&expr, &CellEnv::new(), &HashMap::new());
        let CellValue::Number(n) = result else {
            panic!("expected a Number, got {result:?}");
        };
        // The exact binary-f64 sum is NOT 0.3 — the quirk itself.
        assert_ne!(n, 0.3, "0.1+0.2 is not exactly 0.3 in binary-f64");
        // But it reconciles to the 0.3 oracle within the penny tolerance.
        assert!((n - 0.3).abs() <= TOL, "{n} reconciles to 0.3 within TOL");
    }

    /// QUIRK (named: 1900 leap-year). {formula `IF(serial>59, serial+1, serial)`
    /// over bare `f64` serials, context: Excel's phantom 1900-02-29 (serial 60)
    /// shifts every serial past 1900-02-28 by +1; oracle for serial 61 is `62`;
    /// expected: the `>59` boundary holds and the `+1` offset applies}. This is the
    /// D-08 layer-1 partner of the committed `leap1900-probe.xlsx` reconcile fixture
    /// (SPIKE-1900-leap.md disposition A). `IF` is a Call owned by the semantics
    /// layer (NOT a scalar leaf), so the scalar layer asserts the two component ops
    /// — the `>59` boundary comparison and the `+1` serial offset — that the IF
    /// composes. NO DATE/DATEVALUE is added (it is pure serial f64 arithmetic).
    #[test]
    fn quirk_1900_leap_serial_offset_components() {
        // The boundary: serial 61 (1900-03-01) is strictly past the phantom-leap
        // serial 60, so the `>59` guard is TRUE and the offset applies.
        let gt = Expr::BinaryOp {
            left: Box::new(Expr::Number(61.0)),
            op: BinOp::Gt,
            right: Box::new(Expr::Number(59.0)),
        };
        assert_eq!(
            eval_scalar(&gt, &CellEnv::new(), &HashMap::new()),
            CellValue::Bool(true)
        );
        // The offset: serial + 1 == the Excel serial (61 -> 62). This is the SAME
        // arithmetic the probe's `IF(A1>59, A1+1, A1)` selects in its true branch.
        let offset = Expr::BinaryOp {
            left: Box::new(Expr::Number(61.0)),
            op: BinOp::Add,
            right: Box::new(Expr::Number(1.0)),
        };
        assert_eq!(
            eval_scalar(&offset, &CellEnv::new(), &HashMap::new()),
            CellValue::Number(62.0)
        );
        // At/below the boundary (serial 59 = 1900-02-28) the guard is FALSE — no
        // shift — which is why the offset is conditional, not unconditional.
        let at_boundary = Expr::BinaryOp {
            left: Box::new(Expr::Number(59.0)),
            op: BinOp::Gt,
            right: Box::new(Expr::Number(59.0)),
        };
        assert_eq!(
            eval_scalar(&at_boundary, &CellEnv::new(), &HashMap::new()),
            CellValue::Bool(false)
        );
    }
}
