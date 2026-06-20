//! The PURE tool-surface projection + render behind `cargo pmcp workbook explain`
//! (WBV2-06, §8). Dependency-light (compiler `ingest`/`synth` + `serde`/`anyhow`,
//! NO `clap`/`GlobalFlags`) so it mounts into the lib target via `#[path]` — the
//! `workbook_explain` example and the `workbook_explain` integration test reach
//! [`explain_workbook`]/[`format_tool_surface`] through that seam, NOT the bin-only
//! `commands::*` tree (mirrors the `templates_workbook_server` convention).
//!
//! ## What it previews (the served multi-tool surface, Plan 04)
//!
//! The served binary fans a compiled bundle out into ONE MCP tool per output Excel
//! Table — each with a DAG-derived `inputSchema` (only the inputs that flow into that
//! Table's outputs) and a non-empty `outputSchema`. This module reconstructs the SAME
//! surface from a raw `.xlsx` by harvesting the Inputs Table (the shared input pool)
//! plus each output Table (one tool), then deriving each tool's minimal inputs by
//! walking its output cells' formula references back to the input pool — so the preview
//! matches what an LLM will see (e.g. `calculate_tax` advertises `income` while
//! `estimate_refund` additionally advertises `withheld`).
//!
//! It writes NO bundle and runs no compile gate — a pure read-only projection.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use pmcp_workbook_compiler::ingest::{self, CellRecord, SheetRecord, TableRecord, WorkbookMap};
use pmcp_workbook_compiler::manifest::synth::{harvest_allowed_values, HarvestRow};
use pmcp_workbook_compiler::{Dtype, Role};

/// One input parameter on a previewed tool's `inputSchema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputParam {
    /// The LLM-facing JSON key (the Inputs-Table `name` column).
    pub key: String,
    /// The JSON-schema type (`"number"` | `"string"` | `"boolean"`).
    pub ty: String,
    /// The declared unit (`USD`/`rate`/…), when authored.
    pub unit: Option<String>,
    /// The closed enum domain (from the value cell's list data-validation), when any.
    pub enum_values: Option<Vec<String>>,
}

/// One output field on a previewed tool's `outputSchema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputField {
    /// The LLM-facing JSON key (the output-Table `name` column).
    pub key: String,
    /// The JSON-schema type (`"number"` | `"string"` | `"boolean"`).
    pub ty: String,
    /// The declared unit, when authored.
    pub unit: Option<String>,
}

/// One previewed tool — the served projection of ONE output Excel Table (Plan 04):
/// its sanitized MCP name, its caption description, and its per-tool input/output
/// schema (the inputs DAG-derived from the Table's output-cell formula references).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSurface {
    /// The MCP tool name (the output-Table name, sanitized to the MCP charset).
    pub name: String,
    /// The tool description (the caption cell above the Table), when authored.
    pub description: Option<String>,
    /// The minimal per-tool input parameters (DAG-derived), sorted by key.
    pub inputs: Vec<InputParam>,
    /// The output fields this tool projects, in authored order.
    pub outputs: Vec<OutputField>,
}

/// Ingest the workbook read-only and project its served tool surface.
///
/// # Errors
/// Returns an error if ingest fails or the workbook declares no output Table.
pub fn explain_workbook(path: &Path) -> Result<Vec<ToolSurface>> {
    let (map, _findings) = ingest::ingest(path)
        .with_context(|| format!("failed to ingest workbook {}", path.display()))?;
    let tools = project_tool_surface(&map);
    if tools.is_empty() {
        anyhow::bail!(
            "{} declares no output Table — a served workbook must have at least one \
             output Table to expose a tool",
            path.display()
        );
    }
    Ok(tools)
}

/// Project the served multi-tool surface from an ingested workbook (read-only):
/// harvest the shared input pool, then one [`ToolSurface`] per output Table with its
/// DAG-derived minimal inputs. Returns the tools in authored order (Inputs Table
/// excluded — it is the shared pool, not a tool).
#[must_use]
pub fn project_tool_surface(map: &WorkbookMap) -> Vec<ToolSurface> {
    let mut tools = Vec::new();
    for sheet in &map.sheets {
        let pool = harvest_input_pool(sheet, map);
        let output_tables: Vec<&TableRecord> = sheet
            .table_records
            .iter()
            .filter(|t| !is_input_table(t))
            .collect();

        // A variable input reached by NO output Table's formulas is a workbook-wide
        // governed input (the BA authored it as a caller parameter, but no formula
        // consumes its cell — e.g. a `filing`-status enum that gates a lookup table).
        // Surface it on EVERY tool so the preview never silently drops an authored
        // input; inputs that ARE formula-reached stay per-tool (the disjoint surface).
        let fed = inputs_fed_by_any_table(sheet, &output_tables, &pool);

        for table in &output_tables {
            tools.push(tool_for_table(sheet, table, &pool, &fed));
        }
    }
    tools
}

