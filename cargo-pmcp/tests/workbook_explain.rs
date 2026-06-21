//! WBV2-06 — `cargo pmcp workbook explain <wb.xlsx>` snapshot coverage.
//!
//! Drives the read-only `explain_workbook` projection over the REAL shipped
//! `template.xlsx` (the Inputs Table + the two output Tables `Calculate_Tax` +
//! `Estimate_Refund`) and asserts the previewed tool surface against a committed
//! text snapshot (the SC3 Wave-0 gap): the per-Table tool names, their captions, and
//! their DAG-derived per-tool input/output schemas.
//!
//! The disjointness proof a BA can SEE: `calculate_tax` advertises `income` while
//! `estimate_refund` ALSO advertises `withheld` (only its `refund` formula —
//! `withheld - tax_owed` — reaches that input). The text render is a PURE function,
//! so the snapshot is exact (no stdout capture).
//!
//! ## H1 — the preview now equals the SERVED surface (no walker lie)
//!
//! Phase 100-08 re-pointed `explain` at the production projection
//! (`pmcp_workbook_compiler::project_tool_surface_from_workbook` → `build_tools` /
//! `json_key_for_role`). The previous bespoke A1 walker SURFACED `filing` on every
//! tool (a "workbook-wide governed input" heuristic) and decorated `income` with a
//! `[USD]` unit harvested from its number format — but the SERVED binary advertises
//! NEITHER: `filing`'s value cell is not referenced by any output formula (the DAG
//! does not reach it, so it is a "feeds-no-tool" input, not a served param), and the
//! colour-synth role for `income` carries no unit (the unit lived only on the old
//! walker's table-harvest projector). The snapshot below is the TRUE served surface;
//! the old snapshot encoded the divergence H1 eliminates. The parity test that proves
//! preview == served lives in `pmcp-workbook-compiler`'s `template_compile_e2e`
//! (`explain_projection_matches_the_served_tool_surface`).

use std::path::{Path, PathBuf};

use cargo_pmcp::workbook_explain::{explain_workbook, format_tool_surface, ToolSurface};

/// The committed compiler test-fixtures copy of the shipped template (Plan 01) — the
/// single authored reference workbook this preview snapshots.
fn template_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx")
}

/// The committed WR-01 hardening multi-sheet fixture (Plan 100-08-HARDENING): a REAL
/// `SUM(range)` + cross-sheet workbook whose `total_sales` tool reaches `q1`/`q2` ONLY
/// via a range and `adjustment` ONLY via a cross-sheet ref.
fn range_cross_sheet_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-workbook-compiler/tests/fixtures/range-cross-sheet.xlsx")
}

/// The committed text snapshot of the template's previewed tool surface.
const TEMPLATE_SURFACE_SNAPSHOT: &str = "\
tool calculate_tax
  description: Compute federal tax from income & filing
  inputs:
    income: number
  outputs:
    tax_owed: number
    effective_rate: number
tool estimate_refund
  description: Estimate refund given withholding
  inputs:
    income: number
    withheld: number
  outputs:
    refund: number
";

fn projected_tools(path: &Path) -> Vec<ToolSurface> {
    explain_workbook(path).expect("project the template tool surface")
}

#[test]
fn template_text_render_matches_committed_snapshot() {
    let tools = projected_tools(&template_fixture());
    let text = format_tool_surface(&tools, "text").expect("text render");
    assert_eq!(
        text, TEMPLATE_SURFACE_SNAPSHOT,
        "the previewed tool surface drifted from the committed snapshot:\n{text}"
    );
}

#[test]
fn template_projects_exactly_the_two_output_table_tools() {
    let tools = projected_tools(&template_fixture());
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["calculate_tax", "estimate_refund"],
        "one tool per output Table (the Inputs Table is the shared pool, not a tool)"
    );
}

