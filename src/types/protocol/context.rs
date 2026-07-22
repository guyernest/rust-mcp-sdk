//! Per-request protocol context and W3C trace-context value types.
//!
//! These are the additive foundation value types the v2.5 milestone era-gates
//! off. [`ProtocolContext`] carries the once-resolved-at-ingress era plus the
//! negotiated version and optional client identity. [`TraceContext`] surfaces
//! the W3C distributed-tracing headers a client self-reports through the request
//! `_meta` object.

use super::version::{Era, SUPPORTED_PROTOCOL_VERSIONS};
use super::{Implementation, ProtocolVersion};
use crate::types::capabilities::ClientCapabilities;

/// The v1-only default protocol accept-list (Phase 112 D-02/D-04).
///
/// Maps the legacy [`SUPPORTED_PROTOCOL_VERSIONS`] string slice into owned
/// [`ProtocolVersion`] values. This is the accept-list a server carries when the
/// author never calls `.with_supported_protocol_versions(...)` — it deliberately
/// EXCLUDES `2026-07-28` (v2), so an un-opted-in server runs zero era-detection
/// and its v1 request path is byte-for-byte unchanged. It is also the safe
/// fallback for an explicitly-empty accept-list (never produce an all-reject
/// server).
#[must_use]
pub(crate) fn default_accept_list() -> Vec<ProtocolVersion> {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .map(|v| ProtocolVersion((*v).to_string()))
        .collect()
}

/// Maximum accepted length, in bytes, for any single W3C trace value.
///
/// A legitimate `traceparent` is a fixed ~55-byte string; `tracestate` and
/// `baggage` are spec-capped but we allow a generous bound. Any value at
/// ingress exceeding this cap is rejected (see [`TraceContext::from_meta`]) so
/// an attacker-controlled oversized tracing value is never propagated to a
/// handler (threat T-112-09, bounded ingress).
const MAX_TRACE_VALUE_LEN: usize = 8192;

/// The protocol context resolved once at request ingress and threaded through
/// dispatch.
///
/// This is an additive `#[non_exhaustive]` value type: construct it with
/// [`ProtocolContext::new`] and layer optional fields via the `with_*`
/// builders. All four fields are public and directly readable.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProtocolContext {
    /// The behavioral era the negotiated version belongs to (v1 vs v2).
    pub era: Era,
    /// The exact protocol version negotiated for this session/request.
    pub negotiated_version: ProtocolVersion,
    /// The client's self-reported implementation info, if known.
    pub client_info: Option<Implementation>,
    /// The client's advertised capabilities, if known.
    pub client_capabilities: Option<ClientCapabilities>,
}

impl ProtocolContext {
    /// Construct a `ProtocolContext` from the resolved era and negotiated
    /// version. `client_info` and `client_capabilities` default to `None`.
    #[must_use]
    pub fn new(era: Era, negotiated_version: ProtocolVersion) -> Self {
        Self {
            era,
            negotiated_version,
            client_info: None,
            client_capabilities: None,
        }
    }

    /// Attach the client's implementation info.
    #[must_use]
    pub fn with_client_info(mut self, client_info: Implementation) -> Self {
        self.client_info = Some(client_info);
        self
    }

    /// Attach the client's advertised capabilities.
    #[must_use]
    pub fn with_client_capabilities(mut self, client_capabilities: ClientCapabilities) -> Self {
        self.client_capabilities = Some(client_capabilities);
        self
    }
}

/// Reserved `_meta` key carrying the per-request self-reported protocol version
/// (Phase 112, D-11). Read at ingress by [`resolve_protocol_context`].
pub(crate) const RESERVED_PROTOCOL_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

/// Reserved `_meta` key carrying the per-request self-reported `clientInfo`.
pub(crate) const RESERVED_CLIENT_INFO_KEY: &str = "io.modelcontextprotocol/clientInfo";

/// Reserved `_meta` key carrying the per-request self-reported `clientCapabilities`.
pub(crate) const RESERVED_CLIENT_CAPABILITIES_KEY: &str =
    "io.modelcontextprotocol/clientCapabilities";

