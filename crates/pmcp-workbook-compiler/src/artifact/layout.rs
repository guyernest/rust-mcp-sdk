//! `layout.json` capture (D-05) — the FULL workbook-layout descriptor built from
//! the ACTUAL ingested [`WorkbookMap`].
//!
//! This is a PURE transform of the owned [`WorkbookMap`] (no `umya` read happens
//! here — the ingest pass already converted every `umya` read into owned
//! `String`/`bool`/`u32`/`f64`). It captures the FULL layout (every sheet, every
//! grid cell, merges, per-column widths, hidden cols) — a real "copy of the
//! workbook," NOT a hand-synthesized minimal stub. The named output regions are a
//! SUBSET of this full capture.
//!
//! `source_workbook_hash` is the canonical content projection the `BUNDLE.lock`
//! records and the served loader cross-checks against `lock.workbook_hash`
//! (threat T-92-02 stamp binding) — it is stored verbatim on the descriptor.
//!
//! The runtime-safe serde shapes live in [`pmcp_workbook_runtime::render`] so BOTH
//! the offline emitter and the served writer share ONE definition (re-exported,
//! never re-declared).

use pmcp_workbook_runtime::a1_to_zero_indexed_row_col;

use crate::ingest::{SheetRecord, WorkbookMap};

pub use pmcp_workbook_runtime::{
    CellLayout, LayoutDescriptor, SheetLayout, LAYOUT_DESCRIPTOR_VERSION,
};

/// Build the FULL [`LayoutDescriptor`] (D-05) from the ACTUAL ingested
/// [`WorkbookMap`] — a pure transform of owned data, capturing EVERY sheet/cell so
/// the served writer can replay "a copy of the workbook, filled in."
///
/// `descriptor_version` is pinned to [`LAYOUT_DESCRIPTOR_VERSION`].
#[must_use]
pub fn build_layout_descriptor(map: &WorkbookMap, source_workbook_hash: &str) -> LayoutDescriptor {
    LayoutDescriptor {
        descriptor_version: LAYOUT_DESCRIPTOR_VERSION,
        source_workbook_hash: Some(source_workbook_hash.to_string()),
        sheets: map.sheets.iter().map(sheet_layout).collect(),
    }
}

/// Transform one owned [`SheetRecord`] into a [`SheetLayout`] (FULL capture).
///
/// Only cells with a VALID A1 grid address are captured: the reader occasionally
/// surfaces a defined-name pseudo-cell whose "address" is a name, not a `COLROW`
/// coordinate. Such a pseudo-cell is NOT a renderable grid cell, so it is skipped
/// at capture (the named output regions are always real A1 cells, so coverage is
/// unaffected).
///
/// Cells are emitted in CANONICAL row-major `(row, col)` order (determinism): the
/// ingest `Vec` order varies per process, so sorting here makes `layout.json`
/// byte-deterministic across emits (the idempotent re-emit gate hashes these exact
/// bytes).
fn sheet_layout(sheet: &SheetRecord) -> SheetLayout {
    let mut keyed_cells: Vec<((u32, u16), CellLayout)> = sheet
        .cells
        .iter()
        .filter_map(|c| {
            a1_to_zero_indexed_row_col(&c.addr).map(|rc| {
                (
                    rc,
                    CellLayout {
                        addr: c.addr.clone(),
                        formula: c.formula.clone(),
                        value: c.value.clone(),
                        number_format: c.number_format.clone(),
                        fill_argb: c.fill_argb.clone(),
                        font_argb: c.font_argb.clone(),
                    },
                )
            })
        })
        .collect();
    keyed_cells.sort_by_key(|(rc, _)| *rc);
    SheetLayout {
        name: sheet.name.clone(),
        // veryHidden + hidden both project to `hidden = true`.
        hidden: sheet.state != "visible",
        cells: keyed_cells.into_iter().map(|(_, cl)| cl).collect(),
        merges: sheet
            .merges
            .iter()
            .map(|m| {
                if m.start == m.end {
                    m.start.clone()
                } else {
                    format!("{}:{}", m.start, m.end)
                }
            })
            .collect(),
        // The owned col_widths are `(u32, f64)`; the descriptor narrows the index
        // to `u16` (Excel caps columns at 16384, well within u16).
        col_widths: sheet
            .col_widths
            .iter()
            .map(|(col, w)| (*col as u16, *w))
            .collect(),
        hidden_cols: sheet.hidden_cols.iter().map(|c| *c as u16).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{CellRecord, FormulaKind, RangeRef};

    fn cell(
        addr: &str,
        formula: Option<&str>,
        value: Option<&str>,
        nf: Option<&str>,
    ) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: formula.map(str::to_string),
            value: value.map(str::to_string),
            fill_argb: None,
            font_argb: None,
            number_format: nf.map(str::to_string),
            is_formula: formula.is_some(),
            formula_kind: FormulaKind::Normal,
        }
    }

    fn sheet(name: &str, cells: Vec<CellRecord>) -> SheetRecord {
        SheetRecord {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![],
            hidden_cols: vec![],
            col_widths: vec![(3, 12.5)],
            merges: vec![RangeRef {
                sheet: name.to_string(),
                start: "A1".to_string(),
                end: "B1".to_string(),
            }],
            cf_ranges: vec![],
            tables: vec![],
            data_validations: vec![],
            notes: vec![],
            cells,
        }
    }

    fn map(sheets: Vec<SheetRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets,
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    #[test]
    fn build_layout_descriptor_is_a_pure_transform_matching_input() {
        // A small synthetic WorkbookMap: one sheet, a formula cell at B11 + a
        // value cell. The descriptor's sheet/cell addrs/formula match the input
        // and descriptor_version == LAYOUT_DESCRIPTOR_VERSION.
        let wb = map(vec![sheet(
            "3_Outputs",
            vec![
                cell("B11", Some("SUM(B9:B10)"), Some("898.3"), Some("#,##0.00")),
                cell("B9", None, Some("532.66"), None),
            ],
        )]);
        let hash = "f".repeat(64);
        let d = build_layout_descriptor(&wb, &hash);

        assert_eq!(d.descriptor_version, LAYOUT_DESCRIPTOR_VERSION);
        assert_eq!(d.source_workbook_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(d.sheets.len(), 1);
        let s = &d.sheets[0];
        assert_eq!(s.name, "3_Outputs");
        assert!(!s.hidden);
        assert_eq!(s.cells.len(), 2);
        // Canonical row-major order: B9 (row 9) precedes B11 (row 11) regardless
        // of ingest Vec order.
        assert_eq!(s.cells[0].addr, "B9");
        assert_eq!(s.cells[1].addr, "B11");
        assert_eq!(s.cells[1].formula.as_deref(), Some("SUM(B9:B10)"));
        assert_eq!(s.cells[1].number_format.as_deref(), Some("#,##0.00"));
        assert_eq!(s.merges, vec!["A1:B1".to_string()]);
        assert_eq!(s.col_widths, vec![(3u16, 12.5)]);
    }

    #[test]
    fn veryhidden_and_hidden_sheets_project_to_hidden_true() {
        let mut s = sheet("hidden_sheet", vec![]);
        s.state = "veryHidden".to_string();
        let d = build_layout_descriptor(&map(vec![s]), &"0".repeat(64));
        assert!(d.sheets[0].hidden, "veryHidden projects to hidden=true");
    }
}
