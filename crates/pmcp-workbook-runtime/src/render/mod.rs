//! The serve-side, WRITER-ONLY render module (Phase 12).
//!
//! Plan 01 landed the shared, versioned [`LayoutDescriptor`] serde shape
//! ([`layout`]) — the umya-free, zip-free single definition the offline emitter
//! and the serve-time writer share. Plan 02 (this code) adds the writer itself:
//! [`render_xlsx`] replays a [`LayoutDescriptor`] and injects the executor's
//! already-computed values into a "copy of the workbook, filled in," producing
//! valid, DETERMINISTIC `.xlsx` bytes IN MEMORY (no filesystem — Lambda-safe,
//! RESEARCH Pitfall 6).
//!
//! The writer links `rust_xlsxwriter` (a WRITER; it pulls the `zip` deflate
//! container but NO workbook reader — D-01, the single deliberate cross-phase
//! purity-gate relaxation). It is reader-free: `just purity-check` proves
//! `umya`/`quick-xml` stay absent from the served tree while asserting the
//! writer is present.
//!
//! Determinism (review item 8, T-12-15) is a FIRST-CLASS invariant: the writer
//! pins the workbook's document properties to a FIXED creation datetime + empty
//! author/metadata so two renders of the same `(layout, run)` are byte-identical.
//! Plan 03's regenerate-on-read returns fresh bytes every read and relies on
//! this byte-stability; the invariant is proven HERE (a determinism test) where
//! it is introduced, not deferred.

use std::collections::HashMap;

use rust_xlsxwriter::{Color, DocProperties, ExcelDateTime, Format, Formula, Workbook};

use crate::cell_key;
use crate::resolve::{a1_to_zero_indexed_row_col, parse_a1};
use crate::sheet_ir::value::CellValue;
use crate::sheet_ir::RunResult;

/// Map a `rust_xlsxwriter` error into [`RenderError::Writer`]. One shared
/// converter so every writer call site uses `.map_err(writer_err)` instead of
/// re-spelling the closure (simplify pass).
fn writer_err(e: rust_xlsxwriter::XlsxError) -> RenderError {
    RenderError::Writer(e.to_string())
}

/// The shared, versioned `LayoutDescriptor`/`SheetLayout`/`CellLayout` serde
/// shapes (D-05) — the FULL workbook-layout descriptor the bundle's `layout.json`
/// member serializes and the writer replays.
pub mod layout;

pub use layout::*;

/// A fallible render failure (review item 8 — the writer value path is
/// panic-free; a malformed coordinate / non-finite value / writer error surfaces
/// as an `Err`, NEVER a panic and NEVER a bogus cell). Owned `String` detail to
/// match the crate's `LintFinding` error style (no borrow across the API).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// A cell's A1 address in the descriptor did not parse to a `(row, col)`
    /// coordinate (T-12 panic-freedom — a malformed addr is an `Err`).
    #[error("malformed cell address {addr} on sheet {sheet}")]
    MalformedAddr {
        /// The owning sheet name.
        sheet: String,
        /// The A1 address that failed to parse.
        addr: String,
    },
    /// A merge range in the descriptor did not parse into two valid endpoints
    /// (or is degenerate / out of order).
    #[error("malformed merge range {range} on sheet {sheet}")]
    MalformedMerge {
        /// The owning sheet name.
        sheet: String,
        /// The merge range that failed to parse.
        range: String,
    },
    /// A computed value for a cell was a non-finite `f64` (NaN/Inf), which Excel
    /// cannot represent (handler.rs WR-06 reuse, T-12-05) — never written as a
    /// bogus number.
    #[error("non-finite computed value at {cell}")]
    NonFiniteValue {
        /// The `cell_key` (`sheet!addr`) whose computed value was non-finite.
        cell: String,
    },
    /// The underlying `rust_xlsxwriter` writer returned an error (e.g. a name or
    /// dimension limit). Carried as an owned `String` (the crate's error is not
    /// `Clone`/`Eq`).
    #[error("xlsx writer error: {0}")]
    Writer(String),
}

/// A fixed creation datetime for byte-stable output (review item 8, T-12-15):
/// the UFH milestone epoch `2024-01-01T00:00:00`. ANY constant works — what
/// matters is that it does NOT vary per render. Building it is fallible only on
/// an out-of-range constant, which this one is not.
fn fixed_creation_datetime() -> Result<ExcelDateTime, RenderError> {
    ExcelDateTime::from_ymd(2024, 1, 1)
        .and_then(|d| d.and_hms(0, 0, 0))
        .map_err(writer_err)
}

