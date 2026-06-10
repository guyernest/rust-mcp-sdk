//! Range/reference resolution PRIMITIVES (CMP-02, D-06/D-07) — the umya-free,
//! SAFE FALLIBLE subset RELOCATED into `workbook-runtime` (Phase 11, Plan 05).
//!
//! The runtime executor's `run()` needs to expand an [`Expr::Range`](crate::Expr)
//! into its member `cell_key`s + 2-D [`RangeShape`] at SERVE time, and to parse
//! / split A1 addresses for the loop-instance keying — all WITHOUT pulling
//! `crate::dialect` (findings) or `crate::ingest` (`DefinedNameRecord`, umya). So
//! the pure functions over `RangeRef` + `cell_key` live here:
//!
//! - [`expand_range`] / [`RangeShape`] / [`ResolveError`] / [`MAX_RANGE_CELLS`]
//! - [`parse_a1`] / [`split_ref`]
//!
//! The FINDING-PUSHING DAG-build path (`collect_refs` / `expand_range_into_report`
//! / `resolve_name`, which need `LintReport` + `DefinedNameRecord`) STAYS in
//! `workbook-compiler`; it imports these primitives from here.

use serde::Serialize;

use crate::range_ref::{cell_key, RangeRef};

/// The maximum number of member cells a single range reference may expand into
/// (finding #6, threat T-09-08). A range exceeding this cap (e.g.
/// `A1:XFD1048576`, ~17 billion cells) is NOT expanded — it produces an `Err`
/// (the compiler translates it to ONE located `dag/range-too-large` finding), so
/// a hostile or careless whole-sheet range can never allocate millions of edges.
pub const MAX_RANGE_CELLS: usize = 10_000;

/// Strip `$`-anchors from an A1 address, normalizing `$C$16`/`C16`/`$C16`/`C$16`
/// to the canonical `COLROW` form `C16` (D-07, RESEARCH Pitfall 2). The address
/// may NOT carry a sheet qualifier (the caller splits that off first).
fn strip_anchors(addr: &str) -> String {
    addr.replace('$', "")
}

/// Split a possibly sheet-qualified, possibly `$`-anchored reference string into
/// `(sheet, canonical_addr)`, defaulting the sheet to `current_sheet` when the
/// reference is unqualified. The sheet name keeps any surrounding `'…'` quoting
/// stripped.
///
/// Public so per-room row-offset rebasing can reuse the SAME
/// cross-sheet/anchor-stripping split (no second A1 parser). Total + fallible:
/// it never panics.
pub fn split_ref(reference: &str, current_sheet: &str) -> (String, String) {
    match reference.rsplit_once('!') {
        Some((sheet, addr)) => (sheet.trim_matches('\'').to_string(), strip_anchors(addr)),
        None => (current_sheet.to_string(), strip_anchors(reference)),
    }
}

/// Parse a canonical (anchor-stripped) A1 cell address `C16` into its
/// `(column_letters, row_number)` parts. Returns `None` for a malformed address
/// (no panic — the value path stays fallible).
pub fn parse_a1(addr: &str) -> Option<(String, u32)> {
    let split = addr.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None; // no column letters
    }
    let (col, row) = addr.split_at(split);
    if !col.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let row: u32 = row.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((col.to_ascii_uppercase(), row))
}

/// Convert a column-letter run (`A`, `Z`, `AA`, `XFD`) to its 1-based index.
fn col_to_index(col: &str) -> Option<u32> {
    if col.is_empty() {
        return None;
    }
    let mut idx: u32 = 0;
    for b in col.bytes() {
        if !b.is_ascii_alphabetic() {
            return None;
        }
        let v = (b.to_ascii_uppercase() - b'A') as u32 + 1;
        idx = idx.checked_mul(26)?.checked_add(v)?;
    }
    Some(idx)
}

