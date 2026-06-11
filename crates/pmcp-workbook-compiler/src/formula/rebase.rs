//! Per-cell row-offset rebasing for a loop / row-block template (WBCO-03).
//!
//! ## The `$`-anchor problem
//!
//! The formula AST ([`Expr`]) has ALREADY STRIPPED `$`-anchors at the DAG layer
//! ("dependency identity does not depend on absolute vs relative"). So rebasing
//! CANNOT decide which rows must NOT shift from anchor metadata — the metadata is
//! gone by the time we hold an [`Expr`].
//!
//! ## The sound, anchor-metadata-free substitute: the WITHIN-BLOCK-RANGE rule
//!
//! We rebase ONLY references whose TARGET cell falls WITHIN the loop's iterated
//! row-block range (`start_row..=end_row` on the block's sheet). References to
//! cells OUTSIDE the block (a header, a shared constant, a cross-sheet total) are
//! left FIXED regardless of their original anchor. The per-iteration template
//! body lives IN the block (must rebase); the shared constants/headers it reads
//! live OUTSIDE the block (must stay fixed) — and this distinction needs NO
//! discarded `$`-anchor data.
//!
//! ## Reuse, do NOT re-parse
//!
//! Address splitting + A1 parsing REUSE the runtime
//! [`pmcp_workbook_runtime::resolve`] primitives (`split_ref`/`parse_a1`) — there
//! is NO second A1 parser here.
//!
//! ## Value-path discipline
//!
//! [`rebase`] is TOTAL and panic-free (lib.rs deny gate). An address that does
//! not parse is returned UNCHANGED rather than panicking — the address was
//! already validated at ingest, so return-unchanged is the conservative
//! no-corruption choice at this layer.

use pmcp_workbook_runtime::resolve::{parse_a1, split_ref};
use pmcp_workbook_runtime::{Expr, RangeRef};

/// The loop's iterated row-block range — the `(sheet, start_row..=end_row)` the
/// WITHIN-BLOCK-RANGE rule tests every reference's target against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRange {
    /// The sheet the iterated row-block lives on.
    pub sheet: String,
    /// The 1-based first iteration row (inclusive).
    pub start_row: u32,
    /// The 1-based last iteration row (inclusive).
    pub end_row: u32,
}

impl BlockRange {
    /// True iff `(sheet, row)` falls WITHIN this block range — the sound
    /// anchor-metadata-free "this reference is part of the per-iteration template
    /// body, so it must rebase" test. A reference outside this range (a header, a
    /// constant, a cross-sheet total) is FIXED.
    #[must_use]
    pub fn contains(&self, sheet: &str, row: u32) -> bool {
        sheet == self.sheet && row >= self.start_row && row <= self.end_row
    }
}

/// Rebase every in-block-range Ref/Range address in `expr` by `row_offset`,
/// returning a NEW owned [`Expr`].
///
/// A reference whose TARGET ROW falls within `block.start_row..=block.end_row`
/// (on `block.sheet`) is shifted by `row_offset` (column + sheet unchanged);
/// every reference whose target is OUTSIDE the block range is left UNCHANGED.
/// Recurses through `BinaryOp`/`UnaryOp`/`Call`; literals and `Name` are
/// untouched. Total + panic-free: an unparseable address is returned unchanged.
#[must_use]
pub fn rebase(expr: &Expr, row_offset: i64, block: &BlockRange) -> Expr {
    match expr {
        Expr::Ref(reference) => Expr::Ref(rebase_addr(reference, row_offset, block)),
        Expr::Range(range) => Expr::Range(rebase_range(range, row_offset, block)),
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(rebase(left, row_offset, block)),
            op: *op,
            right: Box::new(rebase(right, row_offset, block)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(rebase(operand, row_offset, block)),
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| rebase(a, row_offset, block)).collect(),
        },
        // Literals + defined-names carry no rebasable address.
        Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::ErrorLit(_) | Expr::Name(_) => {
            expr.clone()
        },
    }
}

