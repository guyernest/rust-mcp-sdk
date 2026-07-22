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
