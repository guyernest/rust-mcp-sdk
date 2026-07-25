//! Multi-round-trip request (MRTR) wire types for MCP 2026-07-28.
//!
//! This module is the SINGLE place the 2026-07-28 MRTR wire shape is assembled and
//! parsed (Phase-113 D-10). Everything else in the crate — the HTTP transport, the
//! dispatch layer, the client retry loop — reads the spelling from here and never
//! re-types a key name.
//!
//! # What MRTR is
//!
//! A server that cannot complete a `tools/call` / `prompts/get` / `resources/read`
//! without more information answers with an `InputRequiredResult` instead of a
//! completed result:
//!
//! ```json
//! { "resultType": "input_required",
//!   "inputRequests": { "user_name": { "method": "elicitation/create", "params": { … } } },
//!   "requestState": "<opaque>" }
//! ```
//!
//! The client fulfills each entry and RETRIES the original request (with a DIFFERENT
//! JSON-RPC id) carrying the symmetric map plus the echoed state as **top-level
//! `params` siblings** of `name`/`arguments`/`uri`:
//!
//! ```json
//! { "name": "search", "arguments": {},
//!   "inputResponses": { "user_name": { "action": "accept", "content": { … } } },
//!   "requestState": "<opaque>" }
//! ```
//!
//! **These fields are NOT in `_meta`.** Getting that wrong is the single most likely
//! silent interop failure in the phase, which is why [`splice_mrtr_params`] and
//! [`extract_mrtr_params`] are the only two places the key spelling exists.
//!
//! # The `Mcp-Name` header rule (locked, cross-plan)
//!
//! `Mcp-Name` is PRESENT on EVERY v2 request. For a method that carries no logical
//! name the value is the EMPTY STRING. The server's `require_three_headers` demands
//! presence (Phase-112 D-05) while its `cross_check_name` skips the value comparison
//! for non-name-bearing methods. Provenance: `113-SPEC-RECHECK.md`, section
//! `## Mcp-Name Header Rule` (and `DRIFT-1`, which records that pmcp is deliberately
//! STRICTER than the draft transport spec here). [`encode_header_value`] therefore
//! MUST round-trip the empty string unchanged.
//!
//! # Visibility
//!
//! D-10 describes an INTERNAL adapter. Only the handler-AUTHORING types
//! ([`MrtrSignal`], [`InputRequest`], [`InputRequests`]) and the client-facing
//! RESULT types ([`InputRequiredResult`], [`MrtrOutcome`], [`InputResponse`],
//! [`InputResponses`], [`InputRequestKind`]) are `pub`. Every parsing/plumbing
//! helper is `pub(crate)`, which reaches all in-crate consumers while keeping the
//! `cargo public-api` delta small.

// Why: this module is the Phase-113 wire adapter and lands in Wave 1, AHEAD of its
// production consumers — the header codec is wired by plan 04, the ingress parse and
// `requestState` verification by plans 03/06, the client outcome by plan 07, and the
// `input_required` emission + eligible-method tripwire by plan 09. Until those land,
// the `pub(crate)` surface is exercised only by this module's own tests, which
// `-D warnings` would otherwise reject as dead code. Plan 12's public-API audit
// removes this allow once every consumer is wired.
#![allow(dead_code)]

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::types::elicitation::{ElicitRequestParams, ElicitResult};
use crate::types::roots::ListRootsResult;
use crate::types::sampling::{CreateMessageParams, CreateMessageResult};

// ===========================================================================
// Wire key spellings — declared once, used only by splice/extract.
// ===========================================================================

/// Wire key for the client→server input-response map (top-level `params`).
pub(crate) const INPUT_RESPONSES_KEY: &str = "inputResponses";

/// Wire key for the opaque continuation token (top-level `params` and `result`).
pub(crate) const REQUEST_STATE_KEY: &str = "requestState";

/// Wire key for the server→client input-request map (inside `result`).
pub(crate) const INPUT_REQUESTS_KEY: &str = "inputRequests";

/// Wire key of the reserved metadata object, on BOTH a request's `params` and a
/// result.
///
/// Declared here with its three siblings so the spelling exists once: the client's
/// v2 `_meta` injection, the transport's raw `params._meta` reader, the result
/// envelope and the signal strip all read it from this block rather than each
/// re-typing the literal.
pub(crate) const META_KEY: &str = "_meta";

/// Wire KEY of the v2 result discriminator (top level, inside `result`).
///
/// The one top-level reserved key that used to be spelled as a bare literal
/// while its three siblings came from this block — and the most
/// protocol-critical of them, since the client's whole gather->resend loop
/// branches on it. Server and client now read the key AND the value from here.
pub(crate) const RESULT_TYPE_KEY: &str = "resultType";

/// Wire value of [`RESULT_TYPE_KEY`] on an [`InputRequiredResult`].
pub(crate) const INPUT_REQUIRED_RESULT_TYPE: &str = "input_required";

// ===========================================================================
// The ONE method table (T-113-16).
// ===========================================================================

/// One row of the MRTR method table.
///
/// Every per-method fact MRTR needs lives here as a column, so a method cannot be
/// half-registered: adding a row supplies its logical-name key and its digest-salient
/// params at the same time. Before this was a struct, the same three method strings
/// were spelled out in three independent `match` arms, and omitting one of them
/// failed *silently in the security direction* — a method missing from the salient
/// list digests to the empty object, which quietly removes replay clause 5c's
/// parameter binding.
pub(crate) struct MrtrMethod {
    /// The JSON-RPC method string.
    pub method: &'static str,
    /// Where this method carries its logical name (`params.name`, `params.uri`, …).
    pub name_key: &'static str,
    /// The WHITELIST of params folded into the AAD digest.
    pub salient: &'static [&'static str],
}

/// The ONE method table: the exact set of client requests MRTR applies to.
///
/// The spec is explicit: "Servers **MUST NOT** send `InputRequiredResult` responses
/// on any other client requests." This table is the single source of truth for that
/// rule — the dispatch-layer tripwire that refuses to emit `input_required` on a
/// forbidden method reads it, and so does the client-side retry loop.
pub(crate) const MRTR_METHODS: [MrtrMethod; 3] = [
    MrtrMethod {
        method: CALL_TOOL_METHOD,
        name_key: "name",
        salient: &["name", "arguments"],
    },
    MrtrMethod {
        method: GET_PROMPT_METHOD,
        name_key: "name",
        salient: &["name", "arguments"],
    },
    MrtrMethod {
        method: READ_RESOURCE_METHOD,
        name_key: "uri",
        salient: &["uri"],
    },
];

// The three method constants are the LITERALS and the table REFERENCES them —
// not the other way round. Deriving them as `MRTR_METHODS[0].method` made row
// order a load-bearing positional contract defended only by a test that
// re-spelled the same literals it was meant to protect: inserting or reordering
// a row silently repointed `CALL_TOOL_METHOD` at `prompts/get`, and the client
// would then have sent tool params under the prompts method. This direction has
// no index to get wrong.

/// `tools/call` — the spelling the table row references.
pub(crate) const CALL_TOOL_METHOD: &str = "tools/call";

/// `prompts/get`. See [`CALL_TOOL_METHOD`].
pub(crate) const GET_PROMPT_METHOD: &str = "prompts/get";

/// `resources/read`. See [`CALL_TOOL_METHOD`].
pub(crate) const READ_RESOURCE_METHOD: &str = "resources/read";

/// The table row for `method`, if it is an MRTR method.
///
/// A linear scan over three `&str` beats hashing at this size, and the table is
/// consulted once per request.
fn mrtr_row(method: &str) -> Option<&'static MrtrMethod> {
    MRTR_METHODS.iter().find(|row| row.method == method)
}

/// Whether `method` may carry an `input_required` result.
///
/// Derived from [`MRTR_METHODS`] so the set cannot drift.
pub(crate) fn mrtr_eligible(method: &str) -> bool {
    mrtr_row(method).is_some()
}

/// Resolve a wire method string to the TABLE's `&'static str` for that row.
///
/// Lets a caller that only has a borrowed method name (e.g. one read off a
/// serialized request frame) obtain the table-owned spelling, so its result is
/// derived from [`MRTR_METHODS`] rather than from a match the caller wrote. That
/// is what keeps "add a row" the ONLY edit needed to make a new method bind: a
/// caller resolving through here cannot return `None` for a method the table
/// considers eligible.
pub(crate) fn mrtr_method_static(method: &str) -> Option<&'static str> {
    Some(mrtr_row(method)?.method)
}

