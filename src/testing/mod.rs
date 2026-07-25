//! Conformance helpers for proving the `tasks/*` wire surface round-trips
//! through the SDK's real client deserialization types.
//!
//! **Why this module exists.** The tools-as-tasks incident (Phase 101) was
//! caused by hand-written `tasks/*` JSON diverging from the typed structs the
//! client deserializes into. A regression test fed the helper an
//! *author-written fixture* and passed green while the live wire path failed.
//! The lesson, adopted as an acceptance gate: for protocol-shape requirements,
//! "resolved" is gated on feeding the **actual server dispatch output** through
//! the real client type, never a hand-built value.
//!
//! [`assert_roundtrips_through_client`] is the primitive that enforces this. It
//! is feature-gated behind `testing` (folded into the `full` feature set) so it
//! is available to the integration tests / examples and the quality gate, but
//! omitted from lean default release builds.

use serde::de::DeserializeOwned;

/// Assert that real server dispatch output deserializes into the client type
/// `T`, panicking with a diagnostic message if it does not.
///
/// Feed this the `serde_json::Value` carried by an actual
/// `ResponsePayload::Result(..)` produced by
/// [`ServerCore::handle_request`](crate::server::core::ProtocolHandler::handle_request)
/// — **never** an author-written fixture. If `real_dispatch_output` does not
/// deserialize into `T`, the call panics with a message naming `T`, the serde
/// error, and the pretty-printed offending output.
///
/// # Type Parameters
///
/// - `T`: a client-facing wire type such as
///   [`GetTaskResult`](crate::types::tasks::GetTaskResult),
///   [`CallToolResult`](crate::types::CallToolResult), or
///   [`CreateTaskResult`](crate::types::tasks::CreateTaskResult). Because serde
///   ignores unknown fields by default, the extra `_meta` carried by the
///   create envelope deserializes cleanly into `CreateTaskResult`.
///
/// # Panics
///
/// Panics if `real_dispatch_output` cannot be deserialized into `T`. This is
/// the intended behavior in a test context: a deliberately-wrong wire shape
/// (e.g. a flat `Task` where a `{ "task": ... }` wrapper is expected) makes the
/// helper fail loudly.
///
/// # Examples
///
/// ```rust
/// use pmcp::testing::assert_roundtrips_through_client;
/// use pmcp::types::tasks::{GetTaskResult, Task, TaskStatus};
///
/// // A correctly-shaped `tasks/get` payload wraps the task under `task`.
/// let task = Task::new("t-1", TaskStatus::Working)
///     .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z");
/// let dispatch_output = serde_json::to_value(GetTaskResult::new(task)).unwrap();
///
/// // Deserializes cleanly into the client type — returns normally.
/// assert_roundtrips_through_client::<GetTaskResult>(dispatch_output);
/// ```
pub fn assert_roundtrips_through_client<T>(real_dispatch_output: serde_json::Value)
where
    T: DeserializeOwned,
{
    // Pre-render the diagnostic before moving the owned value into `from_value`
    // (the value is consumed by the deserialize, so it is genuinely owned — not
    // merely borrowed).
    let pretty = serde_json::to_string_pretty(&real_dispatch_output)
        .unwrap_or_else(|_| real_dispatch_output.to_string());
    if let Err(error) = serde_json::from_value::<T>(real_dispatch_output) {
        panic!(
            "dispatch output does not deserialize into `{}`: {error}\noffending output was:\n{pretty}",
            std::any::type_name::<T>(),
        );
    }
}

/// Reserved `_meta` key carrying the per-request protocol version.
///
/// Re-exported so tests read the crate's constant instead of re-spelling the
/// `io.modelcontextprotocol/*` string and silently drifting from it.
pub const META_PROTOCOL_VERSION: &str =
    crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;

/// Reserved `_meta` key carrying the per-request client identity.
pub const META_CLIENT_INFO: &str = crate::types::protocol::context::RESERVED_CLIENT_INFO_KEY;

/// Reserved `_meta` key carrying the per-request client capabilities.
pub const META_CLIENT_CAPABILITIES: &str =
    crate::types::protocol::context::RESERVED_CLIENT_CAPABILITIES_KEY;

/// The opening marker of the `Mcp-Name` base64 sentinel form.
pub const HEADER_SENTINEL_PREFIX: &str = crate::types::mrtr::HEADER_SENTINEL_PREFIX;

/// The closing marker of the `Mcp-Name` base64 sentinel form.
pub const HEADER_SENTINEL_SUFFIX: &str = crate::types::mrtr::HEADER_SENTINEL_SUFFIX;

/// Encode a value for the `Mcp-Name` header, using the PRODUCTION codec.
///
/// **Why this wrapper exists.** The codec in `crate::types::mrtr` is `pub(crate)`
/// (Phase-113 D-10 keeps the MRTR plumbing off the public API), so the v2 test
/// harness previously carried a hand-copied mirror of it — and the mirror had
/// already drifted, omitting the `MAX_HEADER_VALUE_LEN` term from its passthrough
/// predicate. That meant the harness could emit a raw oversized header the real
/// encoder would have sentinel-encoded, so every test built on it was validating
/// the harness against itself rather than against the shipped encoder.
///
/// A `pub use` of a `pub(crate)` item does not compile (E0365), so this thin
/// wrapper is the seam. Six Phase-113 plans build their requests through it.
#[must_use]
pub fn encode_mcp_name(value: &str) -> String {
    crate::types::mrtr::encode_header_value(value)
}

