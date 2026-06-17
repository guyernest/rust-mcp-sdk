//! The eval bridge (D-03/D-04): the cell-keyed evaluation environment + the
//! value-boundary mapping + the D-04 error preflight, plus [`eval_leaf`] which
//! now delegates to the PURE-RUST [`super::scalar_eval::eval_scalar`] (Phase 11,
//! Plan 05) instead of the `pmcp-code-mode` (SWC/JS) kernel.
//!
//! # eval-owns vs semantics-owns — the explicit boundary (review finding #3)
//!
//! - **the scalar evaluator OWNS** the supported LEAF ops on NON-error SCALAR
//!   operands: `+ - * /`, the comparisons `= <> < > <= >=`, and `&` concat.
//!   [`eval_leaf`] delegates to [`super::scalar_eval::eval_scalar`]. It is the
//!   ONE arithmetic tree-walker.
//! - **the semantics layer OWNS** the PREFLIGHT [`CellValue::Error`] walk
//!   ([`preflight_error`], short-circuit BEFORE the evaluator), the unsupported
//!   Excel ops `^`/`%` (computed in `f64` — [`powf`]/[`percent`]), and
//!   Excel-specific coercions (empty-cell-as-0, NaN→`#VALUE!`,
//!   div-by-zero→`#DIV/0!`).
//!
//! [`powf`]: f64::powf

use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::excel_error::ExcelError;
use crate::formula::Expr;
use crate::range_ref::cell_key;

use super::value::CellValue;

/// A cell-keyed evaluation environment: a thin newtype over a
/// `HashMap<String, JsonValue>` keyed on `cell_key(sheet, addr)` (e.g.
/// `"5_Quantities!C6"`). The topo executor fills this map in `cell_key` order as
/// cells compute; tests seed it directly.
#[derive(Debug, Default, Clone)]
pub struct CellEnv {
    values: HashMap<String, JsonValue>,
}

impl CellEnv {
    /// An empty environment.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Insert a pre-lowered `JsonValue` under a raw cell key (e.g.
    /// `"2_Constants!C17"`). Returns the env for builder-style seeding in tests.
    pub fn with_value(mut self, key: impl Into<String>, value: JsonValue) -> Self {
        self.values.insert(key.into(), value);
        self
    }

    /// Insert a [`CellValue`] under a raw cell key, lowering it via [`to_json`].
    /// An [`CellValue::Error`] does not lower (D-04) and is silently skipped —
    /// callers that need error short-circuiting use [`preflight_error`] instead.
    pub fn seed_cell(mut self, key: impl Into<String>, value: &CellValue) -> Self {
        if let Some(j) = to_json(value) {
            self.values.insert(key.into(), j);
        }
        self
    }

    /// Look up a raw cell key's lowered `JsonValue` (the pure-Rust scalar
    /// evaluator's variable-resolution primitive). `None` iff the key is ABSENT.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.values.get(key)
    }
}

/// Lower a [`CellValue`] to a `JsonValue` for the evaluator. Returns `None` for
/// [`CellValue::Error`] — the D-04 short-circuit signal: an Excel error NEVER
/// enters the evaluator. `Empty` lowers to `0` (the empty-cell-as-0 primitive).
pub fn to_json(v: &CellValue) -> Option<JsonValue> {
    match v {
        CellValue::Number(n) => serde_json::Number::from_f64(*n).map(JsonValue::Number),
        CellValue::Bool(b) => Some(JsonValue::Bool(*b)),
        CellValue::Text(s) => Some(JsonValue::String(s.clone())),
        CellValue::Empty => Some(JsonValue::Number(0.into())), // empty-cell-as-0
        CellValue::Error(_) => None,                           // D-04: never lowers
    }
}

