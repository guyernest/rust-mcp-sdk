//! `read_workbook_version` — the read-only workbook-declared-version accessor.
//!
//! The bundle version is a property the BUSINESS ANALYST declares INSIDE the
//! workbook (D-02/D-11): the CLI never supplies a `--version` flag and `pmcp.toml`
//! never carries it. This module surfaces that declared value as a PUBLIC,
//! read-only accessor over the already-ingested [`WorkbookMap`] — it adds NO new
//! reader linkage (no umya) and invents NO default. A workbook that does not
//! declare a version is an ERROR, never a silently-defaulted `0.0.0`.
//!
//! # The declaration convention (mirrors the `out_*` named-range convention)
//!
//! The accessor mirrors the EXISTING [`promote_named_outputs`](crate) defined-name
//! convention: it scans `WorkbookMap.defined_names` for a single-cell defined name
//! named `version` (or its alias `wb_version`), resolves the target cell, and reads
//! that cell's cached `value`. There is exactly ONE source of truth for the version
//! — the workbook — so a flag/toml can never spoof it (threat T-94-00-VERSION).

use std::path::Path;

use crate::error::CompileError;
use crate::ingest::{self, WorkbookMap};

/// The defined name a workbook uses to declare its bundle version.
const VERSION_NAME: &str = "version";

/// An accepted alias for [`VERSION_NAME`] (some workbooks name the cell
/// `wb_version` to avoid colliding with an Excel built-in sense of "version").
const VERSION_ALIAS: &str = "wb_version";

/// Read the workbook-declared bundle version from `workbook_path`.
///
/// The version comes SOLELY from a workbook defined name (`version`, or the alias
/// `wb_version`) whose target is a SINGLE cell — mirroring the existing `out_*`
/// named-output convention. The accessor re-uses [`ingest::ingest`] and then reads
/// only the owned [`WorkbookMap`] (`defined_names` + the matching `CellRecord.value`):
/// it links no new umya path and reads no new part.
///
/// # Errors
///
/// - [`CompileError::Ingest`] if the workbook cannot be opened/parsed.
/// - [`CompileError::Lint`] if the workbook declares NO `version`/`wb_version`
///   single-cell defined name, or the target cell carries no value. A missing
///   declaration is ALWAYS an error — never a default (D-02/D-11): the message
///   names the `version` convention the workbook must declare.
pub fn read_workbook_version(workbook_path: &Path) -> Result<String, CompileError> {
    let (map, _findings) =
        ingest::ingest(workbook_path).map_err(|e| CompileError::Ingest(e.to_string()))?;
    declared_version_cell(&map).ok_or_else(|| {
        CompileError::Lint(format!(
            "the workbook declares no `{VERSION_NAME}` (or `{VERSION_ALIAS}`) \
             single-cell defined name carrying a value — the bundle version MUST \
             come from the workbook (declare a `{VERSION_NAME}` named range \
             targeting a single cell whose value is the semver, e.g. `1.2.0`)"
        ))
    })
}

/// Resolve the declared version string from the owned [`WorkbookMap`], or `None`
/// when no `version`/`wb_version` single-cell defined name resolves to a non-empty
/// cell value. Read-only over the owned map — factored out for testability so the
/// round-trip property can drive it against a synthetic map without a real `.xlsx`.
fn declared_version_cell(map: &WorkbookMap) -> Option<String> {
    for dn in &map.defined_names {
        if !is_version_name(&dn.name) {
            continue;
        }
        // Single-cell target only (start == end): a range "version" is malformed.
        if dn.target.start != dn.target.end {
            continue;
        }
        if let Some(value) = cell_value_for_key(map, &dn.target.sheet, &dn.target.start) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        // A matched-but-empty target falls through to the next candidate name
        // (and ultimately to the typed Err): an empty cell is not a declaration.
    }
    None
}

/// Whether `name` is the version-declaring defined name (case-insensitively
/// matching `version` or the alias `wb_version`).
fn is_version_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(VERSION_NAME) || name.eq_ignore_ascii_case(VERSION_ALIAS)
}

