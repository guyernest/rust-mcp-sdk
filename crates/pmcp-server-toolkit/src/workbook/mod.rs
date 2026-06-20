//! Governed-Excel workbook served-tool module (Phase 92,
//! `bundlesource-served-tool-toolkit-module`).
//!
//! This is the toolkit-side home for the served `calculate` / `explain` /
//! `get_manifest` / `diff_version` / `render_workbook` tools that operate on a
//! verified [`pmcp_workbook_runtime::WorkbookBundle`] (loaded fail-closed via
//! the runtime's `BundleSource` + `BundleLoader`).
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

use std::sync::Arc;

use pmcp::ServerBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

pub mod error;
pub mod handler;
pub mod input;
pub mod render_resource;
pub mod render_uri;
pub mod schema;

#[doc(inline)]
pub use error::{to_iserror_result, WorkbookToolError};
#[doc(inline)]
pub use handler::{
    CalculateHandler, DiffVersionHandler, ExplainHandler, GetManifestHandler, RenderWorkbookHandler,
};
#[doc(inline)]
pub use input::{validate_input, ValidatedInput};
#[doc(inline)]
pub use render_resource::RenderWorkbookResource;
#[doc(inline)]
pub use render_uri::{decode, encode, DecodedRender, MAX_ENCODED_URI_LEN, WORKBOOK_XLSX_MIME};

/// Re-export of the verified runtime bundle the served tools operate on (loaded
/// fail-closed via [`pmcp_workbook_runtime::load_bundle`]).
pub use pmcp_workbook_runtime::{CellMap, Manifest, WorkbookBundle};

/// Re-export of the full boot surface (D-11) so Shape A/B consumers register a
/// served workbook WITHOUT ever naming `pmcp-workbook-runtime`: the
/// `BundleSource` trait + its on-disk impl, the fail-closed loader entry point,
/// and both error types. The `EmbeddedSource` impl is re-exported separately
/// under the `workbook-embedded` feature (it needs the runtime's `embedded`
/// include_dir support).
pub use pmcp_workbook_runtime::{
    load_bundle, BundleLoadError, BundleSource, BundleSourceError, LocalDirSource,
};

/// The binary-baked [`BundleSource`] (WBSV-09), re-exported only when the
/// toolkit's `workbook-embedded` feature layers the runtime's `embedded`
/// (include_dir) support on top of the LocalDirSource-only `workbook` build.
///
/// To construct one, invoke the `include_dir::include_dir!` macro over a
/// committed bundle directory (add `include_dir` as a dependency — the macro
/// emits unqualified `include_dir::` paths so the crate must be nameable at the
/// consumer's root) and pass the resulting `&'static Dir` to
/// [`EmbeddedSource::new`].
#[cfg(feature = "workbook-embedded")]
pub use pmcp_workbook_runtime::EmbeddedSource;

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
/// The field names ARE the wire contract (pinned by
/// `tests/workbook_provstamp_contract.rs`), so the serde derives serialize the
/// stamp directly — every projection (`to_json`, the `workbook://` URI payload,
/// the advertised schema) shares this one definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        // Infallible: ProvStamp is three plain strings.
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// === Builder extension — the single Shape A/B registration call (D-09) =========

/// Composable builder extension wiring a verified workbook bundle into a
/// [`pmcp::ServerBuilder`] in ONE call.
///
/// [`WorkbookBuilderExt::with_workbook_bundle`] /
/// [`WorkbookBuilderExt::try_with_workbook_bundle`] load + integrity-verify a
/// [`BundleSource`] at boot (fail-closed — a tampered bundle aborts the boot,
/// WBSV-08), then register all FIVE served tools (`calculate`, `explain`,
/// `get_manifest`, `diff_version`, `render_workbook`) plus the `workbook://`
/// render resource. Mirrors [`crate::builder_ext::ServerBuilderExt`]'s
/// panicking-convenience + fallible-companion pair (review R7): production
/// servers should prefer the `try_` form so a tampered/malformed bundle surfaces
/// as a `Result`, not a crash.
///
/// This is THE consumer-side contract: Shape A/B servers depend ONLY on
/// `pmcp-server-toolkit` and never name `pmcp-workbook-runtime` (the loader,
/// source impls, and error types are re-exported at this module / the crate
/// root, D-11).
pub trait WorkbookBuilderExt: Sized {
    /// Load + verify `source` and register all five workbook tools + the
    /// `workbook://` resource. Panicking convenience wrapping
    /// [`WorkbookBuilderExt::try_with_workbook_bundle`].
    ///
    /// # Panics
    ///
    /// Panics with `"with_workbook_bundle: ..."` if the bundle fails to load or
    /// its recomputed integrity hashes do not match its lock (a tampered /
    /// malformed bundle, [`BundleLoadError`]). Prefer
    /// [`WorkbookBuilderExt::try_with_workbook_bundle`] for production servers
    /// where a bad bundle must surface as a `Result` (WBSV-08).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};
    ///
    /// let source = LocalDirSource::new("bundles/tax-calc@1.1.0");
    /// let _builder = Server::builder()
    ///     .name("workbook-tax-calc")
    ///     .version("1.1.0")
    ///     .with_workbook_bundle(&source);
    /// ```
    fn with_workbook_bundle(self, source: &dyn BundleSource) -> Self;