/// The outcome of protocol negotiation failing at ingress (Phase 112, VERS-01).
///
/// Produced by [`resolve_protocol_context`] when a per-request `_meta` signal
/// cannot be honored. The caller (the native dispatch ingress) maps each variant
/// to a structured JSON-RPC rejection rather than silently disagreeing with the
/// wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolNegotiationError {
    /// A per-request version was present but is not in the server's configured
    /// accept-list (or a v2-only server received no v2 signal). Carries the
    /// offending/absent version string.
    UnsupportedVersion(String),
    /// A RESERVED `_meta` key was present but malformed (non-string
    /// `protocolVersion`, non-deserializable `clientInfo`/`clientCapabilities`,
    /// or a non-object `_meta`). Carries a static description.
    MalformedMeta(&'static str),
}

/// Resolve the per-request [`ProtocolContext`] ONCE at ingress from the server's
/// configured accept-list and the request's `_meta` (Phase 112, VERS-01; the
/// load-bearing spine).
///
/// This is a PURE, deterministic, `cfg`-agnostic function — the single source of
/// era resolution. The native dispatch sites (`core.rs`, `server/mod.rs`) call it
/// once and thread the result; the HTTP layer (Plan 06) resolves once for its
/// header gate and passes the SAME value in — it is never re-derived downstream
/// (D-11: `_meta`-authoritative, transport-agnostic). It compiles on wasm32 (no
/// wasm caller this phase) so the wasm build stays green.
///
/// # Behavior
///
/// - A per-request `protocolVersion` present and in `accept_list` → classified
///   via [`protocol_era`](super::version::protocol_era).
/// - A per-request version present but NOT in `accept_list` →
///   [`ProtocolNegotiationError::UnsupportedVersion`].
/// - No per-request version → falls back to the first v1 version in
///   `accept_list`; a v2-only accept-list with no v2 signal →
///   `UnsupportedVersion("")` (a v2-only server never silently serves v1).
/// - A malformed RESERVED `_meta` key → [`ProtocolNegotiationError::MalformedMeta`];
///   unrelated/unknown extension keys are IGNORED.
///
/// The per-request signal is authoritative over any session-stored version
/// (Pitfall 2) — this function never consults session state.
pub(crate) fn resolve_protocol_context(
    accept_list: &[ProtocolVersion],
    meta: Option<&serde_json::Value>,
) -> Result<Option<ProtocolContext>, ProtocolNegotiationError> {
    // A present `_meta` MUST be an object; a non-object reserved carrier can never
    // be reconciled with the wire, so fail closed rather than silently ignore it.
    let meta_obj =
        match meta {
            Some(value) => Some(value.as_object().ok_or(
                ProtocolNegotiationError::MalformedMeta("_meta is not an object"),
            )?),
            None => None,
        };

    // Resolve the negotiated version + era from the per-request signal, enforcing
    // the accept-list. The per-request signal is authoritative over any session
    // state (Pitfall 2) — session is never consulted here.
    let negotiated_version = resolve_negotiated_version(accept_list, meta_obj)?;
    let era = super::version::protocol_era(negotiated_version.as_str());

    let mut ctx = ProtocolContext::new(era, negotiated_version);
    if let Some(info) = parse_reserved_object::<Implementation>(
        meta_obj,
        RESERVED_CLIENT_INFO_KEY,
        "clientInfo is not deserializable",
    )? {
        ctx = ctx.with_client_info(info);
    }
    if let Some(caps) = parse_reserved_object::<ClientCapabilities>(
        meta_obj,
        RESERVED_CLIENT_CAPABILITIES_KEY,
        "clientCapabilities is not deserializable",
    )? {
        ctx = ctx.with_client_capabilities(caps);
    }
    Ok(Some(ctx))
}

