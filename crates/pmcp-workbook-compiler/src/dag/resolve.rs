//! Reference resolution (WBCO-03, D-06/D-07): the typed-error DAG-build helper.
//!
//! The umya-free range/A1 PRIMITIVES (`expand_range`, `parse_a1`, `split_ref`,
//! `RangeShape`, `ResolveError`, `MAX_RANGE_CELLS`) live in
//! [`pmcp_workbook_runtime::resolve`] (the served binary needs them); they are
//! re-exported here so `crate::dag::resolve::{…}` resolves. The reference-walk
//! ([`collect_refs`]) stays compiler-side.
//!
//! # Typed errors, NOT lint findings (Codex MEDIUM)
//!
//! Like the parser, this layer returns a typed [`DagBuildError`] — it does NOT
//! push `LintFinding`s. Locating + reporting findings is the linter's concern;
//! the DAG build is a pure IR→graph transform with a typed failure mode. It also
//! consumes a SYNTHETIC defined-name slice ([`crate::dialect::DefinedName`]) —
//! never 93-02's owned cell model — so this plan stays parallel with 93-02.

// Re-export the runtime resolution primitives.
pub use pmcp_workbook_runtime::range_ref::cell_key;
pub use pmcp_workbook_runtime::resolve::{
    expand_range, parse_a1, split_ref, RangeShape, ResolveError, MAX_RANGE_CELLS,
};

use pmcp_workbook_runtime::{Expr, RangeRef};
use serde::Serialize;

use crate::dialect::DefinedName;

/// The typed failure surface of the DAG build (no `LintFinding` here — keeps the
/// parser/DAG transform boundary crisp vs the linter's reporting boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum DagBuildError {
    /// A range reference expanded to more than [`MAX_RANGE_CELLS`] member cells
    /// (DoS guard) — carries the offending cell count + the cap.
    RangeTooLarge {
        /// The number of member cells the range would expand to.
        cells: u64,
        /// The cap that was exceeded.
        cap: usize,
    },
    /// A range reference whose endpoints could not be parsed into A1 members.
    MalformedRange {
        /// The start endpoint as authored.
        start: String,
        /// The end endpoint as authored.
        end: String,
    },
    /// A defined name that does not resolve to any target cell.
    UnknownName(String),
    /// The dependency graph contains a cycle — carries the residual cells IN the
    /// cycle (sorted, for a deterministic message).
    Cycle(Vec<String>),
}

impl std::fmt::Display for DagBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagBuildError::RangeTooLarge { cells, cap } => {
                write!(
                    f,
                    "range expands to {cells} cells, exceeding the cap of {cap}"
                )
            },
            DagBuildError::MalformedRange { start, end } => {
                write!(
                    f,
                    "could not parse range {start}:{end} into A1 member cells"
                )
            },
            DagBuildError::UnknownName(name) => {
                write!(
                    f,
                    "defined name `{name}` does not resolve to any target cell"
                )
            },
            DagBuildError::Cycle(cells) => {
                write!(f, "dependency cycle through cells: {}", cells.join(" → "))
            },
        }
    }
}

impl std::error::Error for DagBuildError {}

/// Expand a [`RangeRef`] into its member `cell_key`s, mapping a runtime
/// [`ResolveError`] onto the typed [`DagBuildError`].
fn expand_range_keys(range: &RangeRef, loc_sheet: &str) -> Result<Vec<String>, DagBuildError> {
    match expand_range(range, loc_sheet) {
        Ok((keys, _shape)) => Ok(keys),
        Err(ResolveError::MalformedRange { start, end }) => {
            Err(DagBuildError::MalformedRange { start, end })
        },
        Err(ResolveError::RangeTooLarge { cells, cap }) => {
            Err(DagBuildError::RangeTooLarge { cells, cap })
        },
    }
}

/// Resolve a defined name to its target `cell_key`s via the synthetic
/// [`DefinedName`] table (D-07). A name whose target is a range expands under the
/// same [`MAX_RANGE_CELLS`] cap; an unresolvable name is a typed
/// [`DagBuildError::UnknownName`].
fn resolve_name(
    name: &str,
    names: &[DefinedName],
    loc_sheet: &str,
) -> Result<Vec<String>, DagBuildError> {
    let Some(record) = names.iter().find(|r| r.name == name) else {
        return Err(DagBuildError::UnknownName(name.to_string()));
    };
    expand_range_keys(&record.target, loc_sheet)
}

/// Walk a parsed [`Expr`] tree and collect the canonical `cell_key` dependency
/// keys for every reference it reads (WBCO-03). Each [`Expr::Ref`] resolves to
/// one key; each [`Expr::Range`] expands to member-cell keys (bounded by
/// [`MAX_RANGE_CELLS`]); each [`Expr::Name`] resolves via the [`DefinedName`]
/// table.
///
/// # Errors
/// Returns [`DagBuildError`] on a too-large/malformed range or an unknown
/// defined name.
pub fn collect_refs(
    expr: &Expr,
    current_sheet: &str,
    names: &[DefinedName],
) -> Result<Vec<String>, DagBuildError> {
    let mut keys = Vec::new();
    walk(expr, current_sheet, names, &mut keys)?;
    Ok(keys)
}

fn walk(
    expr: &Expr,
    current_sheet: &str,
    names: &[DefinedName],
    keys: &mut Vec<String>,
) -> Result<(), DagBuildError> {
    match expr {
        Expr::Ref(_) | Expr::Range(_) | Expr::Name(_) => {
            collect_leaf_refs(expr, current_sheet, names, keys)
        },
        Expr::BinaryOp { .. } | Expr::UnaryOp { .. } | Expr::Call { .. } => {
            walk_children(expr, current_sheet, names, keys)
        },
        Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::ErrorLit(_) => Ok(()),
    }
}

