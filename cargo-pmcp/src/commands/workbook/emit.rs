//! `cargo pmcp workbook emit` — emit an UNGATED bundle for dev/reference (WBCL-03).
//!
//! The handler body (compile minus the gate, with a loud ungated banner + an
//! ungated evidence marker) is delivered by Plan 94-04. This file currently
//! exposes only the [`EmitArgs`] clap struct the [`super::WorkbookCommand::Emit`]
//! variant references, plus a handler that returns a defined runtime error naming
//! the delivering plan — so Wave 2 compiles while the emit logic is owned by
//! Plan 94-04.

use anyhow::Result;

use super::GlobalFlags;

/// Arguments for `cargo pmcp workbook emit` (fields land with Plan 94-04).
#[derive(Debug, clap::Args)]
pub struct EmitArgs;

/// Emit an UNGATED bundle for dev/reference.
///
/// # Errors
/// Returns an error until Plan 94-04 wires the emit pipeline.
pub fn execute(_args: EmitArgs, _gf: &GlobalFlags) -> Result<()> {
    anyhow::bail!("workbook emit is delivered by plan 94-04")
}
