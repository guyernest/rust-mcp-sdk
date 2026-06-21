//! WBV2-02 END-TO-END integration — ingest the REAL shipped `template.xlsx` and
//! verify the row-level harvested fields + the table/caption→tool linkage.
//!
//! This is the deep integration test (review finding #7): it drives the ACTUAL
//! committed artifact through the real `ingest::ingest` → §3.3 harvest path — NO
//! hand-built `TableRecord`s, NO mocked rows. It catches real workbook/table
//! coordinate bugs the thin-harness property test (`harvest_roundtrip_prop`)
//! cannot.
//!
//! Together the ALWAYS set for WBV2-02 is: unit (synth.rs) + fuzz
//! (`workbook_table_ingest`) + property over arbitrary rows
//! (`harvest_roundtrip_prop`) + THIS integration over the real file.
//!
//! The §7 template authoring under test (single sheet `Data`):
//! - Table `Inputs` (A3:D7): income=100000 `$#,##0`, filing=single (list DV
//!   single,married), withheld=15000 `$#,##0`, rate=0.22 `0.0%` strict.
//! - Caption A9 "Compute federal tax from income & filing" above Table
//!   `Calculate_Tax` (A10:C12): tax_owed=18241, effective_rate=0.182.
//! - Caption A15 "Estimate refund given withholding" above Table `Estimate_Refund`
//!   (A16:C17): refund=-3241.

use std::path::{Path, PathBuf};

use pmcp_workbook_compiler::ingest::{self, CellRecord, SheetRecord, TableRecord};
use pmcp_workbook_compiler::manifest::synth::{
    harvest_allowed_values, harvest_input_row, harvest_output_row, HarvestRow,
};
use pmcp_workbook_compiler::{Dtype, InputTier, Role};

/// The committed compiler test-fixtures copy of the shipped template (Plan 01).
fn template_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template.xlsx")
}

/// Find a cell on a sheet by its A1 address.
fn cell<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a CellRecord> {
    sheet.cells.iter().find(|c| c.addr == addr)
}

/// The trimmed value text of a cell, if present.
fn value_of<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a str> {
    cell(sheet, addr).and_then(|c| c.value.as_deref())
}

/// The number-format code of a cell, if present.
fn fmt_of<'a>(sheet: &'a SheetRecord, addr: &str) -> Option<&'a str> {
    cell(sheet, addr).and_then(|c| c.number_format.as_deref())
}

/// Build a `HarvestRow` from a sheet's cells given the row's column addresses.
/// `tier` is `None` for output rows.
fn row<'a>(
    sheet: &'a SheetRecord,
    name_addr: &str,
    value_addr: &str,
    desc_addr: &str,
    tier_addr: Option<&str>,
) -> (String, HarvestRow<'a>) {
    let key = value_of(sheet, name_addr).expect("a name cell");
    let r = HarvestRow {
        key,
        value: value_of(sheet, value_addr),
        number_format: fmt_of(sheet, value_addr),
        description: value_of(sheet, desc_addr),
        tier: tier_addr.and_then(|a| value_of(sheet, a)),
    };
    (key.to_string(), r)
}

/// The harvested Excel Table on a sheet by ListObject name.
fn table<'a>(sheet: &'a SheetRecord, name: &str) -> &'a TableRecord {
    sheet
        .table_records
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("the {name} Table is harvested"))
}

#[test]
fn real_template_harvests_input_rows_with_correct_row_level_fields() {
    let (map, _findings) =
        ingest::ingest(&template_fixture_path()).expect("ingest the real template.xlsx");
    let data = map
        .sheets
        .iter()
        .find(|s| s.name == "Data")
        .expect("the Data sheet");

    // income (B4): Number + USD unit + its description.
    let (_, income) = row(data, "A4", "B4", "C4", Some("D4"));
    let income_role = harvest_input_row("Data!B4".to_string(), &income);
    assert_eq!(income_role.dtype, Dtype::Number);
    assert_eq!(income_role.unit.as_deref(), Some("USD"));
    assert_eq!(income_role.name.as_deref(), Some("income"));
    assert_eq!(income_role.meaning.as_deref(), Some("annual gross (USD $)"));
    assert!(matches!(income_role.tier, Some(InputTier::Variable { .. })));

    // filing (B5): the list DV freezes to the [single, married] enum.
    let (_, filing) = row(data, "A5", "B5", "C5", Some("D5"));
    let filing_role = harvest_input_row("Data!B5".to_string(), &filing);
    assert_eq!(filing_role.dtype, Dtype::Text, "filing is a text value");
    let filing_enum = harvest_allowed_values(data, "B5", filing_role.dtype, &map);
    assert_eq!(
        filing_enum,
        Some(vec!["single".to_string(), "married".to_string()]),
        "the filing value cell's list DV freezes to a closed enum"
    );
    assert_eq!(filing_role.meaning.as_deref(), Some("filing status"));

    // withheld (B6): Number + USD + its description.
    let (_, withheld) = row(data, "A6", "B6", "C6", Some("D6"));
    let withheld_role = harvest_input_row("Data!B6".to_string(), &withheld);
    assert_eq!(withheld_role.dtype, Dtype::Number);
    assert_eq!(withheld_role.unit.as_deref(), Some("USD"));
    assert_eq!(
        withheld_role.meaning.as_deref(),
        Some("tax withheld YTD (USD)")
    );

    // rate (B7): percent unit "rate" + tier=strict → a Role::Constant (NOT a
    // caller-exposed input).
    let (_, rate) = row(data, "A7", "B7", "C7", Some("D7"));
    let rate_role = harvest_input_row("Data!B7".to_string(), &rate);
    assert_eq!(rate_role.unit.as_deref(), Some("rate"));
    assert_eq!(
        rate_role.role,
        Role::Constant,
        "a strict tier harvests a constant, not a caller-exposed input"
    );
    assert_eq!(
        rate_role.tier, None,
        "a strict input is an untiered constant"
    );

    // Every input row carries its description column text (no blank meaning).
    for desc_addr in ["C4", "C5", "C6", "C7"] {
        assert!(
            value_of(data, desc_addr).is_some_and(|d| !d.trim().is_empty()),
            "input row description at {desc_addr} is present"
        );
    }
}

