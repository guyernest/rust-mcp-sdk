//! Governed-Excel workbook served-tool module (Phase 92,
//! `bundlesource-served-tool-toolkit-module`).
//!
//! This is the toolkit-side home for the served `calculate` / `explain` /
//! `get_manifest` / `diff_version` / `render_workbook` tools that operate on a
//! verified [`pmcp_workbook_runtime::WorkbookBundle`] (loaded fail-closed via
//! the runtime's `BundleSource` + `BundleLoader`).
//!
//! # Wiring discipline (Codex HIGH #1 / HIGH #2)
//!
//! The feature + this gated module skeleton land EARLY (this plan, wave 2) so
//! the wave-3/4 plans can `cargo test --features workbook` before the handlers
//! exist. The skeleton intentionally exposes NOTHING yet — the submodule
//! declarations below stay COMMENTED until the plan that creates each file
//! uncomments the matching `pub mod` line (never declare a `pub mod` before its
//! file exists — Codex HIGH #2):
//!
//! - Plan 03 Task 1 creates `error`/`schema`/`input`/`handler` and uncomments
//!   their declarations as each file lands.
//! - Plan 05 adds the builder-ext wiring + crate-root re-exports.
//!
//! Until the handlers land the `workbook` / `workbook-embedded` builds compile
//! against the lifted error/schema surface, proving the feature gate is wired
//! correctly.
//!
//! # Domain failure vs infrastructure failure (Codex LOW)
//!
//! The served tools draw a sharp line between two failure classes:
//!
//! - A **domain failure** (invalid input, an out-of-range / non-finite output,
//!   a strict-constant override) is NOT a protocol error. It returns
//!   `isError:true` INSIDE `structuredContent` via
//!   [`error::to_iserror_result`] so the MCP App widget can read a stable,
//!   machine-actionable repair code — never an `Err(pmcp::Error)`.
//! - An **infrastructure failure** (a poisoned/malformed in-memory bundle state,
//!   a resource-handler internal fault, a genuine bug) MAY still surface as a
//!   protocol `Err`. The lift does NOT blanket-swallow infrastructure faults as
//!   domain errors.
//!
//! # The served provenance stamp ([`ProvStamp`], Codex HIGH #3)
//!
//! Every tool result (success AND error envelope) carries a [`ProvStamp`] of
//! `{ bundle_id, version, combined_hash }`. The `combined_hash` field carries
//! the `BUNDLE.lock` COMBINED hash-of-hashes
//! ([`pmcp_workbook_runtime::BundleLock::combined`]). It is named `combined_hash`
//! — NEVER `workbook_hash` — so it can never be confused with
//! [`pmcp_workbook_runtime::BundleLock::workbook_hash`], which is the SOURCE
//! workbook content hash, a DIFFERENT value.

use serde_json::{json, Value};

pub mod error;
pub mod handler;
pub mod input;
pub mod schema;

#[doc(inline)]
pub use error::{to_iserror_result, WorkbookToolError};
#[doc(inline)]
pub use handler::CalculateHandler;
#[doc(inline)]
pub use input::{validate_input, ValidatedInput};

/// Re-export of the verified runtime bundle the served tools operate on (loaded
/// fail-closed via [`pmcp_workbook_runtime::load_bundle`]).
pub use pmcp_workbook_runtime::{CellMap, Manifest, WorkbookBundle};

/// The UI resource URI every workbook tool advertises (MCP Apps widget hook).
///
/// The widget resource itself lands in Plan 04 (`render_workbook` + the
/// `workbook://` resource); the tools advertise this stable pointer now so a
/// client's `structuredContent` is widget-routable from the first handler.
pub const WORKBOOK_TOOL_UI: &str = "ui://workbook/result";

/// The provenance stamp on EVERY served tool result (success AND error
/// envelope) — the `bundle_id@version` identity plus the `combined_hash`
/// integrity anchor (Codex HIGH #3).
///
/// Constructed from a verified [`WorkbookBundle::stamp`]
/// ([`pmcp_workbook_runtime::BundleLock`]) by [`ProvStamp::from_bundle`]. The
/// `combined_hash` field carries [`pmcp_workbook_runtime::BundleLock::combined`]
/// — NOT [`pmcp_workbook_runtime::BundleLock::workbook_hash`] (the source-workbook
/// hash). The two MUST never be conflated: `combined_hash` flips when ANY bundle
/// artifact changes, binding the response to the exact verified bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvStamp {
    /// The neutral bundle identifier (e.g. `"tax-calc"`).
    pub bundle_id: String,
    /// The semver version (e.g. `"1.1.0"`).
    pub version: String,
    /// The `BUNDLE.lock` COMBINED hash-of-hashes (NEVER the source-workbook
    /// hash — Codex HIGH #3).
    pub combined_hash: String,
}

impl ProvStamp {
    /// Build the served provenance stamp from a verified [`WorkbookBundle`].
    ///
    /// The `combined_hash` is taken from `bundle.stamp.combined` (the
    /// `BUNDLE.lock` combined hash-of-hashes) — explicitly NOT
    /// `bundle.stamp.workbook_hash`, so the served stamp can never carry the
    /// source-workbook hash (Codex HIGH #3).
    #[must_use]
    pub fn from_bundle(bundle: &WorkbookBundle) -> Self {
        Self {
            bundle_id: bundle.stamp.bundle_id.clone(),
            version: bundle.stamp.version.clone(),
            combined_hash: bundle.stamp.combined.clone(),
        }
    }

    /// The stamp as a JSON object attached to every result payload.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "bundle_id": self.bundle_id,
            "version": self.version,
            "combined_hash": self.combined_hash,
        })
    }
}
