//! The owned, serde/schemars-clean formula AST (CMP-01) the hand-rolled parser
//! (workbook-compiler) builds and the DAG/`sheet_ir` executor consume.
//!
//! umya-quarantine invariant: no `umya`/`quick-xml`/`zip`/`pmcp-code-mode` type
//! appears in any public signature here. Every node is owned
//! `String`/`f64`/`bool`/`Box<Expr>`/owned-enum.
//!
//! Design (D-02):
//! - A function call is a GENERIC [`Expr::Call`] carrying `name: String` +
//!   `args: Vec<Expr>` — NOT per-function variants. The parser checks `name`
//!   against the dialect `WHITELIST` at build time; the semantics layer
//!   dispatches on the string. Widening the whitelist never churns this enum.
//! - A range ([`Expr::Range`]) reuses [`crate::range_ref::RangeRef`], which
//!   stores the sheet ONCE (`sheet` + `start` + `end`) — never duplicated.
//! - [`Expr::ErrorLit`] wraps the SHARED [`crate::excel_error::ExcelError`]
//!   (imported, NOT redefined) to represent a literal `#REF!`/`#N/A` parsed from
//!   formula text.
//!
//! Derive note: every type derives `PartialEq` but NOT `Eq`, because
//! [`Expr::Number`] carries an `f64` (which is not `Eq`).

use serde::{Deserialize, Serialize};

use crate::excel_error::ExcelError;
use crate::range_ref::RangeRef;

/// An owned Excel formula expression node.
///
/// Built by the workbook-compiler parser from `CellRecord.formula`; walked by
/// the DAG reconstructor (refs/ranges/names → dependency edges) and the
/// `sheet_ir` executor (leaf arithmetic → the pure-Rust scalar evaluator).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Expr {
    /// A single-cell reference, `$`-anchors already stripped (e.g. `"1_Inputs!E6"`
    /// or a bare `"E6"`). Dependency identity does not depend on absolute vs
    /// relative (D-07), so the parser normalizes to a plain A1/sheet!A1 string.
    Ref(String),
    /// A range reference (e.g. `B2:B10`), reusing the structured [`RangeRef`]
    /// (sheet stored ONCE). Expanded to per-member-cell edges by the DAG (D-06).
    Range(RangeRef),
    /// A numeric literal.
    Number(f64),
    /// A string literal (the `""`-doubling already un-escaped by the lexer).
    Str(String),
    /// A boolean literal (`TRUE`/`FALSE`).
    Bool(bool),
    /// A binary operation (`left op right`).
    BinaryOp {
        /// The left operand.
        left: Box<Expr>,
        /// The operator.
        op: BinOp,
        /// The right operand.
        right: Box<Expr>,
    },
    /// A unary operation (`op operand`).
    UnaryOp {
        /// The operator.
        op: UnOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// A generic function call (D-02): `name` is checked against the dialect
    /// `WHITELIST` at parse time; the semantics layer dispatches on it.
    Call {
        /// The function name (e.g. `"CEILING"`), case as authored.
        name: String,
        /// The positional arguments.
        args: Vec<Expr>,
    },
    /// A defined-name reference (resolved against the manifest defined-names in
    /// the DAG layer, D-07).
    Name(String),
    /// A literal Excel error parsed from the formula text (e.g. `#REF!`),
    /// wrapping the SHARED [`ExcelError`] (imported from [`crate::excel_error`]).
    ErrorLit(ExcelError),
}

/// The Excel binary operators (precedence handled by the parser, not encoded
/// here).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `^` (exponentiation; right-associative — handled by `f64::powf` in the
    /// semantics layer, NOT lowered to a kernel which has no `Pow`).
    Pow,
    /// `&` (text concatenation).
    Concat,
    /// `=`
    Eq,
    /// `<>`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
}

/// The Excel unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum UnOp {
    /// Unary minus (negation).
    Neg,
    /// Unary plus (no-op, retained for fidelity).
    Pos,
    /// Postfix `%` (percent; `x%` == `x / 100`, applied in the semantics layer).
    Percent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_serializes_to_its_value() {
        let v = serde_json::to_value(Expr::Number(1.0)).expect("serialize");
        assert_eq!(v["Number"], 1.0);
    }

    #[test]
    fn call_carries_name_and_args() {
        let expr = Expr::Call {
            name: "CEILING".to_string(),
            args: vec![Expr::Number(700.0), Expr::Number(50.0)],
        };
        let v = serde_json::to_value(&expr).expect("serialize Call");
        assert_eq!(v["Call"]["name"], "CEILING");
        assert_eq!(v["Call"]["args"][0]["Number"], 700.0);
        assert_eq!(v["Call"]["args"][1]["Number"], 50.0);
    }

    #[test]
    fn range_stores_sheet_once_via_rangeref() {
        let expr = Expr::Range(RangeRef {
            sheet: "5_Quantities".to_string(),
            start: "B2".to_string(),
            end: "B10".to_string(),
        });
        let v = serde_json::to_value(&expr).expect("serialize Range");
        assert_eq!(v["Range"]["sheet"], "5_Quantities");
        assert_eq!(v["Range"]["start"], "B2");
        assert_eq!(v["Range"]["end"], "B10");
    }

    #[test]
    fn binary_op_round_trips_shape() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ref("A1".to_string())),
            op: BinOp::Mul,
            right: Box::new(Expr::Number(1.05)),
        };
        let v = serde_json::to_value(&expr).expect("serialize BinaryOp");
        assert_eq!(v["BinaryOp"]["left"]["Ref"], "A1");
        assert_eq!(v["BinaryOp"]["op"], "Mul");
        assert_eq!(v["BinaryOp"]["right"]["Number"], 1.05);
    }

    #[test]
    fn error_lit_wraps_shared_excel_error() {
        let expr = Expr::ErrorLit(ExcelError::Ref);
        let v = serde_json::to_value(&expr).expect("serialize ErrorLit");
        assert_eq!(v["ErrorLit"], "Ref");
    }

    #[test]
    fn unary_op_serializes() {
        let expr = Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Number(3.0)),
        };
        let v = serde_json::to_value(&expr).expect("serialize UnaryOp");
        assert_eq!(v["UnaryOp"]["op"], "Neg");
        assert_eq!(v["UnaryOp"]["operand"]["Number"], 3.0);
    }
}
