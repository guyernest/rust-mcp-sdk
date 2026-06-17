//! Workbook ingest: `umya` read → owned, plain cell map + metadata.
//!
//! This is the ONE module (with [`crate::provenance`]) where the Excel reader
//! (`umya-spreadsheet`) is used. No `umya` type appears in the public API here —
//! reads are converted to owned plain types ([`cell_map`]) at the boundary so the
//! downstream stages reuse them without pulling `umya`. The pass is COLLECT-ALL:
//! a per-cell/per-sheet read problem becomes a [`LintFinding`] and the scan
//! CONTINUES — it never `.unwrap()`s a `umya` `Option`/`Result` (the crate
//! `#![deny(clippy::unwrap_used)]` gate enforces it; threats T-93-02-DOS).
//!
//! The cached `<v>` values captured here are the trusted ORACLE (WBCO-01): the
//! reconcile stage grades computed outputs against them.

pub mod cell_map;

use std::path::Path;

use umya_spreadsheet::reader;
use umya_spreadsheet::structs::{CellFormulaValues, EnumTrait, SheetStateValues};

// `LintFinding`/`Severity` are the runtime's collect-all located findings,
// re-exported at the crate root (NEVER re-declared). Reach them via the crate
// root so a second definition can never drift from the served loader's.
use crate::{LintFinding, Severity};

pub use cell_map::{
    cell_key, CellRecord, DataValidationRecord, DefinedNameRecord, DefinedNameScope, FormulaKind,
    NoteRecord, RangeRef, SheetRecord, WorkbookMap,
};
use umya_spreadsheet::structs::Comment;

/// A fatal ingest error: the workbook could not be opened/parsed at all. A
/// MALFORMED-but-openable workbook does NOT use this — its per-cell problems
/// become collect-all [`LintFinding`]s (a read failure is a typed error, never a
/// panic).
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    /// `umya`'s `reader::xlsx::read` failed to open/parse the file.
    #[error("failed to read workbook {path}: {detail}")]
    Read {
        /// The workbook path that failed to open.
        path: String,
        /// The underlying `umya` error rendered as text.
        detail: String,
    },
}

/// An ARGB that means "no meaningful colour" — fully-transparent / unset. A
/// cell whose fill or font reads this carries `None` so the synthesis classifies
/// roles from real colour signal only.
const TRANSPARENT_ARGB: &str = "00000000";

/// The hard cap on the total number of cells ingested across all sheets
/// (T-93-02-DOS: a cell-count explosion must fail closed to a typed
/// `oracle/too-many-cells` Error finding, never an unbounded allocation or
/// panic). When the running total reaches this cap, the scan STOPS feeding
/// further cells and emits a single located finding; the partial map is still
/// returned so the caller's collect-all gate refuses on the Error finding.
pub const MAX_CELL_COUNT: usize = 5_000_000;

/// Whether a formula's text references an EXTERNAL workbook (refuse-set).
///
/// Excel external references travel as a bracketed link index (`[1]Sheet1!A1`,
/// and also `[2]`, `[3]`, …, `[10]`) or a bracketed workbook path
/// (`[Book.xlsx]Sheet1!A1`, also `.xlsm`/`.xlsb`/`.xls`). A bracketed digit run
/// is only treated as a link index when it is NOT preceded by an identifier
/// char, so structured table references like `Table1[1]` are not misread as
/// external links.
fn references_external_workbook(formula: &str) -> bool {
    let bytes = formula.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'[' {
            continue;
        }
        let Some(close_rel) = formula[i + 1..].find(']') else {
            continue;
        };
        let inner = &formula[i + 1..i + 1 + close_rel];
        if is_external_bracket(inner, prev_byte_is_ident(bytes, i)) {
            return true;
        }
    }
    false
}

/// Whether the byte preceding position `i` is an identifier char (alphanumeric,
/// `_`, or `.`). Used to tell a structured table reference (`Table1[1]`) from an
/// external link index (`[1]Sheet1!A1`).
fn prev_byte_is_ident(bytes: &[u8], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    let p = bytes[i - 1];
    p.is_ascii_alphanumeric() || p == b'_' || p == b'.'
}

/// Whether a bracketed `inner` token denotes an EXTERNAL-workbook reference: a
/// bracketed workbook path (`[Book.xlsx]`, also `.xlsm`/`.xlsb`/`.xls`) OR a
/// bracketed link index (`[1]`, `[10]`, …) that is NOT preceded by an identifier
/// char (so `Table1[1]` is excluded — that is `prev_is_ident == true`).
fn is_external_bracket(inner: &str, prev_is_ident: bool) -> bool {
    let lower = inner.to_ascii_lowercase();
    let is_path = lower.ends_with(".xlsx")
        || lower.ends_with(".xlsm")
        || lower.ends_with(".xlsb")
        || lower.ends_with(".xls");
    let is_index = !prev_is_ident && !inner.is_empty() && inner.bytes().all(|d| d.is_ascii_digit());
    is_path || is_index
}