/// Rebase a single Ref address string (which may be sheet-qualified) by
/// `row_offset` IFF its target row is within `block`. Reuses the runtime
/// `resolve::{split_ref, parse_a1}` for the address split + A1 parse — no second
/// parser. Preserves the original sheet-qualification form. An unparseable
/// address is returned UNCHANGED (never a panic).
fn rebase_addr(reference: &str, row_offset: i64, block: &BlockRange) -> String {
    let (sheet, addr) = split_ref(reference, &block.sheet);
    let was_qualified = reference.contains('!');
    match shift_in_block(&sheet, &addr, row_offset, block) {
        Some(shifted) => {
            if was_qualified {
                format!("{sheet}!{shifted}")
            } else {
                shifted
            }
        },
        // Out of block OR unparseable → leave the address exactly as authored.
        None => reference.to_string(),
    }
}

/// Rebase a [`RangeRef`]'s start/end endpoints by `row_offset` IFF each
/// endpoint's target row is within `block`. An endpoint outside the block (or
/// unparseable) stays fixed; the range's stored sheet is preserved.
fn rebase_range(range: &RangeRef, row_offset: i64, block: &BlockRange) -> RangeRef {
    let sheet = if range.sheet.is_empty() {
        block.sheet.clone()
    } else {
        range.sheet.trim_matches('\'').to_string()
    };
    let start = shift_endpoint(&sheet, &range.start, row_offset, block);
    let end = shift_endpoint(&sheet, &range.end, row_offset, block);
    RangeRef {
        sheet: range.sheet.clone(),
        start,
        end,
    }
}

/// Shift one range endpoint A1 address (anchor-stripped) by `row_offset` IFF its
/// target `(sheet, row)` is within `block`; otherwise return it UNCHANGED.
fn shift_endpoint(sheet: &str, addr: &str, row_offset: i64, block: &BlockRange) -> String {
    let stripped = addr.replace('$', "");
    shift_in_block(sheet, &stripped, row_offset, block).unwrap_or_else(|| addr.to_string())
}

