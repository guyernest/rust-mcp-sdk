//! WBV2-01 shipped-`template.xlsx` provenance + integrity test.
//!
//! Proves the three properties that make the §7 template a trustworthy anchor for
//! every downstream Phase-100 plan (harvest / multi-tool / `workbook explain`):
//!
//! 1. **RAW `ExcelTrusted` (review finding #5).** The committed template
//!    classifies [`ProvenanceClass::ExcelTrusted`] by the RAW
//!    `classify_xlsx_bytes` entry point — the override-free classification the
//!    production gate performs internally. There is NO
//!    `template.provenance-override.json` sidecar: the template is genuinely
//!    `rust_xlsxwriter`-authored, so its raw `calcPr`/`app.xml` identity is
//!    Excel-trusted by construction (§6 orthogonal provenance). Asserting the RAW
//!    class (not an override-softened one) proves the AUTHORING path, so a
//!    umya-authored regression OR an override-masked classification fails here.
//!
//! 2. **Byte-identical canonical + copy (review finding #8).** The canonical CLI
//!    template (`cargo-pmcp/src/templates/workbook_bundle/template.xlsx`) and the
//!    compiler test-fixtures copy (`tests/fixtures/template.xlsx`) are byte-equal,
//!    so a drift between the two committed copies fails CI.
//!
//! 3. **The §7 declaration tables are present.** The template carries Excel Tables
//!    named `Inputs`, `Calculate_Tax`, and `Estimate_Refund`, with the tier and a
//!    sample enum `list` data-validation present.
//!
//! Regenerate the template with:
//! `PMCP_REGEN_FIXTURES=1 cargo test -p pmcp-workbook-compiler regenerate_template -- --ignored`.

use std::path::{Path, PathBuf};

use pmcp_workbook_compiler::provenance::{classify_xlsx_bytes, ProvenanceClass};
use umya_spreadsheet::structs::EnumTrait;

/// The canonical CLI-templates copy of the shipped template.
fn cli_template_path() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/pmcp-workbook-compiler; the workspace root is two
    // levels up.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root two levels above the compiler crate")
        .join("cargo-pmcp/src/templates/workbook_bundle/template.xlsx")
}

/// The compiler test-fixtures copy of the shipped template.
fn fixture_template_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template.xlsx")
}

/// The template classifies RAW `ExcelTrusted` (no override sidecar consulted) —
/// the authoring path is genuinely Excel-trusted (review finding #5).
#[test]
fn template_classifies_raw_excel_trusted() {
    let bytes = std::fs::read(cli_template_path()).expect("read the committed canonical template");
    let class = classify_xlsx_bytes(&bytes).expect("the template's raw provenance is readable");
    assert_eq!(
        class,
        ProvenanceClass::ExcelTrusted,
        "the shipped template.xlsx must classify RAW ExcelTrusted by its genuine \
         rust_xlsxwriter calcPr/app.xml identity — with NO provenance-override sidecar"
    );
}

/// NO `template.provenance-override.json` sidecar exists beside the canonical
/// template (review finding #5: the template is genuinely trusted; an override
/// would be unnecessary AND would stop the test from proving the authoring path).
#[test]
fn template_has_no_provenance_override_sidecar() {
    let override_path = cli_template_path().with_extension("provenance-override.json");
    assert!(
        !override_path.exists(),
        "the shipped template must NOT carry a provenance-override sidecar — it is RAW \
         ExcelTrusted by construction (found {})",
        override_path.display()
    );
}

/// The two committed copies (CLI templates dir + compiler test fixtures dir) are
/// BYTE-IDENTICAL — a drift between them fails CI (review finding #8).
#[test]
fn template_copies_are_byte_identical() {
    let canonical = std::fs::read(cli_template_path()).expect("read canonical CLI template");
    let fixture = std::fs::read(fixture_template_path()).expect("read fixtures copy");
    assert_eq!(
        canonical, fixture,
        "the CLI-templates template.xlsx and the test-fixtures template.xlsx must be \
         byte-identical (regenerate via PMCP_REGEN_FIXTURES=1 ... regenerate_template)"
    );
}

/// The template carries the §7 declaration Tables (`Inputs`, `Calculate_Tax`,
/// `Estimate_Refund`) — read back via umya `tables()`.
#[test]
fn template_carries_named_declaration_tables() {
    let book = umya_spreadsheet::reader::xlsx::read(cli_template_path())
        .expect("re-read the committed template");
    let ws = book.sheet_by_name("Data").expect("the Data sheet exists");
    let names: Vec<&str> = ws
        .tables()
        .iter()
        .map(umya_spreadsheet::Table::name)
        .collect();
    for expected in ["Inputs", "Calculate_Tax", "Estimate_Refund"] {
        assert!(
            names.contains(&expected),
            "the template must carry an Excel Table named {expected:?} (got {names:?})"
        );
    }
}

/// The template carries the tier `{variable,strict}` dropdown AND a sample enum
/// dropdown — both `list` data-validations (dogfooding the enum-from-dropdown
/// mechanism per §3.3).
#[test]
fn template_carries_list_data_validations() {
    let book = umya_spreadsheet::reader::xlsx::read(cli_template_path())
        .expect("re-read the committed template");
    let ws = book.sheet_by_name("Data").expect("the Data sheet exists");
    let dvs = ws
        .data_validations()
        .expect("the template's Data sheet carries data validations");
    let list_count = dvs
        .data_validation_list()
        .iter()
        .filter(|dv| dv.get_type().value_string() == "list")
        .count();
    assert!(
        list_count >= 2,
        "the template must carry at least two `list` dropdowns (the tier governance \
         dropdown + a sample enum dropdown); found {list_count}"
    );
}
