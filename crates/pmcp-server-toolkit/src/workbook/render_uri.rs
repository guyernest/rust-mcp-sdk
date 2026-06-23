//! The `workbook://` render-pointer URI codec (WBSV-05, V12).
//!
//! `render_workbook` does NOT return the `.xlsx` bytes. It validates the inputs,
//! then returns a `workbook://` URI that encodes the (canonical) inputs PLUS the
//! bundle provenance stamp. The bytes are recomputed per `resources/read` by
//! decoding the URI, re-verifying provenance, re-validating the inputs, re-running
//! the executor, and rendering (see [`super::render_resource`]). This keeps the
//! server STATELESS (Lambda-safe — no session, no server-side render cache, V3).
//!
//! # The URI as an attacker-controlled payload
//!
//! The pointer round-trips through the client, so the URI handed back to
//! `resources/read` is UNTRUSTED — an attacker may forge, truncate, oversize, or
//! cross-wire it. The codec is hardened accordingly:
//!
//! - **Size guard FIRST (T-92-14 / V12):** [`decode`] rejects any URI longer than
//!   [`MAX_ENCODED_URI_LEN`] BEFORE any base64 work — an oversized payload never
//!   reaches the allocator-heavy decode path (DoS mitigation).
//! - **Total, panic-free decode (T-92-17):** every malformed / truncated / garbage
//!   input returns `Err(WorkbookToolError)`, NEVER a panic. The crate `deny(panic)`
//!   lint plus the [`prop_decode_total`](tests) proptest enforce totality over
//!   arbitrary/adversarial input.
//!
//! Provenance verification (decoded stamp == bundle stamp) and input re-validation
//! happen on the READ side ([`super::render_resource`]), not here — this module is
//! purely the codec.
//!
//! # Privacy note (Codex MEDIUM #10)
//!
//! The `workbook://` URI ENCODES the caller's inputs in its payload. A client,
//! proxy, or gateway that logs resource URIs will therefore log the inputs.
//! Operators handling sensitive inputs must treat the URI as sensitive. See
//! `docs/workbook-uri-spec.md` for the published contract + privacy warning.

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the runtime).
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use base64::Engine;
use pmcp_workbook_runtime::RenderMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::WorkbookToolError;
use super::ProvStamp;

/// The `workbook://` scheme prefix every render pointer carries.
pub const RENDER_URI_PREFIX: &str = "workbook://render/";

/// The MIME type of the rendered `.xlsx` workbook (the OOXML spreadsheet type).
/// Advertised by `render_workbook` and carried on the `resources/read` content so
/// the client knows the base64 payload is a downloadable spreadsheet.
pub const WORKBOOK_XLSX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// The hard upper bound on an encoded `workbook://` URI length, in bytes.
///
/// [`decode`] rejects any URI longer than this BEFORE doing any base64 decode —
/// the size guard is the first thing checked, so an oversized attacker payload
/// never reaches the allocating decode path (T-92-14 / V12, DoS mitigation).
///
/// 64 KiB is generous for a tax-style input map (a handful of scalars + a small
/// provenance triple) while bounding the per-read decode cost. It is part of the
/// published `workbook://` contract (`docs/workbook-uri-spec.md`).
pub const MAX_ENCODED_URI_LEN: usize = 64 * 1024;

/// The decoded render payload: the canonical input DTO plus the provenance stamp
/// that was bound into the URI at `render_workbook` time.
///
/// The read side ([`super::render_resource`]) VERIFIES `provenance` against the
/// live bundle stamp and RE-VALIDATES `dto` through
/// [`super::input::validate_input`] before re-running — neither is trusted as-is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedRender {
    /// The canonical wire DTO (`{ inputs, overrides }`) — the SAME shape
    /// [`super::input::validate_input`] accepts, so it re-validates on read.
    pub dto: Value,
    /// The provenance stamp bound into the URI at encode time. The read side
    /// rejects the URI if this does not equal the live bundle stamp
    /// (cross-provenance spoofing guard, T-92-15).
    pub provenance: ProvStamp,
    /// The render mode bound into the URI at encode time (WBVER-02). A pre-phase
    /// URI (no `mode` key) decodes to [`RenderMode::Filled`] (back-compat); a
    /// present-but-malformed value is a decode `Err` (never a silent `Filled`).
    pub mode: RenderMode,
}