/// Single source of truth for a name-bearing method's logical-name location.
///
/// `tools/call` and `prompts/get` carry it in `params.name`; `resources/read` in
/// `params.uri` (a `ReadResourceRequest` has a `uri` field and NO `name` field);
/// every other method is not name-bearing and its `Mcp-Name` header value is the
/// EMPTY STRING (see the module doc's header rule).
///
/// Derived from [`MRTR_METHODS`] so the client and the server read ONE table.
pub(crate) fn logical_name_key(method: &str) -> Option<&'static str> {
    Some(mrtr_row(method)?.name_key)
}

/// Resolve a request's logical name from its params, method-awarely.
///
/// Returns `None` for a non-name-bearing method, or when the params object does not
/// carry a string at the method's logical-name key.
pub(crate) fn logical_name_of(method: &str, params: &Value) -> Option<String> {
    let key = logical_name_key(method)?;
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Read the `(method, logical name)` routing pair off a raw JSON-RPC frame.
///
/// The ONE implementation of the `Mcp-Method` / `Mcp-Name` derivation, shared by
/// the two ends of that contract: the client emits the pair as headers, the
/// server reads it back and cross-checks it against the body. Those two are
/// halves of a single cross-check, so they must not derive it separately — they
/// previously each hand-rolled this traversal, which left
/// [`logical_name_of`] with no production caller at all and let the two halves
/// disagree (one yielding `""` for a missing name, the other `None`).
///
/// Returns `None` only when the frame carries no string `method`. A `Some` with
/// a `None` name means the method is not name-bearing, or carries no string at
/// its logical-name key — both of which the presence-only cross-check treats
/// the same way.
pub(crate) fn frame_routing_pair(frame: &Value) -> Option<(&str, Option<String>)> {
    let method = frame.get("method")?.as_str()?;
    let name = frame
        .get("params")
        .and_then(|params| logical_name_of(method, params));
    Some((method, name))
}

// ===========================================================================
// `Mcp-Name` value encoding (T-113-47).
// ===========================================================================

/// Case-sensitive opening marker of the base64 sentinel form.
pub(crate) const HEADER_SENTINEL_PREFIX: &str = "=?base64?";

/// Case-sensitive closing marker of the base64 sentinel form.
pub(crate) const HEADER_SENTINEL_SUFFIX: &str = "?=";

/// Upper bound on a v2 header value, for BOTH the transport's raw ingress check and
/// this module's sentinel decoder.
///
/// Single-sourced deliberately. These two bounds are coupled: the transport admits a
/// raw header, then `cross_check_name` runs it through [`decode_header_value`]. Were
/// they separate constants "mirroring" each other by doc comment, raising only the
/// transport's would reject a legitimate conformant `Mcp-Name` in the gap as a
/// malformed sentinel — the wrong error for a well-formed request — while lowering
/// only the transport's would make this check dead code.
///
/// Bounds a decompression-style amplification `DoS` where a short base64 sentinel
/// expands into a large allocation.
pub(crate) const MAX_HEADER_VALUE_LEN: usize = 8192;

/// Upper bound on the RAW `=?base64?…?=` sentinel form of a value that is itself
/// within [`MAX_HEADER_VALUE_LEN`].
///
/// Base64 expands by 4/3, so bounding the raw header at [`MAX_HEADER_VALUE_LEN`]
/// would reject the sentinel form of any non-header-safe logical name longer than
/// ~6141 bytes — a value [`encode_header_value`] is perfectly willing to produce.
/// The pair would then fail to round-trip and the server would answer a
/// `HEADER_MISMATCH` for a well-formed conformant request (most plausibly a
/// `resources/read` whose URI carries a `,`/`;`/non-ASCII byte).
///
/// The amplification bound the raw check exists for is preserved by the DECODED
/// length check in [`decode_header_value`], which still enforces
/// [`MAX_HEADER_VALUE_LEN`].
pub(crate) const MAX_HEADER_SENTINEL_LEN: usize = HEADER_SENTINEL_PREFIX.len()
    + MAX_HEADER_VALUE_LEN.div_ceil(3) * 4
    + HEADER_SENTINEL_SUFFIX.len();

/// Whether a byte may travel verbatim in an `Mcp-Name` header value.
///
/// Printable US-ASCII (`0x20..=0x7E`) EXCLUDING the RFC 9110 field-value delimiters
/// `"`, `,`, `;` and `\` — the bytes an intermediary (WAF, proxy, CDN) may re-split
/// a header on, which is the header-splitting surface of T-113-47.
fn header_byte_is_safe(byte: u8) -> bool {
    (0x20..=0x7E).contains(&byte) && !matches!(byte, b'"' | b',' | b';' | b'\\')
}

/// Encode a logical name for the `Mcp-Name` header.
///
/// Returns `value` unchanged when every byte is safe (including the EMPTY string,
/// which is the name-less-method case). Otherwise returns the sentinel form
/// `=?base64?<standard-base64-with-padding>?=`.
///
/// A value that itself begins with [`HEADER_SENTINEL_PREFIX`] is always
/// sentinel-encoded even when otherwise safe, so decoding stays unambiguous.
pub(crate) fn encode_header_value(value: &str) -> String {
    let passthrough = value.bytes().all(header_byte_is_safe)
        && !value.starts_with(HEADER_SENTINEL_PREFIX)
        && value.len() <= MAX_HEADER_VALUE_LEN;
    if passthrough {
        return value.to_string();
    }
    format!(
        "{HEADER_SENTINEL_PREFIX}{}{HEADER_SENTINEL_SUFFIX}",
        BASE64_STANDARD.encode(value.as_bytes())
    )
}

/// Decode an `Mcp-Name` header value produced by [`encode_header_value`].
///
/// Returns the input verbatim for a non-sentinel value, the decoded UTF-8 for a
/// well-formed sentinel, and `None` for a malformed sentinel, invalid UTF-8, a
/// verbatim value longer than [`MAX_HEADER_VALUE_LEN`], a sentinel longer than
/// [`MAX_HEADER_SENTINEL_LEN`], or a DECODING longer than
/// [`MAX_HEADER_VALUE_LEN`]. Never panics.
///
/// The two raw bounds differ on purpose: a sentinel is a 4/3 expansion of the
/// value it carries, so bounding it at the value bound would refuse to decode
/// exactly the sentinels [`encode_header_value`] produces for long non-safe
/// values. The decoded check below is the one that actually caps allocation.
pub(crate) fn decode_header_value(raw: &str) -> Option<String> {
    let Some(rest) = raw.strip_prefix(HEADER_SENTINEL_PREFIX) else {
        if raw.len() > MAX_HEADER_VALUE_LEN {
            return None;
        }
        return Some(raw.to_string());
    };
    if raw.len() > MAX_HEADER_SENTINEL_LEN {
        return None;
    }
    let payload = rest.strip_suffix(HEADER_SENTINEL_SUFFIX)?;
    let bytes = BASE64_STANDARD.decode(payload).ok()?;
    if bytes.len() > MAX_HEADER_VALUE_LEN {
        return None;
    }
    String::from_utf8(bytes).ok()
}

// ===========================================================================
// Wire types.
// ===========================================================================

/// Which of the three MRTR input-request kinds an entry is.
///
/// Carried alongside an [`InputRequest`] so a response can be decoded
/// KIND-DIRECTED ([`InputResponse::decode_for`]) instead of guessed from an
/// overlapping untagged shape (T-113-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputRequestKind {
    /// `elicitation/create` — ask the user for structured input.
    Elicitation,
    /// `sampling/createMessage` — ask the client's model for a completion.
    Sampling,
    /// `roots/list` — ask the client for its filesystem roots.
    Roots,
}

impl InputRequestKind {
    /// The JSON-RPC method string this kind travels as on the wire.
    #[must_use]
    pub const fn wire_method(self) -> &'static str {
        match self {
            Self::Elicitation => "elicitation/create",
            Self::Sampling => "sampling/createMessage",
            Self::Roots => "roots/list",
        }
    }
}

/// One entry of an `inputRequests` map: a full request object with `method` and
/// `params`.
///
/// The spec constrains the values to exactly `ElicitRequest`, `CreateMessageRequest`
/// or `ListRootsRequest`, which is the same adjacently-tagged shape as the existing
/// [`ServerRequest`](crate::types::ServerRequest). The EXISTING
/// [`ElicitRequestParams`] and [`CreateMessageParams`] are reused rather than
/// duplicated (D-10: one handler-facing type per concept).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum InputRequest {
    /// Ask the user for structured input.
    #[serde(rename = "elicitation/create")]
    Elicitation(Box<ElicitRequestParams>),
    /// Ask the client's model for a completion.
    #[serde(rename = "sampling/createMessage")]
    Sampling(Box<CreateMessageParams>),
    /// Ask the client for its filesystem roots.
    #[serde(rename = "roots/list")]
    ListRoots,
}

impl InputRequest {
    /// The [`InputRequestKind`] discriminant for this request.
    #[must_use]
    pub const fn kind(&self) -> InputRequestKind {
        match self {
            Self::Elicitation(_) => InputRequestKind::Elicitation,
            Self::Sampling(_) => InputRequestKind::Sampling,
            Self::ListRoots => InputRequestKind::Roots,
        }
    }
}