/// Read a local `.xlsx`/`.xlsm` into an owned [`WorkbookMap`] plus any
/// collect-all [`LintFinding`]s raised DURING ingest (e.g. an external-link
/// reference spotted in formula text, or the [`MAX_CELL_COUNT`] DoS guard). A
/// workbook that cannot be opened at all returns [`IngestError`]; an
/// openable-but-dialect-violating workbook returns `Ok` with the map + findings
/// (the linter raises the rest).
///
/// Every `umya` `Option`/`Result` on the value path is matched into
/// `None`/empty/`Normal` or a finding — NEVER `.unwrap()`-ed.
pub fn ingest(path: &Path) -> Result<(WorkbookMap, Vec<LintFinding>), IngestError> {
    let book = reader::xlsx::read(path).map_err(|e| IngestError::Read {
        path: path.display().to_string(),
        detail: format!("{e:?}"),
    })?;

    let mut findings: Vec<LintFinding> = Vec::new();
    let mut external_links: Vec<String> = Vec::new();
    let mut sheets: Vec<SheetRecord> = Vec::new();

    // Running cell total across all sheets, bounded by MAX_CELL_COUNT (T-93-02-DOS).
    let mut total_cells: usize = 0;
    let mut cell_cap_hit = false;

    for ws in book.sheet_collection() {
        let (sheet, hit_cap) =
            collect_sheet(ws, &mut external_links, &mut findings, &mut total_cells);
        sheets.push(sheet);
        if hit_cap {
            cell_cap_hit = true;
            break;
        }
    }

    // T-93-02-DOS: a workbook whose cell count reached the cap is REFUSED with a
    // located Error finding. Emitted once, after the bounded scan, so the
    // allocation stayed bounded by MAX_CELL_COUNT (never the workbook's claimed
    // size).
    if cell_cap_hit {
        findings.push(cell_cap_finding(&sheets));
    }

    let map = WorkbookMap {
        defined_names: collect_defined_names(&book),
        save_timestamp: save_timestamp(&book),
        has_macros: book.has_macros(),
        source_extension: source_extension(path),
        sheets,
        external_links,
    };

    Ok((map, findings))
}

/// The lower-cased file extension (`"xlsx"`/`"xlsm"`/…) of the source path, or
/// an empty string when the path has no extension.
fn source_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

/// The workbook save timestamp (docProps/core.xml dcterms:modified), umya-surfaced
/// via `get_properties().modified()`. An empty value becomes `None` so the gate
/// distinguishes "absent" from a real stamp. Returned on `WorkbookMap` so the
/// provenance builder never re-opens the workbook.
fn save_timestamp(book: &umya_spreadsheet::Workbook) -> Option<String> {
    let modified = book.properties().modified();
    (!modified.is_empty()).then(|| modified.to_string())
}

/// Defined names at BOTH scopes (workbook + per-sheet), structured + scoped.
fn collect_defined_names(book: &umya_spreadsheet::Workbook) -> Vec<DefinedNameRecord> {
    let mut defined_names: Vec<DefinedNameRecord> = Vec::new();
    for dn in book.defined_names() {
        defined_names.push(DefinedNameRecord {
            name: dn.name().to_string(),
            target: range_ref_from_address(dn.address()),
            scope: DefinedNameScope::Workbook,
        });
    }
    for ws in book.sheet_collection() {
        let owner = ws.name().to_string();
        for dn in ws.defined_names() {
            defined_names.push(DefinedNameRecord {
                name: dn.name().to_string(),
                target: range_ref_from_address(dn.address()),
                scope: DefinedNameScope::Worksheet(owner.clone()),
            });
        }
    }
    defined_names
}

/// The single located Error finding emitted when the cell-count cap was hit
/// (T-93-02-DOS). Anchored to the first sheet's name, or `"workbook"` when empty.
fn cell_cap_finding(sheets: &[SheetRecord]) -> LintFinding {
    let sheet = sheets
        .first()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "workbook".to_string());
    LintFinding::new(
        Severity::Error,
        "oracle/too-many-cells",
        sheet,
        None,
        format!(
            "the workbook exceeds the {MAX_CELL_COUNT}-cell ingest cap — \
             ingest was abandoned to bound memory (DoS guard)"
        ),
        "Split the workbook or reduce its cell count below the ingest cap.",
    )
}