/// Convert a canonical A1 cell address (`C16`) to the ZERO-indexed
/// `(row, col)` coordinate the `rust_xlsxwriter` writer expects (`C16` →
/// `(15, 2)`) — review item 8, RESEARCH Pitfall 3.
///
/// This is the SINGLE shared A1→`(row, col)` conversion: the Plan-02 writer
/// reuses it rather than duplicating column-letter math. Built on the existing
/// [`parse_a1`] (which canonicalizes + rejects malformed) + [`col_to_index`]
/// (1-based column index). Returns `None` for any malformed address — never a
/// panic. The returned column is `u16` and the row is `u32`, matching the
/// writer's `ColNum`/`RowNum` (so the writer needs no second cast layer).
///
/// The caller MUST strip any sheet qualifier + `$`-anchors first (e.g. via
/// [`split_ref`]); this takes the canonical `COLROW` form `parse_a1` accepts.
pub fn a1_to_zero_indexed_row_col(addr: &str) -> Option<(u32, u16)> {
    let (col_letters, row) = parse_a1(addr)?;
    let col_1based = col_to_index(&col_letters)?;
    // parse_a1 guarantees row >= 1; col_to_index guarantees col >= 1. Convert to
    // zero-indexed for the writer. col_1based fits a u16 only up to 65_536; the
    // sheet column cap (XFD = 16_384) keeps it well inside u16, but guard anyway.
    let col_zero = u16::try_from(col_1based.checked_sub(1)?).ok()?;
    Some((row - 1, col_zero))
}

/// Convert a 1-based column index back to its letter run (`1` → `A`, `27` → `AA`).
///
/// WR-07: build the run from `char`s directly so there is NO fallible UTF-8
/// decode.
fn index_to_col(mut idx: u32) -> String {
    let mut chars = Vec::new();
    while idx > 0 {
        let rem = ((idx - 1) % 26) as u8;
        chars.push(char::from(b'A' + rem));
        idx = (idx - 1) / 26;
    }
    chars.iter().rev().collect()
}

/// The 2-D shape (`rows` × `cols`) a [`RangeRef`] expands to — published
/// alongside the member `cell_key`s so the executor can rebuild a shape-correct
/// `Vec<Vec<CellValue>>` for `VLOOKUP`/`INDEX`/`MATCH`.
///
/// `rows = row_hi - row_lo + 1`, `cols = col_hi - col_lo + 1` — both ≥ 1 for any
/// valid range (a single cell is `{rows: 1, cols: 1}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct RangeShape {
    /// The number of rows the range spans (inclusive).
    pub rows: u32,
    /// The number of columns the range spans (inclusive).
    pub cols: u32,
}

/// A fallible range-expansion failure (D-06, T-09-08). [`expand_range`] returns
/// this as an `Err` — NEVER a panic and NEVER a silent empty `Vec` that looks
/// like a 0-cell range. The compiler's finding-pushing path translates each
/// variant back into the located `dag/malformed-range` / `dag/range-too-large`
/// `LintFinding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum ResolveError {
    /// A range endpoint did not parse as a valid A1 address (or column).
    MalformedRange {
        /// The (anchor-stripped) start endpoint as authored.
        start: String,
        /// The (anchor-stripped) end endpoint as authored.
        end: String,
    },
    /// The range expands to more than [`MAX_RANGE_CELLS`] member cells.
    RangeTooLarge {
        /// The member-cell count the range would expand to.
        cells: u64,
        /// The cap that was exceeded ([`MAX_RANGE_CELLS`]).
        cap: usize,
    },
}

