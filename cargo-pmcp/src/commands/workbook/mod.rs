//! `cargo pmcp workbook <subcommand>` — governed Excel workbook bundle tooling.
//!
//! This module shells over the `pmcp-workbook-compiler` library verbs. Plan
//! 94-01 introduces the project-config parser ([`config`]); Plan 94-02 adds the
//! command-group enum ([`WorkbookCommand`]), the `lint` handler, the shared
//! exit-code transport ([`WorkbookExit`] + the `EXIT_*` constants), and the
//! neutral `compile`/`emit` handler skeletons that Plans 94-03/94-04 fill in.
//!
//! ## Exit-code contract (D-10)
//!
//! The whole phase shares ONE definition of the process-exit codes:
//! - [`EXIT_OK`] (`0`): success / warnings-only lint.
//! - [`EXIT_ERROR`] (`1`): a compile/lint error — this is also anyhow's default
//!   exit code, so an ordinary `anyhow::bail!` already maps here.
//! - [`EXIT_GATE_BLOCK`] (`2`): a governance gate block (Plan 94-03). This is a
//!   DISTINCT code so CI can tell a gate block apart from a compile error. It is
//!   carried out of a handler via the typed [`WorkbookExit`] error, which
//!   `main.rs` downcasts to recover the code (an ordinary `anyhow::Error` would
//!   collapse it to `1`).

pub mod compile;
pub mod config;
pub mod emit;
pub mod lint;
mod targets;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// Success / warnings-only exit code.
pub const EXIT_OK: i32 = 0;
/// A compile or lint error (equals anyhow's default exit code).
pub const EXIT_ERROR: i32 = 1;
/// A governance gate block — DISTINCT from a compile error (D-10).
pub const EXIT_GATE_BLOCK: i32 = 2;

/// A typed error carrying a specific process-exit code out of a workbook handler.
///
/// Ordinary compile/lint errors use plain `anyhow::bail!` and map to anyhow's
/// default exit code (`1` == [`EXIT_ERROR`]). Only the gate-block path needs the
/// DISTINCT [`EXIT_GATE_BLOCK`] (`2`) code, so it constructs a `WorkbookExit`
/// (via [`WorkbookExit::gate_block`]) that `main.rs` downcasts and turns into a
/// `std::process::exit(code)`. The gate-block render is printed by the handler
/// BEFORE it constructs the error; `message` re-surfaces a concise summary.
#[derive(Debug)]
pub struct WorkbookExit {
    /// The process-exit code to surface to the shell (one of the `EXIT_*`).
    pub code: i32,
    /// A concise message re-printed to stderr by `main.rs` before exiting.
    pub message: String,
}

impl WorkbookExit {
    /// Construct a gate-block exit ([`EXIT_GATE_BLOCK`], `2`).
    pub fn gate_block(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_GATE_BLOCK,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WorkbookExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WorkbookExit {}

/// `cargo pmcp workbook <subcommand>` — the governed-workbook command group (D-04).
#[derive(Debug, Subcommand)]
pub enum WorkbookCommand {
    /// Compile a workbook into a gated, served bundle (delivered by Plan 94-03).
    Compile(compile::CompileArgs),
    /// Lint a workbook against the dialect, standalone (WBCL-02).
    Lint(lint::LintArgs),
    /// Emit an UNGATED bundle for dev/reference (delivered by Plan 94-04).
    Emit(emit::EmitArgs),
}

impl WorkbookCommand {
    /// Dispatch the subcommand to its handler.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            WorkbookCommand::Compile(args) => compile::execute(args, global_flags),
            WorkbookCommand::Lint(args) => lint::execute(args, global_flags),
            WorkbookCommand::Emit(args) => emit::execute(args, global_flags),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_constants_are_distinct() {
        assert_eq!(EXIT_OK, 0);
        assert_eq!(EXIT_ERROR, 1);
        assert_eq!(EXIT_GATE_BLOCK, 2);
        assert_ne!(EXIT_ERROR, EXIT_GATE_BLOCK);
    }

    #[test]
    fn gate_block_carries_distinct_code_and_message() {
        let wx = WorkbookExit::gate_block("blocked: output delta exceeds policy");
        assert_eq!(wx.code, EXIT_GATE_BLOCK);
        assert_eq!(wx.message, "blocked: output delta exceeds policy");
        // Display surfaces the message verbatim (re-printed by main.rs).
        assert_eq!(wx.to_string(), "blocked: output delta exceeds policy");
    }

    #[test]
    fn workbook_exit_is_a_std_error() {
        // Round-trips through anyhow so main.rs can downcast_ref it back.
        let err: anyhow::Error = anyhow::Error::new(WorkbookExit::gate_block("x"));
        let wx = err
            .downcast_ref::<WorkbookExit>()
            .expect("downcast back to WorkbookExit");
        assert_eq!(wx.code, EXIT_GATE_BLOCK);
    }
}
