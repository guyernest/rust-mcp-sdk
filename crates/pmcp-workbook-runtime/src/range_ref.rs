//! The owned, structured A1 range reference [`RangeRef`] + the canonical cell-key
//! helper [`cell_key`] — RELOCATED into `workbook-runtime` (Phase 11, Plan 05).
//!
//! These two were originally in `workbook-compiler`'s `ingest/cell_map.rs`, but
//! the IR (`Expr::Range`) and the runtime executor reach them, and `ingest`
//! links `umya`. Relocating the TYPE + the key builder here keeps the runtime
//! crate umya-free; `workbook-compiler` re-exports both so existing
//! `crate::ingest::RangeRef` / `cell_key` call sites resolve unchanged.
//!
//! Owned, serde/schemars-clean: no `umya`/`quick-xml`/`zip`/`pmcp-code-mode`
//! type appears here.

use serde::{Deserialize, Serialize};

/// Build the canonical manifest cell key `sheet!addr` (e.g. `"1_Inputs!E6"`).
///
/// The single home for this shape: the linter, synthesis overlap check, range
/// resolution, and the runtime executor all key cells the same way, so they
/// share one helper instead of re-inlining `format!("{}!{}", …)`.
pub fn cell_key(sheet: &str, addr: &str) -> String {
    format!("{sheet}!{addr}")
}

/// A structured, owned A1-range reference — the SINGLE range type reused for
/// merges, CF ranges, tables, named-range targets, and the IR's `Expr::Range`
/// (replaces all `(String,String)` tuples; Codex HIGH). For a single-cell range
/// `start == end`.
///
/// Example: `RangeRef { sheet: "1_Inputs", start: "E6", end: "E6" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RangeRef {
    /// The sheet the range lives on.
    pub sheet: String,
    /// The top-left A1 cell of the range (e.g. `"E6"`).
    pub start: String,
    /// The bottom-right A1 cell of the range (`== start` for a single cell).
    pub end: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_key_builds_sheet_bang_addr() {
        assert_eq!(cell_key("2_Constants", "C17"), "2_Constants!C17");
    }

    #[test]
    fn range_ref_round_trips_through_serde() {
        let r = RangeRef {
            sheet: "5_Quantities".to_string(),
            start: "B2".to_string(),
            end: "B10".to_string(),
        };
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["sheet"], "5_Quantities");
        assert_eq!(v["start"], "B2");
        assert_eq!(v["end"], "B10");
        let back: RangeRef = serde_json::from_value(v).expect("deserialize");
        assert_eq!(r, back);
    }
}
