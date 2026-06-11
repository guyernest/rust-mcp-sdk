//! Artifact emission stage — write the served bundle.
//!
//! Emits the seven-member bundle (`manifest.json`, `executable.ir.json`,
//! `cell_map.json`, `layout.json`, `BUNDLE.lock`, `evidence/`) the served
//! toolkit loads at boot. Uses the runtime's `build_bundle_lock` /
//! `fold_evidence_hash` / `sha256_hex` (re-used from `pmcp-workbook-runtime`;
//! NEVER hand-rolled — the served loader recomputes with these). WR-01:
//! tier-ratification skips frozen-enum inputs. Wave 1 ships a typed stub; Plan 07
//! fills the body.

use crate::error::CompileError;

/// Emit the served bundle artifacts for a compiled manifest + IR.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 07 wires the
/// bundle emitter here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until emission is wired.
pub fn emit() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("artifact::emit"))
}
