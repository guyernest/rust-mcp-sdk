//! Formula parse stage — tokenize + parse to the runtime's owned `Expr` AST.
//!
//! Parses cell formulas into the runtime's owned `Expr`/`BinOp`/`UnOp` AST
//! (re-exported from `pmcp-workbook-runtime`; NEVER re-declared) with a
//! whitelist-at-parse check against the dialect contract. Wave 1 ships a typed
//! stub; Plan 05 fills the body.

use crate::error::CompileError;

/// Parse all formulas in the ingested cell model to the owned `Expr` AST.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 05 wires the
/// tokenizer + parser here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the parser is wired.
pub fn parse_all() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("formula::parse_all"))
}