/// Read the cached `value` of the cell at (`sheet`, `addr`) from the owned map,
/// or `None` when the sheet/cell is absent or the cell has no value.
///
/// Shared by [`crate::dialect_version`] so the single-cell read rule lives once.
pub(crate) fn cell_value_for_key<'a>(
    map: &'a WorkbookMap,
    sheet: &str,
    addr: &str,
) -> Option<&'a str> {
    let sheet_rec = map.sheets.iter().find(|s| s.name == sheet)?;
    let cell = sheet_rec.cells.iter().find(|c| c.addr == addr)?;
    cell.value.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::cell_map::{CellRecord, DefinedNameRecord, DefinedNameScope, FormulaKind};
    use crate::ingest::{RangeRef, SheetRecord};

    fn cell(addr: &str, value: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: None,
            value: value.map(str::to_string),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: false,
            formula_kind: FormulaKind::Normal,
        }
    }

    fn single_cell_range(sheet: &str, addr: &str) -> RangeRef {
        RangeRef {
            sheet: sheet.to_string(),
            start: addr.to_string(),
            end: addr.to_string(),
        }
    }

    fn defined_name(name: &str, sheet: &str, addr: &str) -> DefinedNameRecord {
        DefinedNameRecord {
            name: name.to_string(),
            target: single_cell_range(sheet, addr),
            scope: DefinedNameScope::Workbook,
        }
    }

    /// A synthetic map declaring `name` -> (`sheet`!`addr`) with `cell_value`.
    fn map_declaring(name: &str, sheet: &str, addr: &str, cell_value: Option<&str>) -> WorkbookMap {
        WorkbookMap {
            sheets: vec![SheetRecord {
                name: sheet.to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells: vec![cell(addr, cell_value)],
            }],
            defined_names: vec![defined_name(name, sheet, addr)],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    #[test]
    fn declared_version_happy_path() {
        let map = map_declaring("version", "0_Meta", "B1", Some("1.2.0"));
        assert_eq!(declared_version_cell(&map).as_deref(), Some("1.2.0"));
    }

    #[test]
    fn declared_version_accepts_wb_version_alias() {
        let map = map_declaring("wb_version", "0_Meta", "B1", Some("2.5.1"));
        assert_eq!(declared_version_cell(&map).as_deref(), Some("2.5.1"));
    }

    #[test]
    fn declared_version_is_case_insensitive() {
        let map = map_declaring("Version", "0_Meta", "B1", Some("3.0.0"));
        assert_eq!(declared_version_cell(&map).as_deref(), Some("3.0.0"));
    }

    #[test]
    fn declared_version_trims_whitespace() {
        let map = map_declaring("version", "0_Meta", "B1", Some("  1.4.2  "));
        assert_eq!(declared_version_cell(&map).as_deref(), Some("1.4.2"));
    }

    #[test]
    fn missing_version_yields_none_never_a_default() {
        // No `version` defined name at all.
        let map = WorkbookMap {
            sheets: vec![],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        };
        // The accessor returns None (NEVER a defaulted "0.0.0"); the public fn maps
        // this to a typed Err.
        assert_eq!(declared_version_cell(&map), None);
    }

    #[test]
    fn empty_version_cell_is_not_a_declaration() {
        let map = map_declaring("version", "0_Meta", "B1", Some("   "));
        assert_eq!(declared_version_cell(&map), None);
    }

    #[test]
    fn range_target_version_is_rejected() {
        // A `version` name targeting a multi-cell range is NOT a scalar version.
        let map = WorkbookMap {
            sheets: vec![SheetRecord {
                name: "0_Meta".to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells: vec![cell("B1", Some("1.0.0"))],
            }],
            defined_names: vec![DefinedNameRecord {
                name: "version".to_string(),
                target: RangeRef {
                    sheet: "0_Meta".to_string(),
                    start: "B1".to_string(),
                    end: "B3".to_string(),
                },
                scope: DefinedNameScope::Workbook,
            }],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        };
        assert_eq!(declared_version_cell(&map), None);
    }

    /// Round-trip PROPERTY (exhaustive over a representative grid, no new
    /// dependency): any semver-shaped string written into the declared cell of a
    /// synthetic map reads back EQUAL — the read is lossless (no transform, no
    /// default). The grid spans single/multi-digit major/minor/patch components so
    /// it covers the lexical width variations a real workbook declares.
    #[test]
    fn declared_version_round_trips_over_a_semver_grid() {
        let components = [0u32, 1, 7, 12, 99, 123, 999];
        for &major in &components {
            for &minor in &components {
                for &patch in &components {
                    let semver = format!("{major}.{minor}.{patch}");
                    let map = map_declaring("version", "0_Meta", "B1", Some(&semver));
                    assert_eq!(
                        declared_version_cell(&map).as_deref(),
                        Some(semver.as_str()),
                        "round-trip must be lossless for {semver}"
                    );
                }
            }
        }
    }
}