#[test]
fn per_tool_inputs_are_dag_derived_disjoint_on_withheld() {
    let tools = projected_tools(&template_fixture());
    let calc = tools.iter().find(|t| t.name == "calculate_tax").unwrap();
    let refund = tools.iter().find(|t| t.name == "estimate_refund").unwrap();

    let calc_keys: Vec<&str> = calc.inputs.iter().map(|p| p.key.as_str()).collect();
    let refund_keys: Vec<&str> = refund.inputs.iter().map(|p| p.key.as_str()).collect();

    // The SERVED surface (H1): only DAG-reached inputs. tax_owed = ROUND(B4*G3-3759)
    // reaches income (B4) but NOT filing (B5) — so the served calculate_tax advertises
    // only `income`, and the preview matches it (no walker-invented `filing`).
    assert_eq!(calc_keys, vec!["income"], "calculate_tax inputs");
    assert_eq!(
        refund_keys,
        vec!["income", "withheld"],
        "estimate_refund inputs"
    );
    // The disjointness the multi-tool surface proves: withheld reaches refund only.
    assert!(
        !calc_keys.contains(&"withheld"),
        "withheld is NOT a calculate_tax input (its formula does not reach it)"
    );
}

#[test]
fn json_render_is_parseable_over_the_same_surface() {
    let tools = projected_tools(&template_fixture());
    let json = format_tool_surface(&tools, "json").expect("json render");
    let back: Vec<ToolSurface> = serde_json::from_str(&json).expect("parse JSON surface");
    assert_eq!(back, tools, "JSON round-trips the projected tool surface");
    // The JSON carries the disjoint per-tool inputs too.
    let refund = back.iter().find(|t| t.name == "estimate_refund").unwrap();
    assert!(refund.inputs.iter().any(|p| p.key == "withheld"));
}

/// WR-01 HARDENING (Plan 100-08-HARDENING) — the explain CLI projection over a REAL
/// `SUM(range)` + cross-sheet workbook surfaces BOTH the range-reached inputs
/// (`q1`, `q2` via `SUM(B2:B3)`) AND the cross-sheet-reached input (`adjustment` via
/// `Aux!B2`) on the served tool — exactly the inputs the OLD bespoke explain walker
/// dropped.
///
/// This is the CLI-render half of the hardening: the load-bearing explain↔served
/// PARITY assertion (these preview keys == the served `input_schema_for_tool` keys)
/// lives in the compiler's `template_compile_e2e`
/// (`explain_projection_matches_served_surface_over_range_and_cross_sheet`), where the
/// trusted-fixture override compile + the served schema are reachable; here we prove
/// the same `explain_workbook` CLI entrypoint surfaces both inputs over the committed
/// fixture (the read-only `Preview` policy drives the SAME production projection).
#[test]
fn explain_surfaces_range_and_cross_sheet_inputs() {
    let tools = projected_tools(&range_cross_sheet_fixture());

    // Exactly one output Table (Total_Sales) → one tool.
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["total_sales"],
        "the one output Table sanitizes to `total_sales`"
    );

    let total = tools.iter().find(|t| t.name == "total_sales").unwrap();
    let mut input_keys: Vec<&str> = total.inputs.iter().map(|p| p.key.as_str()).collect();
    input_keys.sort_unstable();
    assert_eq!(
        input_keys,
        vec!["adjustment", "q1", "q2"],
        "explain surfaces BOTH the SUM(range) members (q1, q2) AND the cross-sheet \
         input (adjustment) — the inputs the old A1 walker dropped"
    );

    // The single output is `total`.
    let output_keys: Vec<&str> = total.outputs.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(output_keys, vec!["total"], "the single output is `total`");
}

#[test]
fn outputs_carry_their_authored_units() {
    let tools = projected_tools(&template_fixture());
    let refund = tools.iter().find(|t| t.name == "estimate_refund").unwrap();
    let refund_field = refund.outputs.iter().find(|o| o.key == "refund").unwrap();
    assert_eq!(refund_field.ty, "number");
    // The refund output's `$#,##0`-style cells: this template authors the refund
    // value cell without a currency format, so the unit is absent — the preview
    // reflects exactly what is authored (no invented unit).
    assert_eq!(refund_field.unit, None);
}