/// The core within-block-range row shift: parse `addr` (anchor-stripped) into
/// `(col, row)`; if `(sheet, row)` is within `block`, return
/// `Some(col{row+offset})` (column + sheet unchanged); else `None` (out of block
/// OR unparseable → caller leaves the address fixed). Returns `None` on any
/// malformed address or non-positive resulting row (never a panic).
fn shift_in_block(sheet: &str, addr: &str, row_offset: i64, block: &BlockRange) -> Option<String> {
    let (col, row) = parse_a1(addr)?;
    if !block.contains(sheet, row) {
        return None;
    }
    let new_row = i64::from(row).checked_add(row_offset)?;
    if new_row <= 0 {
        return None;
    }
    Some(format!("{col}{new_row}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::{BinOp, ExcelError, UnOp};

    fn block() -> BlockRange {
        BlockRange {
            sheet: "Sheet".to_string(),
            start_row: 9,
            end_row: 11,
        }
    }

    fn rr(sheet: &str, start: &str, end: &str) -> RangeRef {
        RangeRef {
            sheet: sheet.to_string(),
            start: start.to_string(),
            end: end.to_string(),
        }
    }

    #[test]
    fn in_range_ref_rebases_out_of_range_stays_fixed() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Ref("Sheet!B9".to_string()), 3, &b),
            Expr::Ref("Sheet!B12".to_string())
        );
        assert_eq!(
            rebase(&Expr::Ref("Sheet!B5".to_string()), 3, &b),
            Expr::Ref("Sheet!B5".to_string())
        );
        assert_eq!(
            rebase(&Expr::Ref("2_Constants!C17".to_string()), 3, &b),
            Expr::Ref("2_Constants!C17".to_string())
        );
    }

    #[test]
    fn bare_in_range_ref_rebases_and_stays_bare() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Ref("B9".to_string()), 3, &b),
            Expr::Ref("B12".to_string())
        );
        assert_eq!(
            rebase(&Expr::Ref("B5".to_string()), 3, &b),
            Expr::Ref("B5".to_string())
        );
    }

    #[test]
    fn range_endpoints_in_block_offset_both() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Range(rr("Sheet", "B9", "D9")), 2, &b),
            Expr::Range(rr("Sheet", "B11", "D11"))
        );
        assert_eq!(
            rebase(&Expr::Range(rr("Sheet", "B2", "D3")), 2, &b),
            Expr::Range(rr("Sheet", "B2", "D3"))
        );
        assert_eq!(
            rebase(&Expr::Range(rr("2_Constants", "C9", "C11")), 2, &b),
            Expr::Range(rr("2_Constants", "C9", "C11"))
        );
    }

    #[test]
    fn nested_expr_rebases_in_block_leaves_out_of_block_and_literals() {
        let b = block();
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Call {
                name: "CEILING".to_string(),
                args: vec![
                    Expr::BinaryOp {
                        left: Box::new(Expr::Ref("Sheet!B9".to_string())),
                        op: BinOp::Mul,
                        right: Box::new(Expr::Number(1.05)),
                    },
                    Expr::Number(50.0),
                ],
            }),
            op: BinOp::Add,
            right: Box::new(Expr::Ref("2_Constants!C17".to_string())),
        };
        let expected = Expr::BinaryOp {
            left: Box::new(Expr::Call {
                name: "CEILING".to_string(),
                args: vec![
                    Expr::BinaryOp {
                        left: Box::new(Expr::Ref("Sheet!B12".to_string())),
                        op: BinOp::Mul,
                        right: Box::new(Expr::Number(1.05)),
                    },
                    Expr::Number(50.0),
                ],
            }),
            op: BinOp::Add,
            right: Box::new(Expr::Ref("2_Constants!C17".to_string())),
        };
        assert_eq!(rebase(&expr, 3, &b), expected);
    }

    #[test]
    fn unary_op_rebases_its_operand() {
        let b = block();
        assert_eq!(
            rebase(
                &Expr::UnaryOp {
                    op: UnOp::Neg,
                    operand: Box::new(Expr::Ref("Sheet!B10".to_string())),
                },
                1,
                &b
            ),
            Expr::UnaryOp {
                op: UnOp::Neg,
                operand: Box::new(Expr::Ref("Sheet!B11".to_string())),
            }
        );
    }

    #[test]
    fn offset_zero_is_identity() {
        let b = block();
        let expr = Expr::Call {
            name: "SUM".to_string(),
            args: vec![
                Expr::Ref("Sheet!B9".to_string()),
                Expr::Range(rr("Sheet", "C9", "C11")),
                Expr::Ref("2_Constants!C17".to_string()),
                Expr::Number(42.0),
            ],
        };
        assert_eq!(rebase(&expr, 0, &b), expr);
    }

    #[test]
    fn name_and_error_literals_are_untouched() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Name("Foo".to_string()), 3, &b),
            Expr::Name("Foo".to_string())
        );
        assert_eq!(
            rebase(&Expr::ErrorLit(ExcelError::Ref), 3, &b),
            Expr::ErrorLit(ExcelError::Ref)
        );
    }

    #[test]
    fn anchor_stripped_in_range_ref_still_rebases() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Range(rr("Sheet", "$B$9", "$B$11")), 1, &b),
            Expr::Range(rr("Sheet", "B10", "B12"))
        );
    }

    #[test]
    fn unparseable_address_is_returned_unchanged_not_panicking() {
        let b = block();
        assert_eq!(
            rebase(&Expr::Ref("Sheet!9B".to_string()), 3, &b),
            Expr::Ref("Sheet!9B".to_string())
        );
    }
}
