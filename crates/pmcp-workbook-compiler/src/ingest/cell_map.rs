//! The OWNED, plain workbook model the ingest pass converts `umya` reads into.
//!
//! NO `umya` type — and NO `quick-xml`/`zip` type — appears in any public
//! signature here: the ingest boundary ([`super`]) converts every `umya` read
//! into these owned `String`/`bool`/`u32`/plain-enum types so the downstream
//! linter / synthesis / round-trip stages consume owned data and NEVER reach
//! back into `umya` (RESEARCH anti-pattern "Leaking umya types"). The
//! provenance module's quarantined `quick-xml`/`zip` part reader is held to the
//! SAME quarantine: no `quick-xml`/`zip` type may cross into these owned models
//! either.
//!
//! Field rationale (why owned + structured, not bools/tuples):
//! - [`RangeRef`] is the SINGLE structured range type reused for merges, CF
//!   ranges, tables, and named-range targets — replacing every `(String,String)`
//!   tuple so the consumers key on `sheet`/`start`/`end`.
//! - [`SheetRecord::cf_ranges`] is `Vec<RangeRef>` (NOT a
//!   `has_conditional_formatting: bool`) so synthesis can intersect CF ranges
//!   with role cells; `hidden_rows` is `Vec<u32>` (NOT an opaque string) so
//!   findings stay locatable.
//! - [`CellRecord::formula_kind`] is per-cell so the linter emits
//!   `formula/array` without re-reading `umya`.
//! - [`WorkbookMap`] carries `has_macros`/`external_links`/scoped `defined_names`/
//!   `source_extension` so the linter emits `structure/macro` +
//!   `structure/external-link` and round-trip detects name collisions — all from
//!   owned fields.

use serde::Serialize;

// `RangeRef` + `cell_key` live in `pmcp-workbook-runtime` (the IR + served-binary
// executor reach them, and `ingest` links umya). They are re-exported here so
// every `crate::ingest::RangeRef` and `crate::ingest::cell_map::cell_key` path
// (incl. the owned models below) resolves unchanged.
pub use pmcp_workbook_runtime::range_ref::{cell_key, RangeRef};

/// Per-cell formula classification so the linter emits `formula/array` without
/// re-reading `umya` (read from `cell.formula_obj().formula_type()` +
/// `cell.formula_shared_index()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum FormulaKind {
    /// A normal single-cell formula (or a non-formula cell's default).
    Normal,
    /// A shared formula (`cell.formula_shared_index()` is `Some`).
    Shared,
    /// A legacy CSE array formula (`<f t="array">`) — a refuse-set violation.
    Array,
    /// A dynamic-array / spilled formula (`<f t="dataTable">` and modern spills).
    DynamicArray,
}

/// The scope a [`DefinedNameRecord`] is registered at (names live at BOTH
/// `book.defined_names()` and `ws.defined_names()` — tag each with its origin so
/// the round-trip collision check keys on name + scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum DefinedNameScope {
    /// A workbook-global defined name (`book.defined_names()`).
    Workbook,
    /// A worksheet-local defined name (`ws.defined_names()`), tagged with the
    /// owning sheet name.
    Worksheet(String),
}

/// A structured, scoped named-range record (NOT a tuple) so the synthesis
/// overlap check and the round-trip collision check key on name + target + scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct DefinedNameRecord {
    /// The defined name (`dn.name()`).
    pub name: String,
    /// The range the name targets (parsed from `dn.address()` into a [`RangeRef`]).
    pub target: RangeRef,
    /// Workbook- vs worksheet-scope.
    pub scope: DefinedNameScope,
}

/// One owned cell note/comment (legacy `<comments>` or threaded), anchored to an
/// A1 cell on the owning sheet. Threaded vs legacy is flagged so the
/// manifest/explain surface can distinguish analyst threads from legacy
/// annotations. Owned `String`/`bool` only — no `umya` type crosses (the
/// module-doc quarantine invariant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NoteRecord {
    /// A1 anchor within the owning sheet (e.g. `"C16"`).
    pub addr: String,
    /// The note author (`"Unknown Author"` when absent).
    pub author: String,
    /// The plain-text note body (rich-text runs flattened to text).
    pub text: String,
    /// `true` for an Office-2019 threaded comment, `false` for a legacy note.
    pub threaded: bool,
}

/// One owned (data validation × range) record: an Excel `<dataValidation>`
/// surfaced per TARGET range so a multi-range `sqref` (e.g. `"C6 E6"`) emits one
/// record per range — never a single collapsed record. `dv_type` is umya's
/// `value_string()` rendering (`"list"` | `"whole"` | …); `formula1` carries the
/// RAW formula text (literal quotes NOT stripped — token parsing is synth's job),
/// `None` when empty. Owned `RangeRef`/`String` only — no `umya` type crosses
/// (the module-doc quarantine invariant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct DataValidationRecord {
    /// The single target range this record covers (one per `range_collection()`
    /// entry of the DV's `sqref`).
    pub target: RangeRef,
    /// The DV type as text (`"list"` | `"whole"` | `"decimal"` | …).
    pub dv_type: String,
    /// The RAW `formula1` text (e.g. `"\"a,b\""` with literal quotes), `None`
    /// when the DV carries no/empty formula1.
    pub formula1: Option<String>,
}