    /// Fallible companion to [`WorkbookBuilderExt::with_workbook_bundle`]
    /// (review R7) — the boot LOAD is fail-closed (WBSV-08): a tampered or
    /// malformed bundle returns `Err` BEFORE any tool is registered, so the
    /// server never boots on an unverified bundle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] (wrapping a [`BundleLoadError`]) if the
    /// bundle fails to load — typically a source read error, a JSON parse
    /// failure, or an integrity-hash mismatch (a swapped / tampered artifact).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let source = LocalDirSource::new("bundles/tax-calc@1.1.0");
    /// let _builder = Server::builder()
    ///     .name("workbook-tax-calc")
    ///     .version("1.1.0")
    ///     .try_with_workbook_bundle(&source)?;
    /// # Ok(()) }
    /// ```
    fn try_with_workbook_bundle(self, source: &dyn BundleSource) -> Result<Self>;
}

impl WorkbookBuilderExt for ServerBuilder {
    fn with_workbook_bundle(self, source: &dyn BundleSource) -> Self {
        self.try_with_workbook_bundle(source).expect(
            "with_workbook_bundle: BundleLoader load/verify returned an error — \
             prefer try_with_workbook_bundle to handle a tampered/malformed bundle \
             as a Result (WBSV-08 fail-closed)",
        )
    }

    fn try_with_workbook_bundle(self, source: &dyn BundleSource) -> Result<Self> {
        // WBSV-08 fail-closed: load + integrity-verify the bundle BEFORE any
        // tool is registered. A `WorkbookBundle` value is proof the bundle was
        // untampered at load, so the server cannot boot on an unverified bundle.
        let bundle = Arc::new(load_bundle(source)?);

        // Operator visibility (mirrors builder_ext.rs:273-279): a bundle that
        // declares zero outputs would serve tools that compute nothing useful —
        // surface that as a warning rather than a silently-empty server.
        // TRANSITIONAL (Plan 03→04): flat `.outputs()` accessor; Plan 04 reshapes the
        // zero-output check per-tool and drops the accessor.
        #[allow(deprecated)]
        let has_no_outputs = bundle.cell_map.outputs().is_empty();
        if has_no_outputs {
            tracing::warn!(
                target: "pmcp_server_toolkit::workbook",
                bundle_id = %bundle.stamp.bundle_id,
                version = %bundle.stamp.version,
                "with_workbook_bundle: bundle declares zero outputs — the served \
                 tools will compute no output projections (set RUST_LOG=warn to \
                 surface this)"
            );
        }

        // Register the five served tools over the shared verified bundle. Each
        // handler is `Arc`-cloned so they share ONE verified bundle (no copies).
        let builder = self
            .tool_arc(
                CalculateHandler::NAME,
                Arc::new(CalculateHandler::new(bundle.clone())),
            )
            .tool_arc(
                ExplainHandler::NAME,
                Arc::new(ExplainHandler::new(bundle.clone())),
            )
            .tool_arc(
                GetManifestHandler::NAME,
                Arc::new(GetManifestHandler::new(bundle.clone())),
            )
            .tool_arc(
                DiffVersionHandler::NAME,
                Arc::new(DiffVersionHandler::new(bundle.clone())),
            )
            .tool_arc(
                RenderWorkbookHandler::NAME,
                Arc::new(RenderWorkbookHandler::new(bundle.clone())),
            )
            // The single `workbook://` render resource (A3 — no DispatchingResource
            // wrapper, exactly one resource handler).
            .resources_arc(Arc::new(RenderWorkbookResource::new(bundle)));

        Ok(builder)
    }
}
