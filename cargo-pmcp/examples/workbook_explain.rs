//! WBV2-06 demonstration (the CLAUDE.md ALWAYS `cargo run --example` requirement):
//! preview the served tool surface an AI will see BEFORE deploy.
//!
//! "Here is the tool surface an AI will see." A business analyst authors an Inputs
//! Excel Table + one named OUTPUT Table per result group; this example ingests the
//! shipped `template.xlsx` READ-ONLY (no bundle written, no compile gate), projects
//! the served multi-tool surface — ONE MCP tool per output Table, each with a
//! DAG-derived `inputSchema` + a non-empty `outputSchema` — and prints the text
//! render. The two tools' input key sets are DISJOINT on `withheld` (only
//! `estimate_refund`'s refund formula reaches it) — the DAG-derivation proof a reader
//! can SEE in the printed schema.
//!
//! This is the runnable form of `cargo pmcp workbook explain template.xlsx`: the
//! single best guard against the silent-broken-deploy class.
//!
//! Run with:
//! ```sh
//! cargo run --example workbook_explain -p cargo-pmcp
//! ```

use std::path::PathBuf;

use cargo_pmcp::workbook_explain::{explain_workbook, format_tool_surface};

/// The shipped reference template (the Inputs Table + the two output Tables
/// `Calculate_Tax` + `Estimate_Refund`) — the committed compiler test fixture, the
/// SAME byte-identical artifact the CLI templates dir embeds.
fn template_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("== Preview the tool surface an AI will see, BEFORE deploy ==\n");
    println!("Ingesting the shipped template.xlsx (read-only — no bundle written)...\n");

    // Read-only ingest → synth → per-Table tool-surface projection.
    let tools = explain_workbook(&template_path())?;
    let text = format_tool_surface(&tools, "text")?;
    print!("{text}");

    println!(
        "\nNote: only `estimate_refund` advertises `withheld` — its refund formula\n\
         (withheld - tax_owed) is the only path that reaches that input. The per-tool\n\
         input schemas are DAG-derived, so an LLM sees exactly the inputs each tool needs.\n\
         \n\
         Run the same preview from the CLI with:\n\
           cargo pmcp workbook explain <your-workbook>.xlsx\n\
           cargo pmcp workbook explain <your-workbook>.xlsx --format json"
    );
    Ok(())
}