/// Determine the negotiated [`ProtocolVersion`] from the per-request signal +
/// accept-list, or the v1 fallback when no signal is present.
fn resolve_negotiated_version(
    accept_list: &[ProtocolVersion],
    meta_obj: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Result<ProtocolVersion, ProtocolNegotiationError> {
    match meta_obj.and_then(|m| m.get(RESERVED_PROTOCOL_VERSION_KEY)) {
        Some(raw) => {
            let requested = raw.as_str().ok_or(ProtocolNegotiationError::MalformedMeta(
                "protocolVersion is not a string",
            ))?;
            if accept_list.iter().any(|v| v.as_str() == requested) {
                Ok(ProtocolVersion(requested.to_string()))
            } else {
                Err(ProtocolNegotiationError::UnsupportedVersion(
                    requested.to_string(),
                ))
            }
        },
        // Absent signal: fall back to the first v1 version in the accept-list.
        // A v2-only accept-list (no v1 version) never silently serves v1.
        None => accept_list
            .iter()
            .find(|v| super::version::protocol_era(v.as_str()) == Era::V1)
            .cloned()
            .ok_or(ProtocolNegotiationError::UnsupportedVersion(String::new())),
    }
}

/// Deserialize a present RESERVED `_meta` object key into `T`, mapping a
/// present-but-malformed value to [`ProtocolNegotiationError::MalformedMeta`].
/// Absent keys return `Ok(None)`.
fn parse_reserved_object<T: serde::de::DeserializeOwned>(
    meta_obj: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    malformed: &'static str,
) -> Result<Option<T>, ProtocolNegotiationError> {
    match meta_obj.and_then(|m| m.get(key)) {
        Some(raw) => serde_json::from_value::<T>(raw.clone())
            .map(Some)
            .map_err(|_| ProtocolNegotiationError::MalformedMeta(malformed)),
        None => Ok(None),
    }
}

/// W3C trace-context values extracted from a request `_meta` object.
///
/// # Security: values are RAW, UNVALIDATED, and self-reported
///
/// The `traceparent`, `tracestate`, and `baggage` strings are **untrusted**
/// data taken verbatim from the client-supplied `_meta` JSON. They are only
/// **length-bounded** (see [`MAX_TRACE_VALUE_LEN`]); no W3C syntax validation
/// is performed. These values MUST NOT be treated as trusted, authenticated,
/// or safe to interpolate into logs/queries without independent sanitization.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TraceContext {
    /// RAW, UNVALIDATED, self-reported W3C `traceparent` (length-bounded only).
    pub traceparent: String,
    /// RAW, UNVALIDATED, self-reported W3C `tracestate` (length-bounded only).
    pub tracestate: Option<String>,
    /// RAW, UNVALIDATED, self-reported W3C `baggage` (length-bounded only).
    pub baggage: Option<String>,
}

impl TraceContext {
    /// Extract a `TraceContext` from a request `_meta` JSON value.
    ///
    /// Returns `Some` only when the `_meta` object carries a `traceparent`
    /// string within [`MAX_TRACE_VALUE_LEN`]; returns `None` when it is absent,
    /// not a string, or over the bound. The optional `tracestate`/`baggage`
    /// keys are surfaced when present and in-bounds, and silently dropped when
    /// over the bound. Never panics on arbitrary untrusted input.
    ///
    /// The returned values are RAW/UNVALIDATED — see the type-level security
    /// note.
    #[must_use]
    pub fn from_meta(meta: &serde_json::Value) -> Option<Self> {
        // `traceparent` is required and gates the whole extraction: absent,
        // non-string, or over-bound => no trace context at all.
        let traceparent = bounded_trace_value(meta, "traceparent")?;
        // `tracestate`/`baggage` are optional: an over-bound value is dropped
        // (treated as absent for that field), never propagated.
        let tracestate = bounded_trace_value(meta, "tracestate");
        let baggage = bounded_trace_value(meta, "baggage");
        Some(Self {
            traceparent,
            tracestate,
            baggage,
        })
    }
}

