//! Shared authoring of the neutral `tax-calc.xlsx` producer/consumer fixture.
//!
//! Authored with `rust_xlsxwriter` — a PURE WRITER, NEVER umya (the provenance
//! gate refuses a umya-authored fixture, Pitfall 3). Every formula cell is written
//! WITH its cached `<v>` result via [`rust_xlsxwriter::Formula::set_result`] so the
//! compiler's reconcile stage has the Plan-04 oracle to grade against. Cell roles
//! are authored from the SDK dialect colours (blue font = input, green fill =
//! governed constant) and the named outputs from `out_*` defined names — so a real
//! synthesis pass classifies them with NO hand-built per-workbook manifest.
//!
//! Domain: a neutral progressive tax calculation (tax-calc). ZERO customer
//! identifiers. This file is `include!`d (not a test of its own) by both the
//! `compile_a_workbook` example and the `reemit_tax_calc_golden` proof so there is
//! ONE authoring path.

#![allow(dead_code)]

use rust_xlsxwriter::{Color, Format, Formula, Workbook};

/// The committed trusted-fixture provenance override (test/dev path ONLY). It is a
/// marker the proof reads to confirm the fixture is the designated trusted one
/// before passing the override into the compiler — it NEVER weakens the production
/// refuse path (production classifies from raw bytes and ignores this file).
pub const PROVENANCE_OVERRIDE_JSON: &str = r#"{
  "kind": "trusted-fixture",
  "fixture": "tax-calc.xlsx",
  "reason": "authored by rust_xlsxwriter (a pure writer, NOT Excel), so it carries no Excel provenance; honoured ONLY on the test/dev path so the producer/consumer proof can compile a neutral fixture. The production compile path NEVER reads this file and still refuses non-Excel provenance.",
  "authored_by": "rust_xlsxwriter",
  "scope": "test-path-only"
}
"#;

/// The blue input-font colour the SDK dialect classifies as `Role::Input`
/// (`FF0000FF` ARGB → `0x0000FF` RGB).
const INPUT_BLUE: Color = Color::RGB(0x0000FF);
/// The green governed-constant FILL the SDK dialect classifies as `Role::Constant`
/// (`FFE2EFDA` ARGB → `0xE2EFDA` RGB).
const GOVERNED_GREEN: Color = Color::RGB(0xE2EFDA);

/// Author the neutral `tax-calc.xlsx` bytes.
///
/// Layout (mirrors the committed `tax-calc@1.1.0` golden's semantics):
/// - `1_Inputs`: 3 inputs (blue font) — gross income (B2), filing status (B3, an
///   inline-DV enum), deductions (B4).
/// - `2_Brackets`: a governed bracket table (green fill) — boundaries (A2/A3) and
///   rates (B2/B3).
/// - `3_Outputs`: 4 output formulas (B2..B5) with cached results, each tagged by an
///   `out_*` defined name so the driver promotes them to `Role::Output`.
///
/// # Panics
/// Authoring uses `rust_xlsxwriter`'s in-memory writer; a write failure panics
/// (this is test/example setup code, never the library value path).
pub fn author_tax_calc_xlsx() -> Vec<u8> {
    let mut book = Workbook::new();
    let input_fmt = Format::new().set_font_color(INPUT_BLUE);
    let governed_fmt = Format::new().set_background_color(GOVERNED_GREEN);

    // ---- 1_Inputs (blue-font inputs) ---------------------------------------
    let inputs = book.add_worksheet();
    inputs.set_name("1_Inputs").expect("name 1_Inputs");
    inputs.write_string(0, 0, "Gross income").expect("label");
    inputs
        .write_number_with_format(1, 1, 60000.0, &input_fmt)
        .expect("B2 gross income");
    inputs.write_string(2, 0, "Filing status").expect("label");
    inputs
        .write_string_with_format(2, 1, "single", &input_fmt)
        .expect("B3 filing status");
    inputs.write_string(3, 0, "Deductions").expect("label");
    inputs
        .write_number_with_format(3, 1, 12000.0, &input_fmt)
        .expect("B4 deductions");
    // Inline-DV enum on B3 (the WR-01 frozen-enum input).
    let dv = rust_xlsxwriter::DataValidation::new()
        .allow_list_strings(&["single", "married_joint", "head_of_household"])
        .expect("dv list");
    inputs.add_data_validation(2, 1, 2, 1, &dv).expect("add dv B3");

    // ---- 2_Brackets (green-fill governed constants) ------------------------
    let brackets = book.add_worksheet();
    brackets.set_name("2_Brackets").expect("name 2_Brackets");
    brackets.write_string(0, 0, "boundary").expect("hdr");
    brackets.write_string(0, 1, "rate").expect("hdr");
    brackets
        .write_number_with_format(1, 0, 0.0, &governed_fmt)
        .expect("A2 boundary 1");
    brackets
        .write_number_with_format(1, 1, 0.10, &governed_fmt)
        .expect("B2 rate 1");
    brackets
        .write_number_with_format(2, 0, 40000.0, &governed_fmt)
        .expect("A3 boundary 2");
    brackets
        .write_number_with_format(2, 1, 0.22, &governed_fmt)
        .expect("B3 rate 2");

    // ---- 3_Outputs (formulas WITH cached results) --------------------------
    // taxable_income = gross - deductions = 60000 - 12000 = 48000
    // tax_owed       = taxable * rate1     = 48000 * 0.10 = 4800
    // effective_rate = tax_owed / gross    = 4800 / 60000 = 0.08
    // marginal_rate  = rate2               = 0.22
    let outputs = book.add_worksheet();
    outputs.set_name("3_Outputs").expect("name 3_Outputs");
    outputs.write_string(0, 0, "output").expect("hdr");
    outputs
        .write_formula(1, 1, Formula::new("='1_Inputs'!B2-'1_Inputs'!B4").set_result("48000"))
        .expect("B2 taxable_income");
    outputs
        .write_formula(2, 1, Formula::new("='3_Outputs'!B2*'2_Brackets'!B2").set_result("4800"))
        .expect("B3 tax_owed");
    outputs
        .write_formula(3, 1, Formula::new("='3_Outputs'!B3/'1_Inputs'!B2").set_result("0.08"))
        .expect("B4 effective_rate");
    outputs
        .write_formula(4, 1, Formula::new("='2_Brackets'!B3").set_result("0.22"))
        .expect("B5 marginal_rate");

    // ---- out_* defined names → the driver promotes these to Role::Output ----
    book.define_name("out_taxable_income", "='3_Outputs'!$B$2")
        .expect("name out_taxable_income");
    book.define_name("out_tax_owed", "='3_Outputs'!$B$3")
        .expect("name out_tax_owed");
    book.define_name("out_effective_rate", "='3_Outputs'!$B$4")
        .expect("name out_effective_rate");
    book.define_name("out_marginal_rate", "='3_Outputs'!$B$5")
        .expect("name out_marginal_rate");

    book.save_to_buffer().expect("author tax-calc.xlsx bytes")
}
