//! Client-side caching hints for the MCP `2026-07-28` `CacheableResult` base.
//!
//! This module carries the MCP `2026-07-28` `CacheableResult` vocabulary: the
//! `ttlMs` freshness hint and the `cacheScope` sharing scope, plus the single
//! projection point that decides whether either key reaches the wire.
//!
//! # What carries these hints
//!
//! Exactly six results extend `CacheableResult` in the vendored
//! `2026-07-28` schema — `DiscoverResult`, `ListToolsResult`,
//! `ListResourcesResult`, `ListResourceTemplatesResult`, `ReadResourceResult`
//! and `ListPromptsResult`. Each of the corresponding Rust types carries an
//! `Option`-typed `ttl_ms` / `cache_scope` slot a handler MAY set.
//!
//! # v2 only (D-11)
//!
//! The hints exist on the **v2 projection only**. A `2025-11-25` (or earlier)
//! response never carries them: [`project_caching_hints`] REMOVES both keys on
//! every non-v2 input, so a handler that sets a hint and then serves a legacy
//! client emits a byte-identical legacy response. That severability is what
//! keeps the v1 compatibility layer cleanly removable.
//!
//! # Handler-set, SDK-defaulted (D-08 / D-12)
//!
//! The values are chosen by the handler and defaulted by the SDK at ONE shared
//! projection point. A handler that expresses no preference gets the safe
//! defaults — [`DEFAULT_TTL_MS`] and [`CacheScope::default()`] — injected on the
//! v2 wire, where both keys are REQUIRED.
//!
//! # `ttlMs` here is NOT a task TTL (D-10)
//!
//! The `ttlMs` in this module is a CACHE-FRESHNESS hint: how long a client may
//! reuse a response body. It is **not**
//! [`TaskV2::ttl_ms`](crate::types::tasks::TaskV2::ttl_ms), which is a task
//! LIFETIME: how long the server retains a task record. The two live in
//! deliberately separate modules (`types::caching` versus `types::tasks`) and
//! neither imports the other. Copying a long task lifetime into a cache hint
//! would make stale data look fresh.
//!
//! # Why this module carries no `cfg`
//!
//! This module is deliberately `cfg`-free, so it compiles on every target and
//! [`project_caching_hints`] is callable from ALL dispatchers: the native
//! `ServerCore` / `Server` paths (`src/server/core.rs` and `src/server/mod.rs`,
//! both gated `cfg(not(target_arch = "wasm32"))`) AND `WasmMcpServer`
//! (`src/server/wasm_server.rs`, gated `cfg(target_arch = "wasm32")`). Those two
//! `cfg` sets are disjoint, so a projector living in either one would be
//! structurally unreachable from the other — and the wasm dispatcher
//! serializes handler-constructed `ReadResourceResult` / `ListResourcesResult`
//! values directly, which is exactly the path a hint could leak onto a v1 wire.
//! Do not "simplify" this back into a server module.

use serde::{Deserialize, Serialize};

