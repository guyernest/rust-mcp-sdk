//! The stateless regen-on-read `workbook://` resource handler (WBSV-05, V3/V12).
//!
//! [`RenderWorkbookResource`] implements [`pmcp::server::ResourceHandler`] over a
//! verified [`WorkbookBundle`]. `render_workbook` (the tool) hands the client a
//! `workbook://` POINTER; the client reads that pointer via `resources/read`, and
//! THIS handler regenerates the `.xlsx` from the URI on EVERY read — there is NO
//! server-side session or render cache (V3, Lambda-safe). Because the URI is
//! attacker-controlled (it round-trips through the client), every read runs the
//! full hardening pipeline before it renders a single byte:
//!
//! 1. **Decode** ([`render_uri::decode`]) — the size guard (T-92-14) is the first
//!    thing checked, so an oversized URI is rejected before any base64 work; the
//!    decode is total and panic-free (T-92-17).
//! 2. **Verify provenance** — the decoded provenance MUST equal the live bundle
//!    stamp (`combined_hash`, Codex HIGH #3). A cross-provenance / forged URI is
//!    rejected BEFORE rendering (spoofing guard, T-92-15).
//! 3. **Re-validate inputs** — the decoded inputs are run through
//!    [`super::input::validate_input`] AGAIN (the inputs rode through an untrusted
//!    round-trip; an out-of-range / injected input is rejected here, T-92-16).
//! 4. **Re-run + render** — re-run the executor over the validated seeds, then
//!    [`pmcp_workbook_runtime::render::render_xlsx`] (writer-only, reader-free).
//! 5. **base64 (STANDARD)** the bytes into a [`ReadResourceResult`].
//!
//! `render_xlsx` pins document properties to a fixed datetime, so reading the
//! SAME URI twice yields BYTE-IDENTICAL bytes (stateless determinism).
//!
//! There is exactly ONE resource on this handler (no dispatching wrapper — A3).

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the runtime).
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use pmcp::types::{Content, ListResourcesResult, ReadResourceResult, ResourceInfo};
use pmcp::ResourceHandler;

use pmcp_workbook_runtime::render::render_xlsx;
use pmcp_workbook_runtime::RenderMode;

use super::input::validate_input;
use super::render_uri::{self, WORKBOOK_XLSX_MIME};
use super::WorkbookBundle;

/// The single resource URI advertised by `resources/list` for the render surface.
///
/// It is the SCHEME root (no encoded payload) — a stable, listable handle, the
/// same canonical prefix the codec mints URIs under. The concrete
/// `workbook://render/<payload>` URIs are minted per call by `render_workbook`
/// and read back through [`RenderWorkbookResource::read`].
pub const RENDER_RESOURCE_LIST_URI: &str = render_uri::RENDER_URI_PREFIX;

/// The stateless regen-on-read resource handler for `workbook://` render
/// pointers (WBSV-05). Holds the shared verified bundle; every read regenerates
/// the `.xlsx` from the (untrusted) URI — provenance-verified, re-validated,
/// re-run, rendered, base64-encoded.
pub struct RenderWorkbookResource {
    bundle: Arc<WorkbookBundle>,
}

