//! The eval-boundary cell value (D-04): [`CellValue`].
//!
//! This is the value type that crosses into — and short-circuits ABOVE — the
//! pure-Rust scalar evaluator. Only `Number`/`Text`/`Bool`/`Empty` lower to a
//! `JsonValue` and enter the evaluator; [`CellValue::Error`] NEVER lowers — the
//! Excel-semantics layer short-circuits error propagation above the evaluator,
//! keeping it a pure arithmetic evaluator with no Excel-error awareness (D-04).
//!
//! [`ExcelError`] is RE-EXPORTED from [`crate::excel_error`] (the shared
//! value-semantics module) — it is NOT redefined here.
//!
//! Derive note: `CellValue` derives `PartialEq` but NOT `Eq`, because
//! [`CellValue::Number`] carries an `f64`.
//!
//! `Deserialize` is derived (additive to the original `Serialize`-only shape) so
//! the BA-owned manifest governed-data table that carries a typed `CellValue`
//! constant round-trips through serde (Phase 10 Plan 02, D-03).

use serde::{Deserialize, Serialize};

pub use crate::excel_error::ExcelError;

/// An owned cell value at the eval boundary (D-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CellValue {
    /// A numeric value (full precision retained; the evaluator sees a JSON number).
    Number(f64),
    /// A text value.
    Text(String),
    /// A boolean value.
    Bool(bool),
    /// An empty cell (lowers to `0` for the evaluator — the empty-cell-as-0 rule).
    Empty,
    /// An Excel error value — NEVER lowers into the evaluator; the semantics
    /// layer short-circuits propagation above it (D-04).
    Error(ExcelError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_serializes_to_its_value() {
        assert_eq!(
            serde_json::to_value(CellValue::Number(238.728)).unwrap()["Number"],
            238.728
        );
    }

    #[test]
    fn text_serializes() {
        assert_eq!(
            serde_json::to_value(CellValue::Text("hi".to_string())).unwrap()["Text"],
            "hi"
        );
    }

    #[test]
    fn bool_serializes() {
        assert_eq!(
            serde_json::to_value(CellValue::Bool(true)).unwrap()["Bool"],
            true
        );
    }

    #[test]
    fn empty_serializes_to_tag() {
        assert_eq!(serde_json::to_value(CellValue::Empty).unwrap(), "Empty");
    }

    #[test]
    fn error_serializes_re_exported_excel_error() {
        let v = serde_json::to_value(CellValue::Error(ExcelError::DivZero)).unwrap();
        assert_eq!(v["Error"], "DivZero");
    }
}