/// Normalize a captured formula for the `rust_xlsxwriter` writer (review item 4).
///
/// `rust_xlsxwriter` expects a formula string WITH a single leading `=`. The
/// descriptor MAY carry a formula already prefixed (`=SUM(A1:A2)`) or bare
/// (`SUM(A1:A2)`). This returns the formula with EXACTLY one leading `=`: a bare
/// formula is prefixed, an already-prefixed formula is returned UNCHANGED (never
/// double-prefixed into `==`). Whitespace before a leading `=` is tolerated.
#[must_use]
pub fn normalize_formula_for_writer(f: &str) -> String {
    if f.trim_start().starts_with('=') {
        f.to_string()
    } else {
        format!("={f}")
    }
}

/// Build a `Color` from a captured 8-hex ARGB string (`FFE2EFDA`) or a 6-hex RGB
/// string (`E2EFDA`). `rust_xlsxwriter`'s `Color::RGB` is a 24-bit value, so the
/// leading alpha byte (when present) is dropped. Returns `None` for an
/// unparseable string (the caller treats colour as best-effort).
fn argb_to_color(argb: &str) -> Option<Color> {
    let hex = argb.trim();
    let rgb_hex = match hex.len() {
        // ARGB -> drop the alpha byte. `get` (not a byte-indexed slice) so an
        // 8-BYTE string whose byte 2 is not a char boundary (multibyte UTF-8)
        // is `None`, never a slice panic (CR-01 — `layout.json` is untrusted
        // bundle input; the writer value path must stay panic-free).
        8 => hex.get(2..)?,
        6 => hex, // already RGB
        _ => return None,
    };
    let rgb = u32::from_str_radix(rgb_hex, 16).ok()?;
    Some(Color::RGB(rgb))
}

/// Build an optional `Format` from a cell's number-format + fill + font ARGBs.
/// Returns `None` when the cell carries no styling so unstyled cells skip the
/// format allocation. Colour/format application is BEST-EFFORT (full visual
/// fidelity is explicitly NOT the bar — RESEARCH anti-pattern); an unparseable
/// ARGB is silently skipped, never an error.
fn cell_format(cell: &CellLayout) -> Option<Format> {
    if cell.number_format.is_none() && cell.fill_argb.is_none() && cell.font_argb.is_none() {
        return None;
    }
    let mut fmt = Format::new();
    if let Some(nf) = &cell.number_format {
        fmt = fmt.set_num_format(nf.clone());
    }
    if let Some(fill) = cell.fill_argb.as_deref().and_then(argb_to_color) {
        fmt = fmt.set_background_color(fill);
    }
    if let Some(font) = cell.font_argb.as_deref().and_then(argb_to_color) {
        fmt = fmt.set_font_color(font);
    }
    Some(fmt)
}

/// The set of `(row, col)` coordinates that are INTERIOR to (but not the
/// top-left of) a merged range — written by `merge_range`, NEVER again by the
/// per-cell loop (review item 8: writing the interior of a merge is an
/// overwrite error in Excel).
type MergeInterior = std::collections::HashSet<(u32, u16)>;

/// Replay every merge range on a sheet, writing the value/format ONLY to the
/// top-left cell (review item 8). Returns the interior coordinates the per-cell
/// loop must SKIP. Merges are processed in the descriptor's stored order
/// (deterministic). A degenerate / malformed / single-cell merge is a
/// `RenderError`, never a panic.
fn replay_merges(
    ws: &mut rust_xlsxwriter::Worksheet,
    sheet: &SheetLayout,
    top_left_text: &HashMap<(u32, u16), String>,
) -> Result<MergeInterior, RenderError> {
    let mut interior = MergeInterior::new();
    let blank = Format::new();
    for range in &sheet.merges {
        let (start, end) = range
            .split_once(':')
            .ok_or_else(|| RenderError::MalformedMerge {
                sheet: sheet.name.clone(),
                range: range.clone(),
            })?;
        let malformed = || RenderError::MalformedMerge {
            sheet: sheet.name.clone(),
            range: range.clone(),
        };
        let (r0, c0) = a1_to_zero_indexed_row_col(start.trim()).ok_or_else(malformed)?;
        let (r1, c1) = a1_to_zero_indexed_row_col(end.trim()).ok_or_else(malformed)?;
        let (row_lo, row_hi) = (r0.min(r1), r0.max(r1));
        let (col_lo, col_hi) = (c0.min(c1), c0.max(c1));
        // merge_range rejects a single cell; a 1x1 "merge" is malformed input.
        if row_lo == row_hi && col_lo == col_hi {
            return Err(malformed());
        }
        // Write the top-left cell text via merge_range (it owns the interior).
        let text = top_left_text
            .get(&(row_lo, col_lo))
            .cloned()
            .unwrap_or_default();
        ws.merge_range(row_lo, col_lo, row_hi, col_hi, &text, &blank)
            .map_err(writer_err)?;
        // Record every interior coordinate (including the top-left, which
        // merge_range already wrote) so the per-cell loop skips them all.
        for r in row_lo..=row_hi {
            for c in col_lo..=col_hi {
                interior.insert((r, c));
            }
        }
    }
    Ok(interior)
}