/// Resolve the three leaf reference forms ([`Expr::Ref`], [`Expr::Range`],
/// [`Expr::Name`]) to canonical `cell_key`s, appending them to `keys`. Other
/// variants are unreachable here (the [`walk`] dispatch guarantees the form).
fn collect_leaf_refs(
    expr: &Expr,
    current_sheet: &str,
    names: &[DefinedName],
    keys: &mut Vec<String>,
) -> Result<(), DagBuildError> {
    match expr {
        Expr::Ref(reference) => {
            let (sheet, addr) = split_ref(reference, current_sheet);
            keys.push(cell_key(&sheet, &addr));
        },
        Expr::Range(range) => keys.extend(expand_range_keys(range, current_sheet)?),
        Expr::Name(name) => keys.extend(resolve_name(name, names, current_sheet)?),
        _ => {},
    }
    Ok(())
}

/// Recurse into the operand-bearing variants ([`Expr::BinaryOp`],
/// [`Expr::UnaryOp`], [`Expr::Call`]), walking each child. Other variants are
/// unreachable here (the [`walk`] dispatch guarantees the form).
fn walk_children(
    expr: &Expr,
    current_sheet: &str,
    names: &[DefinedName],
    keys: &mut Vec<String>,
) -> Result<(), DagBuildError> {
    match expr {
        Expr::BinaryOp { left, right, .. } => {
            walk(left, current_sheet, names, keys)?;
            walk(right, current_sheet, names, keys)?;
        },
        Expr::UnaryOp { operand, .. } => walk(operand, current_sheet, names, keys)?,
        Expr::Call { args, .. } => {
            for arg in args {
                walk(arg, current_sheet, names, keys)?;
            }
        },
        _ => {},
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rr(sheet: &str, start: &str, end: &str) -> RangeRef {
        RangeRef {
            sheet: sheet.to_string(),
            start: start.to_string(),
            end: end.to_string(),
        }
    }

    #[test]
    fn all_four_anchor_forms_resolve_to_the_same_key() {
        let names: Vec<DefinedName> = Vec::new();
        let resolve =
            |raw: &str| collect_refs(&Expr::Ref(raw.to_string()), "5_Quantities", &names).unwrap();
        let dollar_both = resolve("$C$16");
        assert_eq!(dollar_both, vec!["5_Quantities!C16".to_string()]);
        assert_eq!(dollar_both, resolve("C16"));
        assert_eq!(dollar_both, resolve("$C16"));
        assert_eq!(dollar_both, resolve("C$16"));
    }

    #[test]
    fn cross_sheet_ref_strips_dollars_and_keeps_sheet() {
        let names: Vec<DefinedName> = Vec::new();
        let keys = collect_refs(
            &Expr::Ref("2_Constants!$C$17".to_string()),
            "5_Quantities",
            &names,
        )
        .unwrap();
        assert_eq!(keys, vec!["2_Constants!C17".to_string()]);
    }

    #[test]
    fn range_expands_to_each_member_cell_key() {
        let names: Vec<DefinedName> = Vec::new();
        let keys = collect_refs(
            &Expr::Range(rr("5_Quantities", "B2", "B10")),
            "5_Quantities",
            &names,
        )
        .unwrap();
        let expected: Vec<String> = (2..=10).map(|row| format!("5_Quantities!B{row}")).collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn range_exceeding_cap_is_a_typed_error_not_an_expansion() {
        let names: Vec<DefinedName> = Vec::new();
        let err = collect_refs(&Expr::Range(rr("S", "A1", "XFD1048576")), "S", &names)
            .expect_err("a too-large range must be a typed error");
        assert!(matches!(err, DagBuildError::RangeTooLarge { .. }));
    }

    #[test]
    fn synthetic_defined_name_resolves_to_its_target_cell() {
        let names = vec![DefinedName {
            name: "Foo".to_string(),
            target: rr("2_Constants", "C17", "C17"),
        }];
        let keys = collect_refs(&Expr::Name("Foo".to_string()), "5_Quantities", &names).unwrap();
        assert_eq!(keys, vec!["2_Constants!C17".to_string()]);
    }

    #[test]
    fn unknown_defined_name_is_a_typed_error() {
        let names: Vec<DefinedName> = Vec::new();
        let err = collect_refs(&Expr::Name("Missing".to_string()), "S", &names)
            .expect_err("an unknown name must be a typed error");
        assert_eq!(err, DagBuildError::UnknownName("Missing".to_string()));
    }

    #[test]
    fn nested_call_and_binary_op_collect_every_ref() {
        use pmcp_workbook_runtime::BinOp;
        let names: Vec<DefinedName> = Vec::new();
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Call {
                name: "CEILING".to_string(),
                args: vec![
                    Expr::BinaryOp {
                        left: Box::new(Expr::Ref("C6".to_string())),
                        op: BinOp::Mul,
                        right: Box::new(Expr::Number(1.05)),
                    },
                    Expr::Number(50.0),
                ],
            }),
            op: BinOp::Add,
            right: Box::new(Expr::Ref("2_Constants!$C$17".to_string())),
        };
        let keys = collect_refs(&expr, "5_Quantities", &names).unwrap();
        assert_eq!(
            keys,
            vec!["5_Quantities!C6".to_string(), "2_Constants!C17".to_string()]
        );
    }
}
