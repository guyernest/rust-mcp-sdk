//! `cargo pmcp workbook compile` — compile a governed workbook into a gated bundle.
//!
//! The handler body (ingest → lint → synth → compile → reconcile → gate → write)
//! is delivered by Plan 94-03. This file currently exposes only the [`CompileArgs`]
//! clap struct the [`super::WorkbookCommand::Compile`] variant references, plus a
//! handler that returns a defined runtime error naming the delivering plan — so
//! Wave 2 compiles while the orchestration logic is owned by Plan 94-03.

use anyhow::Result;

use super::GlobalFlags;

/// Arguments for `cargo pmcp workbook compile` (fields land with Plan 94-03).
#[derive(Debug, clap::Args)]
pub struct CompileArgs;

/// Compile a governed workbook into a gated bundle.
///
/// # Errors
/// Returns an error until Plan 94-03 wires the compile pipeline.
pub fn execute(_args: CompileArgs, _gf: &GlobalFlags) -> Result<()> {
    anyhow::bail!("workbook compile is delivered by plan 94-03")
}