/// Render a [`LayoutDescriptor`] + the executor's [`RunResult`] into valid,
/// DETERMINISTIC `.xlsx` bytes IN MEMORY (review item 8, D-01).
///
/// The writer replays the descriptor's sheets/cells/merges and INJECTS each
/// computed value from `run.computed` (keyed `sheet!addr`). Default lean (D-05):
/// a cell with a formula + a FINITE numeric result is written as a
/// formula-with-cached-result (`write_formula` + `Formula::set_result`); a
/// cell with no formula is written as a plain number/string. Every numeric value
/// is finiteness-guarded before write (handler.rs WR-06 reuse, T-12-05) — a
/// non-finite value is a [`RenderError::NonFiniteValue`], never a bogus NaN/Inf
/// cell. The value path is panic-free (`deny(unwrap/expect/panic)`): a malformed
/// addr/merge surfaces as an `Err`.
///
/// Determinism: the workbook's document properties are pinned to a FIXED
/// creation datetime + empty author/metadata so repeated renders are
/// byte-identical (Plan 03 regenerate-on-read relies on this; T-12-15).
///
/// Output is via `save_to_buffer()` ONLY — never a file path (Lambda-safe,
/// RESEARCH Pitfall 6).
pub fn render_xlsx(layout: &LayoutDescriptor, run: &RunResult) -> Result<Vec<u8>, RenderError> {
    let mut wb = init_workbook()?;
    for sheet in &layout.sheets {
        let ws = wb.add_worksheet();
        render_sheet(ws, sheet, run)?;
    }
    wb.save_to_buffer().map_err(writer_err)
}

/// Build the workbook with its determinism-pinned document properties (review
/// item 8, T-12-15): a FIXED creation datetime + empty author so two renders of
/// the same `(layout, run)` are byte-identical.
fn init_workbook() -> Result<Workbook, RenderError> {
    let mut wb = Workbook::new();
    let props = DocProperties::new()
        .set_author("")
        .set_creation_datetime(&fixed_creation_datetime()?);
    wb.set_properties(&props);
    Ok(wb)
}

/// Render a single sheet: scaffold (name/hidden/columns) → top-left text map →
/// merge replay → per-cell value injection. A thin per-sheet orchestrator over
/// the three phase helpers; the per-cell write order is preserved exactly so
/// output stays byte-deterministic.
fn render_sheet(
    ws: &mut rust_xlsxwriter::Worksheet,
    sheet: &SheetLayout,
    run: &RunResult,
) -> Result<(), RenderError> {
    apply_sheet_scaffold(ws, sheet)?;
    // PASS 1: resolve each cell's merge-top-left display TEXT so a merge can
    // fetch it without re-deriving (also validates each addr panic-free).
    let top_left_text = build_top_left_text(sheet, run)?;
    // Replay merges first (top-left only); collect interior coords to skip.
    let interior = replay_merges(ws, sheet, &top_left_text)?;
    // PASS 2: write every non-merge-interior cell, injecting computed values.
    for cell in &sheet.cells {
        write_cell(ws, sheet, run, cell, &interior)?;
    }
    Ok(())
}

/// Apply the sheet-level scaffold: name, hidden flag, per-column widths and
/// hidden columns (best-effort, deterministic descriptor order).
fn apply_sheet_scaffold(
    ws: &mut rust_xlsxwriter::Worksheet,
    sheet: &SheetLayout,
) -> Result<(), RenderError> {
    ws.set_name(&sheet.name).map_err(writer_err)?;
    if sheet.hidden {
        ws.set_hidden(true);
    }
    for (col_1based, width) in &sheet.col_widths {
        if let Some(col) = col_1based.checked_sub(1) {
            ws.set_column_width(col, *width).map_err(writer_err)?;
        }
    }
    for col_1based in &sheet.hidden_cols {
        if let Some(col) = col_1based.checked_sub(1) {
            ws.set_column_hidden(col).map_err(writer_err)?;
        }
    }
    Ok(())
}