/// Read a string-valued key out of a `_meta` object, enforcing the
/// [`MAX_TRACE_VALUE_LEN`] ingress bound.
///
/// Returns `None` when the key is absent, not a string, or over the bound so an
/// attacker-controlled oversized value is never surfaced (threat T-112-09).
fn bounded_trace_value(meta: &serde_json::Value, key: &str) -> Option<String> {
    let value = meta.get(key)?.as_str()?;
    if value.len() > MAX_TRACE_VALUE_LEN {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dual_accept_list() -> Vec<ProtocolVersion> {
        vec![
            ProtocolVersion("2025-11-25".to_string()),
            ProtocolVersion("2026-07-28".to_string()),
        ]
    }

    #[test]
    fn resolve_in_list_v2_signal_classifies_v2() {
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28" });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("v2 in accept-list => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V2);
        assert_eq!(ctx.negotiated_version.as_str(), "2026-07-28");
    }

    #[test]
    fn resolve_absent_signal_falls_back_to_v1() {
        // No per-request version + v1 present in the accept-list => v1 fallback.
        let ctx = resolve_protocol_context(&dual_accept_list(), None)
            .expect("v1 in accept-list => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V1);
        assert_eq!(ctx.negotiated_version.as_str(), "2025-11-25");
    }

    #[test]
    fn resolve_unsupported_version_errors() {
        // A per-request version not in the accept-list => UnsupportedVersion.
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: "1999-01-01" });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect_err("version not in accept-list => Err");
        assert_eq!(
            err,
            ProtocolNegotiationError::UnsupportedVersion("1999-01-01".to_string())
        );
    }

    #[test]
    fn resolve_v2_only_no_signal_errors() {
        // v2-only accept-list + no v2 signal => never silently serve v1.
        let v2_only = vec![ProtocolVersion("2026-07-28".to_string())];
        let err = resolve_protocol_context(&v2_only, None).expect_err("v2-only + no signal => Err");
        assert_eq!(
            err,
            ProtocolNegotiationError::UnsupportedVersion(String::new())
        );
    }

    #[test]
    fn resolve_malformed_reserved_key_errors() {
        // protocolVersion present but not a string => MalformedMeta.
        let meta = json!({ RESERVED_PROTOCOL_VERSION_KEY: 42 });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect_err("non-string protocolVersion => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));

        // _meta present but not an object => MalformedMeta.
        let non_object = json!("not-an-object");
        let err = resolve_protocol_context(&dual_accept_list(), Some(&non_object))
            .expect_err("non-object _meta => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));

        // clientInfo present but not deserializable => MalformedMeta.
        let bad_info = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            RESERVED_CLIENT_INFO_KEY: "should-be-an-object",
        });
        let err = resolve_protocol_context(&dual_accept_list(), Some(&bad_info))
            .expect_err("malformed clientInfo => Err");
        assert!(matches!(err, ProtocolNegotiationError::MalformedMeta(_)));
    }

    #[test]
    fn resolve_unknown_extension_key_is_ignored() {
        // An unrelated extension key must NOT trip the resolver.
        let meta = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            "com.example/whatever": { "anything": [1, 2, 3] },
        });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("unknown key ignored => Ok")
            .expect("resolved => Some");
        assert_eq!(ctx.era, Era::V2);
    }

    #[test]
    fn resolve_populates_client_identity_when_well_formed() {
        let meta = json!({
            RESERVED_PROTOCOL_VERSION_KEY: "2026-07-28",
            RESERVED_CLIENT_INFO_KEY: { "name": "acme-client", "version": "1.2.3" },
            RESERVED_CLIENT_CAPABILITIES_KEY: {},
        });
        let ctx = resolve_protocol_context(&dual_accept_list(), Some(&meta))
            .expect("well-formed => Ok")
            .expect("resolved => Some");
        let info = ctx.client_info.expect("client_info populated");
        assert_eq!(info.name, "acme-client");
        assert_eq!(info.version, "1.2.3");
        assert!(ctx.client_capabilities.is_some());
    }

    #[test]
    fn protocol_context_new_defaults_optionals_to_none() {
        let ctx = ProtocolContext::new(Era::V2, ProtocolVersion("2026-07-28".to_string()));
        assert_eq!(ctx.era, Era::V2);
        assert_eq!(ctx.negotiated_version.as_str(), "2026-07-28");
        assert!(ctx.client_info.is_none());
        assert!(ctx.client_capabilities.is_none());
    }

    #[test]
    fn protocol_context_builders_set_optional_fields() {
        let ctx = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()))
            .with_client_info(Implementation::new("acme-client", "1.2.3"))
            .with_client_capabilities(ClientCapabilities::default());
        assert_eq!(ctx.era, Era::V1);
        let info = ctx.client_info.expect("client_info set");
        assert_eq!(info.name, "acme-client");
        assert_eq!(info.version, "1.2.3");
        assert!(ctx.client_capabilities.is_some());
    }

    #[test]
    fn trace_context_from_meta_extracts_all_fields() {
        let meta = json!({
            "traceparent": "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            "tracestate": "rojo=00f067aa0ba902b7",
            "baggage": "userId=alice"
        });
        let tc = TraceContext::from_meta(&meta).expect("traceparent present => Some");
        assert_eq!(
            tc.traceparent,
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
        assert_eq!(tc.tracestate.as_deref(), Some("rojo=00f067aa0ba902b7"));
        assert_eq!(tc.baggage.as_deref(), Some("userId=alice"));
    }

    #[test]
    fn trace_context_from_meta_traceparent_only() {
        let meta = json!({ "traceparent": "00-abc-def-01" });
        let tc = TraceContext::from_meta(&meta).expect("traceparent present => Some");
        assert_eq!(tc.traceparent, "00-abc-def-01");
        assert!(tc.tracestate.is_none());
        assert!(tc.baggage.is_none());
    }

    #[test]
    fn trace_context_from_meta_absent_returns_none() {
        assert!(TraceContext::from_meta(&json!({})).is_none());
        assert!(TraceContext::from_meta(&json!({ "tracestate": "a=1" })).is_none());
        // Non-string traceparent is treated as absent.
        assert!(TraceContext::from_meta(&json!({ "traceparent": 42 })).is_none());
        // Arbitrary non-object values never panic and yield None.
        assert!(TraceContext::from_meta(&json!("just a string")).is_none());
        assert!(TraceContext::from_meta(&json!([1, 2, 3])).is_none());
        assert!(TraceContext::from_meta(&json!(null)).is_none());
    }

    #[test]
    fn trace_context_over_bound_traceparent_yields_none() {
        let huge = "a".repeat(MAX_TRACE_VALUE_LEN + 1);
        let meta = json!({ "traceparent": huge });
        assert!(TraceContext::from_meta(&meta).is_none());
    }

    #[test]
    fn trace_context_over_bound_tracestate_and_baggage_are_dropped() {
        let huge = "b".repeat(MAX_TRACE_VALUE_LEN + 1);
        let meta = json!({
            "traceparent": "00-abc-def-01",
            "tracestate": huge,
            "baggage": huge,
        });
        let tc = TraceContext::from_meta(&meta).expect("in-bounds traceparent => Some");
        assert_eq!(tc.traceparent, "00-abc-def-01");
        // The oversized values are not surfaced.
        assert!(tc.tracestate.is_none());
        assert!(tc.baggage.is_none());
    }

    proptest::proptest! {
        /// `from_meta` parses UNTRUSTED `_meta` JSON: it must never panic, must
        /// return `None` when `traceparent` is absent, must round-trip an
        /// in-bounds `traceparent` exactly, and must never surface a field over
        /// the bound (threat T-112-09).
        #[test]
        fn from_meta_holds_invariants_over_arbitrary_meta(
            has_traceparent in proptest::prelude::any::<bool>(),
            traceparent in ".*",
            tracestate in proptest::option::of(".*"),
            baggage in proptest::option::of(".*"),
        ) {
            let mut map = serde_json::Map::new();
            if has_traceparent {
                map.insert("traceparent".into(), serde_json::Value::String(traceparent.clone()));
            }
            if let Some(ref ts) = tracestate {
                map.insert("tracestate".into(), serde_json::Value::String(ts.clone()));
            }
            if let Some(ref bg) = baggage {
                map.insert("baggage".into(), serde_json::Value::String(bg.clone()));
            }
            let value = serde_json::Value::Object(map);

            // (a) never panics
            let result = TraceContext::from_meta(&value);

            if !has_traceparent {
                // (b) absent traceparent => None
                proptest::prop_assert!(result.is_none());
            } else if traceparent.len() <= MAX_TRACE_VALUE_LEN {
                // (c) in-bounds traceparent present => Some carrying it exactly
                let tc = result.expect("in-bounds traceparent present => Some");
                proptest::prop_assert_eq!(&tc.traceparent, &traceparent);
                // (d) no surfaced field exceeds the bound
                proptest::prop_assert!(tc.traceparent.len() <= MAX_TRACE_VALUE_LEN);
                if let Some(ref ts) = tc.tracestate {
                    proptest::prop_assert!(ts.len() <= MAX_TRACE_VALUE_LEN);
                }
                if let Some(ref bg) = tc.baggage {
                    proptest::prop_assert!(bg.len() <= MAX_TRACE_VALUE_LEN);
                }
            }
        }
    }
}