/// The set of exposed-input value-cell addresses that ARE formula-reached by at
/// least one output Table (so the complement — exposed inputs reached by none — is
/// surfaced workbook-wide).
fn inputs_fed_by_any_table(
    sheet: &SheetRecord,
    output_tables: &[&TableRecord],
    pool: &[PoolEntry],
) -> BTreeSet<String> {
    let mut fed = BTreeSet::new();
    for table in output_tables {
        let cols = column_addrs(table);
        let output_addrs: Vec<String> = body_rows(table)
            .filter(|r| cell_text(sheet, &col_at(&cols, "name", *r)).is_some())
            .map(|r| col_at(&cols, "value", r))
            .collect();
        let reached = reachable_addrs(sheet, &output_addrs);
        for e in pool.iter().filter(|e| e.exposed) {
            if reached.contains(&e.value_addr) {
                fed.insert(e.value_addr.clone());
            }
        }
    }
    fed
}

/// One harvested input cell in the shared pool: its served key + type/unit/enum and
/// whether it is caller-exposed (a `variable` tier) or a governed `strict` constant.
struct PoolEntry {
    /// The value-cell A1 address on the owning sheet (the formula-ref target).
    value_addr: String,
    /// The served input parameter (key/type/unit/enum).
    param: InputParam,
    /// `true` for a caller-exposed `variable` input; `false` for a `strict` constant.
    exposed: bool,
}

/// Harvest the Inputs Table's rows into the shared input pool (§3.3): each row's
/// `name`/`value`/`tier` columns project to a typed [`PoolEntry`]. A `strict`-tier
/// row is a governed constant (NOT caller-exposed); a `variable`-tier row is an
/// exposed input carrying its unit + (when the value cell has a list DV) its enum.
fn harvest_input_pool(sheet: &SheetRecord, map: &WorkbookMap) -> Vec<PoolEntry> {
    let Some(inputs) = sheet.table_records.iter().find(|t| is_input_table(t)) else {
        return Vec::new();
    };
    let cols = column_addrs(inputs);
    let mut pool = Vec::new();
    for r in body_rows(inputs) {
        let Some(key) = cell_text(sheet, &col_at(&cols, "name", r)) else {
            continue;
        };
        let value_addr = col_at(&cols, "value", r);
        let row = harvest_row(sheet, &cols, &key, r);
        let role = pmcp_workbook_compiler::manifest::synth::harvest_input_row(
            format!("{}!{}", sheet.name, value_addr),
            &row,
        );
        let exposed = role.role == Role::Input;
        let enum_values = if exposed {
            harvest_allowed_values(sheet, &value_addr, role.dtype, map)
        } else {
            None
        };
        pool.push(PoolEntry {
            value_addr,
            param: InputParam {
                key,
                ty: dtype_json_type(role.dtype),
                unit: role.unit,
                enum_values,
            },
            exposed,
        });
    }
    pool
}

/// Build one [`ToolSurface`] for an output Table: its sanitized name, its caption
/// description, its output fields (harvested rows), and its DAG-derived minimal
/// inputs (the exposed pool entries whose value cell is reachable from this Table's
/// output-cell formulas).
fn tool_for_table(
    sheet: &SheetRecord,
    table: &TableRecord,
    pool: &[PoolEntry],
    fed: &BTreeSet<String>,
) -> ToolSurface {
    let cols = column_addrs(table);
    let mut outputs = Vec::new();
    let mut output_addrs = Vec::new();
    for r in body_rows(table) {
        let Some(key) = cell_text(sheet, &col_at(&cols, "name", r)) else {
            continue;
        };
        let value_addr = col_at(&cols, "value", r);
        let row = harvest_row(sheet, &cols, &key, r);
        let role = pmcp_workbook_compiler::manifest::synth::harvest_output_row(
            format!("{}!{}", sheet.name, value_addr),
            &row,
        );
        outputs.push(OutputField {
            key,
            ty: dtype_json_type(role.dtype),
            unit: role.unit,
        });
        output_addrs.push(value_addr);
    }

    let reached = reachable_addrs(sheet, &output_addrs);
    // This tool's inputs: the exposed pool entries this Table's formulas reach,
    // PLUS the workbook-wide governed inputs no Table reaches (surfaced everywhere).
    let mut inputs: Vec<InputParam> = pool
        .iter()
        .filter(|e| e.exposed && (reached.contains(&e.value_addr) || !fed.contains(&e.value_addr)))
        .map(|e| e.param.clone())
        .collect();
    inputs.sort_by(|a, b| a.key.cmp(&b.key));

    ToolSurface {
        name: sanitize(&table.name),
        description: caption_above(sheet, table),
        inputs,
        outputs,
    }
}