impl RenderWorkbookResource {
    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        Self { bundle }
    }

    /// The `ResourceInfo` entry advertised by `resources/list`.
    fn list_entry(&self) -> ResourceInfo {
        ResourceInfo::new(RENDER_RESOURCE_LIST_URI, "Rendered workbook (.xlsx)")
            .with_description(
                "Download the computed workbook as an .xlsx. Read a workbook://render/<...> \
                 URI minted by the render_workbook tool; the spreadsheet is regenerated \
                 statelessly from the URI on each read.",
            )
            .with_mime_type(WORKBOOK_XLSX_MIME)
    }

    /// The stateless regen pipeline as a `Result` so the caller maps a domain
    /// failure to a protocol error ONCE, at the boundary. Decomposed out of the
    /// trait `read` to keep each fn under cognitive complexity 25.
    fn regenerate(&self, uri: &str) -> Result<String, RegenError> {
        // 1. Decode (size guard + total decode are inside render_uri::decode).
        let decoded = render_uri::decode(uri).map_err(|e| RegenError::BadUri(e.reason))?;
        // 2. Verify provenance == the live bundle stamp (cross-provenance guard).
        //    Field-wise against the lock — no allocation per read.
        let lock = &self.bundle.stamp;
        if decoded.provenance.bundle_id != lock.bundle_id
            || decoded.provenance.version != lock.version
            || decoded.provenance.combined_hash != lock.combined
        {
            return Err(RegenError::CrossProvenance);
        }
        // WBVER-02: capture the render mode (Copy) before `dto` is moved into
        // validate_input. `mode` is a RENDER parameter, not a manifest input, so it
        // is NOT re-validated against the manifest.
        let mode = decoded.mode;
        // 3. RE-VALIDATE the decoded inputs (injected/out-of-range guard).
        let validated = validate_input(decoded.dto, &self.bundle.manifest, &self.bundle.cell_map)
            .map_err(|e| RegenError::Invalid(e.reason))?;
        // 4. Re-run + render (writer-only, reader-free) in the URI's chosen mode.
        let run = super::handler::run_bundle(&self.bundle, validated.seeds)
            .map_err(|e| RegenError::Invalid(e.reason))?;
        let bytes = render_xlsx(&self.bundle.layout, &run, mode)
            .map_err(|e| RegenError::Render(e.to_string()))?;
        // 5. base64 STANDARD the bytes (the xlsx payload — STANDARD, not the
        //    URL-safe alphabet the URI itself uses).
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }
}

/// The internal regen failure classes, mapped to a protocol error at the
/// boundary. A `workbook://` read failure is an infrastructure/protocol error
/// (the client handed us a bad resource URI) — distinct from a tool DOMAIN
/// failure (which rides `isError:true` in `structuredContent`).
#[derive(Debug)]
enum RegenError {
    /// The URI was oversized / malformed / not a workbook:// URI.
    BadUri(String),
    /// The decoded provenance did not match the live bundle (spoofing).
    CrossProvenance,
    /// The decoded inputs failed re-validation (injection / out-of-range).
    Invalid(String),
    /// The xlsx render failed.
    Render(String),
}

impl RegenError {
    /// Map to a `pmcp` protocol error (mirrors `resources.rs` `read` errors).
    fn into_protocol(self) -> pmcp::Error {
        match self {
            RegenError::BadUri(r) => pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("invalid workbook:// resource URI: {r}"),
            ),
            RegenError::CrossProvenance => pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "workbook:// URI provenance does not match the served bundle".to_string(),
            ),
            RegenError::Invalid(r) => pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                format!("workbook:// URI inputs failed re-validation: {r}"),
            ),
            RegenError::Render(r) => pmcp::Error::protocol(
                pmcp::ErrorCode::INTERNAL_ERROR,
                format!("workbook render failed: {r}"),
            ),
        }
    }
}