/// One entry of an `inputResponses` map: a bare result object.
///
/// The three shapes OVERLAP on the wire and carry no discriminator, so decoding is
/// deliberately NOT `#[serde(untagged)]`-derived: use [`InputResponse::decode_for`]
/// with the kind of the ORIGINATING [`InputRequest`] wherever the kind is known
/// (T-113-46). Serialization is unambiguous and stays derived.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum InputResponse {
    /// Result of an `elicitation/create` input request.
    Elicitation(Box<ElicitResult>),
    /// Result of a `sampling/createMessage` input request.
    Sampling(Box<CreateMessageResult>),
    /// Result of a `roots/list` input request.
    Roots(Box<ListRootsResult>),
}

impl InputResponse {
    /// Decode a wire value into the variant demanded by `kind`.
    ///
    /// This is the CORRECT decoding path: the kind comes from the originating
    /// [`InputRequest`], so a `CreateMessageResult`-shaped value presented for an
    /// `elicitation/create` entry is REJECTED rather than silently reclassified.
    pub fn decode_for(kind: InputRequestKind, value: Value) -> Result<Self, serde_json::Error> {
        match kind {
            InputRequestKind::Elicitation => {
                serde_json::from_value(value).map(|r| Self::Elicitation(Box::new(r)))
            },
            InputRequestKind::Sampling => {
                serde_json::from_value(value).map(|r| Self::Sampling(Box::new(r)))
            },
            InputRequestKind::Roots => {
                serde_json::from_value(value).map(|r| Self::Roots(Box::new(r)))
            },
        }
    }

    /// Best-effort untagged decode — **server-ingress only**.
    ///
    /// A server reading a client's `inputResponses` map at ingress has not yet
    /// opened `requestState` and therefore does not know which kind each key maps
    /// to. This tries the three shapes most-specific-first. Wherever the kind IS
    /// known, [`decode_for`](Self::decode_for) is the correct path.
    pub fn try_from_value_untagged(value: Value) -> Result<Self, serde_json::Error> {
        // Most-specific-first: `ListRootsResult` requires `roots`,
        // `CreateMessageResult` requires `content` + `model`, `ElicitResult`
        // requires `action`. The last attempt CONSUMES `value` so the common path
        // costs at most two clones.
        if let Ok(decoded) = Self::decode_for(InputRequestKind::Roots, value.clone()) {
            return Ok(decoded);
        }
        if let Ok(decoded) = Self::decode_for(InputRequestKind::Sampling, value.clone()) {
            return Ok(decoded);
        }
        Self::decode_for(InputRequestKind::Elicitation, value).map_err(|_| {
            de::Error::custom(
                "inputResponses value matches none of ElicitResult, CreateMessageResult, ListRootsResult",
            )
        })
    }
}

impl<'de> Deserialize<'de> for InputResponse {
    /// Best-effort untagged deserialization, delegating to
    /// [`try_from_value_untagged`](Self::try_from_value_untagged).
    ///
    /// Exists so `inputResponses` can be typed at server ingress. Prefer
    /// [`decode_for`](Self::decode_for) wherever the originating kind is known.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::try_from_value_untagged(value).map_err(de::Error::custom)
    }
}

/// The `inputRequests` map: server-assigned keys → input requests.
///
/// A `BTreeMap` (not a `HashMap`) so the wire key order is deterministic for tests
/// and digests, and duplicate keys are impossible.
pub type InputRequests = BTreeMap<String, InputRequest>;

/// The `inputResponses` map: the client's symmetric answers, keyed identically.
///
/// A `BTreeMap` for the same determinism reason as [`InputRequests`].
pub type InputResponses = BTreeMap<String, InputResponse>;

/// A parsed `input_required` result.
///
/// # Why this type exists
///
/// [`CallToolResult::content`](crate::types::CallToolResult) carries
/// `#[serde(default)]`, so deserializing an `input_required` result into a
/// `CallToolResult` SUCCEEDS and yields a silently EMPTY success — the
/// `inputRequests` / `requestState` / `resultType` fields are discarded without a
/// word. `ReadResourceResult` is worse: its `contents` field has no default, so the
/// same result fails to deserialize at all. Neither struct can grow a typed MRTR
/// field (`GetPromptRequest` / `ReadResourceRequest` are not `#[non_exhaustive]`).
///
/// This type is the additive carrier that lets a caller receive an unfulfilled
/// `input_required` result instead of an empty success. `raw` holds the verbatim
/// result object so nothing is lost.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputRequiredResult {
    /// The result discriminator; `"input_required"` for an MRTR continuation.
    pub result_type: String,
    /// The inputs the server needs before it can complete the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_requests: Option<InputRequests>,
    /// The opaque continuation token to echo back verbatim on the retry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_state: Option<String>,
    /// The result's `_meta` object, if any.
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    /// The verbatim result object, so a caller loses nothing this type does not
    /// model.
    #[serde(skip_serializing)]
    pub raw: Value,
}

impl InputRequiredResult {
    /// Whether this result is an MRTR continuation (`resultType == "input_required"`).
    #[must_use]
    pub fn is_input_required(&self) -> bool {
        self.result_type == INPUT_REQUIRED_RESULT_TYPE
    }
}

/// Serde-facing mirror of [`InputRequiredResult`] without the `raw` capture.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputRequiredShape {
    #[serde(default)]
    result_type: String,
    #[serde(default)]
    input_requests: Option<InputRequests>,
    #[serde(default)]
    request_state: Option<String>,
    #[serde(default, rename = "_meta")]
    meta: Option<Value>,
}

impl<'de> Deserialize<'de> for InputRequiredResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Value::deserialize(deserializer)?;
        let shape = InputRequiredShape::deserialize(&raw).map_err(de::Error::custom)?;
        Ok(Self {
            result_type: shape.result_type,
            input_requests: shape.input_requests,
            request_state: shape.request_state,
            meta: shape.meta,
            raw,
        })
    }
}

/// The outcome of an MRTR-aware call: either a completed result, or an unfulfilled
/// `input_required` continuation.
///
/// This is the additive public return type the `*_mrtr` client methods use, so an
/// `input_required` result reaches the caller instead of being flattened into an
/// empty success.
// Why: `T` is generic, so clippy sizes the `Complete` variant at 0 bytes and reports
// a difference that does not exist in practice — every instantiation uses
// `CallToolResult` / `GetPromptResult` / `ReadResourceResult`, all in the same size
// class as `InputRequiredResult`. Boxing the payload would degrade the ergonomics of
// a client-FACING return type to satisfy a measurement that cannot see `T`.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum MrtrOutcome<T> {
    /// The server completed the request.
    Complete(T),
    /// The server needs more input before it can complete the request.
    InputRequired(InputRequiredResult),
}

impl<T> MrtrOutcome<T> {
    /// The completed result, or `None` when the server asked for more input.
    #[must_use]
    pub fn complete(self) -> Option<T> {
        match self {
            Self::Complete(value) => Some(value),
            Self::InputRequired(_) => None,
        }
    }

    /// The `input_required` continuation, or `None` when the request completed.
    #[must_use]
    pub fn input_required(self) -> Option<InputRequiredResult> {
        match self {
            Self::Complete(_) => None,
            Self::InputRequired(result) => Some(result),
        }
    }
}

// ===========================================================================
// Handler authoring surface.
// ===========================================================================

/// The reserved result-`_meta` key a handler uses to signal "I need more input".
///
/// A handler returns its normal result type with this key set on `_meta`; the
/// dispatch layer converts the signal into a wire `InputRequiredResult` and
/// **STRIPS the key on EVERY path before serialization — v1 included**. It never
/// appears on the wire.
///
/// # The reserved-field registry
///
/// This key is one member of a closed set of SERVER-OWNED fields that a handler
/// may write but never controls. The authoritative registry lives on
/// `server::core::own_reserved_result_fields`; the set is:
///
/// | Reserved key | Location | Server behavior when a handler set it |
/// |---|---|---|
/// | `resultType` | top-level result | OVERWRITTEN with the server-computed disposition |
/// | `io.modelcontextprotocol/serverInfo` | `result._meta` | OVERWRITTEN with the server's real `Implementation` |
/// | `requestState` | top-level result | REMOVED unless this egress minted it |
/// | `inputRequests` | top-level result | REMOVED unless this egress produced it |
/// | `dev.pmcp/mrtr` (this key) | `result._meta` | REMOVED always, on EVERY path |
///
/// Every OTHER `_meta` key a handler sets is preserved untouched.
///
/// # Why the signal rides in `_meta` rather than a typed `HandlerOutcome`
///
/// Cross-AI review preferred an explicit internal `HandlerOutcome::InputRequired`
/// over smuggling control flow through result metadata, and it is the cleaner
/// design. It is not available here: it requires changing the return type of the
/// public [`ToolHandler`](crate::ToolHandler) /
/// [`PromptHandler`](crate::server::PromptHandler) /
/// [`ResourceHandler`](crate::server::ResourceHandler) traits, which is a MAJOR
/// semver break, and the v2.5 milestone is scoped additive with
/// `cargo semver-checks` gating every phase. A typed outcome is the right shape
/// for a future 3.0; until then this key is the seam, and the server owning it
/// unconditionally is what keeps the smuggled control flow from becoming a
/// handler-controlled wire field.
pub const MRTR_SIGNAL_META_KEY: &str = "dev.pmcp/mrtr";