/// Render `tools` as a String in the requested `format` (PURE — no stdout).
///
/// `"json"` serializes the [`ToolSurface`] list directly; `"text"` renders one
/// block per tool (name / description / `inputs:` / `outputs:`) per OQ-3.
///
/// # Errors
/// Returns an error for an unknown `format` (naming the valid `text`/`json` values),
/// or if JSON serialization fails.
pub fn format_tool_surface(tools: &[ToolSurface], format: &str) -> Result<String> {
    match format {
        "json" => serde_json::to_string_pretty(tools)
            .context("failed to serialize the tool surface to JSON"),
        "text" => Ok(render_text(tools)),
        other => anyhow::bail!("unknown --format `{other}` (expected `text` or `json`)"),
    }
}

/// Render the tool surface as human text (the BA preview, OQ-3): one block per tool
/// — `tool <name>`, its `description`, an `inputs:` list (`key: type [unit] [enum]`)
/// and an `outputs:` list (`key: type [unit]`).
fn render_text(tools: &[ToolSurface]) -> String {
    if tools.is_empty() {
        return "no served tools (the workbook declares no output Table)".to_string();
    }
    let mut out = String::new();
    for tool in tools {
        out.push_str(&format!("tool {}\n", tool.name));
        out.push_str(&format!(
            "  description: {}\n",
            tool.description.as_deref().unwrap_or("(none)")
        ));
        out.push_str("  inputs:\n");
        for p in &tool.inputs {
            out.push_str(&format!("    {}\n", render_input(p)));
        }
        out.push_str("  outputs:\n");
        for o in &tool.outputs {
            out.push_str(&format!("    {}\n", render_output(o)));
        }
    }
    out
}

/// `key: type [unit] [enum: a|b]` for one input parameter.
fn render_input(p: &InputParam) -> String {
    let mut s = format!("{}: {}", p.key, p.ty);
    if let Some(unit) = &p.unit {
        s.push_str(&format!(" [{unit}]"));
    }
    if let Some(values) = &p.enum_values {
        s.push_str(&format!(" [enum: {}]", values.join("|")));
    }
    s
}

/// `key: type [unit]` for one output field.
fn render_output(o: &OutputField) -> String {
    match &o.unit {
        Some(unit) => format!("{}: {} [{unit}]", o.key, o.ty),
        None => format!("{}: {}", o.key, o.ty),
    }
}

// ---- harvest helpers (read-only ingest projection) ---------------------------

/// Whether a Table is the shared Inputs pool (it carries a `tier` column; output
/// Tables carry only `name|value|description`).
fn is_input_table(table: &TableRecord) -> bool {
    table.columns.iter().any(|c| c.eq_ignore_ascii_case("tier"))
}

/// Map a Table's column header names to their column LETTERS (from the Table area).
fn column_addrs(table: &TableRecord) -> BTreeMap<String, String> {
    let start_col = col_letters(&table.area.start);
    let mut map = BTreeMap::new();
    for (i, name) in table.columns.iter().enumerate() {
        if let Some(col) = shift_col(&start_col, i) {
            map.insert(name.to_ascii_lowercase(), col);
        }
    }
    map
}

/// The 1-based body row numbers of a Table (header row excluded).
fn body_rows(table: &TableRecord) -> std::ops::RangeInclusive<u32> {
    let header = row_num(&table.area.start);
    let last = row_num(&table.area.end);
    (header + 1)..=last
}

/// The A1 address of a Table column `col_name` at body row `row`.
fn col_at(cols: &BTreeMap<String, String>, col_name: &str, row: u32) -> String {
    let letters = cols.get(col_name).cloned().unwrap_or_default();
    format!("{letters}{row}")
}

/// Build a [`HarvestRow`] for a Table body row from the sheet cells.
fn harvest_row<'a>(
    sheet: &'a SheetRecord,
    cols: &BTreeMap<String, String>,
    key: &'a str,
    row: u32,
) -> HarvestRow<'a> {
    let value_addr = col_at(cols, "value", row);
    let desc_addr = col_at(cols, "description", row);
    let tier_addr = col_at(cols, "tier", row);
    HarvestRow {
        key,
        value: cell_text_ref(sheet, &value_addr),
        number_format: cell_fmt(sheet, &value_addr),
        description: cell_text_ref(sheet, &desc_addr),
        tier: if cols.contains_key("tier") {
            cell_text_ref(sheet, &tier_addr)
        } else {
            None
        },
    }
}