/// PASS 1: resolve each cell to `(row, col)` + the text it would carry, so a
/// merge can fetch its top-left text without re-deriving it. Validates each addr
/// up front (panic-free — a bad addr is an `Err`) and rejects a non-finite
/// computed number (T-12-05) before it can leak into a merged cell.
fn build_top_left_text(
    sheet: &SheetLayout,
    run: &RunResult,
) -> Result<HashMap<(u32, u16), String>, RenderError> {
    let mut top_left_text: HashMap<(u32, u16), String> = HashMap::new();
    for cell in &sheet.cells {
        // Validate the addr up front (panic-free): a bad addr is an Err.
        if a1_to_zero_indexed_row_col(&cell.addr).is_none() {
            // Distinguish a genuinely malformed addr from one parse_a1 rejects
            // only because it overflows u16: parse_a1 succeeding but the
            // conversion failing is still malformed-for-the-writer.
            let _ = parse_a1(&cell.addr); // documents the reuse; result unused
            return Err(RenderError::MalformedAddr {
                sheet: sheet.name.clone(),
                addr: cell.addr.clone(),
            });
        }
        let key = cell_key(&sheet.name, &cell.addr);
        let display = display_text(run, &key, cell)?;
        if let (Some((r, c)), Some(text)) = (a1_to_zero_indexed_row_col(&cell.addr), display) {
            top_left_text.insert((r, c), text);
        }
    }
    Ok(top_left_text)
}

/// The text a merged top-left should display: prefer the computed value, else
/// the descriptor's captured value text. A non-finite computed number is an
/// `Err` (T-12-05), never a bogus merged cell.
fn display_text(
    run: &RunResult,
    key: &str,
    cell: &CellLayout,
) -> Result<Option<String>, RenderError> {
    let display = match run.computed.get(key) {
        Some(CellValue::Number(n)) if n.is_finite() => Some(format_number(*n)),
        Some(CellValue::Number(_)) => {
            return Err(RenderError::NonFiniteValue {
                cell: key.to_string(),
            })
        },
        Some(CellValue::Text(s)) => Some(s.clone()),
        Some(CellValue::Bool(b)) => Some(b.to_string()),
        _ => cell.value.clone(),
    };
    Ok(display)
}

/// PASS 2: write a single non-merge-interior cell, injecting its computed value.
/// A coordinate owned by a merge range is skipped (merge_range already wrote it).
fn write_cell(
    ws: &mut rust_xlsxwriter::Worksheet,
    sheet: &SheetLayout,
    run: &RunResult,
    cell: &CellLayout,
    interior: &MergeInterior,
) -> Result<(), RenderError> {
    let (row, col) =
        a1_to_zero_indexed_row_col(&cell.addr).ok_or_else(|| RenderError::MalformedAddr {
            sheet: sheet.name.clone(),
            addr: cell.addr.clone(),
        })?;
    if interior.contains(&(row, col)) {
        return Ok(()); // merge_range already owns this coordinate
    }
    let key = cell_key(&sheet.name, &cell.addr);
    let computed = run.computed.get(&key);
    let fmt = cell_format(cell);
    write_computed_value(ws, row, col, cell, computed, key, fmt.as_ref())
}

/// Dispatch a cell's computed value to the right writer (flat match): a finite
/// number → number/formula cell; text/bool → string cell; error/empty/not-computed
/// → fall back to the captured literal. A non-finite number is an `Err` (T-12-05).
fn write_computed_value(
    ws: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    cell: &CellLayout,
    computed: Option<&CellValue>,
    key: String,
    fmt: Option<&Format>,
) -> Result<(), RenderError> {
    match computed {
        Some(CellValue::Number(n)) => {
            // WR-06 / T-12-05: a non-finite computed number is never written as
            // a bogus cell — fail loud.
            if !n.is_finite() {
                return Err(RenderError::NonFiniteValue { cell: key });
            }
            write_number_cell(ws, row, col, cell, *n, fmt)?;
        },
        Some(CellValue::Text(s)) => write_string_cell(ws, row, col, s, fmt)?,
        Some(CellValue::Bool(b)) => write_string_cell(ws, row, col, &b.to_string(), fmt)?,
        // Error / Empty / not-computed: fall back to the captured value text (the
        // descriptor's "copy of the workbook" content) so a non-output cell still
        // renders its original literal.
        _ => {
            if let Some(v) = &cell.value {
                write_string_cell(ws, row, col, v, fmt)?;
            }
        },
    }
    Ok(())
}