/// Expand a [`RangeRef`] into its member `cell_key`s (column-major, D-06) AND
/// the 2-D [`RangeShape`] it spans, bounded by [`MAX_RANGE_CELLS`]. This is the
/// SAFE, FALLIBLE public API: an over-cap or malformed range is an `Err`, never
/// a panic and never a silent empty.
pub fn expand_range(
    range: &RangeRef,
    current_sheet: &str,
) -> Result<(Vec<String>, RangeShape), ResolveError> {
    let sheet = if range.sheet.is_empty() {
        current_sheet.to_string()
    } else {
        range.sheet.trim_matches('\'').to_string()
    };
    let start = strip_anchors(&range.start);
    let end = strip_anchors(&range.end);

    let malformed = || ResolveError::MalformedRange {
        start: start.clone(),
        end: end.clone(),
    };

    let (Some((sc, sr)), Some((ec, er))) = (parse_a1(&start), parse_a1(&end)) else {
        return Err(malformed());
    };
    let (Some(sci), Some(eci)) = (col_to_index(&sc), col_to_index(&ec)) else {
        return Err(malformed());
    };

    let (col_lo, col_hi) = (sci.min(eci), sci.max(eci));
    let (row_lo, row_hi) = (sr.min(er), sr.max(er));
    let cols = col_hi - col_lo + 1;
    let rows = row_hi - row_lo + 1;
    // u64 product avoids overflow on a whole-sheet range before the cap check.
    let n_cells = u64::from(cols) * u64::from(rows);

    if n_cells > MAX_RANGE_CELLS as u64 {
        return Err(ResolveError::RangeTooLarge {
            cells: n_cells,
            cap: MAX_RANGE_CELLS,
        });
    }

    let mut keys = Vec::with_capacity(n_cells as usize);
    for col in col_lo..=col_hi {
        let col_letters = index_to_col(col);
        for row in row_lo..=row_hi {
            keys.push(cell_key(&sheet, &format!("{col_letters}{row}")));
        }
    }
    Ok((keys, RangeShape { rows, cols }))
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
    fn public_expand_range_single_column_returns_keys_and_shape() {
        let (keys, shape) = expand_range(&rr("S", "B2", "B4"), "S").expect("valid range");
        assert_eq!(
            keys,
            vec!["S!B2".to_string(), "S!B3".to_string(), "S!B4".to_string()]
        );
        assert_eq!(shape, RangeShape { rows: 3, cols: 1 });
    }

    #[test]
    fn public_expand_range_2x2_is_column_major_with_2x2_shape() {
        let (keys, shape) = expand_range(&rr("S", "A1", "B2"), "S").expect("valid range");
        assert_eq!(
            keys,
            vec![
                "S!A1".to_string(),
                "S!A2".to_string(),
                "S!B1".to_string(),
                "S!B2".to_string(),
            ]
        );
        assert_eq!(shape, RangeShape { rows: 2, cols: 2 });
    }

    #[test]
    fn public_expand_range_defaults_empty_sheet_to_current() {
        let (keys, _shape) = expand_range(&rr("", "C1", "C2"), "5_Quantities").expect("valid");
        assert_eq!(
            keys,
            vec!["5_Quantities!C1".to_string(), "5_Quantities!C2".to_string()]
        );
    }

    #[test]
    fn public_expand_range_over_cap_is_err() {
        let err = expand_range(&rr("S", "A1", "XFD1048576"), "S")
            .expect_err("an over-cap range must be Err");
        assert!(matches!(
            err,
            ResolveError::RangeTooLarge { cap, cells } if cap == MAX_RANGE_CELLS && cells > MAX_RANGE_CELLS as u64
        ));
    }

    #[test]
    fn public_expand_range_malformed_endpoint_is_err() {
        let err =
            expand_range(&rr("S", "1A", "B2"), "S").expect_err("a malformed endpoint must be Err");
        assert!(matches!(err, ResolveError::MalformedRange { .. }));
    }

    #[test]
    fn public_parse_a1_parses_and_rejects() {
        assert_eq!(parse_a1("C16"), Some(("C".to_string(), 16)));
        assert_eq!(parse_a1("$C$16"), None); // anchors must be stripped by the caller
        assert_eq!(parse_a1("16"), None); // no column letters
        assert_eq!(parse_a1("C0"), None); // row 0 is invalid
        assert_eq!(parse_a1("CC"), None); // no row digits
    }

    #[test]
    fn public_split_ref_strips_anchors_and_defaults_sheet() {
        assert_eq!(
            split_ref("2_Constants!$C$17", "5_Quantities"),
            ("2_Constants".to_string(), "C17".to_string())
        );
        assert_eq!(
            split_ref("$C$16", "5_Quantities"),
            ("5_Quantities".to_string(), "C16".to_string())
        );
    }

    #[test]
    fn a1_to_zero_indexed_row_col_converts_and_rejects() {
        // C16 -> (15, 2) — review item 8 example.
        assert_eq!(a1_to_zero_indexed_row_col("C16"), Some((15, 2)));
        // A1 -> (0, 0) (top-left).
        assert_eq!(a1_to_zero_indexed_row_col("A1"), Some((0, 0)));
        // AA1 -> (0, 26) (col 27 1-based -> 26 zero-indexed).
        assert_eq!(a1_to_zero_indexed_row_col("AA1"), Some((0, 26)));
        // Malformed -> None, never a panic.
        assert_eq!(a1_to_zero_indexed_row_col("1A"), None);
        assert_eq!(a1_to_zero_indexed_row_col("$C$16"), None); // anchors not stripped
        assert_eq!(a1_to_zero_indexed_row_col(""), None);
    }

    #[test]
    fn col_index_round_trips() {
        for (col, idx) in [("A", 1u32), ("Z", 26), ("AA", 27), ("XFD", 16384)] {
            assert_eq!(col_to_index(col), Some(idx));
            assert_eq!(index_to_col(idx), col);
        }
    }
}