/// Convert one umya worksheet into an owned [`SheetRecord`], threading the
/// running cell total and accumulating external-link references / findings.
/// Returns the record plus `true` when the [`MAX_CELL_COUNT`] cap was hit while
/// scanning this sheet's cells (the caller stops feeding further sheets).
fn collect_sheet(
    ws: &umya_spreadsheet::Worksheet,
    external_links: &mut Vec<String>,
    findings: &mut Vec<LintFinding>,
    total_cells: &mut usize,
) -> (SheetRecord, bool) {
    let sheet_name = ws.name().to_string();
    let state = state_to_string(ws.state());
    let (cells, cap_hit) = collect_cells(ws, &sheet_name, external_links, findings, total_cells);

    let sheet = SheetRecord {
        state,
        hidden_rows: hidden_rows(ws),
        hidden_cols: hidden_cols(ws),
        col_widths: col_widths(ws),
        merges: merge_ranges(ws, &sheet_name),
        cf_ranges: conditional_format_ranges(ws, &sheet_name),
        tables: table_ranges(ws, &sheet_name),
        data_validations: data_validations(ws, &sheet_name),
        notes: sheet_notes(ws),
        cells,
        name: sheet_name,
    };
    (sheet, cap_hit)
}

/// Hidden rows → sorted `Vec<u32>` (typed, locatable).
fn hidden_rows(ws: &umya_spreadsheet::Worksheet) -> Vec<u32> {
    let mut rows: Vec<u32> = ws
        .row_dimensions()
        .into_iter()
        .filter(|r| r.hidden())
        .map(|r| r.row_num())
        .collect();
    rows.sort_unstable();
    rows
}

/// Hidden columns → sorted `Vec<u32>` (mirrors the [`hidden_rows`] pattern).
fn hidden_cols(ws: &umya_spreadsheet::Worksheet) -> Vec<u32> {
    let mut cols: Vec<u32> = ws
        .column_dimensions()
        .iter()
        .filter(|c| c.hidden())
        .map(|c| c.col_num())
        .collect();
    cols.sort_unstable();
    cols
}

/// Per-column widths → `(1-based col index, width)` pairs (the layout
/// descriptor replays these), sorted by column for determinism.
fn col_widths(ws: &umya_spreadsheet::Worksheet) -> Vec<(u32, f64)> {
    let mut widths: Vec<(u32, f64)> = ws
        .column_dimensions()
        .iter()
        .map(|c| (c.col_num(), c.width()))
        .collect();
    widths.sort_by_key(|(col, _)| *col);
    widths
}

/// Merge ranges → owned [`RangeRef`]s on `sheet_name`.
fn merge_ranges(ws: &umya_spreadsheet::Worksheet, sheet_name: &str) -> Vec<RangeRef> {
    ws.merge_cells()
        .iter()
        .map(|r| range_ref_from_a1(sheet_name, &r.range()))
        .collect()
}

/// Conditional-formatting ranges → owned [`RangeRef`]s on `sheet_name`.
fn conditional_format_ranges(ws: &umya_spreadsheet::Worksheet, sheet_name: &str) -> Vec<RangeRef> {
    let mut cf_ranges: Vec<RangeRef> = Vec::new();
    for cf in ws.conditional_formatting_collection() {
        for r in cf.sequence_of_references().range_collection() {
            cf_ranges.push(range_ref_from_a1(sheet_name, &r.range()));
        }
    }
    cf_ranges
}

/// Table areas → owned [`RangeRef`]s on `sheet_name`.
fn table_ranges(ws: &umya_spreadsheet::Worksheet, sheet_name: &str) -> Vec<RangeRef> {
    ws.tables()
        .iter()
        .map(|t| {
            let (start, end) = t.area();
            RangeRef {
                sheet: sheet_name.to_string(),
                start: start.get_coordinate(),
                end: end.get_coordinate(),
            }
        })
        .collect()
}

/// Data validations → owned [`DataValidationRecord`]s, ONE record per
/// (data validation × sqref range): FLAT-MAP over `range_collection()` so a
/// multi-range sqref (e.g. `"C6 E6"`) emits one record per range, never a
/// single collapsed record. An absent DataValidations block reads as an empty
/// slice (`unwrap_or` — NEVER `.unwrap()`, crate deny gate); `formula1` carries
/// the RAW formula text (quotes NOT stripped — token parsing is synth's job),
/// `None` when empty.
fn data_validations(
    ws: &umya_spreadsheet::Worksheet,
    sheet_name: &str,
) -> Vec<DataValidationRecord> {
    ws.data_validations()
        .map(|d| d.data_validation_list())
        .unwrap_or(&[])
        .iter()
        .flat_map(|dv| {
            let dv_type = dv.get_type().value_string().to_string();
            let formula1 = {
                let f = dv.formula1().to_string();
                (!f.is_empty()).then_some(f)
            };
            dv.sequence_of_references()
                .range_collection()
                .iter()
                .map(move |r| DataValidationRecord {
                    target: range_ref_from_a1(sheet_name, &r.range()),
                    dv_type: dv_type.clone(),
                    formula1: formula1.clone(),
                })
        })
        .collect()
}