/// Format a finite f64 for a fallback text cell deterministically. Full-precision
/// numbers go through Rust's shortest-round-trip `{}`; this is only used for the
/// merged-top-left TEXT path (numbers in normal cells are written as numbers).
fn format_number(n: f64) -> String {
    // {} on f64 is the shortest round-trip representation — deterministic.
    format!("{n}")
}

/// Write a numeric cell. Default lean (D-05): a cell that HAS a formula and a
/// finite numeric result is written as a formula-with-cached-result
/// (`Formula::set_result`); otherwise a plain number. Format applied when present.
fn write_number_cell(
    ws: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    cell: &CellLayout,
    n: f64,
    fmt: Option<&Format>,
) -> Result<(), RenderError> {
    match (&cell.formula, fmt) {
        (Some(f), Some(fmt)) => {
            let formula =
                Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
            ws.write_formula_with_format(row, col, formula, fmt)
                .map_err(writer_err)?;
        },
        (Some(f), None) => {
            let formula =
                Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
            ws.write_formula(row, col, formula).map_err(writer_err)?;
        },
        (None, Some(fmt)) => {
            ws.write_number_with_format(row, col, n, fmt)
                .map_err(writer_err)?;
        },
        (None, None) => {
            ws.write_number(row, col, n).map_err(writer_err)?;
        },
    }
    Ok(())
}