/// The on-wire JSON payload (pre-base64). Kept private — callers go through
/// [`encode`] / [`decode`] which own the scheme prefix + size guard.
///
/// The `provenance` triple `{ bundle_id, version, combined_hash }` is
/// [`ProvStamp`] itself (its serde derives ARE the wire contract — Codex
/// HIGH #3: the `combined_hash` field, NEVER a source-workbook hash).
#[derive(Debug, Deserialize)]
struct RenderPayload {
    /// The canonical input DTO.
    dto: Value,
    /// The provenance stamp bound into the URI at encode time.
    provenance: ProvStamp,
    /// The render mode (WBVER-02). `#[serde(default)]` makes an ABSENT `mode` key
    /// deserialize to [`RenderMode::default()`] == `Filled` (Pitfall 1
    /// back-compat: pre-phase URIs have no `mode` key). A PRESENT value is decoded
    /// by `RenderMode`'s own `Deserialize`, so a malformed string surfaces as a
    /// decode `Err` — there is deliberately no field-level catch-all.
    #[serde(default)]
    mode: RenderMode,
}

/// Borrowing serialize-only twin of [`RenderPayload`] — same field names and
/// order, so the encoded bytes are identical without cloning the DTO + stamp.
/// `mode` is appended LAST so the existing `dto`/`provenance` byte order is
/// unchanged (keeps `encode_is_deterministic` byte-identical).
#[derive(Serialize)]
struct RenderPayloadRef<'a> {
    dto: &'a Value,
    provenance: &'a ProvStamp,
    mode: RenderMode,
}

/// Encode a validated input DTO + provenance stamp into a `workbook://` render
/// pointer URI.
///
/// The payload `{ dto, provenance }` is serialized to canonical JSON then
/// base64-encoded with the URL-safe, unpadded alphabet (so the result is a clean
/// URI path segment). The bytes are NOT here — they are recomputed on
/// `resources/read` from this URI.
///
/// # Errors
///
/// Returns [`WorkbookToolError::invalid_input`] only if the canonical DTO cannot
/// be serialized (it always can for a [`super::input::ValidatedInput`] DTO; the
/// fallible signature keeps the call site `?`-chained and panic-free).
#[allow(clippy::result_large_err)]
pub fn encode(
    dto: &Value,
    provenance: &ProvStamp,
    mode: RenderMode,
) -> Result<String, WorkbookToolError> {
    let payload = RenderPayloadRef {
        dto,
        provenance,
        mode,
    };
    let json = serde_json::to_vec(&payload).map_err(|e| {
        WorkbookToolError::invalid_input(format!("could not encode render payload: {e}"))
    })?;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
    Ok(format!("{RENDER_URI_PREFIX}{b64}"))
}