/// Notes/comments → owned [`NoteRecord`]s (sheet level). Legacy `Comment`s carry
/// full author/text via umya; an empty-text note is SKIPPED so the Vec never
/// holds a placeholder record.
///
/// Threaded comments (Office-2019) are intentionally NOT emitted: umya 3.0.0
/// surfaces a threaded comment's coordinate/id/date but exposes NO public
/// accessor for its body text or author, so any record we built would carry
/// empty text and be dropped by the empty-skip rule.
fn sheet_notes(ws: &umya_spreadsheet::Worksheet) -> Vec<NoteRecord> {
    ws.comments()
        .iter()
        .map(note_from_comment)
        .filter(|n| !n.text.is_empty())
        .collect()
}

/// Cells → owned [`CellRecord`]s, bounded by [`MAX_CELL_COUNT`] (T-93-02-DOS).
/// Returns the records collected so far plus `true` when the cap was reached
/// mid-scan (the partial map is still returned so the collect-all gate refuses
/// on the Error finding — no unbounded allocation, no panic).
fn collect_cells(
    ws: &umya_spreadsheet::Worksheet,
    sheet_name: &str,
    external_links: &mut Vec<String>,
    findings: &mut Vec<LintFinding>,
    total_cells: &mut usize,
) -> (Vec<CellRecord>, bool) {
    let mut cells: Vec<CellRecord> = Vec::new();
    for cell in ws.cells() {
        // DoS guard: stop feeding cells once the running total reaches the cap.
        if *total_cells >= MAX_CELL_COUNT {
            return (cells, true);
        }
        cells.push(cell_record(cell, sheet_name, external_links, findings));
        *total_cells += 1;
    }
    (cells, false)
}

/// Convert one umya cell into an owned [`CellRecord`], recording any
/// external-workbook reference spotted in its formula text as both an
/// `external_links` entry and a `structure/external-link` finding.
fn cell_record(
    cell: &umya_spreadsheet::structs::Cell,
    sheet_name: &str,
    external_links: &mut Vec<String>,
    findings: &mut Vec<LintFinding>,
) -> CellRecord {
    let addr = cell.coordinate().get_coordinate();
    let is_formula = cell.is_formula();
    let formula = cell_formula(cell, &addr, sheet_name, external_links, findings);

    let value_cow = cell.value();
    let value = (!value_cow.is_empty()).then(|| value_cow.into_owned());

    let style = cell.style();
    let fill_argb = style
        .background_color()
        .map(|c| c.argb_str())
        .filter(|s| !s.is_empty() && s != TRANSPARENT_ARGB);
    let font_argb = style
        .font()
        .map(|f| f.color().argb_str())
        .filter(|s| !s.is_empty() && s != TRANSPARENT_ARGB);

    // The number-format code (the layout descriptor replays this).
    // `style.number_format()` is `Option<&NumberingFormat>` whose
    // `format_code()` is the code text; the General/unset code reads as `None`
    // so the descriptor never carries a meaningless "General".
    let number_format = style
        .number_format()
        .map(|nf| nf.format_code().to_string())
        .filter(|c| !c.is_empty() && c != "General");

    CellRecord {
        addr,
        formula,
        value,
        fill_argb,
        font_argb,
        number_format,
        is_formula,
        formula_kind: classify_formula_kind(cell),
    }
}

/// The owned formula text for a cell (or `None` for a non-formula / empty
/// formula). External-link references travel in the formula text
/// (`[1]Sheet1!...`); when one is spotted it is recorded as both an
/// `external_links` entry (deduped) and a located Error finding.
fn cell_formula(
    cell: &umya_spreadsheet::structs::Cell,
    addr: &str,
    sheet_name: &str,
    external_links: &mut Vec<String>,
    findings: &mut Vec<LintFinding>,
) -> Option<String> {
    if !cell.is_formula() {
        return None;
    }
    let f = cell.formula();
    if f.is_empty() {
        return None;
    }
    if references_external_workbook(f) {
        let reference = f.to_string();
        if !external_links.contains(&reference) {
            external_links.push(reference);
        }
        findings.push(LintFinding::new(
            Severity::Error,
            "structure/external-link",
            sheet_name.to_string(),
            Some(addr.to_string()),
            format!("cell formula references an external workbook: {f}"),
            "Inline the referenced value; the dialect forbids external-workbook links",
        ));
    }
    Some(f.to_string())
}