#[async_trait]
impl ResourceHandler for RenderWorkbookResource {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![self.list_entry()]))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        let b64 = self.regenerate(uri).map_err(RegenError::into_protocol)?;
        // MIME-typed-wire: the base64 .xlsx rides as resource content carrying the
        // OOXML spreadsheet MIME type so the client can decode + download it.
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            uri.to_string(),
            b64,
            WORKBOOK_XLSX_MIME,
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::super::ProvStamp;
    use super::*;
    use std::path::{Path, PathBuf};

    use pmcp_workbook_runtime::{load_bundle, LocalDirSource};
    use serde_json::json;

    fn golden_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc@1.1.0")
    }

    fn golden_bundle() -> Arc<WorkbookBundle> {
        let source = LocalDirSource::new(golden_dir());
        Arc::new(load_bundle(&source).expect("golden bundle boots"))
    }

    /// Mint a valid workbook:// URI for the golden bundle from the given inputs.
    fn valid_uri(bundle: &Arc<WorkbookBundle>, inputs: serde_json::Value) -> String {
        let validated = validate_input(inputs, &bundle.manifest, &bundle.cell_map)
            .expect("inputs validate for fixture");
        render_uri::encode(
            &validated.canonical_dto,
            &ProvStamp::from_bundle(bundle),
            RenderMode::Filled,
        )
        .expect("encode fixture uri")
    }

    #[test]
    fn read_returns_base64_xlsx_and_is_byte_identical_across_reads() {
        let bundle = golden_bundle();
        let res = RenderWorkbookResource::new(bundle.clone());
        let uri = valid_uri(
            &bundle,
            json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" } }),
        );

        let first = res.regenerate(&uri).expect("first read renders");
        let second = res.regenerate(&uri).expect("second read renders");
        // base64 decodes to real bytes.
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&first)
            .expect("valid base64 xlsx");
        // .xlsx is a ZIP container — starts with the PK signature.
        assert_eq!(
            &bytes[..2],
            b"PK",
            "rendered payload is an xlsx (ZIP) container"
        );
        // Stateless determinism: reading the SAME URI twice is byte-identical.
        assert_eq!(first, second, "regen-on-read is byte-identical (stateless)");
    }

    #[test]
    fn cross_provenance_uri_errors_before_rendering() {
        let bundle = golden_bundle();
        let res = RenderWorkbookResource::new(bundle.clone());
        // Encode a URI bound to a DIFFERENT (forged) provenance stamp.
        let forged = ProvStamp {
            bundle_id: "tax-calc".to_string(),
            version: "1.1.0".to_string(),
            combined_hash: "f".repeat(64), // != the real combined_hash
        };
        let dto = json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" }, "overrides": {} });
        let uri = render_uri::encode(&dto, &forged, RenderMode::Filled).expect("encode forged uri");

        let err = res.regenerate(&uri).expect_err("cross-provenance rejected");
        assert!(
            matches!(err, RegenError::CrossProvenance),
            "rejected as cross-provenance BEFORE rendering, got {err:?}"
        );
    }

    #[test]
    fn out_of_range_decoded_input_errors_via_revalidation_not_render() {
        let bundle = golden_bundle();
        let res = RenderWorkbookResource::new(bundle.clone());
        // Hand-encode a URI carrying an OUT-OF-ENUM filing_status with the REAL
        // provenance (so it passes the provenance gate but must fail re-validation).
        let dto = json!({ "inputs": { "filing_status": "alien" }, "overrides": {} });
        let uri = render_uri::encode(&dto, &ProvStamp::from_bundle(&bundle), RenderMode::Filled)
            .expect("encode out-of-range uri");

        let err = res.regenerate(&uri).expect_err("out-of-range rejected");
        assert!(
            matches!(err, RegenError::Invalid(_)),
            "rejected by re-validation (injection guard), not rendered: {err:?}"
        );
    }

    #[test]
    fn oversized_uri_errors_as_bad_uri() {
        let bundle = golden_bundle();
        let res = RenderWorkbookResource::new(bundle);
        let oversized = format!(
            "{}{}",
            render_uri::RENDER_URI_PREFIX,
            "A".repeat(render_uri::MAX_ENCODED_URI_LEN + 1)
        );
        let err = res.regenerate(&oversized).expect_err("oversized rejected");
        assert!(matches!(err, RegenError::BadUri(_)), "size-guard rejection");
    }

    #[tokio::test]
    async fn list_returns_the_single_workbook_resource_entry() {
        let res = RenderWorkbookResource::new(golden_bundle());
        let extra = pmcp::RequestHandlerExtra::default();
        let listed = res.list(None, extra).await.expect("list");
        assert_eq!(listed.resources.len(), 1, "exactly one resource (A3)");
        assert_eq!(listed.resources[0].uri, RENDER_RESOURCE_LIST_URI);
        assert_eq!(
            listed.resources[0].mime_type.as_deref(),
            Some(WORKBOOK_XLSX_MIME)
        );
    }

    #[tokio::test]
    async fn read_via_trait_returns_resource_content_with_xlsx_mime() {
        let bundle = golden_bundle();
        let res = RenderWorkbookResource::new(bundle.clone());
        let uri = valid_uri(
            &bundle,
            json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" } }),
        );
        let extra = pmcp::RequestHandlerExtra::default();
        let result = res.read(&uri, extra).await.expect("read renders");
        assert_eq!(result.contents.len(), 1);
    }
}
