//! The shared Excel error-value set — a SMALL value-semantics module owned by
//! neither the parser AST nor the cell-value boundary, so BOTH can reference it
//! WITHOUT a module cycle (review finding #9).
//!
//! [`ExcelError`] is a value-semantics concept: it is the set of error sentinels
//! an Excel cell can hold (`#REF!`, `#VALUE!`, …). The parser AST
//! (`Expr::ErrorLit`) merely *references* it (a literal error parsed from
//! formula text), and the eval-boundary value type (`CellValue::Error`)
//! *re-exports* it — neither DEFINES it. Placing it in its own module keeps the
//! dependency direction acyclic.
//!
//! Owned, serde/schemars-clean (the umya-quarantine invariant the whole crate
//! holds): no `umya`/`quick-xml`/`zip`/`pmcp-code-mode` type appears here. An
//! error tag carries no `f64`, so `Eq` is sound (unlike `CellValue`).

use serde::{Deserialize, Serialize};

/// The set of Excel error values a cell or a parsed literal can hold.
///
/// These are the seven canonical Excel errors. They never enter any kernel
/// (D-04): an `Error` short-circuits ABOVE the scalar evaluator in the
/// Excel-semantics layer, so the evaluator stays a pure arithmetic evaluator
/// with no Excel-error awareness.
///
/// `Deserialize` is derived (additive to the original `Serialize`-only shape) so
/// the BA-owned manifest governed-data table — whose typed value is a
/// `CellValue` that may be an `Error` — round-trips through serde (Phase 10 Plan
/// 02, D-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ExcelError {
    /// `#REF!` — an invalid cell reference.
    Ref,
    /// `#VALUE!` — a wrong type of argument or operand.
    Value,
    /// `#DIV/0!` — division by zero.
    DivZero,
    /// `#N/A` — a value is not available to a function or formula.
    Na,
    /// `#NAME?` — an unrecognized name in a formula.
    Name,
    /// `#NUM!` — an invalid numeric value in a formula or function.
    Num,
    /// `#NULL!` — an empty intersection of two ranges.
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_each_variant_to_its_tag() {
        // Serialize-only crate convention (cell_map.rs): assert JSON shape, the
        // externally-tagged unit variants render as their name strings.
        assert_eq!(serde_json::to_value(ExcelError::Ref).unwrap(), "Ref");
        assert_eq!(serde_json::to_value(ExcelError::Value).unwrap(), "Value");
        assert_eq!(
            serde_json::to_value(ExcelError::DivZero).unwrap(),
            "DivZero"
        );
        assert_eq!(serde_json::to_value(ExcelError::Na).unwrap(), "Na");
        assert_eq!(serde_json::to_value(ExcelError::Name).unwrap(), "Name");
        assert_eq!(serde_json::to_value(ExcelError::Num).unwrap(), "Num");
        assert_eq!(serde_json::to_value(ExcelError::Null).unwrap(), "Null");
    }
}