/// Decode a `workbook://` render pointer URI back into its [`DecodedRender`]
/// payload — TOTAL and panic-free over arbitrary/adversarial input.
///
/// The size guard is checked FIRST (T-92-14 / V12): a URI longer than
/// [`MAX_ENCODED_URI_LEN`] is rejected BEFORE any base64 decode, so an oversized
/// attacker payload never reaches the allocating decode path.
///
/// # Errors
///
/// Returns [`WorkbookToolError::invalid_input`] for ANY malformed input — an
/// oversized URI, a wrong/absent scheme prefix, non-base64 body, non-UTF-8 or
/// non-JSON decoded bytes, or a payload missing the `dto`/`provenance` fields.
/// NEVER panics (T-92-17, `deny(panic)` + proptest-proven).
#[allow(clippy::result_large_err)]
pub fn decode(uri: &str) -> Result<DecodedRender, WorkbookToolError> {
    // 1. SIZE GUARD FIRST (T-92-14 / V12) — reject oversized BEFORE any decode.
    if uri.len() > MAX_ENCODED_URI_LEN {
        return Err(WorkbookToolError::invalid_input(format!(
            "workbook:// URI exceeds the {MAX_ENCODED_URI_LEN}-byte limit ({} bytes)",
            uri.len()
        )));
    }
    // 2. Scheme prefix (a non-workbook URI is not ours).
    let body = uri.strip_prefix(RENDER_URI_PREFIX).ok_or_else(|| {
        WorkbookToolError::invalid_input(
            "not a workbook://render/ URI (missing scheme prefix)".to_string(),
        )
    })?;
    // 3. base64 (URL-safe, unpadded) — total: a garbage body is an Err.
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|e| {
            WorkbookToolError::invalid_input(format!("workbook:// URI body is not base64: {e}"))
        })?;
    // 4. JSON parse — total: non-UTF-8 / non-JSON / wrong-shape is an Err.
    let payload: RenderPayload = serde_json::from_slice(&bytes).map_err(|e| {
        WorkbookToolError::invalid_input(format!("workbook:// URI payload is not valid: {e}"))
    })?;
    Ok(DecodedRender {
        dto: payload.dto,
        provenance: payload.provenance,
        mode: payload.mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn stamp() -> ProvStamp {
        ProvStamp {
            bundle_id: "tax-calc".to_string(),
            version: "1.1.0".to_string(),
            combined_hash: "a".repeat(64),
        }
    }

    fn dto() -> Value {
        json!({
            "inputs": { "gross_income": 60000.0, "filing_status": "single" },
            "overrides": {},
        })
    }

    #[test]
    fn round_trip_yields_same_dto_and_provenance() {
        let uri = encode(&dto(), &stamp(), RenderMode::Filled).expect("encode");
        assert!(uri.starts_with(RENDER_URI_PREFIX), "carries the scheme");
        let decoded = decode(&uri).expect("decode");
        assert_eq!(decoded.dto, dto(), "dto round-trips");
        assert_eq!(decoded.provenance, stamp(), "provenance round-trips");
        assert_eq!(decoded.mode, RenderMode::Filled, "mode round-trips");
    }

    #[test]
    fn round_trip_carries_inputs_only_mode() {
        // WBVER-02: the chosen mode rides inside the payload and round-trips.
        let uri = encode(&dto(), &stamp(), RenderMode::InputsOnly).expect("encode");
        let decoded = decode(&uri).expect("decode");
        assert_eq!(
            decoded.mode,
            RenderMode::InputsOnly,
            "inputs_only mode round-trips through the URI"
        );
        assert!(
            uri.len() < MAX_ENCODED_URI_LEN,
            "a mode-carrying URI stays under the 64 KiB cap"
        );
    }

    #[test]
    fn prephase_payload_without_mode_key_decodes_to_filled() {
        // BACK-COMPAT (LOW): a LITERAL pre-phase payload string minted WITHOUT a
        // `mode` key (NOT a freshly-serialized struct that merely omits the field)
        // must still decode, defaulting to Filled. This proves `#[serde(default)]`
        // on the DECODE struct keeps old URIs valid.
        let literal_payload = r#"{"dto":{"inputs":{"gross_income":60000.0},"overrides":{}},"provenance":{"bundle_id":"tax-calc","version":"1.1.0","combined_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(literal_payload);
        let uri = format!("{RENDER_URI_PREFIX}{b64}");
        let decoded = decode(&uri).expect("a pre-phase (no-mode) payload still decodes");
        assert_eq!(
            decoded.mode,
            RenderMode::Filled,
            "an ABSENT mode key defaults to Filled (back-compat)"
        );
        assert_eq!(decoded.provenance, stamp(), "the rest still decodes");
    }

    #[test]
    fn payload_with_malformed_mode_value_is_a_decode_err() {
        // MEDIUM #3: a payload carrying a PRESENT-but-malformed `mode` value
        // (e.g. a forged/old URI with "mode":"bogus") is a serde DECODE ERROR —
        // decode returns Err, never a silent Filled, never a panic.
        let bad_payload = r#"{"dto":{"inputs":{},"overrides":{}},"provenance":{"bundle_id":"tax-calc","version":"1.1.0","combined_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"mode":"bogus"}"#;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bad_payload);
        let uri = format!("{RENDER_URI_PREFIX}{b64}");
        let err = decode(&uri).expect_err("a malformed mode value is a decode Err");
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn encode_is_deterministic() {
        // The same (dto, provenance) always encodes to the SAME URI — required for
        // stateless regen-on-read byte-identity downstream.
        let a = encode(&dto(), &stamp(), RenderMode::Filled).expect("encode a");
        let b = encode(&dto(), &stamp(), RenderMode::Filled).expect("encode b");
        assert_eq!(a, b, "encode is deterministic");
    }

    #[test]
    fn oversized_uri_is_rejected_before_decode() {
        // A URI longer than MAX_ENCODED_URI_LEN is rejected by the size guard
        // FIRST, before any base64 work (T-92-14 / V12). Build a body that is
        // valid base64 so the ONLY thing that can reject it is the size guard.
        let big_body = "A".repeat(MAX_ENCODED_URI_LEN + 1);
        let uri = format!("{RENDER_URI_PREFIX}{big_body}");
        assert!(uri.len() > MAX_ENCODED_URI_LEN);
        let err = decode(&uri).expect_err("oversized rejected");
        assert_eq!(err.code, "invalid_input");
        assert!(
            err.reason.contains("limit"),
            "rejected by the size guard, not by base64: {}",
            err.reason
        );
    }

    #[test]
    fn corrupted_uri_decodes_to_err_never_panics() {
        // A truncated / garbage body is an Err, never a panic.
        let uri = encode(&dto(), &stamp(), RenderMode::Filled).expect("encode");
        let truncated = &uri[..uri.len() - 5];
        let _ = decode(truncated); // may be Ok-shaped-but-Err or Err; must not panic
        let garbage = format!("{RENDER_URI_PREFIX}!!!not base64!!!");
        assert!(decode(&garbage).is_err(), "garbage base64 is an Err");
        let wrong_scheme = "https://example.com/evil";
        assert!(decode(wrong_scheme).is_err(), "wrong scheme is an Err");
        // valid base64 of non-JSON bytes
        let not_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xff, 0xfe, 0x00]);
        assert!(
            decode(&format!("{RENDER_URI_PREFIX}{not_json}")).is_err(),
            "valid base64 of non-JSON is an Err"
        );
    }

    proptest! {
        /// Round-trip + determinism over arbitrary valid input maps: any
        /// string-keyed scalar input map encodes then decodes to the SAME dto +
        /// provenance + MODE (WBVER-02), encode is deterministic, and the encoded
        /// URI stays under MAX_ENCODED_URI_LEN.
        #[test]
        fn prop_encode_decode_identity(
            keys in proptest::collection::vec("[a-z_]{1,12}", 0..6),
            nums in proptest::collection::vec(any::<i32>(), 0..6),
            inputs_only in any::<bool>(),
        ) {
            let mode = if inputs_only { RenderMode::InputsOnly } else { RenderMode::Filled };
            let mut inputs = serde_json::Map::new();
            for (k, n) in keys.iter().zip(nums.iter()) {
                inputs.insert(k.clone(), json!(n));
            }
            let d = json!({ "inputs": inputs, "overrides": {} });
            let uri = encode(&d, &stamp(), mode).expect("encode");
            let again = encode(&d, &stamp(), mode).expect("encode again");
            prop_assert_eq!(&uri, &again, "encode deterministic");
            prop_assert!(uri.len() < MAX_ENCODED_URI_LEN, "encoded URI under the 64 KiB cap");
            let decoded = decode(&uri).expect("decode");
            prop_assert_eq!(decoded.dto, d, "dto identity");
            prop_assert_eq!(decoded.provenance, stamp(), "provenance identity");
            prop_assert_eq!(decoded.mode, mode, "mode identity");
        }

        /// Decode totality (the CLAUDE.md ALWAYS-fuzz requirement, via proptest):
        /// `decode` over ARBITRARY/adversarial strings — random text, truncated and
        /// garbage base64, oversized payloads past MAX_ENCODED_URI_LEN, prefixed and
        /// unprefixed — is TOTAL: it NEVER panics and ALWAYS returns Ok or
        /// Err(WorkbookToolError) (T-92-17). The assertion is reaching this line
        /// without unwinding; we additionally exercise oversized + prefixed shapes.
        #[test]
        fn prop_decode_total(s in ".{0,2048}") {
            // bare arbitrary string
            let _ = decode(&s);
            // with our scheme prefix (drives the base64/JSON arms)
            let _ = decode(&format!("{RENDER_URI_PREFIX}{s}"));
            // an oversized variant (drives the size guard arm)
            let oversized = format!("{}{}", RENDER_URI_PREFIX, "A".repeat(MAX_ENCODED_URI_LEN + 1));
            match decode(&oversized) {
                Ok(_) | Err(_) => {}, // total: Ok|Err, never a panic
            }
        }
    }
}
