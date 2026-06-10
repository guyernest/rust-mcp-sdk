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
        }
        Expr::UnaryOp { op, operand } => {
            let v = eval_json(operand, env)?;
            eval_unop(*op, &v)
        }
        // Ranges, names, calls, and error literals NEVER lower to the evaluator —
        // the semantics layer owns them (mirrors lower_scalar's None arms).
        Expr::Range(_) | Expr::Name(_) | Expr::Call { .. } | Expr::ErrorLit(_) => {
            Err(ScalarError::NotLowerable)
        }
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
        }
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
        }
        UnOp::Neg => {
            let n = to_number(value);
            JsonValue::Number(
                serde_json::Number::from_f64(-n).unwrap_or_else(|| serde_json::Number::from(0)),
            )
        }
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
        }
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
        }
        (JsonValue::String(a), JsonValue::String(b)) => a == b,
        (JsonValue::Number(n), JsonValue::String(s))
        | (JsonValue::String(s), JsonValue::Number(n)) => {
            if let Ok(parsed) = s.parse::<f64>() {
                n.as_f64().unwrap_or(f64::NAN) == parsed
            } else {
                false
            }
        }
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
}