/// Write a string cell (format applied when present).
fn write_string_cell(
    ws: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    s: &str,
    fmt: Option<&Format>,
) -> Result<(), RenderError> {
    match fmt {
        Some(fmt) => ws
            .write_string_with_format(row, col, s, fmt)
            .map_err(writer_err)?,
        None => ws.write_string(row, col, s).map_err(writer_err)?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::excel_error::ExcelError;
    use std::collections::HashMap;

    /// The ZIP local-file-header magic an `.xlsx` (a ZIP container) leads with.
    const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

    fn run_with(pairs: &[(&str, CellValue)]) -> RunResult {
        let mut computed = HashMap::new();
        for (k, v) in pairs {
            computed.insert((*k).to_string(), v.clone());
        }
        RunResult {
            computed,
            traces: HashMap::new(),
        }
    }

    fn cell(addr: &str, formula: Option<&str>, value: Option<&str>) -> CellLayout {
        CellLayout {
            addr: addr.to_string(),
            formula: formula.map(str::to_string),
            value: value.map(str::to_string),
            number_format: None,
            fill_argb: None,
            font_argb: None,
        }
    }

    /// Unzip an in-memory `.xlsx` buffer (the writer's output is a ZIP container)
    /// and return the UTF-8 XML of a named worksheet entry (e.g.
    /// `"xl/worksheets/sheet1.xml"`). DEV-ONLY: consumed by the WBVER-01/02 unit
    /// tests (Plans 02/03) to assert `<f>`/`<v>` presence/absence on SPECIFIC known
    /// cells. Current render tests only check the ZIP magic — this opens the
    /// container so callers can read worksheet XML.
    ///
    /// Total against the well-formed buffers `render_xlsx` produces; a test-local
    /// `expect` surfaces a malformed buffer / missing entry as a test failure.
    fn extract_sheet_xml(buf: &[u8], sheet_path: &str) -> String {
        use std::io::Read;
        let reader = std::io::Cursor::new(buf.to_vec());
        let mut archive = zip::ZipArchive::new(reader).expect("xlsx buffer is a valid ZIP");
        let mut entry = archive
            .by_name(sheet_path)
            .unwrap_or_else(|_| panic!("worksheet entry {sheet_path} present in the xlsx"));
        let mut xml = String::new();
        entry
            .read_to_string(&mut xml)
            .expect("worksheet entry is UTF-8 XML");
        xml
    }

    /// Return the `<c r="A1"> … </c>` element SLICE for a given A1 address within a
    /// worksheet XML string, or `None` when that cell is absent. Lets consumers
    /// assert `<f>`/`<v>` presence/absence WITHIN one known cell rather than a
    /// whole-sheet count (MEDIUM #6 — shared/inline strings false-positive a global
    /// `<f>`/`<v>` tally). Scans for `<c r="<a1>"` and returns up to the matching
    /// `</c>` (or the self-closing `/>` for an empty cell).
    fn cell_xml<'a>(sheet_xml: &'a str, a1: &str) -> Option<&'a str> {
        let needle = format!("<c r=\"{a1}\"");
        let start = sheet_xml.find(&needle)?;
        let rest = &sheet_xml[start..];
        // A cell element ends at the first "</c>" (a cell with children) OR a
        // self-closing "/>" that precedes any "</c>" / "<c " boundary (empty cell).
        let close = rest.find("</c>").map(|i| i + "</c>".len());
        let next_open = rest.find("<c ").unwrap_or(rest.len());
        let self_close = rest[..next_open.min(rest.len())].find("/>").map(|i| i + 2);
        let end = match (close, self_close) {
            (Some(c), Some(s)) => c.min(s),
            (Some(c), None) => c,
            (None, Some(s)) => s,
            (None, None) => return None,
        };
        Some(&rest[..end])
    }

    fn one_sheet(name: &str, cells: Vec<CellLayout>, merges: Vec<String>) -> LayoutDescriptor {
        LayoutDescriptor {
            descriptor_version: LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: None,
            sheets: vec![SheetLayout {
                name: name.to_string(),
                hidden: false,
                cells,
                merges,
                col_widths: vec![],
                hidden_cols: vec![],
            }],
        }
    }

    #[test]
    fn render_xlsx_produces_valid_zip_container() {
        let layout = one_sheet("7_Quote", vec![cell("C11", None, Some("0"))], vec![]);
        let run = run_with(&[("7_Quote!C11", CellValue::Number(1594.93))]);
        let bytes = render_xlsx(&layout, &run).expect("render");
        assert!(!bytes.is_empty(), "non-empty output");
        assert_eq!(
            &bytes[..4],
            ZIP_MAGIC,
            "leads with the ZIP magic PK\\x03\\x04"
        );
    }

    #[test]
    fn render_xlsx_is_deterministic_byte_identical() {
        // review item 8 / T-12-15: two renders of the SAME (layout, run) are
        // byte-identical (creation datetime + metadata suppressed).
        let layout = one_sheet(
            "7_Quote",
            vec![cell("C11", Some("SUM(C9:C10)"), Some("0"))],
            vec![],
        );
        let run = run_with(&[("7_Quote!C11", CellValue::Number(1594.93))]);
        let a = render_xlsx(&layout, &run).expect("render a");
        let b = render_xlsx(&layout, &run).expect("render b");
        assert_eq!(a, b, "two renders of the same input are byte-identical");
    }

    #[test]
    fn normalize_formula_for_writer_never_double_prefixes() {
        // review item 4: a bare formula gains one '='; an already-prefixed one is
        // unchanged (never '==').
        assert_eq!(normalize_formula_for_writer("SUM(A1:A2)"), "=SUM(A1:A2)");
        assert_eq!(normalize_formula_for_writer("=SUM(A1:A2)"), "=SUM(A1:A2)");
        // Both forms round-trip to a single leading '='.
        for f in ["SUM(A1:A2)", "=SUM(A1:A2)"] {
            let out = normalize_formula_for_writer(f);
            assert!(out.starts_with('='), "has a leading '='");
            assert!(!out.starts_with("=="), "never double-prefixed");
        }
    }

    #[test]
    fn render_xlsx_rejects_non_finite_computed_value() {
        // WR-06 / T-12-05: a NaN/Inf computed value is a RenderError, never a cell.
        let layout = one_sheet("7_Quote", vec![cell("C11", None, None)], vec![]);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let run = run_with(&[("7_Quote!C11", CellValue::Number(bad))]);
            let err = render_xlsx(&layout, &run).expect_err("non-finite must be Err");
            assert!(
                matches!(err, RenderError::NonFiniteValue { .. }),
                "got {err:?}"
            );
        }
    }

    #[test]
    fn render_xlsx_surfaces_malformed_addr_as_error_not_panic() {
        // A malformed descriptor addr is a RenderError (the value path is panic-free).
        let layout = one_sheet("7_Quote", vec![cell("1A", None, Some("x"))], vec![]);
        let run = run_with(&[]);
        let err = render_xlsx(&layout, &run).expect_err("malformed addr must be Err");
        assert!(
            matches!(err, RenderError::MalformedAddr { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn render_xlsx_writes_formula_with_finite_cached_result() {
        // A formula cell + a finite result writes the (normalized, single '=')
        // formula with its cached result; render succeeds and bytes are produced.
        let layout = one_sheet(
            "7_Quote",
            vec![cell("C11", Some("=SUM(C9:C10)"), None)],
            vec![],
        );
        let run = run_with(&[("7_Quote!C11", CellValue::Number(1594.93))]);
        let bytes = render_xlsx(&layout, &run).expect("render");
        assert_eq!(&bytes[..4], ZIP_MAGIC);
    }

    #[test]
    fn render_xlsx_replays_merge_top_left_only() {
        // review item 8: a merge A1:B2 with a value at the top-left A1 produces a
        // valid xlsx. The interior cells (A2/B1/B2) being ALSO present in the
        // descriptor must NOT cause a double-write error — they are skipped.
        let layout = one_sheet(
            "7_Quote",
            vec![
                cell("A1", None, Some("merged")),
                cell("A2", None, Some("interior")),
                cell("B1", None, Some("interior")),
                cell("B2", None, Some("interior")),
            ],
            vec!["A1:B2".to_string()],
        );
        let run = run_with(&[("7_Quote!A1", CellValue::Text("merged".to_string()))]);
        let bytes = render_xlsx(&layout, &run).expect("render with merge");
        assert_eq!(
            &bytes[..4],
            ZIP_MAGIC,
            "merge replay still yields a valid xlsx"
        );
    }

    #[test]
    fn render_xlsx_rejects_single_cell_merge() {
        // A degenerate 1x1 merge is malformed input (Excel rejects single-cell
        // merges) — surfaced as MalformedMerge, never a panic.
        let layout = one_sheet(
            "7_Quote",
            vec![cell("A1", None, Some("x"))],
            vec!["A1:A1".to_string()],
        );
        let run = run_with(&[]);
        let err = render_xlsx(&layout, &run).expect_err("single-cell merge must be Err");
        assert!(
            matches!(err, RenderError::MalformedMerge { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn render_xlsx_writes_text_and_bool_and_falls_back_on_error_value() {
        // Text/Bool computed values write; an Error value falls back to the
        // captured descriptor text (no panic, no NaN).
        let layout = one_sheet(
            "7_Quote",
            vec![
                cell("A1", None, None),
                cell("A2", None, None),
                cell("A3", None, Some("orig")),
            ],
            vec![],
        );
        let run = run_with(&[
            ("7_Quote!A1", CellValue::Text("hi".to_string())),
            ("7_Quote!A2", CellValue::Bool(true)),
            ("7_Quote!A3", CellValue::Error(ExcelError::DivZero)),
        ]);
        let bytes = render_xlsx(&layout, &run).expect("render");
        assert_eq!(&bytes[..4], ZIP_MAGIC);
    }

    #[test]
    fn extract_sheet_xml_locates_formula_and_value_on_a_specific_cell() {
        // WBVER-01/02 groundwork: the Filled render of a SPECIFIC numeric formula
        // cell carries BOTH an <f> (the formula) and a <v> (the cached result)
        // WITHIN that cell's <c> element. The self-test scopes the assertion to the
        // cell BY A1 ADDRESS via `cell_xml` (NOT a brittle whole-sheet <f>/<v> count
        // — shared/inline strings false-positive a global count, MEDIUM #6).
        let layout = one_sheet(
            "7_Quote",
            vec![cell("C11", Some("SUM(C9:C10)"), Some("0"))],
            vec![],
        );
        let run = run_with(&[("7_Quote!C11", CellValue::Number(1594.93))]);
        let bytes = render_xlsx(&layout, &run).expect("render");

        let sheet_xml = extract_sheet_xml(&bytes, "xl/worksheets/sheet1.xml");
        let c11 = cell_xml(&sheet_xml, "C11").expect("the C11 cell element is present");
        assert!(
            c11.contains("<f>") || c11.contains("<f "),
            "the numeric formula cell carries an <f> element within its own <c>"
        );
        assert!(
            c11.contains("<v>"),
            "the numeric formula cell carries a cached <v> within its own <c>"
        );

        // A non-existent address yields None (total against well-formed buffers).
        assert!(
            cell_xml(&sheet_xml, "Z99").is_none(),
            "an absent cell address resolves to None, never a panic"
        );
    }

    #[test]
    fn render_xlsx_text_and_bool_formula_cells_carry_f_and_v_per_cell() {
        // WBVER-01: a TEXT formula output (cell.formula = Some) and a BOOL formula
        // output must each render as a formula-with-cached-result — their OWN <c>
        // element carries BOTH an <f> (the formula) AND a <v> (the cached result),
        // exactly like the proven numeric formula cell. Scoped per cell BY A1 ADDRESS
        // via cell_xml (NOT a whole-sheet count — shared/inline strings false-positive
        // a global tally, MEDIUM #6).
        let layout = one_sheet(
            "3_Outputs",
            vec![
                // bracket_label: a text formula output (Plan-01 fixture B6).
                cell(
                    "B6",
                    Some("IF(taxable_income>=40000,\"bracket_2\",\"bracket_1\")"),
                    None,
                ),
                // is_taxable: a bool formula output (Plan-01 fixture B7).
                cell("B7", Some("taxable_income>0"), None),
            ],
            vec![],
        );
        let run = run_with(&[
            (
                "3_Outputs!B6",
                CellValue::Text("bracket_2".to_string()),
            ),
            ("3_Outputs!B7", CellValue::Bool(true)),
        ]);
        let bytes = render_xlsx(&layout, &run).expect("render");

        let sheet_xml = extract_sheet_xml(&bytes, "xl/worksheets/sheet1.xml");

        let b6 = cell_xml(&sheet_xml, "B6").expect("the B6 text-formula cell is present");
        assert!(
            b6.contains("<f>") || b6.contains("<f "),
            "the TEXT formula cell carries an <f> element within its own <c>: {b6}"
        );
        assert!(
            b6.contains("<v>"),
            "the TEXT formula cell carries a cached <v> within its own <c>: {b6}"
        );

        let b7 = cell_xml(&sheet_xml, "B7").expect("the B7 bool-formula cell is present");
        assert!(
            b7.contains("<f>") || b7.contains("<f "),
            "the BOOL formula cell carries an <f> element within its own <c>: {b7}"
        );
        assert!(
            b7.contains("<v>"),
            "the BOOL formula cell carries a cached <v> within its own <c>: {b7}"
        );
    }

    #[test]
    fn render_xlsx_non_formula_text_and_bool_remain_plain_literals() {
        // No-regression: a text/bool cell with cell.formula = None still renders as a
        // plain value (NO <f>) — unchanged behavior.
        let layout = one_sheet(
            "3_Outputs",
            vec![cell("A1", None, None), cell("A2", None, None)],
            vec![],
        );
        let run = run_with(&[
            ("3_Outputs!A1", CellValue::Text("plain".to_string())),
            ("3_Outputs!A2", CellValue::Bool(false)),
        ]);
        let bytes = render_xlsx(&layout, &run).expect("render");
        let sheet_xml = extract_sheet_xml(&bytes, "xl/worksheets/sheet1.xml");

        let a1 = cell_xml(&sheet_xml, "A1").expect("A1 plain text cell present");
        assert!(
            !a1.contains("<f>") && !a1.contains("<f "),
            "a non-formula text cell carries NO <f>: {a1}"
        );
        let a2 = cell_xml(&sheet_xml, "A2").expect("A2 plain bool cell present");
        assert!(
            !a2.contains("<f>") && !a2.contains("<f "),
            "a non-formula bool cell carries NO <f>: {a2}"
        );
    }

    #[test]
    fn argb_to_color_non_ascii_eight_byte_input_is_none_not_a_panic() {
        // CR-01 regression: "€abcde" is 8 BYTES (3 + 5) but byte index 2 falls
        // inside the multibyte '€' — the old `&hex[2..]` slice panicked. The
        // fix returns None (unparseable ARGB is silently skipped, per the
        // documented contract).
        assert_eq!("€abcde".len(), 8, "the reproducer is byte-length 8");
        assert_eq!(argb_to_color("€abcde"), None);
        // Valid forms still parse.
        assert!(argb_to_color("FFE2EFDA").is_some());
        assert!(argb_to_color("E2EFDA").is_some());
    }

    #[test]
    fn render_xlsx_with_non_ascii_argb_renders_without_panic() {
        // CR-01 end-to-end: a corrupt/attacker-influenced bundle ARGB reaching
        // cell_format via CellLayout must render (colour skipped), never panic.
        let mut bad = cell("A1", None, Some("x"));
        bad.fill_argb = Some("€abcde".to_string());
        bad.font_argb = Some("€abcde".to_string());
        let layout = one_sheet("7_Quote", vec![bad], vec![]);
        let bytes = render_xlsx(&layout, &run_with(&[])).expect("render");
        assert_eq!(&bytes[..4], ZIP_MAGIC);
    }
}