/// The payload a handler places under [`MRTR_SIGNAL_META_KEY`].
///
/// This is the handler AUTHORING surface, hence `pub`: `input_requests` is what the
/// client is asked to fulfill, and `continuation` is arbitrary handler-owned JSON
/// that the dispatch layer seals into the opaque `requestState` token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrtrSignal {
    /// The inputs the handler needs before it can complete.
    pub input_requests: InputRequests,
    /// Handler-owned continuation state, sealed into `requestState`.
    #[serde(default)]
    pub continuation: Value,
}

impl MrtrSignal {
    /// Convert this signal into the `(key, value)` pair a handler inserts into
    /// its result's `_meta`.
    ///
    /// This is the ENTIRE authoring surface for server-side MRTR: build the
    /// requests you need answered, attach whatever continuation state lets you
    /// resume, and put the returned pair on `_meta`. The dispatch layer takes it
    /// from there — it seals `continuation` into an AEAD `requestState`, emits
    /// `resultType: "input_required"` with your `inputRequests`, and removes this
    /// key before serialization.
    ///
    /// # Handler requirements
    ///
    /// **A handler that returns this signal MUST be idempotent up to the point of
    /// that return.** When a client presents a `requestState` this server cannot
    /// decrypt — another instance's per-process key (D-04), or an expired token
    /// (D-05) — the D-15 verdict router clears every MRTR signal from the request
    /// context and RE-RUNS your handler from scratch as a pristine first call, so
    /// that the re-elicitation carries real, answerable `inputRequests` instead of
    /// a bare token. Any side effect you performed before returning the signal
    /// will therefore happen again. This is inherently satisfiable: a handler that
    /// returned `input_required` had not completed its operation.
    ///
    /// # Errors
    ///
    /// Returns the `serde_json` error if the signal cannot be serialized. This is
    /// deliberately fallible rather than an infallible pair: `input_requests`
    /// carries handler-supplied JSON schemas and sampling params, so serialization
    /// CAN fail, and swallowing that with an `unwrap` would violate the repo's
    /// `make check-unwraps` gate.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::types::elicitation::ElicitRequestParams;
    /// use pmcp::types::mrtr::{InputRequest, InputRequests, MrtrSignal};
    /// use serde_json::json;
    ///
    /// // A tool handler that needs the caller's city before it can answer.
    /// let mut input_requests = InputRequests::new();
    /// input_requests.insert(
    ///     "city".to_string(),
    ///     InputRequest::Elicitation(Box::new(ElicitRequestParams::Form {
    ///         message: "Which city should I check the weather for?".to_string(),
    ///         requested_schema: json!({
    ///             "type": "object",
    ///             "properties": { "city": { "type": "string" } },
    ///         }),
    ///     })),
    /// );
    ///
    /// let signal = MrtrSignal {
    ///     input_requests,
    ///     continuation: json!({ "units": "metric" }),
    /// };
    /// let (key, value) = signal.into_meta_entry()?;
    /// assert_eq!(key, pmcp::types::mrtr::MRTR_SIGNAL_META_KEY);
    ///
    /// // Attach it to the result's `_meta` and return as normal.
    /// let mut meta = serde_json::Map::new();
    /// meta.insert(key, value);
    /// assert!(meta.contains_key("dev.pmcp/mrtr"));
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn into_meta_entry(self) -> Result<(String, Value), serde_json::Error> {
        Ok((
            MRTR_SIGNAL_META_KEY.to_string(),
            serde_json::to_value(self)?,
        ))
    }
}

/// Take [`MRTR_SIGNAL_META_KEY`] off a result's `_meta`, dropping an `_meta` the
/// removal empties. Returns the removed payload, or `None` if the key was absent.
///
/// **Deliberately NOT feature-gated.** [`MrtrSignal`] and its key are `pub` on
/// EVERY build, so a handler can write them on a target or a feature set that has
/// no `streamable-http` MRTR egress to consume them (`server::core::mrtr_egress`
/// is `streamable-http`-only by D-14, and `own_reserved_result_fields` only runs
/// on v2 results). The payload is the handler's PLAINTEXT continuation — the very
/// state the AEAD `requestState` token exists to seal — so the removal itself
/// must be reachable everywhere the key is writable.
pub(crate) fn remove_mrtr_signal(result: &mut Value) -> Option<Value> {
    let object = result.as_object_mut()?;
    let removed = object
        .get_mut(META_KEY)
        .and_then(Value::as_object_mut)
        .and_then(|meta| meta.remove(MRTR_SIGNAL_META_KEY))?;
    if object
        .get(META_KEY)
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        object.remove(META_KEY);
    }
    Some(removed)
}

// ===========================================================================
// Request-params splice / extract (T-113-15, T-113-44, T-113-45).
// ===========================================================================

/// Upper bound on an accepted `requestState` string. Bounds a memory-amplification
/// `DoS` where a client posts a multi-megabyte token the server must buffer and
/// attempt to authenticate (T-113-14).
pub(crate) const MAX_REQUEST_STATE_LEN: usize = 8192;

/// Upper bound on the number of `inputResponses` entries. Bounds a per-entry
/// work-amplification `DoS` (each entry costs a decode) (T-113-14).
pub(crate) const MAX_INPUT_RESPONSES: usize = 64;

/// Upper bound on ONE serialized `inputResponses` entry. Bounds a single-huge-value
/// memory `DoS` (T-113-14).
pub(crate) const MAX_INPUT_RESPONSE_BYTES: usize = 65_536;

/// Upper bound on the TOTAL serialized size of all `inputResponses` entries. Bounds
/// the many-medium-values `DoS` the per-entry cap alone would let through (T-113-14).
pub(crate) const MAX_INPUT_RESPONSES_TOTAL_BYTES: usize = 262_144;

/// Upper bound on the nesting depth of ONE `inputResponses` entry. Bounds a
/// stack-exhaustion `DoS` in recursive JSON walks (T-113-14).
pub(crate) const MAX_INPUT_RESPONSE_DEPTH: usize = 32;

/// Upper bound on the nesting depth the canonicalizer will descend for the AAD
/// digest before emitting a cap marker (T-113-14).
const MAX_CANONICAL_DEPTH: usize = 64;

/// The MRTR fields carried on a client→server request's top-level `params`.
///
/// `Default` means "no MRTR fields present" — which [`extract_mrtr_params`] returns
/// ONLY for a genuinely ABSENT key, never for a malformed one.
///
/// Deliberately NOT `PartialEq`: `InputResponse` wraps `ElicitResult` /
/// `CreateMessageResult` / `ListRootsResult`, none of which are `PartialEq`, and
/// widening those public v1 types purely for a test comparison is out of scope.
#[derive(Debug, Clone, Default)]
pub(crate) struct MrtrRequestParams {
    /// The client's answers to a previous round's `inputRequests`.
    pub input_responses: Option<InputResponses>,
    /// The opaque continuation token echoed back verbatim from a previous round.
    pub request_state: Option<String>,
}

