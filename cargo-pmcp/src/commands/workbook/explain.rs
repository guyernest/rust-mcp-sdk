//! `cargo pmcp workbook explain <wb.xlsx>` — the dry-run tool-surface preview
//! (WBV2-06, §8): "here is the tool surface an AI will see" rendered BEFORE deploy,
//! the single best guard against the silent-broken-deploy class.
//!
//! Modelled on the proven read-only [`super::lint`] shape: a [`ExplainArgs`] with a
//! dual `--format text|json`, a read-only `pmcp_workbook_compiler::ingest::ingest`
//! (NO bundle written), and a PURE [`format_tool_surface`] String renderer so JSON
//! is testable without stdout capture. Per Phase-74 D-11: the rendered surface (the
//! data) → stdout; the advisory header → stderr (gated on `should_output()` /
//! `PMCP_QUIET`).
//!
//! The PURE projection + render lives in [`super::explain_surface`] (mounted into the
//! lib target via `#[path]` so the `workbook_explain` example + integration test
//! reach it WITHOUT the bin-only `commands::*` tree). This module is the thin CLI
//! arm: arg parsing, the advisory header, and the stdout print.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

// The pure projection + render the CLI arm drives (the full surface API — types +
// `project_tool_surface` — is reached via the lib seam `crate::workbook_explain` by
// the example + integration test).
use super::explain_surface::{explain_workbook, format_tool_surface};
use super::GlobalFlags;

/// Arguments for `cargo pmcp workbook explain`.
#[derive(Debug, Args)]
pub struct ExplainArgs {
    /// Path to the `.xlsx` workbook whose tool surface to preview.
    pub workbook_path: PathBuf,

    /// Output format: `text` (default) or `json`.
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Execute `cargo pmcp workbook explain`.
///
/// # Errors
/// Returns an error if the workbook cannot be ingested, if the workbook declares no
/// output Table (nothing to serve), or if `--format` is unknown.
pub fn execute(args: ExplainArgs, gf: &GlobalFlags) -> Result<()> {
    let tools = explain_workbook(&args.workbook_path)?;

    let not_quiet = gf.should_output() && std::env::var("PMCP_QUIET").is_err();
    if not_quiet && args.format == "text" {
        eprintln!("workbook tool-surface preview — {} tool(s)", tools.len());
    }

    let rendered = format_tool_surface(&tools, &args.format)?;
    println!("{rendered}");
    Ok(())
}