/// Map an evaluator result `JsonValue` back to a [`CellValue`] applying Excel
/// coercions: a `NaN` `Number` (the div-by-zero result) becomes
/// [`ExcelError::DivZero`] and any other non-finite `Number` becomes
/// [`ExcelError::Num`]; `Null` becomes `Empty`; otherwise the obvious mapping. A
/// structurally-unexpected JSON shape (array/object) → `#VALUE!`.
pub fn from_json(v: &JsonValue) -> CellValue {
    match v {
        JsonValue::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => CellValue::Number(f),
            Some(f) if f.is_nan() => CellValue::Error(ExcelError::DivZero),
            _ => CellValue::Error(ExcelError::Num),
        },
        JsonValue::Bool(b) => CellValue::Bool(*b),
        JsonValue::String(s) => CellValue::Text(s.clone()),
        JsonValue::Null => CellValue::Empty,
        JsonValue::Array(_) | JsonValue::Object(_) => CellValue::Error(ExcelError::Value),
    }
}

/// Walk an [`Expr`]'s referenced leaves and return `Some(err)` if ANY of them
/// resolves (in the companion `errors` map) to a [`CellValue::Error`] — i.e. if
/// evaluating the expression would propagate an Excel error.
///
/// Structured as its OWN `pub` fn precisely so a unit test can call it DIRECTLY
/// to prove the error-leaf path bypasses the evaluator (finding #3). Returns the
/// FIRST error encountered in a left-to-right pre-order walk.
pub fn preflight_error(expr: &Expr, errors: &HashMap<String, ExcelError>) -> Option<ExcelError> {
    match expr {
        Expr::Ref(addr) => errors.get(addr).copied(),
        Expr::ErrorLit(e) => Some(*e),
        Expr::Range(_) | Expr::Name(_) | Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) => None,
        Expr::BinaryOp { left, right, .. } => {
            preflight_error(left, errors).or_else(|| preflight_error(right, errors))
        },
        Expr::UnaryOp { operand, .. } => preflight_error(operand, errors),
        Expr::Call { args, .. } => args.iter().find_map(|a| preflight_error(a, errors)),
    }
}