/// The intended sharing scope of a cached response.
///
/// Analogous to HTTP `Cache-Control: public` versus `Cache-Control: private`.
///
/// # Security
///
/// The MCP `2026-07-28` schema defines the two values as follows (quoted
/// verbatim from `schema/vendored/core-2026-07-28/schema.ts`):
///
/// > - `"public"`: The response does not contain user-specific data. Any
/// >   client or intermediary (e.g., shared gateway, caching proxy) MAY cache
/// >   the response and serve it across authorization contexts.
/// > - `"private"`: The response MAY be cached and reused only within the
/// >   same authorization context. Caches MUST NOT be shared across
/// >   authorization contexts (e.g., a different access token requires a different cache).
///
/// The consequence, in our own words: marking a per-user response
/// [`CacheScope::Public`] is a cross-authorization-context data leak. A shared
/// gateway is then entitled to serve one caller's response body to a different
/// caller holding a different access token, and the server has told it that is
/// allowed. When in doubt use [`CacheScope::Private`].
///
/// # Why `Private` is the SDK default
///
/// [`CacheScope::default()`] is [`CacheScope::Private`] (D-08). Defaulting to
/// `Public` would make every response nobody explicitly considered
/// cross-caller cacheable — a leak by omission rather than by decision. This is
/// the same defect class the tasks surface's own privacy rules exist to
/// prevent: the safe value is the one you get for free.
///
/// # Why this enum is NOT `#[non_exhaustive]`
///
/// The published `2026-07-28` schema declares the property as a CLOSED union of
/// exactly two values (`$defs.CacheableResult.properties.cacheScope.enum` is
/// `["private", "public"]`). Marking the enum `#[non_exhaustive]` would force
/// every downstream `match` to carry an unreachable catch-all arm for a variant
/// set the spec fixes. The closedness is deliberate and is fenced by a test
/// asserting that an unknown value FAILS to deserialize — do not add
/// `#[serde(other)]` or a catch-all variant.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::CacheScope;
///
/// // The SDK default is the safe one.
/// assert_eq!(CacheScope::default(), CacheScope::Private);
///
/// // The wire spellings are lowercase, as the schema's enum declares.
/// assert_eq!(
///     serde_json::to_string(&CacheScope::Public).unwrap(),
///     "\"public\""
/// );
/// assert_eq!(
///     serde_json::to_string(&CacheScope::Private).unwrap(),
///     "\"private\""
/// );
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    /// > `"public"`: The response does not contain user-specific data. Any
    /// > client or intermediary (e.g., shared gateway, caching proxy) MAY cache
    /// > the response and serve it across authorization contexts.
    ///
    /// Only assert this for a response whose body is identical for every
    /// caller regardless of identity, token or tenant.
    Public,
    /// > `"private"`: The response MAY be cached and reused only within the
    /// > same authorization context. Caches MUST NOT be shared across
    /// > authorization contexts (e.g., a different access token requires a different cache).
    ///
    /// The SDK default, because it is the value that cannot leak.
    #[default]
    Private,
}

/// The SDK-supplied default for `ttlMs` when a handler expresses no preference.
///
/// The value is `0`, which the MCP `2026-07-28` schema documents as:
///
/// > - If 0, The response SHOULD be considered immediately stale,
/// >   The client MAY re-fetch every time the result is needed.
///
/// That is precisely why `0` is the right default: it asserts NOTHING about
/// cacheability. A conformant peer receiving it behaves exactly as it would
/// have without the field, so the SDK-supplied default is inert while still
/// satisfying the v2 wire's requirement that the key be present.
///
/// # Why `u64`
///
/// This is a MEASURED mapping, not an inference. The TypeScript source spells
/// the field `ttlMs: number`, which would admit fractions — but the GENERATED
/// JSON Schema that a conformant peer actually validates against declares
/// `$defs.CacheableResult.properties.ttlMs` as
/// `{"type": "integer", "minimum": 0}`. Integrality and non-negativity are
/// therefore contract, and `u64` is exact across the whole declared domain
/// except for the absent upper bound. At millisecond resolution `u64::MAX` is
/// roughly 584 million years, so that residual is an ACCEPTED risk rather than
/// a reason to reach for a bignum type.
///
/// `tests/v2_core_schema_facts.rs` asserts the schema side of this and the
/// `cacheable_result_serde_locks` module below asserts the Rust side: if a
/// re-vendoring ever widens the declared type to `"number"`, those tests fail
/// and the Rust representation must change with them.
pub const DEFAULT_TTL_MS: u64 = 0;