/// The caption cell directly above an output Table (§4: caption = tool description):
/// the cell in the Table's first column, one row above its header row.
fn caption_above(sheet: &SheetRecord, table: &TableRecord) -> Option<String> {
    let col = col_letters(&table.area.start);
    let header = row_num(&table.area.start);
    if header <= 1 {
        return None;
    }
    cell_text(sheet, &format!("{col}{}", header - 1))
}

/// The set of input value-cell addresses reachable from `output_addrs` by walking
/// formula references transitively (the per-tool DAG derivation §4.2): start from
/// each output cell, follow its formula's A1 references into other formula cells,
/// and collect every cell address visited.
fn reachable_addrs(sheet: &SheetRecord, output_addrs: &[String]) -> BTreeSet<String> {
    let mut reached = BTreeSet::new();
    let mut stack: Vec<String> = output_addrs.to_vec();
    while let Some(addr) = stack.pop() {
        if !reached.insert(addr.clone()) {
            continue;
        }
        if let Some(formula) = cell_formula(sheet, &addr) {
            for r in extract_a1_refs(&formula) {
                if !reached.contains(&r) {
                    stack.push(r);
                }
            }
        }
    }
    reached
}

/// Extract the bare A1 cell references (e.g. `B4`, `G3`) from a formula's text. Only
/// single-cell same-sheet references are recognized (sufficient for the table model);
/// ranges/cross-sheet refs are ignored.
fn extract_a1_refs(formula: &str) -> Vec<String> {
    let bytes = formula.as_bytes();
    let mut refs = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        // Skip a reference that is part of a longer identifier (function name etc.):
        // a valid A1 ref is letters followed immediately by digits with no trailing
        // alphabetic/`(`/`!` char.
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let letters_end = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let has_digits = i > letters_end;
        let trailing_ident = i < bytes.len()
            && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'(' || bytes[i] == b'!');
        if has_digits && !trailing_ident {
            refs.push(formula[start..i].to_string());
        }
    }
    refs
}

/// Map a [`Dtype`] to its JSON-schema type string.
fn dtype_json_type(dtype: Dtype) -> String {
    match dtype {
        Dtype::Number => "number",
        Dtype::Text => "string",
        Dtype::Bool => "boolean",
    }
    .to_string()
}

/// Sanitize an output-Table name to the MCP tool-name charset (the SAME shared
/// sanitizer the served registration + compiler collision lint call); falls back to
/// the raw name when unmappable (the preview still names the offender).
fn sanitize(raw: &str) -> String {
    pmcp_workbook_compiler::sanitize_tool_name(raw).unwrap_or_else(|_| raw.to_string())
}

// ---- thin cell accessors -----------------------------------------------------

/// The owned cell at A1 `addr` on `sheet`, if present.
fn cell<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a CellRecord> {
    sheet.cells.iter().find(|c| c.addr == addr)
}

/// The owned VALUE text of a cell, if present.
fn cell_text(sheet: &SheetRecord, addr: &str) -> Option<String> {
    cell(sheet, addr).and_then(|c| c.value.clone())
}

/// The borrowed VALUE text of a cell, if present.
fn cell_text_ref<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a str> {
    cell(sheet, addr).and_then(|c| c.value.as_deref())
}

/// The number-format code of a cell, if present.
fn cell_fmt<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a str> {
    cell(sheet, addr).and_then(|c| c.number_format.as_deref())
}

/// The formula text (no leading `=`) of a cell, if it is a formula.
fn cell_formula(sheet: &SheetRecord, addr: &str) -> Option<String> {
    cell(sheet, addr).and_then(|c| c.formula.clone())
}

// ---- A1 address arithmetic ---------------------------------------------------

/// The column-letter prefix of an A1 address (e.g. `"B"` from `"B4"`).
fn col_letters(addr: &str) -> String {
    addr.chars().take_while(char::is_ascii_alphabetic).collect()
}