/// Compute Excel `^` in `f64` (the evaluator has no `Pow`). `base ^ exp`.
pub fn powf(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

/// Compute Excel postfix `%` in `f64` (the evaluator has no percent). `x% == x/100`.
pub fn percent(x: f64) -> f64 {
    x / 100.0
}

/// Evaluate a SCALAR leaf [`Expr`] over `env`, with an explicit `errors` map for
/// the D-04 preflight short-circuit. Delegates to the PURE-RUST
/// [`super::scalar_eval::eval_scalar`] (Phase 11, Plan 05) — no kernel, no SWC.
///
/// A node that does not lower (a `^`/`%` op, a range/name/call) yields
/// [`ExcelError::Value`] — those are the SEMANTICS layer's job, not the bridge's.
pub fn eval_leaf(expr: &Expr, env: &CellEnv, errors: &HashMap<String, ExcelError>) -> CellValue {
    crate::scalar_eval::eval_scalar(expr, env, errors)
}

/// Canonical cell key for an env entry — re-uses the shared `cell_key` so the
/// bridge never re-inlines `format!`.
pub fn env_key(sheet: &str, addr: &str) -> String {
    cell_key(sheet, addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::{BinOp, UnOp};
    use crate::range_ref::RangeRef;

    fn num(n: f64) -> JsonValue {
        JsonValue::Number(serde_json::Number::from_f64(n).unwrap())
    }

    #[test]
    fn to_json_empty_is_zero_and_error_is_none() {
        assert_eq!(
            to_json(&CellValue::Empty),
            Some(JsonValue::Number(0.into()))
        );
        assert_eq!(to_json(&CellValue::Error(ExcelError::Ref)), None);
        assert_eq!(to_json(&CellValue::Number(2.5)), Some(num(2.5)));
        assert_eq!(to_json(&CellValue::Bool(true)), Some(JsonValue::Bool(true)));
        assert_eq!(
            to_json(&CellValue::Text("x".into())),
            Some(JsonValue::String("x".into()))
        );
    }

    #[test]
    fn from_json_maps_null_to_empty_and_array_to_value() {
        assert_eq!(from_json(&num(3.0)), CellValue::Number(3.0));
        assert_eq!(from_json(&JsonValue::Null), CellValue::Empty);
        assert_eq!(from_json(&JsonValue::Bool(false)), CellValue::Bool(false));
        assert_eq!(
            from_json(&JsonValue::String("hi".into())),
            CellValue::Text("hi".into())
        );
        assert_eq!(
            from_json(&JsonValue::Array(vec![])),
            CellValue::Error(ExcelError::Value)
        );
    }

    #[test]
    fn coil_band_leaf_arithmetic_via_pure_rust() {
        // C6 * 2_Constants!C17 + 2_Constants!C18  (the coil-band C8 leaf)
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ref("5_Quantities!C6".into())),
                op: BinOp::Mul,
                right: Box::new(Expr::Ref("2_Constants!C17".into())),
            }),
            op: BinOp::Add,
            right: Box::new(Expr::Ref("2_Constants!C18".into())),
        };
        let env = CellEnv::new()
            .with_value("5_Quantities!C6", num(10.0))
            .with_value("2_Constants!C17", num(1.05))
            .with_value("2_Constants!C18", num(50.0));
        // 10 * 1.05 + 50 = 60.5
        assert_eq!(
            eval_leaf(&expr, &env, &HashMap::new()),
            CellValue::Number(60.5)
        );
    }

    #[test]
    fn concat_via_pure_rust() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Str("a".into())),
            op: BinOp::Concat,
            right: Box::new(Expr::Str("b".into())),
        };
        assert_eq!(
            eval_leaf(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Text("ab".into())
        );
    }

    #[test]
    fn comparison_le_returns_bool() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Number(2.0)),
            op: BinOp::Le,
            right: Box::new(Expr::Number(3.0)),
        };
        assert_eq!(
            eval_leaf(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Bool(true)
        );
    }

    #[test]
    fn pow_is_computed_in_f64_not_lowered() {
        assert_eq!(powf(2.0, 3.0), 8.0);
    }

    #[test]
    fn percent_is_computed_in_f64() {
        assert_eq!(percent(50.0), 0.5);
    }

    #[test]
    fn preflight_error_returns_some_for_an_error_leaf_and_none_otherwise() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(1.0)),
        };
        let mut errors = HashMap::new();
        assert_eq!(preflight_error(&expr, &errors), None);
        errors.insert("S!A1".to_string(), ExcelError::Ref);
        assert_eq!(preflight_error(&expr, &errors), Some(ExcelError::Ref));
    }

    #[test]
    fn eval_leaf_short_circuits_an_error_leaf_above_eval() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("S!A1".into())),
            op: BinOp::Add,
            right: Box::new(Expr::Number(1.0)),
        };
        let env = CellEnv::new().with_value("S!A1", num(100.0));
        let mut errors = HashMap::new();
        errors.insert("S!A1".to_string(), ExcelError::Na);
        let result = eval_leaf(&expr, &env, &errors);
        assert_eq!(result, CellValue::Error(ExcelError::Na));
        assert_ne!(result, CellValue::Number(101.0));
    }

    #[test]
    fn error_lit_is_caught_by_preflight() {
        let expr = Expr::ErrorLit(ExcelError::DivZero);
        assert_eq!(
            eval_leaf(&expr, &CellEnv::new(), &HashMap::new()),
            CellValue::Error(ExcelError::DivZero)
        );
    }

    #[test]
    fn range_and_name_do_not_lower() {
        let r = Expr::Range(RangeRef {
            sheet: "S".into(),
            start: "A1".into(),
            end: "A3".into(),
        });
        assert_eq!(
            eval_leaf(&r, &CellEnv::new(), &HashMap::new()),
            CellValue::Error(ExcelError::Value)
        );
        assert_eq!(
            eval_leaf(&Expr::Name("foo".into()), &CellEnv::new(), &HashMap::new()),
            CellValue::Error(ExcelError::Value)
        );
        let _ = UnOp::Neg; // keep the import used across cfgs
    }

    #[test]
    fn env_key_reuses_cell_key() {
        assert_eq!(env_key("2_Constants", "C17"), "2_Constants!C17");
    }
}