/// Whether a given result is one of the six that extend `CacheableResult`.
///
/// The `2026-07-28` schema gives exactly six results a `CacheableResult` base:
/// `DiscoverResult`, `ListToolsResult`, `ListResourcesResult`,
/// `ListResourceTemplatesResult`, `ReadResourceResult` and `ListPromptsResult`.
/// Everything else — `tools/call`, `prompts/get`, every task method, every
/// notification acknowledgement — is [`Cacheable::No`].
///
/// The value is decided by the CALLER rather than derived inside the projector,
/// because at the native chokepoint where the projection runs the request has
/// already been moved and the response is an opaque `serde_json::Value`. The
/// classifier that produces it is a separate shared function
/// (`request_is_cacheable`), so the two native dispatchers cannot drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Why `allow(dead_code)`: this plan (115-05) adds the projector and its
// classification input; 115-06 wires them into the three dispatchers. Until
// then the only non-test constructor is the test module below.
#[allow(dead_code)]
pub(crate) enum Cacheable {
    /// The result extends `CacheableResult` and participates in the projection.
    Yes,
    /// The result does not extend `CacheableResult`; the projection is a no-op.
    No,
}

/// Project the `2026-07-28` caching hints onto (or off) a serialized result.
///
/// This is the ONLY writer of the `ttlMs` and `cacheScope` wire keys in the
/// tree (D-12 — a single shared projection point; fenced structurally by a
/// source tripwire). It is deliberately total: every input either ensures both
/// keys or removes both keys, so there is no path that leaves a result half
/// projected.
///
/// # Behaviour
///
/// - [`Cacheable::No`] — returns immediately, touching nothing (D-07: only the
///   six `CacheableResult` extenders carry these keys).
/// - A non-object `value` — returns; a scalar, array or null result body cannot
///   carry a key.
/// - `Some(Era::V2)` — ENSURES both keys, without overwriting: a handler-set
///   value survives verbatim and an unset one receives the safe defaults
///   [`DEFAULT_TTL_MS`] and [`CacheScope::default()`] (D-08). The default scope
///   is produced by SERIALIZING the enum, never by typing a string literal, so
///   the injected default and the enum cannot drift apart.
/// - Anything else — `Some(Era::V1)` **or** `None` — REMOVES both keys if
///   present (D-11). This is not merely "don't add": it is an active strip, so
///   a handler that deliberately set a hint and then served a legacy client
///   still emits a byte-identical legacy response. A strip is normal operation,
///   not an error, and is not logged.
///
/// # Why the `None` arm matters
///
/// `WasmMcpServer` (`src/server/wasm_server.rs`) has no era awareness at all —
/// it carries no `ProtocolContext` — so it passes `None`. Its `WasmResource`
/// handlers construct `ReadResourceResult` / `ListResourcesResult` values that
/// the file serializes directly, which without this arm would put a
/// handler-set hint straight onto the wasm server's v1 wire. The `None` arm is
/// what makes that leak structurally impossible, and it is why this function
/// lives in a `cfg`-free module rather than in either server module.
// Why `allow(dead_code)`: 115-06 wires this into `inject_v2_result_envelope`
// and into the wasm serialization path; until then the callers are the unit
// tests below.
#[allow(dead_code)]
pub(crate) fn project_caching_hints(
    value: &mut serde_json::Value,
    era: Option<crate::types::protocol::Era>,
    cacheable: Cacheable,
) {
    if matches!(cacheable, Cacheable::No) {
        return;
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if matches!(era, Some(crate::types::protocol::Era::V2)) {
        object
            .entry("ttlMs")
            .or_insert_with(|| serde_json::Value::from(DEFAULT_TTL_MS));
        object.entry("cacheScope").or_insert_with(|| {
            serde_json::to_value(CacheScope::default()).expect("a unit enum always serializes")
        });
    } else {
        object.remove("ttlMs");
        object.remove("cacheScope");
    }
}

/// Unit coverage for [`project_caching_hints`] (115-05, SCHM-03).
#[cfg(test)]
mod projection_tests {
    use super::{project_caching_hints, CacheScope, Cacheable, DEFAULT_TTL_MS};
    use crate::types::protocol::Era;
    use serde_json::json;

    #[test]
    fn v2_inserts_the_safe_defaults() {
        let mut value = json!({ "tools": [] });
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
        assert_eq!(
            value["ttlMs"],
            json!(DEFAULT_TTL_MS),
            "a v2 projection must carry the required `ttlMs`, got {value}"
        );
        assert_eq!(
            value["cacheScope"],
            json!("private"),
            "an un-considered response must default to the non-leaking scope, got {value}"
        );
        assert_eq!(value["tools"], json!([]), "existing keys must be untouched");
    }

    #[test]
    fn v2_preserves_handler_set_values() {
        let mut value = json!({ "ttlMs": 300_000, "cacheScope": "public" });
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
        assert_eq!(
            value["ttlMs"],
            json!(300_000),
            "a handler-set ttlMs must survive the projection verbatim, got {value}"
        );
        assert_eq!(
            value["cacheScope"],
            json!("public"),
            "a handler-set cacheScope must survive the projection verbatim, got {value}"
        );
    }

    #[test]
    fn v1_strips_handler_set_values() {
        let mut value = json!({ "resources": [], "ttlMs": 300_000, "cacheScope": "public" });
        project_caching_hints(&mut value, Some(Era::V1), Cacheable::Yes);
        assert!(
            value.get("ttlMs").is_none(),
            "D-11: a v1 response must never carry `ttlMs`, got {value}"
        );
        assert!(
            value.get("cacheScope").is_none(),
            "D-11: a v1 response must never carry `cacheScope`, got {value}"
        );
        assert_eq!(
            value["resources"],
            json!([]),
            "the strip must not disturb any other key"
        );
    }

    /// The `era = None` path, which is exactly what `WasmMcpServer` passes.
    ///
    /// `src/server/wasm_server.rs` carries no `ProtocolContext`, so it can only
    /// ever pass `None`; its `WasmResource` handlers construct results that the
    /// file serializes directly. That file is compiled ONLY for `wasm32`, and
    /// its own `cfg(all(test, target_arch = "wasm32"))` test module does not
    /// compile at all today, so this NATIVE unit test is the only RUNNABLE
    /// proof that the wasm dispatcher's era-less input strips rather than
    /// leaks. The compile-time proof is `make wasm-build`; the structural proof
    /// is the source tripwire added by 115-08.
    #[test]
    fn no_context_strips_both_keys_which_is_the_wasm_path() {
        let mut value = json!({
            "contents": [],
            "ttlMs": 300_000,
            "cacheScope": "public",
            "_meta": { "keep": true }
        });
        project_caching_hints(&mut value, None, Cacheable::Yes);
        assert!(
            value.get("ttlMs").is_none(),
            "an era-less dispatcher must strip `ttlMs`, got {value}"
        );
        assert!(
            value.get("cacheScope").is_none(),
            "an era-less dispatcher must strip `cacheScope`, got {value}"
        );
        assert_eq!(
            value["contents"],
            json!([]),
            "every other key must be untouched by the strip"
        );
        assert_eq!(
            value["_meta"],
            json!({ "keep": true }),
            "every other key must be untouched by the strip"
        );
    }

    #[test]
    fn not_cacheable_is_the_identity() {
        let before = json!({ "content": [], "ttlMs": 5, "cacheScope": "public" });
        let mut value = before.clone();
        project_caching_hints(&mut value, Some(Era::V2), Cacheable::No);
        assert_eq!(
            value, before,
            "a non-CacheableResult body must not be touched at all"
        );

        let mut value = before.clone();
        project_caching_hints(&mut value, Some(Era::V1), Cacheable::No);
        assert_eq!(
            value, before,
            "the identity must hold on every era, not just v2"
        );
    }

    #[test]
    fn a_non_object_value_is_untouched() {
        for mut value in [json!(null), json!([1, 2, 3]), json!("a string"), json!(7)] {
            let before = value.clone();
            project_caching_hints(&mut value, Some(Era::V2), Cacheable::Yes);
            assert_eq!(
                value, before,
                "a non-object result body cannot carry a key and must be left alone"
            );
        }
        // The scope enum is still the safe one; nothing above may mutate it.
        assert_eq!(CacheScope::default(), CacheScope::Private);
    }
}