/// Why a PRESENT MRTR field could not be accepted.
///
/// Every variant means "present but unusable" — never "absent". Plan 06 maps each to
/// a JSON-RPC error before dispatch, which is what stops a malformed `requestState`
/// from being silently treated as absent and bypassing the verdict table
/// (T-113-44).
///
/// The `key` fields carry attacker-controlled content for programmatic use; the
/// [`Display`](std::fmt::Display) impl deliberately names only the BOUND, never the
/// offending content, so nothing attacker-controlled is echoed into logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MrtrParseError {
    /// `requestState` was present but not a JSON string.
    RequestStateNotAString,
    /// `requestState` exceeded [`MAX_REQUEST_STATE_LEN`].
    RequestStateTooLong {
        /// Observed length in bytes.
        len: usize,
        /// The bound that was exceeded.
        max: usize,
    },
    /// `inputResponses` was present but not a JSON object.
    InputResponsesNotAnObject,
    /// `inputResponses` carried more than [`MAX_INPUT_RESPONSES`] entries.
    TooManyInputResponses {
        /// Observed entry count.
        count: usize,
        /// The bound that was exceeded.
        max: usize,
    },
    /// One `inputResponses` entry exceeded [`MAX_INPUT_RESPONSE_BYTES`].
    InputResponseTooLarge {
        /// The offending map key (NOT echoed by `Display`).
        key: String,
        /// Observed serialized size in bytes.
        bytes: usize,
        /// The bound that was exceeded.
        max: usize,
    },
    /// The `inputResponses` entries totalled more than
    /// [`MAX_INPUT_RESPONSES_TOTAL_BYTES`].
    InputResponsesTotalTooLarge {
        /// Observed total serialized size in bytes.
        bytes: usize,
        /// The bound that was exceeded.
        max: usize,
    },
    /// One `inputResponses` entry nested deeper than [`MAX_INPUT_RESPONSE_DEPTH`].
    InputResponseTooDeep {
        /// The offending map key (NOT echoed by `Display`).
        key: String,
        /// Observed nesting depth.
        depth: usize,
        /// The bound that was exceeded.
        max: usize,
    },
    /// One `inputResponses` entry matched none of the three spec-permitted result
    /// shapes (`ElicitResult`, `CreateMessageResult`, `ListRootsResult`).
    InputResponseUndecodable {
        /// The offending map key (NOT echoed by `Display`).
        key: String,
    },
}

impl std::fmt::Display for MrtrParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestStateNotAString => write!(f, "requestState must be a string"),
            Self::RequestStateTooLong { max, .. } => {
                write!(f, "requestState exceeds the {max}-byte limit")
            },
            Self::InputResponsesNotAnObject => write!(f, "inputResponses must be an object"),
            Self::TooManyInputResponses { max, .. } => {
                write!(f, "inputResponses exceeds the {max}-entry limit")
            },
            Self::InputResponseTooLarge { max, .. } => {
                write!(f, "an inputResponses entry exceeds the {max}-byte limit")
            },
            Self::InputResponsesTotalTooLarge { max, .. } => {
                write!(f, "inputResponses exceeds the {max}-byte total limit")
            },
            Self::InputResponseTooDeep { max, .. } => {
                write!(f, "an inputResponses entry exceeds the {max}-level depth limit")
            },
            Self::InputResponseUndecodable { .. } => write!(
                f,
                "an inputResponses entry is not a valid ElicitResult, CreateMessageResult or ListRootsResult"
            ),
        }
    }
}

impl std::error::Error for MrtrParseError {}

/// Iterative JSON nesting depth, early-exiting past [`MAX_INPUT_RESPONSE_DEPTH`].
///
/// Deliberately iterative (explicit stack) so an adversarially nested value cannot
/// exhaust the call stack before the bound is reported.
fn json_depth(value: &Value) -> usize {
    let mut deepest = 0usize;
    let mut stack = vec![(value, 1usize)];
    while let Some((current, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        if depth > MAX_INPUT_RESPONSE_DEPTH {
            return depth;
        }
        match current {
            Value::Array(items) => stack.extend(items.iter().map(|item| (item, depth + 1))),
            Value::Object(entries) => {
                stack.extend(entries.iter().map(|(_, item)| (item, depth + 1)));
            },
            _ => {},
        }
    }
    deepest
}

/// Read the `requestState` field: absent → `Ok(None)`, present-but-bad → `Err`.
fn extract_request_state(
    params: &serde_json::Map<String, Value>,
) -> Result<Option<String>, MrtrParseError> {
    let Some(value) = params.get(REQUEST_STATE_KEY) else {
        return Ok(None);
    };
    let state = value
        .as_str()
        .ok_or(MrtrParseError::RequestStateNotAString)?;
    if state.len() > MAX_REQUEST_STATE_LEN {
        return Err(MrtrParseError::RequestStateTooLong {
            len: state.len(),
            max: MAX_REQUEST_STATE_LEN,
        });
    }
    Ok(Some(state.to_string()))
}

/// Enforce the per-entry bounds on one `inputResponses` value.
fn check_input_response_bounds(key: &str, value: &Value) -> Result<usize, MrtrParseError> {
    let depth = json_depth(value);
    if depth > MAX_INPUT_RESPONSE_DEPTH {
        return Err(MrtrParseError::InputResponseTooDeep {
            key: key.to_string(),
            depth,
            max: MAX_INPUT_RESPONSE_DEPTH,
        });
    }
    let bytes = serde_json::to_string(value).map_or(usize::MAX, |s| s.len());
    if bytes > MAX_INPUT_RESPONSE_BYTES {
        return Err(MrtrParseError::InputResponseTooLarge {
            key: key.to_string(),
            bytes,
            max: MAX_INPUT_RESPONSE_BYTES,
        });
    }
    Ok(bytes)
}

/// Read the `inputResponses` field: absent → `Ok(None)`, present-but-bad → `Err`.
fn extract_input_responses(
    params: &serde_json::Map<String, Value>,
) -> Result<Option<InputResponses>, MrtrParseError> {
    let Some(value) = params.get(INPUT_RESPONSES_KEY) else {
        return Ok(None);
    };
    let entries = value
        .as_object()
        .ok_or(MrtrParseError::InputResponsesNotAnObject)?;
    if entries.len() > MAX_INPUT_RESPONSES {
        return Err(MrtrParseError::TooManyInputResponses {
            count: entries.len(),
            max: MAX_INPUT_RESPONSES,
        });
    }
    let mut total = 0usize;
    let mut decoded = InputResponses::new();
    for (key, entry) in entries {
        total = total.saturating_add(check_input_response_bounds(key, entry)?);
        if total > MAX_INPUT_RESPONSES_TOTAL_BYTES {
            return Err(MrtrParseError::InputResponsesTotalTooLarge {
                bytes: total,
                max: MAX_INPUT_RESPONSES_TOTAL_BYTES,
            });
        }
        let response = InputResponse::try_from_value_untagged(entry.clone())
            .map_err(|_| MrtrParseError::InputResponseUndecodable { key: key.clone() })?;
        decoded.insert(key.clone(), response);
    }
    Ok(Some(decoded))
}

/// Extract the MRTR fields from a request's top-level `params`.
///
/// Pairs with [`splice_mrtr_params`] — one writes the keys, the other reads them,
/// and the key spelling exists in exactly these two places (T-113-15).
///
/// A non-object `params` value, or an absent key, yields `Ok(Default::default())`.
/// A PRESENT but wrong-shaped, oversized, over-deep or over-count value yields
/// `Err`, so ABSENT is never conflated with INVALID (T-113-44). Never panics.
pub(crate) fn extract_mrtr_params(params: &Value) -> Result<MrtrRequestParams, MrtrParseError> {
    let Some(object) = params.as_object() else {
        return Ok(MrtrRequestParams::default());
    };
    Ok(MrtrRequestParams {
        input_responses: extract_input_responses(object)?,
        request_state: extract_request_state(object)?,
    })
}

/// Write the MRTR fields onto a request's top-level `params`.
///
/// Pairs with [`extract_mrtr_params`]. Both keys are removed UNCONDITIONALLY before
/// anything is inserted, so a later round can never carry a previous round's data
/// (T-113-45) — splicing a `MrtrRequestParams::default()` leaves neither key. The
/// fields land as TOP-LEVEL siblings of `name`/`arguments`/`uri`, never inside
/// `_meta` and never inside `arguments`. No-op on a non-object value.
pub(crate) fn splice_mrtr_params(params: &mut Value, mrtr: &MrtrRequestParams) {
    let Some(object) = params.as_object_mut() else {
        return;
    };
    object.remove(INPUT_RESPONSES_KEY);
    object.remove(REQUEST_STATE_KEY);
    if let Some(responses) = mrtr.input_responses.as_ref() {
        if let Ok(value) = serde_json::to_value(responses) {
            object.insert(INPUT_RESPONSES_KEY.to_string(), value);
        }
    }
    if let Some(state) = mrtr.request_state.as_ref() {
        object.insert(REQUEST_STATE_KEY.to_string(), Value::String(state.clone()));
    }
}

// ===========================================================================
// Originating-request binding for the AEAD AAD (T-113-03).
// ===========================================================================

/// Append a canonical, key-sorted rendering of `value` to `out`.
///
/// `serde_json`'s `preserve_order` feature is enabled crate-wide, so a plain
/// `to_string` would leak the client's key INSERTION order into the digest and make
/// two logically identical requests produce different AADs. Sorting fixes that.
fn write_canonical(value: &Value, depth: usize, out: &mut String) {
    if depth > MAX_CANONICAL_DEPTH {
        out.push_str("\"__mrtr_depth_capped__\"");
        return;
    }
    match value {
        Value::Object(entries) => {
            let mut keys: Vec<&String> = entries.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push(':');
                write_canonical(&entries[key.as_str()], depth + 1, out);
            }
            out.push('}');
        },
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, depth + 1, out);
            }
            out.push(']');
        },
        other => out.push_str(&other.to_string()),
    }
}