/// The 1-based row number of an A1 address (e.g. `4` from `"B4"`); `0` on a malformed
/// address (no served Table uses row 0, so this is a safe sentinel).
fn row_num(addr: &str) -> u32 {
    addr.chars()
        .skip_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Shift a single-letter-or-multi-letter column by `delta` columns (A=0); `None` on
/// overflow. Supports the A..ZZ range output Tables live in.
fn shift_col(col: &str, delta: usize) -> Option<String> {
    let base = col_to_index(col)?;
    Some(index_to_col(base + delta))
}

/// Map column LETTERS to a 0-based index (`A`→0, `Z`→25, `AA`→26).
fn col_to_index(col: &str) -> Option<usize> {
    if col.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    for ch in col.chars() {
        let c = ch.to_ascii_uppercase();
        if !c.is_ascii_uppercase() {
            return None;
        }
        idx = idx * 26 + (c as usize - 'A' as usize + 1);
    }
    Some(idx - 1)
}

/// Map a 0-based column index back to LETTERS (`0`→`A`, `26`→`AA`).
fn index_to_col(mut idx: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push((b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    letters.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tools() -> Vec<ToolSurface> {
        vec![
            ToolSurface {
                name: "calculate_tax".to_string(),
                description: Some("Compute federal tax".to_string()),
                inputs: vec![InputParam {
                    key: "income".to_string(),
                    ty: "number".to_string(),
                    unit: Some("USD".to_string()),
                    enum_values: None,
                }],
                outputs: vec![OutputField {
                    key: "tax_owed".to_string(),
                    ty: "number".to_string(),
                    unit: None,
                }],
            },
            ToolSurface {
                name: "estimate_refund".to_string(),
                description: None,
                inputs: vec![
                    InputParam {
                        key: "filing".to_string(),
                        ty: "string".to_string(),
                        unit: None,
                        enum_values: Some(vec!["single".to_string(), "married".to_string()]),
                    },
                    InputParam {
                        key: "withheld".to_string(),
                        ty: "number".to_string(),
                        unit: Some("USD".to_string()),
                        enum_values: None,
                    },
                ],
                outputs: vec![OutputField {
                    key: "refund".to_string(),
                    ty: "number".to_string(),
                    unit: Some("USD".to_string()),
                }],
            },
        ]
    }

    #[test]
    fn text_render_names_each_tool_and_its_schema() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("tool calculate_tax"));
        assert!(text.contains("tool estimate_refund"));
        assert!(text.contains("description: Compute federal tax"));
        assert!(text.contains("income: number [USD]"));
        assert!(text.contains("tax_owed: number"));
        assert!(text.contains("refund: number [USD]"));
    }

    #[test]
    fn text_render_shows_enum_domain() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("filing: string [enum: single|married]"));
    }

    #[test]
    fn text_render_shows_none_description() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("description: (none)"));
    }

    #[test]
    fn json_render_round_trips_the_surface() {
        let json = format_tool_surface(&sample_tools(), "json").expect("json render");
        let back: Vec<ToolSurface> = serde_json::from_str(&json).expect("deserialize back");
        assert_eq!(back, sample_tools());
    }

    #[test]
    fn unknown_format_errors_naming_valid_formats() {
        let err = format_tool_surface(&sample_tools(), "yaml").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("text"), "got: {msg}");
        assert!(msg.contains("json"), "got: {msg}");
    }

    #[test]
    fn empty_surface_text_says_no_tools() {
        let text = format_tool_surface(&[], "text").expect("text render");
        assert!(text.contains("no served tools"));
    }

    #[test]
    fn extract_a1_refs_finds_cell_refs_not_function_names() {
        // ROUND(B4*G3-1759,0): the function name ROUND is NOT a ref (trailing `(`);
        // B4 and G3 ARE refs; 1759 and 0 are bare numbers (no letters).
        let refs = extract_a1_refs("ROUND(B4*G3-1759,0)");
        assert_eq!(refs, vec!["B4".to_string(), "G3".to_string()]);
    }

    #[test]
    fn extract_a1_refs_ignores_bare_numbers_and_ranges() {
        let refs = extract_a1_refs("B11/B4");
        assert_eq!(refs, vec!["B11".to_string(), "B4".to_string()]);
    }

    #[test]
    fn col_arithmetic_round_trips() {
        for (col, idx) in [("A", 0usize), ("B", 1), ("Z", 25), ("AA", 26), ("AB", 27)] {
            assert_eq!(col_to_index(col), Some(idx));
            assert_eq!(index_to_col(idx), col);
        }
        assert_eq!(shift_col("A", 3).as_deref(), Some("D"));
    }

    #[test]
    fn row_and_col_split_an_a1_address() {
        assert_eq!(col_letters("B4"), "B");
        assert_eq!(row_num("B4"), 4);
        assert_eq!(col_letters("AA10"), "AA");
        assert_eq!(row_num("AA10"), 10);
    }
}