/// One owned cell: its address + formula/value text + the fill/font ARGBs +
/// formula classification. Colour ARGBs are `Some` only when the cell carries a
/// meaningful (non-transparent) colour — an unset colour reads as `None` so the
/// synthesis classifies roles from real signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CellRecord {
    /// A1 address within the owning sheet (e.g. `"E6"`).
    pub addr: String,
    /// The formula text WITHOUT the leading `=` (`None` when not a formula).
    pub formula: Option<String>,
    /// The cell's computed/literal value as text (`None` when empty).
    pub value: Option<String>,
    /// The fill (background) ARGB (e.g. `"FFE2EFDA"`), `None` when unset.
    pub fill_argb: Option<String>,
    /// The font colour ARGB (e.g. `"FF0000FF"`), `None` when unset.
    pub font_argb: Option<String>,
    /// The number-format code (e.g. `"#,##0.00"`), `None` when General/unset
    /// (the layout descriptor replays this; the umya accessor is
    /// `style.number_format().map(|nf| nf.format_code())`).
    pub number_format: Option<String>,
    /// `true` iff `umya` reports this cell as a formula.
    pub is_formula: bool,
    /// The formula classification (`Normal` for non-formula cells).
    pub formula_kind: FormulaKind,
}

/// One owned sheet: its name + visibility state + the located structure
/// (hidden rows, hidden columns, merges, CF ranges, tables, notes) + every owned
/// cell.
///
/// `PartialEq` (not `Eq`): `col_widths` carries `f64`, so this follows the
/// crate's "f64-bearing types drop `Eq`" rule (the same divergence the IR's
/// `Expr::Number` and the `LayoutDescriptor` take).
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct SheetRecord {
    /// The sheet name (e.g. `"1_Inputs"`).
    pub name: String,
    /// The visibility state: `"visible"` | `"hidden"` | `"veryHidden"`.
    pub state: String,
    /// The 1-based row numbers flagged hidden (typed so findings are locatable).
    pub hidden_rows: Vec<u32>,
    /// The 1-based column numbers flagged hidden; same `Vec<u32>` typing as
    /// `hidden_rows`.
    pub hidden_cols: Vec<u32>,
    /// Per-column widths as `(1-based col index, width)` pairs (the layout
    /// descriptor replays these; umya `column_dimensions()` exposes
    /// `get_col_num()`/`get_width()`).
    pub col_widths: Vec<(u32, f64)>,
    /// Merged-cell ranges as owned [`RangeRef`]s.
    pub merges: Vec<RangeRef>,
    /// Conditional-formatting ranges as owned [`RangeRef`]s (NOT a bool) so
    /// synthesis can intersect CF with role cells.
    pub cf_ranges: Vec<RangeRef>,
    /// Excel table ranges as owned [`RangeRef`]s.
    pub tables: Vec<RangeRef>,
    /// Data validations as owned [`DataValidationRecord`]s — ONE record per
    /// (data validation × `sqref` range), so a multi-range sqref is never
    /// silently collapsed. Empty when the sheet has none.
    pub data_validations: Vec<DataValidationRecord>,
    /// Sheet-level cell notes/comments as owned [`NoteRecord`]s (sparse; sheet
    /// level mirrors `merges`/`cf_ranges`/`tables`).
    pub notes: Vec<NoteRecord>,
    /// Every owned cell on the sheet.
    pub cells: Vec<CellRecord>,
}

/// The owned, `umya`-free workbook model — the linter/synthesis INPUT.
///
/// Carries the workbook-level metadata the linter needs for `structure/macro` +
/// `structure/external-link` and the round-trip needs for name-collision
/// detection, alongside every owned sheet.
///
/// `PartialEq` (not `Eq`): it contains [`SheetRecord`], whose `col_widths`
/// carries `f64` — the crate's "f64-bearing types drop `Eq`" rule.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct WorkbookMap {
    /// Every owned sheet, in workbook order.
    pub sheets: Vec<SheetRecord>,
    /// Every defined name at BOTH scopes, structured + scoped.
    pub defined_names: Vec<DefinedNameRecord>,
    /// External-link references detected in formula text (e.g. `"[1]Sheet1"`);
    /// empty when none.
    pub external_links: Vec<String>,
    /// `true` iff the workbook carries VBA macros (`book.has_macros()`).
    pub has_macros: bool,
    /// The source file extension, lowercased without the dot (`"xlsx"`/`"xlsm"`).
    pub source_extension: String,
    /// The workbook save timestamp (`docProps/core.xml dcterms:modified`,
    /// umya-surfaced via `get_properties().modified()`). `None` when the property
    /// is absent/empty. Threaded to the provenance builder so the gate can stamp
    /// `OracleProvenance.save_timestamp` WITHOUT re-opening the workbook.
    pub save_timestamp: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_record_round_trips_through_serde_with_all_four_fields() {
        let note = NoteRecord {
            addr: "C16".to_string(),
            author: "Unknown Author".to_string(),
            text: "Price book shows 0.56".to_string(),
            threaded: false,
        };
        let v = serde_json::to_value(&note).expect("serialize NoteRecord");
        assert_eq!(v["addr"], "C16");
        assert_eq!(v["author"], "Unknown Author");
        assert_eq!(v["text"], "Price book shows 0.56");
        assert_eq!(v["threaded"], false);
    }

    #[test]
    fn sheet_record_with_notes_and_hidden_cols_satisfies_partial_eq() {
        let make = || SheetRecord {
            name: "1_Inputs".to_string(),
            state: "visible".to_string(),
            hidden_rows: vec![3, 7],
            hidden_cols: vec![2, 5],
            col_widths: vec![(1, 8.43), (3, 12.5)],
            merges: Vec::new(),
            cf_ranges: Vec::new(),
            tables: Vec::new(),
            data_validations: Vec::new(),
            notes: vec![NoteRecord {
                addr: "C16".to_string(),
                author: "Unknown Author".to_string(),
                text: "note".to_string(),
                threaded: false,
            }],
            cells: Vec::new(),
        };
        // PartialEq still holds with the new fields populated.
        assert_eq!(make(), make());
    }
}