#[test]
fn real_template_harvests_output_tables_with_caption_to_tool_description_linkage() {
    let (map, _findings) =
        ingest::ingest(&template_fixture_path()).expect("ingest the real template.xlsx");
    let data = map
        .sheets
        .iter()
        .find(|s| s.name == "Data")
        .expect("the Data sheet");

    // The two output Tables are harvested by name from the REAL file (not built).
    let calc = table(data, "Calculate_Tax");
    let refund = table(data, "Estimate_Refund");
    assert!(calc.columns.starts_with(&[
        "name".to_string(),
        "value".to_string(),
        "description".to_string()
    ]));
    assert_eq!(refund.name, "Estimate_Refund");

    // The caption cell DIRECTLY ABOVE each output Table is the tool description.
    // Calculate_Tax starts at the header row 10 (area A10:C12) → caption at A9;
    // Estimate_Refund header row 16 → caption at A15. The caption→tool-description
    // linkage is established from the REAL file by reading the cell above the table.
    let calc_caption = caption_above(data, calc).expect("Calculate_Tax has a caption");
    assert_eq!(calc_caption, "Compute federal tax from income & filing");
    let refund_caption = caption_above(data, refund).expect("Estimate_Refund has a caption");
    assert_eq!(refund_caption, "Estimate refund given withholding");

    // Each harvested output row carries its dtype + value ORACLE from the authored
    // cached <v>. Calculate_Tax body (rows 11-12), Estimate_Refund body (row 17).
    let tax_owed = row(data, "A11", "B11", "C11", None).1;
    let tax_role = harvest_output_row("Data!B11".to_string(), &tax_owed);
    assert_eq!(tax_role.role, Role::Output);
    assert_eq!(tax_role.name.as_deref(), Some("tax_owed"));
    assert_eq!(tax_role.dtype, Dtype::Number);
    assert_eq!(
        value_of(data, "B11"),
        Some("18241"),
        "the tax_owed oracle is the authored cached value"
    );

    let eff = row(data, "A12", "B12", "C12", None).1;
    let eff_role = harvest_output_row("Data!B12".to_string(), &eff);
    assert_eq!(eff_role.name.as_deref(), Some("effective_rate"));
    assert_eq!(eff_role.dtype, Dtype::Number);
    assert_eq!(value_of(data, "B12"), Some("0.182"));

    let refund_row = row(data, "A17", "B17", "C17", None).1;
    let refund_role = harvest_output_row("Data!B17".to_string(), &refund_row);
    assert_eq!(refund_role.name.as_deref(), Some("refund"));
    assert_eq!(value_of(data, "B17"), Some("-3241"));
}

/// The caption cell directly above an output Table (§4: caption = tool description)
/// — the cell in the table's first column, one row above its header row.
fn caption_above<'a>(sheet: &'a SheetRecord, t: &TableRecord) -> Option<&'a str> {
    // area.start is the header top-left (e.g. "A10"); the caption is the cell one
    // row above in the same column ("A9").
    let (col, header_row) = split_a1(&t.area.start)?;
    if header_row <= 1 {
        return None;
    }
    let caption_addr = format!("{col}{}", header_row - 1);
    value_of(sheet, &caption_addr)
}

/// Split an A1 address into its column-letter prefix + 1-based row number.
fn split_a1(addr: &str) -> Option<(String, u32)> {
    let split = addr.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = addr.split_at(split);
    Some((col.to_string(), row.parse().ok()?))
}
