//! Manifest synthesis stage — colour/Guide/header heuristics → roles (WBCO-02/06).
//!
//! Synthesizes the runtime's owned [`Manifest`] (re-exported from
//! [`pmcp_workbook_runtime`] via [`model`]; NEVER re-declared) from the ingested
//! [`WorkbookMap`], applies the BA-string info-flow caps + the DV→enum fork, then
//! records the BA sign-off ([`ratify`]). This is the §5 generalization heart: the
//! manifest is fully workbook-driven (no per-workbook Rust, no hardcoded
//! reference-manifest builder) and the `workflow` name is a parameter.
//!
//! # The WorkbookMap → CellSource wiring seam (93-02 ⋈ 93-03)
//!
//! 93-03 introduced the reader-free [`CellSource`] trait (in `crate::dialect`) so
//! the linter/parser/DAG could run against a synthetic double while 93-02 produced
//! the real [`WorkbookMap`] in parallel. HERE — the first plan that consumes BOTH —
//! [`WorkbookMap`] implements [`CellSource`], so the linter now runs on the REAL
//! workbook. The owned [`CellRecord`]/[`SheetRecord`]/[`DefinedNameRecord`] convert
//! to the trait's [`CellView`]/[`SheetView`]/[`DefinedName`] views at this boundary.

pub mod model;
pub mod projections;
pub mod ratify;
pub mod synth;

pub use model::{
    AnnotationDecl, CapabilityDecl, CellRole, ChangelogEntry, Dtype, GovernedDatum, InputTier,
    LoopDecl, Manifest, Role,
};
pub use projections::{
    resolve_inline_list, sanitize_capped, MAX_ENUM_LABEL_LEN, MAX_INPUT_COUNT, MAX_MEANING_LEN,
    MAX_OUTPUT_COUNT, MAX_SHEET_NAME_LEN, MAX_UNIT_LEN,
};
pub use ratify::{is_conformant, ratify, RatifyError};
pub use synth::{check_overlap, synthesize};

use std::cell::OnceCell;

use crate::dialect::{CellSource, CellView, DefinedName, SheetView};
use crate::ingest::{FormulaKind, WorkbookMap};
use pmcp_workbook_runtime::range_ref::RangeRef;

/// The lazily-materialized [`CellSource`] views over a [`WorkbookMap`]. Built once
/// on first access (the trait returns borrowed slices, so the owned views must
/// outlive the call) and cached. This is the wiring seam: 93-03's linter / parser /
/// DAG run against the REAL 93-02 workbook through this adapter.
pub struct WorkbookCellSource<'a> {
    map: &'a WorkbookMap,
    sheets: OnceCell<Vec<SheetView>>,
    defined_names: OnceCell<Vec<DefinedName>>,
}

impl<'a> WorkbookCellSource<'a> {
    /// Adapt a [`WorkbookMap`] into a [`CellSource`].
    #[must_use]
    pub fn new(map: &'a WorkbookMap) -> Self {
        Self {
            map,
            sheets: OnceCell::new(),
            defined_names: OnceCell::new(),
        }
    }
}

impl CellSource for WorkbookCellSource<'_> {
    fn sheets(&self) -> &[SheetView] {
        self.sheets.get_or_init(|| {
            self.map
                .sheets
                .iter()
                .map(|s| SheetView {
                    name: s.name.clone(),
                    state: s.state.clone(),
                    hidden_rows: s.hidden_rows.clone(),
                    cells: s.cells.iter().map(cell_view).collect(),
                })
                .collect()
        })
    }

    fn has_macros(&self) -> bool {
        self.map.has_macros
    }

    fn external_links(&self) -> &[String] {
        &self.map.external_links
    }

    fn defined_names(&self) -> &[DefinedName] {
        self.defined_names.get_or_init(|| {
            self.map
                .defined_names
                .iter()
                .map(|dn| DefinedName {
                    name: dn.name.clone(),
                    target: RangeRef {
                        sheet: dn.target.sheet.clone(),
                        start: dn.target.start.clone(),
                        end: dn.target.end.clone(),
                    },
                })
                .collect()
        })
    }

    fn source_extension(&self) -> &str {
        &self.map.source_extension
    }
}