/// The per-method WHITELIST of digest-salient params.
///
/// A whitelist, never a blacklist: `_meta`, `inputResponses` and `requestState` are
/// excluded BY CONSTRUCTION because they are simply not on it, so adding or removing
/// them cannot change the digest.
///
/// Derived from [`MRTR_METHODS`] so a newly registered method cannot silently digest
/// to the empty object.
fn salient_params(method: &str, params: &Value) -> Value {
    let mut salient = serde_json::Map::new();
    let keys: &[&str] = mrtr_row(method).map_or(&[], |row| row.salient);
    for key in keys {
        if let Some(value) = params.get(*key) {
            salient.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(salient)
}

/// SHA-256 over the method name and its canonicalized salient params.
///
/// This is the third replay binding the spec requires alongside the authenticated
/// principal and the TTL: "an identifier for the originating request, e.g. the
/// method name and a digest of its salient parameters, rejecting state presented on
/// a request that does not match" (replay-prevention clause 5c). It is fed to the
/// `requestState` AEAD as additional authenticated data, so a token minted for one
/// tool + arguments cannot verify against another (T-113-03).
pub(crate) fn salient_param_digest(method: &str, params: &Value) -> [u8; 32] {
    let mut canonical = String::new();
    write_canonical(&salient_params(method, params), 0, &mut canonical);
    let mut hasher = Sha256::new();
    hasher.update(method.as_bytes());
    hasher.update([0u8]);
    hasher.update(canonical.as_bytes());
    let output = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&output);
    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------
    // InputRequest / InputResponse wire shape
    // -----------------------------------------------------------------

    fn form_elicitation() -> ElicitRequestParams {
        ElicitRequestParams::Form {
            message: "What is your name?".to_string(),
            requested_schema: json!({ "type": "object" }),
        }
    }

    /// The three named method constants are INDEX-derived from the ONE table,
    /// so this pins the row order: reordering `MRTR_METHODS` without updating
    /// the indices fails here rather than silently sending `prompts/get` params
    /// to `tools/call`.
    #[test]
    fn mrtr_method_constants_match_the_table() {
        assert_eq!(CALL_TOOL_METHOD, "tools/call");
        assert_eq!(GET_PROMPT_METHOD, "prompts/get");
        assert_eq!(READ_RESOURCE_METHOD, "resources/read");
        for method in [CALL_TOOL_METHOD, GET_PROMPT_METHOD, READ_RESOURCE_METHOD] {
            assert!(mrtr_eligible(method), "{method} must be in the table");
        }
        assert_eq!(MRTR_METHODS.len(), 3, "a new row needs a new constant");
    }

    #[test]
    fn input_request_elicitation_wire_shape() {
        let request = InputRequest::Elicitation(Box::new(form_elicitation()));
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["method"], "elicitation/create");
        assert_eq!(value["params"]["message"], "What is your name?");
        assert_eq!(request.kind(), InputRequestKind::Elicitation);
    }

    #[test]
    fn input_request_deserializes_all_three_methods() {
        let elicit: InputRequest = serde_json::from_value(json!({
            "method": "elicitation/create",
            "params": { "mode": "form", "message": "hi", "requestedSchema": {} }
        }))
        .unwrap();
        assert_eq!(elicit.kind(), InputRequestKind::Elicitation);

        let sampling: InputRequest = serde_json::from_value(json!({
            "method": "sampling/createMessage",
            "params": { "messages": [], "maxTokens": 16 }
        }))
        .unwrap();
        assert_eq!(sampling.kind(), InputRequestKind::Sampling);

        let roots: InputRequest =
            serde_json::from_value(json!({ "method": "roots/list" })).unwrap();
        assert_eq!(roots.kind(), InputRequestKind::Roots);
    }

    #[test]
    fn input_request_rejects_unknown_method() {
        let result: Result<InputRequest, _> =
            serde_json::from_value(json!({ "method": "tools/list", "params": {} }));
        assert!(result.is_err());
    }

    #[test]
    fn input_request_kind_wire_methods() {
        assert_eq!(
            InputRequestKind::Elicitation.wire_method(),
            "elicitation/create"
        );
        assert_eq!(
            InputRequestKind::Sampling.wire_method(),
            "sampling/createMessage"
        );
        assert_eq!(InputRequestKind::Roots.wire_method(), "roots/list");
    }

    fn elicit_result_value() -> Value {
        json!({ "action": "accept", "content": { "user_name": "Alice" } })
    }

    fn sampling_result_value() -> Value {
        json!({
            "content": { "type": "text", "text": "hello" },
            "model": "test-model"
        })
    }

    fn roots_result_value() -> Value {
        json!({ "roots": [] })
    }

    #[test]
    fn decode_for_elicitation_accepts_elicit_rejects_sampling() {
        let ok = InputResponse::decode_for(InputRequestKind::Elicitation, elicit_result_value());
        assert!(matches!(ok, Ok(InputResponse::Elicitation(_))));

        let err = InputResponse::decode_for(InputRequestKind::Elicitation, sampling_result_value());
        assert!(
            err.is_err(),
            "a CreateMessageResult must not decode as an ElicitResult"
        );
    }

    #[test]
    fn decode_for_sampling_accepts_sampling_rejects_elicit() {
        let ok = InputResponse::decode_for(InputRequestKind::Sampling, sampling_result_value());
        assert!(matches!(ok, Ok(InputResponse::Sampling(_))));

        let err = InputResponse::decode_for(InputRequestKind::Sampling, elicit_result_value());
        assert!(
            err.is_err(),
            "an ElicitResult must not decode as a CreateMessageResult"
        );
    }

    #[test]
    fn decode_for_roots_accepts_list_roots_result() {
        let ok = InputResponse::decode_for(InputRequestKind::Roots, roots_result_value());
        assert!(matches!(ok, Ok(InputResponse::Roots(_))));
    }

    #[test]
    fn input_response_serializes_as_a_bare_result_object() {
        let response =
            InputResponse::decode_for(InputRequestKind::Elicitation, elicit_result_value())
                .unwrap();
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["action"], "accept");
        assert!(
            value.get("Elicitation").is_none(),
            "must be untagged on the wire"
        );
    }

    #[test]
    fn untagged_decode_is_best_effort_over_the_three_shapes() {
        assert!(matches!(
            InputResponse::try_from_value_untagged(roots_result_value()),
            Ok(InputResponse::Roots(_))
        ));
        assert!(matches!(
            InputResponse::try_from_value_untagged(sampling_result_value()),
            Ok(InputResponse::Sampling(_))
        ));
        assert!(matches!(
            InputResponse::try_from_value_untagged(elicit_result_value()),
            Ok(InputResponse::Elicitation(_))
        ));
        assert!(InputResponse::try_from_value_untagged(json!({ "nope": 1 })).is_err());
    }

    // -----------------------------------------------------------------
    // Method tables
    // -----------------------------------------------------------------

    #[test]
    fn mrtr_eligible_is_exactly_three_methods() {
        for method in ["tools/call", "prompts/get", "resources/read"] {
            assert!(mrtr_eligible(method), "{method} must be MRTR-eligible");
        }
        for method in [
            "tools/list",
            "server/discover",
            "completion/complete",
            "subscriptions/listen",
            "initialize",
        ] {
            assert!(!mrtr_eligible(method), "{method} must NOT be MRTR-eligible");
        }
        assert_eq!(MRTR_METHODS.len(), 3);
    }

    /// A method cannot be half-registered.
    ///
    /// Splitting this knowledge across three `match` arms made the failure mode
    /// silent AND security-relevant: a row present in the eligibility list but
    /// missing from the salient list digests to the empty object, dropping replay
    /// clause 5c's parameter binding. As one table, the invariant is checkable.
    #[test]
    fn every_mrtr_method_row_is_completely_populated() {
        for row in &MRTR_METHODS {
            assert!(
                !row.salient.is_empty(),
                "{} is MRTR-eligible but has no salient params — its AAD digest \
                 would degrade to the empty object and stop binding the request",
                row.method
            );
            assert!(
                row.salient.contains(&row.name_key),
                "{}'s logical-name key {:?} must be digest-salient, or a token \
                 minted for one name would verify against another",
                row.method,
                row.name_key
            );
            assert!(
                !row.method.is_empty() && !row.name_key.is_empty(),
                "{} has an empty method or name_key",
                row.method
            );
        }
    }

    #[test]
    fn logical_name_key_table() {
        assert_eq!(logical_name_key("tools/call"), Some("name"));
        assert_eq!(logical_name_key("prompts/get"), Some("name"));
        assert_eq!(logical_name_key("resources/read"), Some("uri"));
        assert_eq!(logical_name_key("tools/list"), None);
    }

    #[test]
    fn logical_name_of_reads_the_method_specific_key() {
        assert_eq!(
            logical_name_of("tools/call", &json!({ "name": "search" })),
            Some("search".to_string())
        );
        assert_eq!(
            logical_name_of("resources/read", &json!({ "uri": "mem://a" })),
            Some("mem://a".to_string())
        );
        assert_eq!(logical_name_of("tools/list", &json!({ "name": "x" })), None);
        assert_eq!(logical_name_of("tools/call", &json!({})), None);
    }

    // -----------------------------------------------------------------
    // Mcp-Name value encoding
    // -----------------------------------------------------------------

    #[test]
    fn encode_header_value_passes_safe_ascii_through() {
        assert_eq!(encode_header_value("search"), "search");
        assert_eq!(encode_header_value("mem://greeting"), "mem://greeting");
    }

    #[test]
    fn encode_header_value_passes_the_empty_string_through() {
        assert_eq!(encode_header_value(""), "");
    }

    #[test]
    fn encode_header_value_sentinel_encodes_non_ascii() {
        let encoded = encode_header_value("日本語");
        assert!(encoded.starts_with("=?base64?"), "got {encoded}");
        assert!(encoded.ends_with("?="), "got {encoded}");
    }

    #[test]
    fn encode_header_value_sentinel_encodes_delimiters() {
        for delimiter in ["a\"b", "a,b", "a;b", "a\\b"] {
            let encoded = encode_header_value(delimiter);
            assert!(
                encoded.starts_with("=?base64?"),
                "{delimiter} must be sentinel-encoded, got {encoded}"
            );
            assert_eq!(decode_header_value(&encoded).as_deref(), Some(delimiter));
        }
    }

    #[test]
    fn encode_header_value_escapes_a_value_that_looks_like_a_sentinel() {
        let raw = "=?base64?abc?=";
        let encoded = encode_header_value(raw);
        assert_ne!(encoded, raw);
        assert_eq!(decode_header_value(&encoded).as_deref(), Some(raw));
    }

    #[test]
    fn decode_header_value_round_trips_ascii_non_ascii_and_empty() {
        for value in ["search", "日本語", "", "mem://a/b?c=d"] {
            assert_eq!(
                decode_header_value(&encode_header_value(value)),
                Some(value.to_string()),
                "round trip failed for {value:?}"
            );
        }
    }

    #[test]
    fn decode_header_value_rejects_a_malformed_sentinel() {
        assert_eq!(decode_header_value("=?base64?not-valid-b64?="), None);
        assert_eq!(decode_header_value("=?base64?no-suffix"), None);
    }

    // -----------------------------------------------------------------
    // splice / extract
    // -----------------------------------------------------------------

    fn responses_fixture() -> InputResponses {
        let mut map = InputResponses::new();
        map.insert(
            "user_name".to_string(),
            InputResponse::decode_for(InputRequestKind::Elicitation, elicit_result_value())
                .unwrap(),
        );
        map
    }

    #[test]
    fn splice_writes_top_level_siblings_not_meta_or_arguments() {
        let mut params = json!({ "name": "search", "arguments": { "q": "x" }, "_meta": {} });
        splice_mrtr_params(
            &mut params,
            &MrtrRequestParams {
                input_responses: Some(responses_fixture()),
                request_state: Some("opaque".to_string()),
            },
        );
        assert_eq!(params["inputResponses"]["user_name"]["action"], "accept");
        assert_eq!(params["requestState"], "opaque");
        assert!(params["arguments"].get("inputResponses").is_none());
        assert!(params["_meta"].get("inputResponses").is_none());
        assert!(params["_meta"].get("requestState").is_none());
        assert_eq!(params["name"], "search");
    }

    #[test]
    fn splice_default_removes_stale_keys() {
        let mut params = json!({
            "name": "search",
            "inputResponses": { "stale": { "action": "accept" } },
            "requestState": "round-1-token"
        });
        splice_mrtr_params(&mut params, &MrtrRequestParams::default());
        assert!(params.get("inputResponses").is_none());
        assert!(params.get("requestState").is_none());
        assert_eq!(params["name"], "search");
    }

    #[test]
    fn splice_is_a_noop_on_a_non_object() {
        let mut params = json!([1, 2, 3]);
        splice_mrtr_params(
            &mut params,
            &MrtrRequestParams {
                input_responses: None,
                request_state: Some("x".to_string()),
            },
        );
        assert_eq!(params, json!([1, 2, 3]));
    }

    /// `MrtrRequestParams` is not `PartialEq` (its `InputResponse` payloads wrap
    /// public v1 result types that are not), so "is the default" is asserted
    /// structurally.
    fn assert_is_default(parsed: &MrtrRequestParams) {
        assert!(parsed.input_responses.is_none());
        assert!(parsed.request_state.is_none());
    }

    #[test]
    fn extract_absent_keys_is_the_default() {
        let params = json!({ "name": "search", "arguments": {} });
        assert_is_default(&extract_mrtr_params(&params).unwrap());
    }

    #[test]
    fn extract_non_object_params_is_the_default() {
        assert_is_default(&extract_mrtr_params(&json!(null)).unwrap());
        assert_is_default(&extract_mrtr_params(&json!([1, 2])).unwrap());
    }

    #[test]
    fn extract_rejects_a_non_string_request_state() {
        let err = extract_mrtr_params(&json!({ "requestState": 42 })).unwrap_err();
        assert_eq!(err, MrtrParseError::RequestStateNotAString);
        let err = extract_mrtr_params(&json!({ "requestState": null })).unwrap_err();
        assert_eq!(err, MrtrParseError::RequestStateNotAString);
    }

    #[test]
    fn extract_rejects_an_oversized_request_state() {
        let big = "x".repeat(MAX_REQUEST_STATE_LEN + 1);
        let err = extract_mrtr_params(&json!({ "requestState": big })).unwrap_err();
        assert_eq!(
            err,
            MrtrParseError::RequestStateTooLong {
                len: MAX_REQUEST_STATE_LEN + 1,
                max: MAX_REQUEST_STATE_LEN,
            }
        );
    }

    #[test]
    fn extract_rejects_a_non_object_input_responses() {
        let err = extract_mrtr_params(&json!({ "inputResponses": [] })).unwrap_err();
        assert_eq!(err, MrtrParseError::InputResponsesNotAnObject);
    }

    #[test]
    fn extract_rejects_too_many_input_responses() {
        let mut entries = serde_json::Map::new();
        for index in 0..=MAX_INPUT_RESPONSES {
            entries.insert(format!("k{index}"), elicit_result_value());
        }
        let err = extract_mrtr_params(&json!({ "inputResponses": entries })).unwrap_err();
        assert_eq!(
            err,
            MrtrParseError::TooManyInputResponses {
                count: MAX_INPUT_RESPONSES + 1,
                max: MAX_INPUT_RESPONSES,
            }
        );
    }

    #[test]
    fn extract_rejects_an_oversized_single_input_response() {
        let huge = "y".repeat(MAX_INPUT_RESPONSE_BYTES + 1);
        let params = json!({
            "inputResponses": { "big": { "action": "accept", "content": { "v": huge } } }
        });
        let err = extract_mrtr_params(&params).unwrap_err();
        assert!(matches!(
            err,
            MrtrParseError::InputResponseTooLarge { ref key, .. } if key == "big"
        ));
    }

    #[test]
    fn extract_rejects_an_oversized_input_responses_total() {
        // Each entry stays under the per-entry cap; together they exceed the total.
        let chunk = "z".repeat(MAX_INPUT_RESPONSE_BYTES - 1_000);
        let mut entries = serde_json::Map::new();
        for index in 0..8 {
            entries.insert(
                format!("k{index}"),
                json!({ "action": "accept", "content": { "v": chunk } }),
            );
        }
        let err = extract_mrtr_params(&json!({ "inputResponses": entries })).unwrap_err();
        assert!(matches!(
            err,
            MrtrParseError::InputResponsesTotalTooLarge { .. }
        ));
    }

    #[test]
    fn extract_rejects_an_over_deep_input_response() {
        let mut nested = json!("leaf");
        for _ in 0..(MAX_INPUT_RESPONSE_DEPTH + 4) {
            nested = json!({ "n": nested });
        }
        let params = json!({
            "inputResponses": { "deep": { "action": "accept", "content": { "v": nested } } }
        });
        let err = extract_mrtr_params(&params).unwrap_err();
        assert!(matches!(
            err,
            MrtrParseError::InputResponseTooDeep { ref key, .. } if key == "deep"
        ));
    }

    #[test]
    fn extract_rejects_an_undecodable_input_response() {
        let params = json!({ "inputResponses": { "bad": { "totally": "wrong" } } });
        let err = extract_mrtr_params(&params).unwrap_err();
        assert!(matches!(
            err,
            MrtrParseError::InputResponseUndecodable { ref key } if key == "bad"
        ));
    }

    #[test]
    fn parse_error_display_never_echoes_the_offending_key() {
        let err = MrtrParseError::InputResponseTooLarge {
            key: "secret-key-name".to_string(),
            bytes: 1,
            max: 2,
        };
        let rendered = err.to_string();
        assert!(!rendered.contains("secret-key-name"));
        assert!(rendered.contains('2'));
    }

    #[test]
    fn splice_then_extract_round_trips() {
        let mut params = json!({ "name": "search" });
        let original = MrtrRequestParams {
            input_responses: Some(responses_fixture()),
            request_state: Some("token".to_string()),
        };
        splice_mrtr_params(&mut params, &original);
        let extracted = extract_mrtr_params(&params).unwrap();
        assert_eq!(extracted.request_state.as_deref(), Some("token"));
        assert_eq!(
            extracted.input_responses.as_ref().map(BTreeMap::len),
            Some(1)
        );
    }

    // -----------------------------------------------------------------
    // AAD digest
    // -----------------------------------------------------------------

    #[test]
    fn salient_digest_is_stable_across_key_insertion_order() {
        let mut first = serde_json::Map::new();
        first.insert("name".to_string(), json!("search"));
        first.insert("arguments".to_string(), json!({ "a": 1, "b": 2 }));

        let mut second = serde_json::Map::new();
        second.insert("arguments".to_string(), json!({ "b": 2, "a": 1 }));
        second.insert("name".to_string(), json!("search"));

        assert_eq!(
            salient_param_digest("tools/call", &Value::Object(first)),
            salient_param_digest("tools/call", &Value::Object(second))
        );
    }

    #[test]
    fn salient_digest_differs_on_name_uri_and_arguments() {
        let base = json!({ "name": "search", "arguments": { "q": "a" } });
        let other_name = json!({ "name": "delete", "arguments": { "q": "a" } });
        let other_args = json!({ "name": "search", "arguments": { "q": "b" } });
        assert_ne!(
            salient_param_digest("tools/call", &base),
            salient_param_digest("tools/call", &other_name)
        );
        assert_ne!(
            salient_param_digest("tools/call", &base),
            salient_param_digest("tools/call", &other_args)
        );
        assert_ne!(
            salient_param_digest("resources/read", &json!({ "uri": "mem://a" })),
            salient_param_digest("resources/read", &json!({ "uri": "mem://b" }))
        );
    }

    #[test]
    fn salient_digest_ignores_meta_input_responses_and_request_state() {
        let bare = json!({ "name": "search", "arguments": {} });
        let noisy = json!({
            "name": "search",
            "arguments": {},
            "_meta": { "io.modelcontextprotocol/protocolVersion": "2026-07-28" },
            "inputResponses": { "k": { "action": "accept" } },
            "requestState": "token"
        });
        assert_eq!(
            salient_param_digest("tools/call", &bare),
            salient_param_digest("tools/call", &noisy)
        );
    }

    #[test]
    fn salient_digest_of_an_ineligible_method_is_the_empty_object_digest() {
        assert_eq!(
            salient_param_digest("tools/list", &json!({ "name": "x", "cursor": "y" })),
            salient_param_digest("tools/list", &json!({}))
        );
    }

    #[test]
    fn salient_digest_binds_the_method_name() {
        let params = json!({ "name": "search", "arguments": {} });
        assert_ne!(
            salient_param_digest("tools/call", &params),
            salient_param_digest("prompts/get", &params)
        );
    }

    // -----------------------------------------------------------------
    // Client-facing outcome types
    // -----------------------------------------------------------------

    #[test]
    fn input_required_result_deserializes_the_wire_shape() {
        let parsed: InputRequiredResult = serde_json::from_value(json!({
            "resultType": "input_required",
            "inputRequests": {},
            "requestState": "abc"
        }))
        .unwrap();
        assert!(parsed.is_input_required());
        assert_eq!(parsed.result_type, "input_required");
        assert!(parsed.input_requests.is_some());
        assert_eq!(parsed.request_state.as_deref(), Some("abc"));
        assert_eq!(parsed.raw["resultType"], "input_required");
    }

    #[test]
    fn input_required_result_keeps_the_verbatim_result_in_raw() {
        let parsed: InputRequiredResult = serde_json::from_value(json!({
            "resultType": "input_required",
            "requestState": "abc",
            "vendorField": { "keep": true }
        }))
        .unwrap();
        assert_eq!(parsed.raw["vendorField"]["keep"], true);
        assert!(parsed.input_requests.is_none());
    }

    #[test]
    fn input_required_result_recognises_a_completed_result() {
        let parsed: InputRequiredResult =
            serde_json::from_value(json!({ "resultType": "complete", "content": [] })).unwrap();
        assert!(!parsed.is_input_required());
    }

    #[test]
    fn input_required_result_serializes_the_wire_keys() {
        let parsed: InputRequiredResult = serde_json::from_value(json!({
            "resultType": "input_required",
            "requestState": "abc"
        }))
        .unwrap();
        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["resultType"], "input_required");
        assert_eq!(value["requestState"], "abc");
        assert!(value.get("raw").is_none());
    }

    #[test]
    fn mrtr_outcome_constructs_and_matches() {
        let complete: MrtrOutcome<crate::types::CallToolResult> =
            MrtrOutcome::Complete(crate::types::CallToolResult::new(vec![]));
        assert!(matches!(complete, MrtrOutcome::Complete(_)));
        assert!(complete.complete().is_some());

        let pending: MrtrOutcome<crate::types::CallToolResult> = MrtrOutcome::InputRequired(
            serde_json::from_value(json!({
                "resultType": "input_required",
                "requestState": "abc"
            }))
            .unwrap(),
        );
        assert!(matches!(pending, MrtrOutcome::InputRequired(_)));
        assert!(pending.clone().complete().is_none());
        assert!(pending.input_required().is_some());
    }

    #[test]
    fn mrtr_signal_uses_camel_case_wire_keys() {
        let mut requests = InputRequests::new();
        requests.insert(
            "user_name".to_string(),
            InputRequest::Elicitation(Box::new(form_elicitation())),
        );
        let signal = MrtrSignal {
            input_requests: requests,
            continuation: json!({ "step": 1 }),
        };
        let value = serde_json::to_value(&signal).unwrap();
        assert_eq!(
            value["inputRequests"]["user_name"]["method"],
            "elicitation/create"
        );
        assert_eq!(value["continuation"]["step"], 1);
        assert_eq!(MRTR_SIGNAL_META_KEY, "dev.pmcp/mrtr");
    }

    // -----------------------------------------------------------------
    // Property tests
    // -----------------------------------------------------------------

    use proptest::prelude::*;

    /// A bounded arbitrary JSON value (depth <= 4) — enough to exercise every
    /// branch of the parser without letting proptest build a stack bomb.
    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i32>().prop_map(|n| json!(n)),
            ".{0,16}".prop_map(Value::String),
        ];
        leaf.prop_recursive(4, 32, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
                prop::collection::hash_map("[a-zA-Z]{1,6}", inner, 0..4)
                    .prop_map(|map| { Value::Object(map.into_iter().collect()) }),
            ]
        })
    }

    proptest! {
        #[test]
        fn extract_never_panics_over_arbitrary_json(value in arb_json()) {
            let _ = extract_mrtr_params(&value);
        }

        #[test]
        fn splice_then_extract_is_identity_for_bounded_request_state(
            state in "[ -~]{0,256}"
        ) {
            let mut params = json!({ "name": "search" });
            let original = MrtrRequestParams {
                input_responses: None,
                request_state: Some(state.clone()),
            };
            splice_mrtr_params(&mut params, &original);
            let extracted = extract_mrtr_params(&params).unwrap();
            prop_assert_eq!(extracted.request_state, Some(state));
        }

        #[test]
        fn default_splice_leaves_no_mrtr_key(value in arb_json()) {
            let mut params = if value.is_object() {
                value
            } else {
                json!({ "wrapped": value })
            };
            splice_mrtr_params(&mut params, &MrtrRequestParams::default());
            prop_assert!(params.get("inputResponses").is_none());
            prop_assert!(params.get("requestState").is_none());
        }

        #[test]
        fn header_value_codec_round_trips(value in ".{0,64}") {
            let encoded = encode_header_value(&value);
            prop_assert_eq!(decode_header_value(&encoded), Some(value));
        }

        #[test]
        fn decode_header_value_never_panics(raw in ".{0,128}") {
            let _ = decode_header_value(&raw);
        }

        #[test]
        fn salient_digest_never_panics(value in arb_json(), method in "[a-z/]{0,20}") {
            let _ = salient_param_digest(&method, &value);
        }
    }
}