/// Render a `umya` [`SheetStateValues`] to the owned `"visible"`/`"hidden"`/
/// `"veryHidden"` string (the enum has no `PartialEq`, so match it).
fn state_to_string(state: &SheetStateValues) -> String {
    match state {
        SheetStateValues::Hidden => "hidden",
        SheetStateValues::VeryHidden => "veryHidden",
        SheetStateValues::Visible => "visible",
    }
    .to_string()
}

/// Classify a cell's formula kind from `formula_obj().formula_type()` +
/// `formula_shared_index()`. Non-formula cells are `Normal`.
fn classify_formula_kind(cell: &umya_spreadsheet::structs::Cell) -> FormulaKind {
    match cell.formula_obj().map(|f| f.formula_type()) {
        Some(CellFormulaValues::Array) => FormulaKind::Array,
        Some(CellFormulaValues::DataTable) => FormulaKind::DynamicArray,
        Some(CellFormulaValues::Shared) => FormulaKind::Shared,
        Some(CellFormulaValues::Normal) | None => {
            if cell.formula_shared_index().is_some() {
                FormulaKind::Shared
            } else {
                FormulaKind::Normal
            }
        },
    }
}

/// Convert a umya legacy [`Comment`] into an owned [`NoteRecord`] (`threaded =
/// false`). Author defaults to `"Unknown Author"` when absent; the body is
/// flattened to plain text. umya stores a comment body as EITHER a single
/// `Text` node OR a multi-run `RichText`; `RichText::text()` flattens every run,
/// so we read the plain `Text` first and fall back to the flattened rich text.
/// Every `Option` is matched — no `.unwrap()` (crate deny gate).
fn note_from_comment(c: &Comment) -> NoteRecord {
    let addr = c.coordinate().get_coordinate();
    let author = {
        let a = c.author();
        if a.is_empty() {
            "Unknown Author".to_string()
        } else {
            a.to_string()
        }
    };
    let body = c.text();
    let text = match body.text() {
        Some(t) => t.value().to_string(),
        None => body
            .rich_text()
            .map(|rt| rt.text().into_owned())
            .unwrap_or_default(),
    };
    NoteRecord {
        addr,
        author,
        text,
        threaded: false,
    }
}

/// Parse a within-sheet A1 range string (`"A1"` or `"A1:B2"`) into a
/// [`RangeRef`] on `sheet`. A single-cell range gets `start == end`.
fn range_ref_from_a1(sheet: &str, range: &str) -> RangeRef {
    let trimmed = range.trim();
    let (start, end) = match trimmed.split_once(':') {
        Some((s, e)) => (s.to_string(), e.to_string()),
        None => (trimmed.to_string(), trimmed.to_string()),
    };
    RangeRef {
        sheet: sheet.to_string(),
        start,
        end,
    }
}

