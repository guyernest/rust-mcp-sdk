//! The shared, versioned [`LayoutDescriptor`] serde model (Phase 12, Plan 01).
//!
//! This is the SINGLE shared definition (umya-free, zip-free) the offline
//! emitter (`workbook-compiler::artifact::layout::build_layout_descriptor`) and
//! the serve-time writer (Plan 02) BOTH use — the Codex HIGH #2 single-definition
//! discipline that `artifact_model.rs` already follows for `CellMap`/`BundleLock`.
//! Defining it here (and NOT in either the compiler or the served binary) keeps
//! the descriptor's serde shape free of any `umya`/`rust_xlsxwriter` type: it
//! derives ONLY over `String`/`Option`/`Vec`/`bool`/`u32`/`u16`/`f64`.
//!
//! The descriptor captures the FULL ingested workbook layout (D-05) — a "copy of
//! the workbook," not a synthetic minimal stub — so the writer can replay it and
//! inject the computed values (D-06). It is hashed into the `BUNDLE.lock` combined
//! hash exactly like `cell_map.json` (so the boot integrity check covers it).
//!
//! The descriptor stores each cell's A1 `addr`; the writer converts A1 → the
//! `rust_xlsxwriter` `(row, col)` coordinate via [`crate::resolve::parse_a1`]
//! (RESEARCH Pitfall 3 — never re-parse A1).

use serde::{Deserialize, Serialize};

/// The current [`LayoutDescriptor`] schema version (review item 6 — the
/// descriptor is explicitly versioned + attributable so the writer can refuse a
/// future incompatible shape).
pub const LAYOUT_DESCRIPTOR_VERSION: u32 = 1;

/// One captured cell: its A1 address within the owning sheet + the original
/// formula/value text + the number format + the fill/font ARGBs. Every field is
/// owned + `Option`-where-absent so the writer replays exactly what the offline
/// ingest captured (no umya type crosses this boundary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellLayout {
    /// A1 address within the owning sheet (e.g. `"C11"`). The writer converts this
    /// to a `(row, col)` coordinate via [`crate::resolve::parse_a1`].
    pub addr: String,
    /// The original formula text WITHOUT the leading `=` (`None` when not a
    /// formula). The writer may replay this as a formula-with-cached-result.
    pub formula: Option<String>,
    /// The cell's original computed/literal value as text (`None` when empty).
    pub value: Option<String>,
    /// The number-format code (e.g. `"#,##0.00"`), `None` when General/unset.
    pub number_format: Option<String>,
    /// The fill (background) ARGB (e.g. `"FFE2EFDA"`), `None` when unset.
    pub fill_argb: Option<String>,
    /// The font colour ARGB (e.g. `"FF0000FF"`), `None` when unset.
    pub font_argb: Option<String>,
}

/// One captured sheet: its name + visibility + every captured cell + the merges
/// (A1 ranges) + the per-column widths + the hidden columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SheetLayout {
    /// The sheet name (e.g. `"7_Quote"`).
    pub name: String,
    /// `true` iff the sheet is hidden (or very-hidden) in the source workbook.
    pub hidden: bool,
    /// Every captured cell on the sheet.
    pub cells: Vec<CellLayout>,
    /// Merged-cell ranges as A1 strings (e.g. `"A1:B2"`).
    pub merges: Vec<String>,
    /// Per-column widths as `(1-based col index, width)` pairs.
    pub col_widths: Vec<(u16, f64)>,
    /// The 1-based column indices flagged hidden.
    pub hidden_cols: Vec<u16>,
}

/// The FULL captured workbook layout (D-05) — the bundle's `layout.json` member.
///
/// Carries an explicit [`descriptor_version`](LayoutDescriptor::descriptor_version)
/// (review item 6) and the optional `source_workbook_hash` provenance anchor (the
/// SAME canonical content projection the `BUNDLE.lock` records), plus every
/// captured sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LayoutDescriptor {
    /// The schema version (= [`LAYOUT_DESCRIPTOR_VERSION`] when emitted).
    pub descriptor_version: u32,
    /// The canonical source-workbook content hash this layout was captured from
    /// (`None` when not anchored).
    pub source_workbook_hash: Option<String>,
    /// Every captured sheet, in workbook order.
    pub sheets: Vec<SheetLayout>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LayoutDescriptor {
        LayoutDescriptor {
            descriptor_version: LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: Some("a".repeat(64)),
            sheets: vec![SheetLayout {
                name: "7_Quote".to_string(),
                hidden: false,
                cells: vec![
                    CellLayout {
                        addr: "C11".to_string(),
                        formula: Some("SUM(C9:C10)".to_string()),
                        value: Some("1594.93".to_string()),
                        number_format: Some("#,##0.00".to_string()),
                        fill_argb: Some("FFE2EFDA".to_string()),
                        font_argb: None,
                    },
                    CellLayout {
                        addr: "C9".to_string(),
                        formula: None,
                        value: Some("532.66".to_string()),
                        number_format: None,
                        fill_argb: None,
                        font_argb: Some("FF0000FF".to_string()),
                    },
                ],
                merges: vec!["A1:B1".to_string()],
                col_widths: vec![(3, 12.5)],
                hidden_cols: vec![7],
            }],
        }
    }

    #[test]
    fn layout_descriptor_round_trips_serialize_deserialize() {
        // Mirror artifact_model's bundle_lock_hashes_stable / the crate's
        // ir_round_trip discipline: serialize -> deserialize is an equal value.
        let d = sample();
        let json = serde_json::to_string_pretty(&d).expect("serialize");
        let back: LayoutDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back, "LayoutDescriptor round-trips to an equal value");
    }

    #[test]
    fn layout_descriptor_serializes_sheet_name_and_cell_addr_and_format() {
        // A cell carrying an addr, a formula, and a number_format serializes to
        // pretty JSON containing the sheet name + "C11".
        let d = sample();
        let json = serde_json::to_string_pretty(&d).expect("serialize");
        assert!(json.contains("7_Quote"), "sheet name present: {json}");
        assert!(json.contains("C11"), "cell addr present");
        assert!(json.contains("SUM(C9:C10)"), "formula present");
        assert!(json.contains("#,##0.00"), "number_format present");
    }

    #[test]
    fn layout_descriptor_carries_a_serializing_version_field() {
        // review item 6: the descriptor_version key serializes.
        let d = sample();
        let v = serde_json::to_value(&d).expect("to value");
        assert_eq!(
            v["descriptor_version"], LAYOUT_DESCRIPTOR_VERSION,
            "descriptor_version serializes to the version key"
        );
        let json = serde_json::to_string(&d).expect("serialize");
        assert!(
            json.contains("descriptor_version"),
            "the emitted JSON carries the version key"
        );
    }

    #[test]
    fn layout_descriptor_optional_fields_round_trip_when_absent() {
        // None number_format/fill/font + empty col_widths must round-trip.
        let d = LayoutDescriptor {
            descriptor_version: LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: None,
            sheets: vec![SheetLayout {
                name: "1_Inputs".to_string(),
                hidden: true,
                cells: vec![CellLayout {
                    addr: "E6".to_string(),
                    formula: None,
                    value: None,
                    number_format: None,
                    fill_argb: None,
                    font_argb: None,
                }],
                merges: vec![],
                col_widths: vec![],
                hidden_cols: vec![],
            }],
        };
        let json = serde_json::to_string(&d).expect("serialize");
        let back: LayoutDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }
}