/// Decode an `Mcp-Name` header value, using the PRODUCTION codec.
///
/// Returns `None` for a malformed sentinel or an over-long value. See
/// [`encode_mcp_name`] for why this wrapper exists.
#[must_use]
pub fn decode_mcp_name(raw: &str) -> Option<String> {
    crate::types::mrtr::decode_header_value(raw)
}

/// Mint a `requestState` continuation token with the PRODUCTION codec
/// (Phase 113, HTTP-02).
///
/// **Why this wrapper exists.** `RequestStateCodec` is `pub(crate)` (Phase-113
/// D-10 keeps the MRTR plumbing off the public API), so an integration test
/// cannot construct one — and a test that hand-rolled the token layout would be
/// validating itself rather than the shipped codec. This is the same one-hop
/// seam [`encode_mcp_name`] provides for the `Mcp-Name` codec.
///
/// `key` is the SAME 32 bytes the server under test was built with via
/// [`ServerBuilder::with_request_state_key`](crate::ServerBuilder::with_request_state_key),
/// so tests configure the key through the builder and never mutate
/// `PMCP_REQUEST_STATE_KEY` (which is process-global and therefore
/// order-dependent under parallel test threads).
///
/// `params` MUST be the params DISPATCH derives from the typed request, because
/// that is what the AEAD binds to:
///
/// | method | params |
/// |--------|--------|
/// | `tools/call` | `{"name": …, "arguments": …}` |
/// | `prompts/get` | `{"name": …, "arguments": …}` |
/// | `resources/read` | `{"uri": …}` |
///
/// A `ttl` of zero mints an ALREADY-EXPIRED token (`exp == now`), which is how a
/// test exercises the expiry verdict deterministically instead of sleeping.
///
/// Returns `None` only if the codec refuses the key or cannot seal the state.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[must_use]
pub fn mint_request_state(
    key: &[u8; 32],
    ttl: std::time::Duration,
    principal: &str,
    method: &str,
    params: &serde_json::Value,
    state: &serde_json::Value,
    round: u8,
) -> Option<String> {
    let codec = crate::server::request_state::RequestStateCodec::new(key, ttl).ok()?;
    let binding =
        crate::server::request_state::RequestBinding::from_request(principal, method, params);
    codec.mint(state, &binding, round).ok()
}

/// Open a `requestState` token with the PRODUCTION codec, returning
/// `(continuation state, round)`.
///
/// The inverse of [`mint_request_state`], and the only way an integration test
/// can assert that a token the SERVER minted carries the round it should — the
/// D-15 expiry path must PRESERVE the round rather than reset it (T-113-49).
///
/// Returns `None` for any verdict other than "authentic and live": an
/// unknown key, a failed tag check, or an expired token all yield `None`.
/// `params` follows the same rule as [`mint_request_state`].
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
#[must_use]
pub fn open_request_state(
    key: &[u8; 32],
    principal: &str,
    method: &str,
    params: &serde_json::Value,
    token: &str,
) -> Option<(serde_json::Value, u8)> {
    let codec = crate::server::request_state::RequestStateCodec::new(
        key,
        std::time::Duration::from_secs(1),
    )
    .ok()?;
    let binding =
        crate::server::request_state::RequestBinding::from_request(principal, method, params);
    match codec.verify(token, &binding) {
        crate::server::request_state::Verdict::Ok(continuation) => {
            Some((continuation.state, continuation.round))
        },
        _ => None,
    }
}

/// The principal a server with NO auth provider binds continuations to.
///
/// Re-exported so a test spells the anonymous principal through the crate's own
/// constant instead of hard-coding the empty string.
#[cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]
pub const ANONYMOUS_PRINCIPAL: &str = crate::server::core::ANONYMOUS_PRINCIPAL;

#[cfg(test)]
mod tests {
    use super::assert_roundtrips_through_client;
    use crate::types::tasks::{CreateTaskResult, GetTaskResult, Task, TaskStatus};

    fn sample_task() -> Task {
        Task::new("t-positive", TaskStatus::Working)
            .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z")
    }

    #[test]
    fn passes_on_valid_get_task_result() {
        let value = serde_json::to_value(GetTaskResult::new(sample_task())).unwrap();
        assert_roundtrips_through_client::<GetTaskResult>(value);
    }

    #[test]
    fn passes_on_valid_create_task_result() {
        // The create envelope serializes as `{ "task": { .. } }`; serde ignores
        // any extra `_meta` the live dispatch envelope also carries.
        let mut value = serde_json::to_value(CreateTaskResult::new(sample_task())).unwrap();
        value.as_object_mut().unwrap().insert(
            "_meta".to_string(),
            serde_json::json!({ "io.modelcontextprotocol/related-task": { "taskId": "t-positive" } }),
        );
        assert_roundtrips_through_client::<CreateTaskResult>(value);
    }

    #[test]
    #[should_panic(expected = "does not deserialize into")]
    fn panics_on_exact_historical_flat_task_shape() {
        // The EXACT historical bug: a serialized `Task` with top-level `taskId`
        // / `status`, NOT wrapped in `{ "task": ... }`. Feeding this into
        // `GetTaskResult` (which requires the `task` wrapper) must panic.
        let flat_task = serde_json::to_value(Task::new("t-1", TaskStatus::Working)).unwrap();
        // Sanity: the flat shape really does have a top-level `taskId`.
        assert!(flat_task.get("taskId").is_some());
        assert!(flat_task.get("task").is_none());
        assert_roundtrips_through_client::<GetTaskResult>(flat_task);
    }
}