/// Convert one owned [`CellRecord`](crate::ingest::CellRecord) into the linter's
/// reader-free [`CellView`]. An array/dynamic-array formula maps to
/// `is_array_formula = true` so the linter's `formula/array` refuse-set fires.
fn cell_view(c: &crate::ingest::CellRecord) -> CellView {
    CellView {
        addr: c.addr.clone(),
        formula: c.formula.clone(),
        fill_argb: c.fill_argb.clone(),
        font_argb: c.font_argb.clone(),
        is_array_formula: matches!(
            c.formula_kind,
            FormulaKind::Array | FormulaKind::DynamicArray
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::{lint, DialectRules};
    use crate::ingest::{
        CellRecord, DefinedNameRecord, DefinedNameScope, SheetRecord, WorkbookMap,
    };

    fn formula_cell(addr: &str, formula: &str) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: Some(formula.to_string()),
            value: None,
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: true,
            formula_kind: FormulaKind::Normal,
        }
    }

    fn wb(sheets: Vec<SheetRecord>) -> WorkbookMap {
        WorkbookMap {
            sheets,
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn sheet(name: &str, cells: Vec<CellRecord>) -> SheetRecord {
        SheetRecord {
            name: name.to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![],
            hidden_cols: vec![],
            col_widths: vec![],
            merges: vec![],
            cf_ranges: vec![],
            tables: vec![],
            table_records: vec![],
            data_validations: vec![],
            notes: vec![],
            cells,
        }
    }

    #[test]
    fn workbookmap_implements_cellsource() {
        // THE WIRING SEAM (93-02 ⋈ 93-03): the real WorkbookMap drives the linter
        // through the CellSource adapter — a conforming workbook lints clean.
        let map = wb(vec![sheet(
            "1_Inputs",
            vec![
                formula_cell("C11", "ROUND(C10*1.05,2)"),
                formula_cell("C14", "IF(C11>0,SUM(A1:A3),0)"),
            ],
        )]);
        let src = WorkbookCellSource::new(&map);
        let report = lint(&src, &DialectRules::default());
        assert!(
            !report.has_errors(),
            "the real workbook lints clean through the CellSource seam: {:?}",
            report.findings
        );
    }

    #[test]
    fn cellsource_surfaces_macros_and_out_of_whitelist_fns() {
        let mut map = wb(vec![sheet(
            "1_Inputs",
            vec![formula_cell("A2", "OFFSET(B1,1,1)")],
        )]);
        map.has_macros = true;
        map.source_extension = "xlsm".to_string();
        map.external_links = vec!["[External.xlsx]S!A1".to_string()];
        let src = WorkbookCellSource::new(&map);
        let report = lint(&src, &DialectRules::default());
        let rules: Vec<&str> = report.findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains(&"structure/macro"));
        assert!(rules.contains(&"structure/external-link"));
        assert!(rules.contains(&"whitelist/unsupported-fn"));
    }

    #[test]
    fn cellsource_maps_array_formula_kind_to_the_array_flag() {
        let mut c = formula_cell("A1", "SUM(B1:B3)");
        c.formula_kind = FormulaKind::Array;
        let map = wb(vec![sheet("S", vec![c])]);
        let src = WorkbookCellSource::new(&map);
        let report = lint(&src, &DialectRules::default());
        assert!(
            report.findings.iter().any(|f| f.rule == "formula/array"),
            "an array-kind cell trips the linter's formula/array refuse-set"
        );
    }

    #[test]
    fn cellsource_surfaces_out_of_bounds_defined_name() {
        let mut map = wb(vec![sheet("1_Inputs", vec![])]);
        map.defined_names = vec![DefinedNameRecord {
            name: "Ghost".to_string(),
            target: RangeRef {
                sheet: "NoSuchSheet".to_string(),
                start: "A1".to_string(),
                end: "A1".to_string(),
            },
            scope: DefinedNameScope::Workbook,
        }];
        let src = WorkbookCellSource::new(&map);
        let report = lint(&src, &DialectRules::default());
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule == "manifest/range-out-of-bounds"));
    }
}