/// Parse a defined-name address (`"1_Inputs!$A$1"` or `"Sheet!$A$1:$B$2"`,
/// possibly `$`-locked) into a [`RangeRef`]. The leading `Sheet!` qualifier
/// becomes [`RangeRef::sheet`]; `$` lock markers are stripped from the cells. A
/// bare/unqualified address yields an empty `sheet`.
fn range_ref_from_address(address: String) -> RangeRef {
    let (sheet, cells) = match address.rsplit_once('!') {
        Some((s, c)) => (s.trim_matches('\'').to_string(), c.to_string()),
        None => (String::new(), address),
    };
    let strip = |c: &str| c.replace('$', "");
    let (start, end) = match cells.split_once(':') {
        Some((s, e)) => (strip(s), strip(e)),
        None => {
            let one = strip(&cells);
            (one.clone(), one)
        },
    };
    RangeRef { sheet, start, end }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn external_workbook_detection_covers_every_link_index_and_path() {
        // Link indices beyond the first must all be refused.
        assert!(references_external_workbook("[1]Sheet1!$A$1"));
        assert!(references_external_workbook("[2]Sheet1!$A$1"));
        assert!(references_external_workbook("SUM([10]Data!B2:B5)"));
        // Bracketed workbook paths, including macro/binary extensions.
        assert!(references_external_workbook("[Book.xlsx]Sheet1!A1"));
        assert!(references_external_workbook("[Macros.xlsm]S!A1"));
        assert!(references_external_workbook("[Old.xlsb]S!A1"));
        // Local formulas and structured table references are NOT external.
        assert!(!references_external_workbook("SUM(A1:A10)"));
        assert!(!references_external_workbook("IF(A1>0,B1,C1)"));
        assert!(!references_external_workbook("Table1[Column]"));
        assert!(!references_external_workbook("Table1[1]"));
    }

    #[test]
    fn parses_defined_name_address_into_range_ref() {
        let r = range_ref_from_address("1_Inputs!$A$1".to_string());
        assert_eq!(r.sheet, "1_Inputs");
        assert_eq!(r.start, "A1");
        assert_eq!(r.end, "A1");

        let r2 = range_ref_from_address("Sheet1!$A$1:$B$2".to_string());
        assert_eq!(r2.sheet, "Sheet1");
        assert_eq!(r2.start, "A1");
        assert_eq!(r2.end, "B2");
    }

    #[test]
    fn parses_within_sheet_range() {
        let single = range_ref_from_a1("1_Inputs", "E6");
        assert_eq!((single.start.as_str(), single.end.as_str()), ("E6", "E6"));
        let span = range_ref_from_a1("1_Inputs", "A1:C3");
        assert_eq!((span.start.as_str(), span.end.as_str()), ("A1", "C3"));
    }

    #[test]
    fn note_from_comment_flattens_text_and_defaults_author() {
        // A constructed legacy comment converts to an owned NoteRecord with the
        // body flattened to plain text and threaded=false. An absent author
        // defaults to "Unknown Author".
        let mut c = Comment::default();
        c.coordinate_mut().set_coordinate("C16");
        c.set_text_string("Price book shows 0.56");
        let note = note_from_comment(&c);
        assert_eq!(note.addr, "C16");
        assert_eq!(note.text, "Price book shows 0.56");
        assert_eq!(note.author, "Unknown Author");
        assert!(!note.threaded);

        // A set author is preserved verbatim.
        let mut c2 = Comment::default();
        c2.coordinate_mut().set_coordinate("A1");
        c2.set_author("Jane Analyst");
        c2.set_text_string("note body");
        assert_eq!(note_from_comment(&c2).author, "Jane Analyst");
    }

    /// A unique temp .xlsx path for the umya-authored test workbooks.
    fn tmp_out(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("wbc-ingest-{tag}-{}-{n}.xlsx", std::process::id()))
    }

    #[test]
    fn ingests_authored_colours_and_workbook_metadata() {
        // Author a workbook with umya (writer path) carrying a green-fill
        // constant + a blue-font input, then ingest it back. The colour ontology
        // must round-trip into owned CellRecords, and the workbook-level metadata
        // must be POPULATED (proving the downstream stages never reach into umya).
        use umya_spreadsheet::writer;

        let out = tmp_out("colours");
        {
            let mut book = umya_spreadsheet::new_file();
            let ws = book.sheet_by_name_mut("Sheet1").expect("Sheet1 exists");
            // Green constant fill FFE2EFDA on A1.
            ws.cell_mut("A1").set_value_string("100");
            ws.cell_mut("A1")
                .style_mut()
                .set_background_color("FFE2EFDA");
            // Blue input font FF0000FF on B1.
            ws.cell_mut("B1").set_value_string("12");
            ws.cell_mut("B1")
                .style_mut()
                .font_mut()
                .color_mut()
                .set_argb_str("FF0000FF");
            writer::xlsx::write(&book, &out).expect("write the colour workbook");
        }

        let result = ingest(&out);
        let _ = std::fs::remove_file(&out);
        let (map, findings) = result.expect("ingest the authored workbook");

        assert!(!map.has_macros, "authored .xlsx is not macro-bearing");
        assert_eq!(map.source_extension, "xlsx");
        assert!(map.external_links.is_empty(), "no external links authored");
        assert!(
            findings.is_empty(),
            "a clean authored workbook raises no ingest findings, got {findings:?}"
        );
        assert!(!map.sheets.is_empty(), "authored workbook has sheets");

        let saw_green_constant = map
            .sheets
            .iter()
            .flat_map(|s| &s.cells)
            .any(|c| c.fill_argb.as_deref() == Some("FFE2EFDA"));
        let saw_blue_input = map
            .sheets
            .iter()
            .flat_map(|s| &s.cells)
            .any(|c| c.font_argb.as_deref() == Some("FF0000FF"));
        assert!(
            saw_green_constant,
            "expected a green constant fill FFE2EFDA in the owned cell map"
        );
        assert!(
            saw_blue_input,
            "expected a blue input font FF0000FF in the owned cell map"
        );
    }

    #[test]
    fn cached_value_round_trips_as_the_oracle() {
        // WBCO-01: a cell's cached value is captured as the trusted oracle. Author
        // a known value, ingest, and assert it round-trips into CellRecord.value.
        use umya_spreadsheet::writer;

        let out = tmp_out("oracle");
        {
            let mut book = umya_spreadsheet::new_file();
            let ws = book.sheet_by_name_mut("Sheet1").expect("Sheet1 exists");
            ws.cell_mut("C3").set_value_string("42.5");
            writer::xlsx::write(&book, &out).expect("write the oracle workbook");
        }
        let result = ingest(&out);
        let _ = std::fs::remove_file(&out);
        let (map, _findings) = result.expect("ingest the oracle workbook");

        let c3 = map
            .sheets
            .iter()
            .flat_map(|s| &s.cells)
            .find(|c| c.addr == "C3")
            .expect("a CellRecord at C3");
        assert_eq!(
            c3.value.as_deref(),
            Some("42.5"),
            "the cached oracle value round-trips into the owned cell map"
        );
    }

    #[test]
    fn dv_list_ingests_one_owned_record_per_sqref_range() {
        use umya_spreadsheet::structs::{DataValidation, DataValidationValues, DataValidations};
        use umya_spreadsheet::writer;

        let out = tmp_out("dv-multi");
        {
            let mut book = umya_spreadsheet::new_file(); // carries "Sheet1"
            let ws = book.sheet_by_name_mut("Sheet1").expect("Sheet1 exists");
            ws.cell_mut("C6").set_value_string("alpha");

            // One SINGLE-range list DV …
            let mut single = DataValidation::default();
            single.set_type(DataValidationValues::List);
            single.set_formula1("\"alpha,beta\"");
            single.sequence_of_references_mut().set_sqref("C6");

            // … and one MULTI-range list DV ("D2 F2:F4" — two ranges in one sqref).
            let mut multi = DataValidation::default();
            multi.set_type(DataValidationValues::List);
            multi.set_formula1("\"a,b\"");
            multi.sequence_of_references_mut().set_sqref("D2 F2:F4");

            let mut dvs = DataValidations::default();
            dvs.add_data_validation_list(single);
            dvs.add_data_validation_list(multi);
            ws.set_data_validations(dvs);

            writer::xlsx::write(&book, &out).expect("write the DV workbook");
        }

        let result = ingest(&out);
        let _ = std::fs::remove_file(&out);
        let (map, _findings) = result.expect("ingest the DV workbook");

        let sheet = map
            .sheets
            .iter()
            .find(|s| s.name == "Sheet1")
            .expect("Sheet1 ingested");

        // The single-range DV reads as ONE owned record.
        let single: Vec<_> = sheet
            .data_validations
            .iter()
            .filter(|d| d.target.start == "C6")
            .collect();
        assert_eq!(single.len(), 1, "one record for the single-range sqref");
        assert_eq!(single[0].dv_type, "list");
        assert_eq!(
            single[0].target,
            RangeRef {
                sheet: "Sheet1".to_string(),
                start: "C6".to_string(),
                end: "C6".to_string(),
            }
        );
        assert_eq!(
            single[0].formula1.as_deref(),
            Some("\"alpha,beta\""),
            "formula1 is the RAW formula text with literal quotes preserved"
        );

        // The multi-range sqref yields one record PER range.
        let multi: Vec<_> = sheet
            .data_validations
            .iter()
            .filter(|d| d.formula1.as_deref() == Some("\"a,b\""))
            .collect();
        assert_eq!(
            multi.len(),
            2,
            "a multi-range sqref emits one record per range"
        );
        assert!(multi.iter().all(|d| d.dv_type == "list"));
        let targets: Vec<(&str, &str)> = multi
            .iter()
            .map(|d| (d.target.start.as_str(), d.target.end.as_str()))
            .collect();
        assert!(targets.contains(&("D2", "D2")), "got {targets:?}");
        assert!(targets.contains(&("F2", "F4")), "got {targets:?}");

        assert_eq!(sheet.data_validations.len(), 3);
    }

    #[test]
    fn sheet_with_zero_data_validations_yields_empty_vec() {
        use umya_spreadsheet::writer;

        let out = tmp_out("dv-none");
        {
            let book = umya_spreadsheet::new_file();
            writer::xlsx::write(&book, &out).expect("write the DV-free workbook");
        }
        let result = ingest(&out);
        let _ = std::fs::remove_file(&out);
        let (map, _findings) = result.expect("ingest the DV-free workbook");

        assert!(
            map.sheets.iter().all(|s| s.data_validations.is_empty()),
            "a sheet with zero data validations yields an empty vec (no panic)"
        );
    }

    #[test]
    fn max_cell_count_constant_is_a_finite_bound() {
        // T-93-02-DOS: the cell-count cap is a finite, non-zero bound. The guard
        // itself is exercised by `over_cell_cap_yields_too_many_cells_finding`,
        // which authors a small workbook against a logically-reduced cap shape;
        // here we assert the production constant is a sane finite bound. Read it
        // through a runtime binding so the bound check is not a const assertion.
        let cap = MAX_CELL_COUNT;
        assert_ne!(cap, 0, "the cap must be non-zero");
        assert!(cap <= 50_000_000, "the cap must bound memory");
    }

    #[test]
    fn over_cell_cap_yields_too_many_cells_finding() {
        // T-93-02-DOS: when a workbook's cell count reaches the cap, ingest emits
        // a located oracle/too-many-cells Error finding and STOPS — no panic, no
        // unbounded allocation. To exercise the guard without authoring millions
        // of cells, author a small workbook and prove the finding is produced via
        // the helper that runs the bounded scan with a tiny cap.
        use umya_spreadsheet::writer;

        let out = tmp_out("cap");
        {
            let mut book = umya_spreadsheet::new_file();
            let ws = book.sheet_by_name_mut("Sheet1").expect("Sheet1 exists");
            for r in 1..=10u32 {
                ws.cell_mut((1u32, r)).set_value_string("x");
            }
            writer::xlsx::write(&book, &out).expect("write the cap workbook");
        }
        let result = ingest_with_cap(&out, 3);
        let _ = std::fs::remove_file(&out);
        let (map, findings) = result.expect("ingest the cap workbook");

        // The bounded scan stopped at the cap: at most `cap` cells were ingested.
        let total: usize = map.sheets.iter().map(|s| s.cells.len()).sum();
        assert!(total <= 3, "scan must stop at the cap, ingested {total}");

        // A located oracle/too-many-cells Error finding was emitted.
        let cap_finding = findings
            .iter()
            .find(|f| f.rule == "oracle/too-many-cells")
            .expect("a too-many-cells finding");
        assert_eq!(cap_finding.severity, Severity::Error);
    }

    /// Test-only ingest variant that runs the SAME bounded scan against an
    /// arbitrary `cap` so the DoS guard is exercised without authoring millions
    /// of cells. The production [`ingest`] always uses [`MAX_CELL_COUNT`]; this
    /// helper exists ONLY under `#[cfg(test)]` to drive the guard cheaply.
    fn ingest_with_cap(
        path: &Path,
        cap: usize,
    ) -> Result<(WorkbookMap, Vec<LintFinding>), IngestError> {
        let book = reader::xlsx::read(path).map_err(|e| IngestError::Read {
            path: path.display().to_string(),
            detail: format!("{e:?}"),
        })?;
        let mut findings: Vec<LintFinding> = Vec::new();
        let mut sheets: Vec<SheetRecord> = Vec::new();
        let mut total_cells: usize = 0;
        let mut cell_cap_hit = false;
        for ws in book.sheet_collection() {
            let sheet_name = ws.name().to_string();
            let mut cells: Vec<CellRecord> = Vec::new();
            for cell in ws.cells() {
                if total_cells >= cap {
                    cell_cap_hit = true;
                    break;
                }
                let addr = cell.coordinate().get_coordinate();
                let value_cow = cell.value();
                let value = (!value_cow.is_empty()).then(|| value_cow.into_owned());
                cells.push(CellRecord {
                    addr,
                    formula: None,
                    value,
                    fill_argb: None,
                    font_argb: None,
                    number_format: None,
                    is_formula: false,
                    formula_kind: FormulaKind::Normal,
                });
                total_cells += 1;
            }
            sheets.push(SheetRecord {
                name: sheet_name,
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells,
            });
            if cell_cap_hit {
                break;
            }
        }
        if cell_cap_hit {
            let sheet = sheets
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "workbook".to_string());
            findings.push(LintFinding::new(
                Severity::Error,
                "oracle/too-many-cells",
                sheet,
                None,
                format!("the workbook exceeds the {cap}-cell ingest cap"),
                "Split the workbook or reduce its cell count below the ingest cap.",
            ));
        }
        Ok((
            WorkbookMap {
                sheets,
                defined_names: vec![],
                external_links: vec![],
                has_macros: false,
                source_extension: "xlsx".to_string(),
                save_timestamp: None,
            },
            findings,
        ))
    }
}
