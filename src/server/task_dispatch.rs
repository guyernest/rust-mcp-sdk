//! Shared task-lifecycle dispatch unit used by BOTH `Server` and `ServerCore`.
//!
//! Phase 101 landed the complete `tasks/*` lifecycle on `ServerCore` /
//! `ServerCoreBuilder` only. Phase 102 extracts that machinery into ONE shared
//! place (this module — research Option A) so the high-level `Server` / HTTP
//! dispatcher can serve the same lifecycle without re-implementing it (drift).
//!
//! This module hosts:
//! - [`apply_tasks_capability_rule`] — the endpoint-backed `tasks`-capability
//!   rule, a free function over explicit params (the two builders hold
//!   `tool_infos` at different lifecycle points, so it cannot be a method).
//! - [`default_tasks_capability`] — the FROZEN advertised `ServerTasksCapability`
//!   shape (do not re-derive its JSON).
//! - [`TaskDispatch`] — a borrow-struct over `(&task_store, &task_router)` that
//!   owns owner-resolution, the create-path response (with the self-enforcing
//!   create gate), `tasks/result` precedence, and `tasks/get|list|cancel`
//!   routing.
//! - [`success_response`] / [`error_response`] — the SINGLE-SOURCE JSON-RPC
//!   envelope builders (`ServerCore` delegates to these; there is exactly one
//!   copy of the wrapping logic).
//!
//! The ENTIRE module is gated `#[cfg(not(target_arch = "wasm32"))]` because every
//! task item is non-wasm (mirrors `ServerCore`'s task fields/methods).

#![cfg(not(target_arch = "wasm32"))]
// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution (mirrors intent, keeps the API surface crate-private).
#![allow(clippy::redundant_pub_crate)]

use crate::error::{Error, Result};
use crate::server::auth::AuthContext;
use crate::server::task_store::TaskStore;
use crate::server::tasks::TaskRouter;
use crate::types::capabilities::{
    ServerCapabilities, ServerTasksCapability, TasksExtensionCapability, TASKS_EXTENSION_KEY,
};
use crate::types::jsonrpc::ResponsePayload;
use crate::types::tasks::{TaskStatus, RELATED_TASK_META_KEY};
use crate::types::tools::TaskSupport;
use crate::types::{
    CallToolResult, ClientRequest, Content, JSONRPCError, JSONRPCResponse, RequestId, ToolInfo,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// `tasks/list` — RETIRED on protocol version 2026-07-28.
///
/// Spelled here rather than in `crate::types::mrtr`: that module's method
/// constants exist to key the routing-NAME table, and a retired method has no
/// row there. This constant exists so an era gate and its refusal message
/// cannot disagree about the spelling.
const TASKS_LIST_METHOD: &str = "tasks/list";

/// `tasks/result` — RETIRED on protocol version 2026-07-28.
///
/// See [`TASKS_LIST_METHOD`].
const TASKS_RESULT_METHOD: &str = "tasks/result";

/// The `-32601` message body for a `tasks/*` method protocol version 2026-07-28
/// RETIRED.
///
/// Emitted as `format!("{method} {V2_TASKS_METHOD_RETIRED}")` so the caller is
/// told WHICH method it asked for as well as why the answer is
/// `METHOD_NOT_FOUND`; the two gates share one builder (`retired_on_v2`) so
/// they cannot drift into two different sentences for one condition.
///
/// # Provenance
///
/// The vendored draft extension schema — `schema/vendored/ext-tasks/schema.ts`
/// at the commit pinned by `schema/vendored/ext-tasks/PROVENANCE.md` (plan
/// 114-01) — declares exactly THREE `tasks/*` request methods: `tasks/get`,
/// `tasks/update` and `tasks/cancel`. `tasks/list` and `tasks/result` are
/// ABSENT from it. They are not "unimplemented here"; they do not exist on that
/// protocol version:
///
/// * `tasks/list` was removed as a SECURITY improvement — with no enumeration
///   primitive a server cannot inadvertently leak the existence of one caller's
///   tasks to another. TASK-03 and TASK-05 are that one improvement seen from
///   two angles.
/// * `tasks/result` was removed because the v2 `tasks/get` inlines `result` /
///   `error` on the terminal task, so a second round trip has nothing left to
///   do (plan 114-11).
///
/// # Why this constant REPLACED `V2_TASKS_NOT_NEGOTIATED`
///
/// Until this plan the v2 `tasks/result` refusal read "the tasks extension is
/// not negotiated". That sentence was true only while pmcp advertised no entry
/// under [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY).
/// Plan 114-05 made [`apply_tasks_capability_rule`] advertise it on every
/// backend-configured server, at which point the message told the caller to fix
/// a negotiation that had already succeeded. A refusal message is the ONLY
/// signal a caller has for choosing its next move, so an untruthful one makes
/// the correct fix undiscoverable (T-114-33).
///
/// The old constant is GONE rather than reworded-and-kept: a second, unreachable
/// spelling of "no" is how two plans come to disagree about one wire string.
/// And there is no "the client did not declare the extension" refusal in the
/// tree today — 114-05 landed the server-side ADVERTISEMENT and 114-06 the
/// CLIENT-side refusal, but no server-side negotiation gate exists, so a
/// constant for that condition would have no emission site. Whichever plan
/// lands that gate mints its own message then.
///
/// # The three `-32601` conditions this module answers, kept distinguishable
///
/// | condition | message | when |
/// |-----------|---------|------|
/// | RETIRED | this constant, prefixed by the method | era is v2 AND a task backend exists |
/// | NO BACKEND | `"Tasks not enabled"` / `"tasks/result not supported"` | neither a `TaskStore` nor a `TaskRouter`, on ANY era |
/// | NOT A `tasks/*` METHOD | `"Method not supported"` | the wildcard arm of `route_tasks_endpoint` |
///
/// Distinguishability is the mitigation, not a nicety: "this method was
/// retired" and "this server serves no tasks at all" call for opposite fixes.
pub(crate) const V2_TASKS_METHOD_RETIRED: &str =
    "is not a method of the tasks extension on protocol version 2026-07-28: the extension \
     declares only tasks/get, tasks/update and tasks/cancel";

/// The owner a v1 task request from an UNAUTHENTICATED caller is bound to —
/// FROZEN.
///
/// A shared bucket: every unauthenticated caller on the server lands in it, and
/// `tests/v1_tasks_golden.rs` pins the resulting wire bytes. Spelled once so the
/// binding and the D-10 migration warn that names it cannot disagree.
///
/// It is a DIFFERENT key from v2's
/// [`ANONYMOUS_PRINCIPAL`](crate::server::core::ANONYMOUS_PRINCIPAL) (`""`).
/// `GenericTaskStore::is_anonymous_owner` treats the two IDENTICALLY for the
/// `allow_anonymous` decision, but `make_key` prefixes every record by owner, so
/// they are DISJOINT key spaces: a task created on v1 by an unauthenticated
/// caller is not reachable by an unauthenticated v2 caller on the same
/// no-auth-provider server, and vice versa. Those two facts are easy to
/// conflate; they are separate.
const V1_UNAUTHENTICATED_OWNER: &str = "local";

/// The FROZEN `-32601` message for a `tasks/*` method on a server with no task
/// backend at all.
///
/// Spelled once because two sites emit it: the per-endpoint handlers'
/// no-backend `else` arms. It is deliberately DIFFERENT from
/// [`V2_TASKS_METHOD_RETIRED`] — "this method was retired" and "this server
/// serves no tasks at all" call for opposite fixes (T-114-33).
const TASKS_NOT_ENABLED: &str = "Tasks not enabled";

/// The FROZEN `-32601` message `tasks/result` uses for the same no-backend
/// condition [`TASKS_NOT_ENABLED`] covers for the other three methods.
///
/// Deliberately a different sentence from its three siblings: `tests/…` and
/// `the_minus_32601_conditions_are_mutually_distinct` assert all four refusals
/// pairwise distinct, so a caller can always tell which one it hit.
const TASKS_RESULT_NOT_SUPPORTED: &str = "tasks/result not supported";

/// The `-32601` message for a request that is not a `tasks/*` method at all.
const NOT_A_TASKS_METHOD: &str = "Method not supported";

/// Build the `-32601` a v2 caller receives for a RETIRED `tasks/*` method.
///
/// The SINGLE builder both era gates use, so `tasks/list` and `tasks/result`
/// answer one condition with one sentence.
fn retired_on_v2(id: RequestId, method: &str) -> JSONRPCResponse {
    error_response(
        id,
        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
        format!("{method} {V2_TASKS_METHOD_RETIRED}"),
    )
}

/// Does this request run under the v1 task lifecycle?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | the v1 task lifecycle is untouched |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | the v2 task surface is not implemented (TASK-03) |
///
/// The v2 row deliberately no longer says "and not negotiated": since 114-05 a
/// tasks-backed server DOES advertise the tasks extension, so the reason this
/// predicate routes v2 away from the `-32002` refusal is the missing v2
/// semantics, not a missing capability entry.
///
/// # Why this predicate exists (Finding 11)
///
/// The `tasks/result` pending refusal emits
/// [`V1_TASK_PENDING`](crate::types::protocol::error_codes::V1_TASK_PENDING)
/// (`-32002`), which protocol version 2026-07-28 **MUST NOT** emit
/// (`docs/specification/draft/basic/index.mdx` § Error Codes). That site *looked*
/// v1-scoped — this module contains no era gating at all — but
/// `tests/v2_prohibited_error_codes.rs` drove a real v2 HTTP `tasks/result` at it
/// and read `-32002` off the response. It is reachable because the HTTP ingress
/// resolves the era from `params._meta` on the RAW body, so a `tasks/result`
/// arrives classified v2 even though the typed
/// [`GetTaskPayloadRequest`](crate::types::tasks::GetTaskPayloadRequest) has no
/// `_meta` field for it to ride on.
///
/// # What this predicate gates, and what it deliberately does NOT
///
/// It is the ONE era definition this module has, and three things now read it:
/// the `-32002` pending emission, [`tasks_list_serves_on_era`] and
/// [`tasks_result_serves_on_era`] — the last two because plan 114-08 RETIRED
/// both of those methods on v2 (see [`V2_TASKS_METHOD_RETIRED`]).
///
/// It does NOT retire `tasks/get` or `tasks/cancel`: both still serve on BOTH
/// eras, because both survive in the v2 extension schema. Their v2 response
/// SHAPE changes (plan 114-11 flattens the result and remaps not-found), but a
/// shape change is not a retirement and this predicate must not be widened into
/// one.
///
/// This block previously claimed the predicate gated only the `-32002` emission
/// and that `tasks/list` was unchanged on every era. Both sentences were
/// falsified by plan 114-08 and are rewritten in the same commit that falsified
/// them: a stale "deliberately does NOT do X" comment actively misleads the next
/// reader, which is the failure class 113-29 recorded.
pub(crate) const fn is_v1_task_era(era: Option<crate::types::protocol::Era>) -> bool {
    !matches!(era, Some(crate::types::protocol::Era::V2))
}

/// Does `tasks/list` serve on this era?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | v1 enumerates a caller's tasks exactly as it always has |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | `tasks/list` is ABSENT from the tasks extension — [`V2_TASKS_METHOD_RETIRED`] |
///
/// # Why this is its own predicate rather than a shared boolean
///
/// [`tasks_result_serves_on_era`] answers the same question for the other
/// retired method and currently returns the same value. They are deliberately
/// two functions: a negative control that disables ONE gate must fail ONLY that
/// gate's probe, which is the orthogonality discipline 113-29 established and
/// which a single shared boolean makes impossible.
///
/// The era answer itself is NOT re-derived here — it delegates to
/// [`is_v1_task_era`] — so the file still has exactly one definition of "which
/// era is this".
///
/// # This predicate alone does not decide the refusal
///
/// A `false` here means "not on this era"; the caller ALSO checks
/// `TaskDispatch::has_task_backend`, because a server with no task backend must
/// keep its existing "not enabled" answer rather than claim a retirement.
pub(crate) const fn tasks_list_serves_on_era(era: Option<crate::types::protocol::Era>) -> bool {
    is_v1_task_era(era)
}

/// Does `tasks/result` serve on this era?
///
/// | `era`           | result  | why |
/// |-----------------|---------|-----|
/// | `Some(Era::V1)` | `true`  | v1 serves the terminal payload, including the FROZEN `-32002` pending refusal |
/// | `None`          | `true`  | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `Some(Era::V2)` | `false` | `tasks/result` is ABSENT from the tasks extension — [`V2_TASKS_METHOD_RETIRED`] |
///
/// Retiring the method on v2 also removes the LAST v2-reachable emission path
/// for `V1_TASK_PENDING` (`-32002`), the code protocol version 2026-07-28 MUST
/// NOT emit: `tests/v2_prohibited_error_codes.rs` proved that path reachable
/// over a real v2 HTTP request, and the gate now returns before the store is
/// ever consulted.
///
/// Separate from [`tasks_list_serves_on_era`] for the orthogonality reason
/// documented there.
pub(crate) const fn tasks_result_serves_on_era(era: Option<crate::types::protocol::Era>) -> bool {
    is_v1_task_era(era)
}

/// Build the default server-level `tasks` capability advertised when a task
/// backend (a [`TaskStore`] or a [`TaskRouter`]) is present.
///
/// This is the exact FROZEN [`ServerTasksCapability`] shape the client
/// `assert_capability` expects; it must not be hand-rolled at any call site.
/// Both [`apply_tasks_capability_rule`] and `ServerCoreBuilder` use this single
/// definition so the advertised capability shape can never drift.
pub(crate) fn default_tasks_capability() -> ServerTasksCapability {
    ServerTasksCapability {
        list: Some(serde_json::json!({})),
        cancel: Some(serde_json::json!({})),
        requests: Some(crate::types::capabilities::ServerTasksRequestCapability {
            tools: Some(crate::types::capabilities::ServerTasksToolsCapability {
                call: Some(serde_json::json!({})),
            }),
        }),
    }
}

/// The value pmcp auto-advertises under
/// [`TASKS_EXTENSION_KEY`](crate::types::capabilities::TASKS_EXTENSION_KEY):
/// the empty object.
///
/// Built through [`TasksExtensionCapability`] rather than a bare
/// `serde_json::json!({})` so there is ONE canonical spelling of the value, in
/// the same way `TASKS_EXTENSION_KEY` gives one canonical spelling of the key.
///
/// `default_tasks_capability()`'s `list` / `cancel` / `requests.tools.call`
/// flags are deliberately NOT projected in here (D-03). Advertising
/// `list: true` on an era where `tasks/list` answers `-32601` is exactly the
/// capability lie the endpoint-backed rule exists to prevent, and the vendored
/// draft schema types this capability as `Record<string, never>` — a value
/// admitting no properties at all.
///
/// Serializing a field-less struct cannot fail; the fallback restates the SAME
/// `{}` rather than introducing a panic path.
pub(crate) fn tasks_extension_value() -> Value {
    serde_json::to_value(TasksExtensionCapability::default())
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
}

/// Apply the endpoint-backed `tasks`-capability rule (D-CAPABILITY-ENDPOINT-BACKED).
///
/// This is the SINGLE shared rule both `ServerCoreBuilder` and (Plan 02)
/// `ServerBuilder` call. It is a free function over explicit params rather than a
/// builder method because the two builders hold `tool_infos` at different
/// lifecycle points (`ServerCoreBuilder` fills it at `.tool()`; `ServerBuilder`
/// builds it locally inside `build()`).
///
/// The `tasks` capability advertised in `initialize` represents REAL endpoint
/// support, never tool metadata alone:
/// - It is auto-advertised only when a backend exists (`has_backend`) and the
///   author has not already configured a custom `tasks` capability (additive-only
///   — an explicit value is preserved verbatim).
/// - A tool declaring [`TaskSupport::Required`] with NO backend is a build-time
///   validation error (rather than a hollow capability whose `tasks/*` endpoints
///   cannot work).
/// - An `Optional`/`Forbidden` task tool with no backend is NOT an error and does
///   NOT by itself trigger advertisement.
///
/// # ONE knob, TWO eras (plan 114-05, D-01)
///
/// The same `has_backend` fact drives BOTH advertisements:
///
/// | era | where it lands | value |
/// |-----|----------------|-------|
/// | MCP 2025-11-25 | `capabilities.tasks` | [`default_tasks_capability()`] |
/// | MCP 2026-07-28 | `capabilities.extensions["io.modelcontextprotocol/tasks"]` | `{}` ([`tasks_extension_value()`]) |
///
/// So no existing tasks server needs a code change to serve a v2 client, and no
/// v2 server with a working store can silently serve nothing. Both writes are
/// ADDITIVE in both directions: an explicitly configured value — `tasks` or the
/// extensions entry — is preserved VERBATIM, an absent `extensions` map is
/// created, and an existing one gains the entry alongside its other keys without
/// disturbing them.
///
/// This rule runs at BUILD time, where no era exists. Era-awareness is NOT its
/// job: the struct carries everything both eras could want, and the
/// serialization boundary decides what each era SEES
/// (`core::discover_result_from_capabilities` for v2 `server/discover`). That
/// split is D-02, and collapsing it — making this rule era-conditional — is what
/// would move v1 `initialize` bytes.
///
/// # Errors
///
/// Returns a validation error if any registered tool declares
/// [`TaskSupport::Required`] but no `TaskStore` or `TaskRouter` backs the
/// `tasks/*` endpoints.
pub(crate) fn apply_tasks_capability_rule(
    capabilities: &mut ServerCapabilities,
    tool_infos: &HashMap<String, ToolInfo>,
    has_backend: bool,
) -> Result<()> {
    let has_required_task_tool = tool_infos.values().any(|info| {
        info.execution
            .as_ref()
            .and_then(|e| e.task_support)
            .is_some_and(|ts| matches!(ts, TaskSupport::Required))
    });

    if has_required_task_tool && !has_backend {
        return Err(Error::validation(
            "a tool declares TaskSupport::Required but no TaskStore or TaskRouter \
             is configured to back the tasks/* endpoints",
        ));
    }

    if capabilities.tasks.is_none() && has_backend {
        capabilities.tasks = Some(default_tasks_capability());
    }

    // The v2 arm of the SAME endpoint-backed rule. `entry(..).or_insert_with(..)`
    // is the additive-only discipline the `tasks.is_none()` guard above expresses
    // for v1: an operator-configured value is never overwritten.
    if has_backend {
        capabilities
            .extensions
            .get_or_insert_with(HashMap::new)
            .entry(TASKS_EXTENSION_KEY.to_string())
            .or_insert_with(tasks_extension_value);
    }

    Ok(())
}

/// Create a success JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::success_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Result(result),
    }
}

/// Create an error JSON-RPC response (SINGLE-SOURCE envelope builder).
///
/// `ServerCore::error_response` delegates to this; there is exactly one copy of
/// the wrapping logic so the shared unit and `ServerCore` cannot drift.
pub(crate) fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
    JSONRPCResponse {
        jsonrpc: "2.0".to_string(),
        id,
        payload: ResponsePayload::Error(JSONRPCError {
            code,
            message,
            data: None,
        }),
    }
}

/// Resolution of a [`ToolHandler`](crate::server::ToolHandler)'s
/// [`ToolOutput`](crate::server::ToolOutput) at a NATIVE dispatch tail.
///
/// This is the SINGLE place (D-05 anti-drift) where the `Payload`-vs-`Result`
/// decision AND the response-middleware-bypass rule live. BOTH native dispatchers
/// (`Server::handle_call_tool` and `ServerCore::handle_call_tool`) resolve their
/// handler's `Result<ToolOutput>` through [`resolve_tool_output`] and branch on
/// this enum identically, so the two dispatchers can never drift on the rule.
pub(crate) enum DispatchOutput {
    /// `ToolOutput::Result` — send this `CallToolResult` to the wire VERBATIM.
    ///
    /// The dispatcher must BYPASS response middleware, the create-path gate, and
    /// text-wrap / widget enrichment for this arm (D-04 + D-04a, USER-APPROVED and
    /// LOCKED — the handler owns the full envelope, including its own redaction).
    /// REQUEST middleware and the handler-error path are unaffected: they run
    /// before this resolution, so only the SUCCESSFUL `Result` arm is verbatim.
    Verbatim(CallToolResult),

    /// `ToolOutput::Payload(v)` OR a handler `Err(_)` — coerced back into the
    /// existing `Result<Value>` middleware variable and fed through the UNCHANGED
    /// tail: response middleware, `handle_tool_error`, the create-path gate, and
    /// the text-wrap / widget enrichment, exactly as before this feature existed.
    Middleware(Result<Value>),
}

/// Build the `-32003` a caller receives for case 4 of the ordered refusal chain.
///
/// The message shape is `subscriptions/listen`'s verbatim, because it is the
/// same condition on the same server and a caller that hits both should not have
/// to learn two sentences for it.
///
/// It deliberately answers at HTTP **200** with a JSON-RPC error body:
/// [`AUTHENTICATION_REQUIRED`](crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED)
/// is not in `v2_status_for_code`'s 400 arm, and putting it there would change
/// the status of every other emitter of that code across the transport
/// (T-114-43). The transport file is untouched by this plan.
///
/// It is not an authentication ORACLE: it fires only for a method that EXISTS on
/// a server that ADVERTISES it, so all it reveals is "this server wants
/// authentication" — already public from the server's `WWW-Authenticate` posture
/// (T-114-40).
pub(crate) fn authentication_required(id: RequestId, method: &str) -> JSONRPCResponse {
    error_response(
        id,
        crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED,
        format!("{method} requires an authenticated caller on this server"),
    )
}

/// Resolve a handler's `Result<ToolOutput>` into the shared [`DispatchOutput`]
/// decision (D-05: one copy of the Payload-vs-Result + bypass rule).
///
/// - `Ok(ToolOutput::Result(r))` → [`DispatchOutput::Verbatim`] (wire-verbatim,
///   bypasses RESPONSE middleware + create-path + wrap);
/// - `Ok(ToolOutput::Payload(v))` → [`DispatchOutput::Middleware(Ok(v))`];
/// - `Err(e)` → [`DispatchOutput::Middleware(Err(e))`] (handler errors STILL flow
///   through `process_response` / `handle_tool_error` — the bypass is scoped to
///   the `Ok(Result(_))` arm only).
///
/// Matching a `#[non_exhaustive]` enum from WITHIN the defining crate is exhaustive
/// (the attribute only constrains downstream crates), so no wildcard arm is needed.
// Why: called by both native dispatch tails (mod.rs + core.rs handle_call_tool);
// production-reachable, no dead_code allow needed.
pub(crate) fn resolve_tool_output(output: Result<crate::server::ToolOutput>) -> DispatchOutput {
    match output {
        Ok(crate::server::ToolOutput::Result(call_result)) => DispatchOutput::Verbatim(call_result),
        Ok(crate::server::ToolOutput::Payload(value)) => DispatchOutput::Middleware(Ok(value)),
        Err(e) => DispatchOutput::Middleware(Err(e)),
    }
}

/// Which high-precision structural marker tripped the double-wrap detector.
///
/// Reported in the TOUT-02 WARN / `debug_assert!` so an author immediately sees
/// WHY a `Value` about to be text-wrapped looked like an already-built
/// [`CallToolResult`]. `Copy` (two field-less variants); it never escapes the
/// server crate (exposed to integration tests only via the hidden
/// `pmcp::__test_support` seam, mirroring `ServerRequestDispatcher`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleWrapMarker {
    /// The value carries a `_meta` object holding [`RELATED_TASK_META_KEY`] — the
    /// envelope key only a built task-augmented `CallToolResult` sets.
    RelatedTaskMeta,
    /// The value is a `CallToolResult`-envelope-shaped object (ONLY envelope
    /// keys: `content`/`isError`/`structuredContent`/`_meta`) carrying a
    /// NON-EMPTY `content` array whose every element deserializes as
    /// [`Content`] (the internally `#[serde(tag = "type")]` enum), i.e. it is
    /// already a wire-shaped result body.
    ContentArray,
}

/// Structural, high-precision detector for an about-to-be-double-wrapped result.
///
/// Detects "this `Value` is ALREADY a built [`CallToolResult`] and is about to
/// be WRONGLY text-wrapped a second time" (TOUT-02 — the exact silent bug
/// behind the agent-lake 2-week outage).
///
/// Returns `Some(marker)` only for a value carrying an unambiguous built-result
/// marker; `None` otherwise. Deliberately NOT a full
/// `from_value::<CallToolResult>` parse (D-02): it checks two cheap, precise
/// structural markers in cost order, so a benign tool payload almost never trips.
///
/// Precision rationale (near-zero false positives):
/// - The content-array marker only fires on a `CallToolResult` *envelope*: an
///   object whose keys are ALL envelope keys (`content`, `isError`,
///   `structuredContent`, `_meta`). A hand-built double-wrap was authored to
///   BE a `CallToolResult`, so only envelope keys accompany its `content`; a
///   chat-message-style payload (`role`, `model`, `stopReason`, ... — common
///   for tools that proxy LLM/sampling APIs) carries foreign keys and must
///   NOT trip.
/// - [`Content`] is internally tagged (`#[serde(tag = "type")]`), so an object
///   lacking a valid `"type"` NEVER deserializes as `Content` — the content-array
///   marker is high precision.
/// - An empty `content: []` is NOT a built-result marker (a benign payload can
///   carry an empty array), so it must NOT fire — hence the `!arr.is_empty()`
///   guard.
///
/// Order matters: the single-lookup `_meta` key check runs first (cheapest and
/// also short-circuits pathological large `content` arrays, T-104-03-02).
// Why: called at BOTH Payload wrap sites (mod.rs + core.rs) through
// `double_wrap_tripwire`; production-reachable, so no `dead_code` allow needed.
pub fn looks_like_call_tool_result(v: &Value) -> Option<DoubleWrapMarker> {
    /// Only these `CallToolResult` wire keys may accompany the `content` array
    /// for the envelope-shaped marker to fire (WR-02 precision fix).
    const RESULT_ENVELOPE_KEYS: [&str; 4] = ["content", "isError", "structuredContent", "_meta"];

    let obj = v.as_object()?;
    // Cheapest first: the task-envelope meta key — a single map lookup.
    if obj
        .get("_meta")
        .and_then(Value::as_object)
        .is_some_and(|meta| meta.contains_key(RELATED_TASK_META_KEY))
    {
        return Some(DoubleWrapMarker::RelatedTaskMeta);
    }
    // An envelope-shaped object (only `CallToolResult` keys) with a NON-EMPTY
    // `content` array whose every element parses as `Content`. The
    // `!arr.is_empty()` guard keeps a benign empty array from firing; the
    // envelope-keys guard keeps chat-message payloads from firing.
    if obj
        .keys()
        .all(|k| RESULT_ENVELOPE_KEYS.contains(&k.as_str()))
        && obj
            .get("content")
            .and_then(Value::as_array)
            .is_some_and(|arr| {
                !arr.is_empty()
                    && arr
                        .iter()
                        .all(|e| serde_json::from_value::<Content>(e.clone()).is_ok())
            })
    {
        return Some(DoubleWrapMarker::ContentArray);
    }
    None
}

/// The TOUT-02 double-wrap tripwire decision function.
///
/// The SINGLE decision fn both Payload wrap sites (`Server::handle_call_tool`
/// in mod.rs and `ServerCore::handle_call_tool` in core.rs) call BEFORE
/// stringifying a produced `Value` into a `CallToolResult`'s text content.
///
/// Behavior:
/// - `suppressed == true` → returns `None`, emits NOTHING (the tool opted out of
///   the check via `suppress_double_wrap_check`; D-08).
/// - otherwise, if [`looks_like_call_tool_result`] returns `Some(marker)`:
///   emits a `tracing::warn!` (EVERY build) AND `debug_assert!(false, ..)`
///   (debug/CI builds hard-fail; D-06: release compiles the assert out and NEVER
///   panics), then returns `Some(marker)`.
/// - benign value → returns `None`, emits nothing.
///
/// Returning the `Option<DoubleWrapMarker>` makes the decision unit-testable in
/// isolation: a RELEASE test asserts the return value (no panic), a DEBUG test
/// asserts the `debug_assert!` panic via `catch_unwind` — NEITHER spins up a
/// dispatch that the assert would abort mid-call (Codex MEDIUM: such end-to-end
/// debug-assert tests are brittle).
///
/// The identical helper is called from BOTH dispatchers, so the WARN/panic rule
/// can never drift between the high-level `Server` and `ServerCore`.
// Why: called at both Payload wrap sites (mod.rs + core.rs) guarded by the
// per-tool suppression check; production-reachable, no dead_code allow needed.
pub fn double_wrap_tripwire(
    tool_name: &str,
    value: &Value,
    suppressed: bool,
) -> Option<DoubleWrapMarker> {
    if suppressed {
        return None;
    }
    let marker = looks_like_call_tool_result(value)?;
    tracing::warn!(
        tool = %tool_name,
        ?marker,
        "value being text-wrapped structurally resembles a built CallToolResult \
         — did you mean ToolOutput::Result? (TOUT-02)"
    );
    // D-06: `debug_assert!` (NOT `assert!`) so release builds compile this out and
    // never panic in production; debug/CI builds hard-fail so the double-wrap is
    // caught by "one local run".
    debug_assert!(
        false,
        "double-wrap tripwire (TOUT-02): tool `{tool_name}` produced a value that \
         structurally resembles a built CallToolResult ({marker:?}); return \
         ToolOutput::Result to send it verbatim, or call \
         suppress_double_wrap_check(\"{tool_name}\") if this payload is legitimate"
    );
    Some(marker)
}

/// The outcome of binding a task request to a task OWNER.
///
/// A two-variant enum rather than `Option<String>` because the two answers mean
/// opposite things and one of them has to reach the wire: `None` used to mean
/// "no task backend", and reusing it for "refused" would make the fail-closed
/// row indistinguishable from a configuration fact at every call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnerBinding {
    /// The owner id every downstream store/router call is scoped to.
    Owner(String),
    /// Row 2 of the v2 identity table: an unauthenticated caller on a server
    /// that HAS an auth provider. The caller receives `-32003`
    /// [`AUTHENTICATION_REQUIRED`](crate::types::protocol::error_codes::AUTHENTICATION_REQUIRED)
    /// and no task is read, minted or enumerated (T-114-37).
    Refused,
}

/// Borrow-struct holding the task backend handles and the identity inputs a
/// dispatcher owns.
///
/// Both `Server` and `ServerCore` construct a `TaskDispatch` borrowing their own
/// `task_store`/`task_router` fields and call into it — the task-lifecycle logic
/// lives HERE, once, never as a divergent second copy.
pub(crate) struct TaskDispatch<'a> {
    /// Standard task backend (polling path). Presence flips `tasks` capability on.
    pub(crate) task_store: &'a Option<Arc<dyn TaskStore>>,
    /// Legacy experimental router backend (fall-through path).
    pub(crate) task_router: &'a Option<Arc<dyn TaskRouter>>,
    /// Whether this server has an auth provider configured — the FAIL-CLOSED
    /// input to the v2 identity table (TASK-05, D-07).
    ///
    /// A PER-SERVER fact, so it lives on the borrow-struct alongside the two
    /// backend handles rather than being threaded through every route. Both
    /// dispatchers read it from their EXISTING auth-provider accessor
    /// (`Server::get_auth_provider` / `ServerCore`'s own field, the same read
    /// `MrtrRound::begin` already makes) — no new field is added to either
    /// server, exactly as `listen_server_view` does for
    /// `subscriptions/listen`.
    pub(crate) has_auth_provider: bool,
}

impl TaskDispatch<'_> {
    /// Does this server have ANY task backend — a [`TaskStore`], a
    /// [`TaskRouter`], or both?
    ///
    /// The two v2 era gates consult it so a backend-LESS server keeps its
    /// existing "not enabled" / "not supported" refusal on EVERY era. "This
    /// method was retired" and "this server serves no tasks at all" are
    /// different facts calling for opposite fixes, and the `-32601` message is
    /// the only place a caller can tell them apart (T-114-33).
    const fn has_task_backend(&self) -> bool {
        self.task_store.is_some() || self.task_router.is_some()
    }

    /// Bind this request to a task owner, ERA-AWARE and FAIL-CLOSED on v2
    /// (TASK-05, D-07).
    ///
    /// Owner is ALWAYS derived from the `AuthContext`/router, NEVER from client
    /// params (IDOR mitigation, T-102-01) — on both eras.
    ///
    /// # v1 (and a request carrying no era code at all) — FROZEN
    ///
    /// Byte-identical to what it has always been. With a [`TaskRouter`],
    /// delegates to [`TaskRouter::resolve_owner`] (priority chain: OAuth subject,
    /// then client id, then session id, then the shared
    /// [`V1_UNAUTHENTICATED_OWNER`] bucket); with only a [`TaskStore`], the owner
    /// IS the OAuth subject; with no backend at all, the value is inert and
    /// collapses onto the same fallback every pre-114-09 caller already applied
    /// with `.unwrap_or_else(|| "local")`. The ONLY addition is the D-10
    /// migration `tracing::warn!` on the unauthenticated row.
    ///
    /// # v2 — the three-row identity table, REUSED not re-derived
    ///
    /// | authenticated subject | `has_auth_provider` | owner |
    /// |---|---|---|
    /// | `Some(sub)` | any | `sub` |
    /// | `None` | `true` | [`OwnerBinding::Refused`] |
    /// | `None` | `false` | [`ANONYMOUS_PRINCIPAL`](crate::server::core::ANONYMOUS_PRINCIPAL) |
    ///
    /// The decision is [`resolve_mrtr_principal`](crate::server::core::resolve_mrtr_principal)
    /// itself — the same function, not a copy of its match — because a task
    /// record and an MRTR continuation are both server-held state a later
    /// request redeems, and "who may redeem it" must have exactly one answer per
    /// server. See that function for why one table rather than two.
    ///
    /// ## The v2 arm never calls [`TaskRouter::resolve_owner`]
    ///
    /// Deliberate, and NOT an oversight to be tidied away by a later "unify the
    /// two paths" change. That method's chain reaches:
    ///
    /// * a **session id**, which TASK-05 forbids outright — v2 is stateless by
    ///   design and has no session, so binding an owner to one would either fail
    ///   or (worse) collide callers who happen to share a synthesised id; and
    /// * a **`client_id`**, which is per-APPLICATION (the OAuth `azp` claim), so
    ///   using it would collapse per-USER isolation into per-APP isolation —
    ///   every user of the same client application would share one task bucket
    ///   (T-114-38).
    ///
    /// ## D-07's caveat, stated plainly rather than implied
    ///
    /// Row 3 means that on a server with **no auth provider at all**, every v2
    /// caller shares ONE bucket. Fail-closed therefore applies to
    /// **auth-configured deployments** (row 2); a no-auth-provider server runs v2
    /// tasks in a single shared bucket BY DESIGN. That is a development / stdio
    /// affordance, NOT per-caller isolation, and it is defensible only because
    /// such a server has no notion of caller identity to separate in the first
    /// place. The production backends refuse that bucket independently:
    /// `TaskSecurityConfig::default()` sets `allow_anonymous: false`
    /// (`crates/pmcp-tasks/src/security.rs:89`), so `GenericTaskStore` rejects an
    /// anonymous owner unless an operator opts in (T-114-39).
    ///
    /// TASK-05's own wording says owner binding "fails closed" when no stable
    /// identity exists, which row 3 does not do; that gap is recorded as its own
    /// row in `114-SPEC-RECHECK.md` rather than left to be inferred, with the
    /// deferred configurable proxy-header identity source named as its future
    /// closure.
    pub(crate) fn resolve_owner(
        &self,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> OwnerBinding {
        if is_v1_task_era(era) {
            return OwnerBinding::Owner(self.resolve_v1_owner(auth_context));
        }
        // v2: the SHARED table, not a second match over the same two inputs.
        let principal = crate::server::core::MrtrPrincipal {
            authenticated_subject: auth_context.map(|ctx| ctx.subject.as_str()),
            has_auth_provider: self.has_auth_provider,
        };
        crate::server::core::resolve_mrtr_principal(principal)
            .map_or(OwnerBinding::Refused, |owner| {
                OwnerBinding::Owner(owner.to_string())
            })
    }

    /// The FROZEN v1 owner binding, plus D-10's migration warn.
    ///
    /// Split out only so the v2 arm of [`Self::resolve_owner`] reads as one
    /// decision; the body is byte-for-byte the pre-114-09 logic with the three
    /// former `None` outcomes collapsed onto [`V1_UNAUTHENTICATED_OWNER`] — which
    /// is what every caller already did with `.unwrap_or_else(|| "local")`.
    fn resolve_v1_owner(&self, auth_context: Option<&AuthContext>) -> String {
        if auth_context.is_none() {
            // D-10 migration warn. Emitted once per unauthenticated v1 owner
            // resolution, and it names the shared bucket rather than describing
            // it, so an operator can grep for the string that is actually in
            // their store.
            tracing::warn!(
                target: "mcp.tasks",
                owner = V1_UNAUTHENTICATED_OWNER,
                "an unauthenticated v1 task request was bound to the shared \"local\" owner \
                 bucket, which every other unauthenticated caller on this server also shares; \
                 protocol version 2026-07-28 binds the owner to the authenticated subject \
                 instead and refuses the request outright when an auth provider is configured"
            );
        }
        // Legacy path: TaskRouter has its own resolve_owner logic.
        if let Some(router) = self.task_router {
            return match auth_context {
                Some(ctx) => {
                    router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)
                },
                None => router.resolve_owner(None, None, None),
            };
        }
        // Standard path: derive owner from auth context when task_store is configured.
        // Key on the OAuth subject (the authenticated principal), matching the
        // router's subject-first priority — NOT client_id, which is per-application
        // (OAuth `azp`) and would collapse per-user isolation to per-app isolation.
        //
        // With NO backend at all the value is inert: every route reaches its
        // frozen `-32601` without reading it.
        match auth_context {
            Some(ctx) => ctx.subject.clone(),
            None => V1_UNAUTHENTICATED_OWNER.to_string(),
        }
    }

    /// Extract the terminal [`CallToolResult`] from a task-shaped tool value.
    ///
    /// Per `D-TERMINAL-RESULT-CONTRACT`: if the value carries a `result` object or
    /// a `content` array, deserialize it into a [`CallToolResult`]; otherwise the
    /// task is genuinely pending and there is no synchronous terminal result.
    pub(crate) fn extract_terminal_result(value: &Value) -> Option<CallToolResult> {
        if let Some(result_value) = value.get("result") {
            return serde_json::from_value::<CallToolResult>(result_value.clone()).ok();
        }
        if value.get("content").is_some() {
            return serde_json::from_value::<CallToolResult>(value.clone()).ok();
        }
        None
    }

    /// Build the `tools/call` create-task response.
    ///
    /// Per `D-STORE-MINTS-ID`: when a [`TaskStore`] is configured the store mints
    /// the canonical task id via `store.create()`; that store-minted id is
    /// reflected on the WIRE in BOTH `CreateTaskResult.task.taskId` AND the
    /// `_meta.relatedTask.taskId` envelope (never the tool's fabricated id). When
    /// the terminal result is present (synchronous completion) it is persisted via
    /// `store.set_result()` and the task is transitioned `Working -> Completed`
    /// BEFORE the response returns, so a subsequent `tasks/get` shows `Completed`.
    ///
    /// SIGNATURE NOTE: this fn does NOT take `task_id` or the terminal `result` as
    /// params — it RE-EXTRACTS them from `value` internally (the store-minted id
    /// comes back from `store.create`, and `extract_terminal_result(&value)`
    /// recovers the terminal result for persistence). A future refactor that stops
    /// re-extracting MUST add explicit params instead — never silently drop the
    /// terminal-result persistence (that would regress synchronous completion).
    ///
    /// Falls back to the legacy tool-fabricated envelope only when no store is
    /// configured (preserves prior behavior for router-only servers).
    ///
    /// `era` reaches the SAME [`Self::resolve_owner`] table every `tasks/*` route
    /// uses, so a task's owner is bound at CREATE by exactly the rule that later
    /// governs who may read it. On the v2 refuse row this answers `-32003` and
    /// mints nothing. WHETHER a v2 `tools/call` becomes a task at all is plan
    /// 114-12's decision (DQ1); this plan only decides WHOSE task it is.
    pub(crate) async fn build_task_created_response(
        &self,
        id: RequestId,
        value: Value,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> JSONRPCResponse {
        let Some(store) = self.task_store.as_ref() else {
            // No store: preserve the legacy tool-fabricated envelope. The
            // tool-fabricated task id is only needed on THIS path; with a store
            // the store-minted id wins, so don't allocate it otherwise.
            let tool_task_id = value
                .get("taskId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let result_value = serde_json::json!({
                "task": value,
                "_meta": { RELATED_TASK_META_KEY: { "taskId": tool_task_id } }
            });
            return success_response(id, result_value);
        };

        let OwnerBinding::Owner(owner_id) = self.resolve_owner(auth_context, era) else {
            return authentication_required(id, crate::types::mrtr::CALL_TOOL_METHOD);
        };

        // Carry the tool's requested TTL onto the store-minted task, if present.
        let ttl = value.get("ttl").and_then(serde_json::Value::as_u64);

        let created = match store.create(&owner_id, ttl).await {
            Ok(task) => task,
            Err(e) => {
                return error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                )
            },
        };
        let store_id = created.task_id.clone();

        // Synchronous completion: persist the terminal result and complete.
        let terminal_result = Self::extract_terminal_result(&value);
        let final_task = if let Some(call_result) = terminal_result {
            if let Err(e) = store.set_result(&store_id, &owner_id, call_result).await {
                return error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                );
            }
            match store
                .update_status(&store_id, &owner_id, TaskStatus::Completed, None)
                .await
            {
                Ok(task) => task,
                Err(e) => {
                    return error_response(
                        id,
                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                        e.to_string(),
                    )
                },
            }
        } else {
            created
        };

        // Build the wire envelope from the STORE-minted task (typed, no
        // hand-written task JSON) so task.taskId == _meta id == store id.
        let create_result = crate::types::tasks::CreateTaskResult::new(final_task);
        let mut envelope = serde_json::to_value(create_result).unwrap_or_default();
        if let Some(obj) = envelope.as_object_mut() {
            obj.insert(
                "_meta".to_string(),
                serde_json::json!({ RELATED_TASK_META_KEY: { "taskId": store_id } }),
            );
        }
        success_response(id, envelope)
    }

    /// Self-enforcing create-path gate: decide whether a `tools/call` becomes a
    /// task and, if so, build the create response.
    ///
    /// This is the SINGLE source of truth for "should this `tools/call` become a
    /// task?". Both dispatchers call it; neither re-derives the gate. The helper
    /// enforces the COMPLETE gate INTERNALLY — the caller passes raw facts
    /// (`task_requested`, the tool's `task_support`, the produced `value`), never a
    /// pre-checked precondition.
    ///
    /// Returns `Some(envelope)` IFF ALL of:
    /// - `task_requested == true` (the request carried a `task` field), AND
    /// - a backend is present (`self.task_store.is_some()`), AND
    /// - `task_support ∈ {Required, Optional}`, AND
    /// - `value` carries BOTH a `taskId` and a `status` (task-shaped).
    ///
    /// `TaskSupport::Forbidden`/`None`, `task_requested == false`, an absent
    /// backend, or a non-task-shaped value ALL return `None` ("fall through to a
    /// normal `CallToolResult`") with NO error leak (T-102-11).
    // Why: proven by the in-module `gate_tests` truth-table in Plan 01 and wired
    // into the `Server` create-path in Plan 02 (`handle_call_tool`), so it is
    // production-reachable — no `dead_code` allow is needed.
    pub(crate) async fn maybe_build_task_created(
        &self,
        id: RequestId,
        value: &Value,
        task_support: Option<TaskSupport>,
        task_requested: bool,
        auth_context: Option<&AuthContext>,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<JSONRPCResponse> {
        let gate_open = task_requested
            && self.task_store.is_some()
            && task_support
                .is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));
        if !gate_open {
            return None;
        }
        // Task-shaped value check: must carry BOTH a taskId and a status.
        let is_task_shaped =
            value.get("taskId").and_then(Value::as_str).is_some() && value.get("status").is_some();
        if !is_task_shaped {
            return None;
        }
        Some(
            self.build_task_created_response(id, value.clone(), auth_context, era)
                .await,
        )
    }

    /// Handle a `tasks/result` request (store-first → router → -32002 → -32601).
    ///
    /// On protocol version 2026-07-28 the method is RETIRED. That gate is case 1
    /// of [`Self::route_tasks_endpoint`]'s ordered refusal chain and fires before
    /// this function is entered at all — see [`tasks_result_serves_on_era`] and
    /// [`V2_TASKS_METHOD_RETIRED`]. The tail `match` below still reads the SAME
    /// predicate, deliberately: ONE era definition, N call sites (114-08).
    ///
    /// On v1 (and on a request carrying no era code at all) the behaviour is
    /// byte-for-byte what it has always been: serves from the configured
    /// [`TaskStore`] FIRST when it `supports_results()`, but FALLS THROUGH to the
    /// [`TaskRouter`] on store `NotFound`/unsupported — never a hard error when a
    /// router can serve it. When the store has no result and NO router is
    /// configured, returns the SPECIFIED "task not completed" error (`-32002`),
    /// distinct from the truly-no-backend `-32601` (FROZEN by Phase 101;
    /// T-102-03); see [`is_v1_task_era`].
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`],
    /// resolved once per request by the caller. This function does not — and
    /// must not — bind a second one.
    pub(crate) async fn handle_tasks_result(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskPayloadRequest,
        owner_id: &str,
        era: Option<crate::types::protocol::Era>,
    ) -> JSONRPCResponse {
        // Store-first: serve a typed CallToolResult when the store persists one.
        if let Some(store) = self.task_store {
            if store.supports_results() {
                match store.get_result(&params.task_id, owner_id).await {
                    Ok(call_result) => {
                        return success_response(
                            id,
                            serde_json::to_value(call_result).unwrap_or_default(),
                        );
                    },
                    // NotFound = store doesn't have it (absent / pending / owner
                    // mismatch): fall through to the router below.
                    Err(crate::server::task_store::TaskStoreError::NotFound { .. }) => {},
                    Err(e) => {
                        return error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        )
                    },
                }
            }
        }

        // Router fallback — behavior UNCHANGED for router-backed servers.
        if let Some(task_router) = self.task_router {
            return match task_router
                .handle_tasks_result(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            };
        }

        // No router: distinguish "store exists but task not completed yet"
        // (specified error) from "no task backend at all".
        //
        // The era axis reads the SAME predicate as case 1 of
        // `route_tasks_endpoint`'s chain, deliberately — not a second,
        // independently-disable-able copy of the era question. A negative control
        // measured why: with this arm keyed on `is_v1_task_era` directly,
        // disabling `tasks_result_serves_on_era` left this arm still refusing v2
        // with an identical body, so the retirement gate could not be proven
        // load-bearing by any test. ONE predicate, two call sites: disable it and
        // the whole gate opens, which is what a negative control has to be able
        // to do.
        match (self.task_store.is_some(), tasks_result_serves_on_era(era)) {
            (true, true) => error_response(
                id,
                // FROZEN wire value -32002 (byte-identical); read by name from the
                // centralized table (Pitfall 6). The
                // pending_tasks_result_preserves_minus_32002 test is the guard.
                // Unreachable on v2 by the arm below, which is what keeps this
                // spec-prohibited code off the v2 wire (Finding 11).
                crate::types::protocol::error_codes::V1_TASK_PENDING,
                "task result not available: task not completed".to_string(),
            ),
            // Required for exhaustiveness, and unreachable in production: case 1
            // already returned for every era-v2 request that has a backend, and
            // `task_store.is_some()` implies one. It answers IDENTICALLY so the
            // two spellings of the refusal cannot diverge.
            (true, false) => retired_on_v2(id, TASKS_RESULT_METHOD),
            (false, _) => error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_RESULT_NOT_SUPPORTED.to_string(),
            ),
        }
    }

    /// Route a `tasks/get` request (store-first, router fall-through).
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    async fn route_tasks_get(
        &self,
        id: RequestId,
        params: &crate::types::tasks::GetTaskRequest,
        owner_id: &str,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            match store.get(&params.task_id, owner_id).await {
                Ok(task) => {
                    let result = crate::types::tasks::GetTaskResult::new(task);
                    success_response(id, serde_json::to_value(result).unwrap_or_default())
                },
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else if let Some(task_router) = self.task_router {
            match task_router
                .handle_tasks_get(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else {
            error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            )
        }
    }

    /// Route a `tasks/list` request (store-first, router fall-through).
    ///
    /// On protocol version 2026-07-28 the method is RETIRED and answers
    /// `-32601` WITHOUT enumerating anything. That gate is case 1 of
    /// [`Self::route_tasks_endpoint`]'s ordered chain and fires before this
    /// function is entered — which is what makes enumeration impossible rather
    /// than merely refused: no store `list`, no router call, and not even an
    /// owner binding, so nothing can leak the existence of a task into the
    /// response body. See [`tasks_list_serves_on_era`] and
    /// [`V2_TASKS_METHOD_RETIRED`]. On v1 the store/router behaviour below is
    /// unchanged.
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    async fn route_tasks_list(
        &self,
        id: RequestId,
        params: &crate::types::tasks::ListTasksRequest,
        owner_id: &str,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            match store.list(owner_id, params.cursor.as_deref()).await {
                Ok((tasks, next_cursor)) => {
                    let mut result = crate::types::tasks::ListTasksResult::new(tasks);
                    if let Some(cursor) = next_cursor {
                        result = result.with_next_cursor(cursor);
                    }
                    success_response(id, serde_json::to_value(result).unwrap_or_default())
                },
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else if let Some(task_router) = self.task_router {
            match task_router
                .handle_tasks_list(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else {
            error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            )
        }
    }

    /// Route a `tasks/cancel` request (store-first, router fall-through).
    ///
    /// `owner_id` is the ALREADY-BOUND owner from [`Self::resolve_owner`].
    async fn route_tasks_cancel(
        &self,
        id: RequestId,
        params: &crate::types::tasks::CancelTaskRequest,
        owner_id: &str,
    ) -> JSONRPCResponse {
        if let Some(store) = self.task_store {
            match store.cancel(&params.task_id, owner_id).await {
                Ok(task) => {
                    let result = crate::types::tasks::CancelTaskResult::new(task);
                    success_response(id, serde_json::to_value(result).unwrap_or_default())
                },
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else if let Some(task_router) = self.task_router {
            match task_router
                .handle_tasks_cancel(serde_json::to_value(params).unwrap_or_default(), owner_id)
                .await
            {
                Ok(result) => success_response(id, result),
                Err(e) => error_response(
                    id,
                    crate::types::protocol::error_codes::INTERNAL_ERROR,
                    e.to_string(),
                ),
            }
        } else {
            error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                TASKS_NOT_ENABLED.to_string(),
            )
        }
    }

    /// Route any `tasks/*` endpoint request to its handler.
    ///
    /// Dispatches `TasksGet`/`TasksList`/`TasksCancel` to their per-endpoint
    /// helpers and `TasksResult` to [`Self::handle_tasks_result`]. Non-`tasks/*`
    /// variants return the FROZEN `-32601 "Method not supported"` (callers only
    /// pass `tasks/*` variants here).
    ///
    /// `protocol_context` is the ALREADY-RESOLVED
    /// [`ProtocolContext`](crate::types::protocol::ProtocolContext) being
    /// CONSUMED here — this module never runs an era resolver of its own and
    /// never re-reads `params._meta`. Two things are read off it: the
    /// [`era`](crate::types::protocol::ProtocolContext::era), by the
    /// `tasks/result` pending refusal (see [`is_v1_task_era`]) and the two v2
    /// retirement gates ([`tasks_list_serves_on_era`],
    /// [`tasks_result_serves_on_era`]); and the client's declared
    /// [`client_capabilities`](crate::types::protocol::ProtocolContext::client_capabilities),
    /// resolved once at ingress by Phase 112.
    ///
    /// `TasksGet` and `TasksCancel` are not era-GATED on purpose: both survive
    /// in the v2 extension schema. Their v2 response SHAPE is plan 114-11's,
    /// not this router's.
    ///
    /// # Rejection cases, IN ORDER
    ///
    /// The order is the contract, not an implementation detail — each case says
    /// something different to the caller, and the wrong order either leaks or
    /// misdirects. The shape mirrors `subscriptions/listen`'s ordered chain
    /// (D-08), which this reuses down to the `-32003` placement.
    ///
    /// 1. **RETIRED on this era → `-32601`.** A method that does not exist on
    ///    protocol version 2026-07-28 answers "no such method" FIRST, so a
    ///    `tasks/list` cannot be answered "authenticate yourself" and thereby
    ///    imply that authenticating would enumerate anything (T-114-32).
    /// 2. **No task backend → `-32601`.** Answered by the per-endpoint handlers
    ///    below, where each method's FROZEN message lives
    ///    ([`TASKS_NOT_ENABLED`] / [`TASKS_RESULT_NOT_SUPPORTED`]). Cases 3 and 4
    ///    are therefore SKIPPED for a backendless server: it advertises no tasks
    ///    extension at all, so telling such a caller to declare one — or to
    ///    authenticate — would send it to fix the wrong thing (T-114-33).
    /// 3. **Client did not declare the extension → `-32021`.** A
    ///    method-availability-class refusal like cases 1 and 2, and placed with
    ///    them, because it says "this method is not available to you as
    ///    configured" and reveals NOTHING about authentication state.
    ///    NOT YET IMPLEMENTED — it lands in the next commit of this plan, which
    ///    also adds the ordering probes. The slot is documented here because
    ///    case 4's placement is only meaningful relative to it.
    /// 4. **Unauthenticated on an auth-configured server → `-32003`.** Row 2 of
    ///    the identity table. Placed AFTER cases 1–3 so a retired method, a
    ///    backendless server or an under-declaring client each keeps its own
    ///    truthful answer rather than being told to authenticate; and BEFORE the
    ///    params are used, so a refused caller's body is never read and no
    ///    store or router is ever consulted (T-114-37).
    /// 5. **The params, finally.** Everything below this line consumes
    ///    `request`'s typed params. Nothing above it does.
    ///
    /// The owner is bound EXACTLY ONCE here and passed down as a `&str`; no
    /// handler resolves a second one.
    pub(crate) async fn route_tasks_endpoint(
        &self,
        id: RequestId,
        request: &ClientRequest,
        auth_context: Option<&AuthContext>,
        protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        let era = protocol_context.map(|context| context.era);

        if self.has_task_backend() {
            // --- case 1 -----------------------------------------------------
            if let Some(method) = Self::retired_method(request, era) {
                return retired_on_v2(id, method);
            }
        }

        // --- case 4 ---------------------------------------------------------
        let owner_id = match self.resolve_owner(auth_context, era) {
            OwnerBinding::Owner(owner) => owner,
            // Case 2 owns the answer for a backendless server, and every handler
            // below reaches its frozen `-32601` WITHOUT reading the owner, so
            // this value is inert. It is spelled as the v1 fallback rather than
            // as the v2 anonymous principal so that no reader mistakes it for a
            // bucket a task could ever land in: no backend means no task.
            OwnerBinding::Refused if !self.has_task_backend() => {
                V1_UNAUTHENTICATED_OWNER.to_string()
            },
            OwnerBinding::Refused => {
                return authentication_required(id, Self::method_of(request));
            },
        };

        // --- case 5 ---------------------------------------------------------
        match request {
            ClientRequest::TasksGet(params) => self.route_tasks_get(id, params, &owner_id).await,
            ClientRequest::TasksResult(params) => {
                self.handle_tasks_result(id, params, &owner_id, era).await
            },
            ClientRequest::TasksList(params) => self.route_tasks_list(id, params, &owner_id).await,
            ClientRequest::TasksCancel(params) => {
                self.route_tasks_cancel(id, params, &owner_id).await
            },
            _ => error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                NOT_A_TASKS_METHOD.to_string(),
            ),
        }
    }

    /// The `tasks/*` method name IFF this request's method is RETIRED on `era`
    /// — case 1 of [`Self::route_tasks_endpoint`]'s chain.
    ///
    /// A dispatch TABLE over the two EXISTING era predicates, not a third era
    /// decision: `tasks/list` and `tasks/result` each keep exactly one predicate
    /// (so a negative control that disables one fails only that method's
    /// probes), and `tasks/get`/`tasks/cancel` survive on both eras so neither
    /// has one at all.
    fn retired_method(
        request: &ClientRequest,
        era: Option<crate::types::protocol::Era>,
    ) -> Option<&'static str> {
        match request {
            ClientRequest::TasksList(_) if !tasks_list_serves_on_era(era) => {
                Some(TASKS_LIST_METHOD)
            },
            ClientRequest::TasksResult(_) if !tasks_result_serves_on_era(era) => {
                Some(TASKS_RESULT_METHOD)
            },
            _ => None,
        }
    }

    /// The method string a `tasks/*` request names, for a refusal message.
    ///
    /// Every spelling is read from an existing constant, never re-typed here.
    fn method_of(request: &ClientRequest) -> &'static str {
        match request {
            ClientRequest::TasksGet(_) => crate::types::mrtr::TASKS_GET_METHOD,
            ClientRequest::TasksResult(_) => TASKS_RESULT_METHOD,
            ClientRequest::TasksList(_) => TASKS_LIST_METHOD,
            ClientRequest::TasksCancel(_) => crate::types::mrtr::TASKS_CANCEL_METHOD,
            _ => NOT_A_TASKS_METHOD,
        }
    }
}

#[cfg(test)]
// Test-ergonomic helpers: `///` summaries name gate-table inputs by their literal
// arg/enum spelling (clippy::doc_markdown), and the `store_backend()` helper always
// returns `Some` by design so each test reads as a backend-present row
// (clippy::unnecessary_wraps). Both are noise in a truth-table test module.
#[allow(clippy::doc_markdown, clippy::unnecessary_wraps)]
mod gate_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::RequestId;

    fn store_backend() -> Option<Arc<dyn TaskStore>> {
        Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>)
    }

    fn task_shaped_value() -> Value {
        serde_json::json!({
            "taskId": "tool-fabricated",
            "status": "completed",
            "result": { "content": [{ "type": "text", "text": "done" }] }
        })
    }

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    /// task_requested == false → None regardless of other inputs.
    #[tokio::test]
    async fn gate_rejects_when_not_task_requested() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), false, None, None)
            .await;
        assert!(out.is_none(), "task_requested=false must yield None");
    }

    /// task_requested == true but no backend → None.
    #[tokio::test]
    async fn gate_rejects_when_no_backend() {
        let store = None;
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None, None)
            .await;
        assert!(out.is_none(), "no backend must yield None");
    }

    /// task_requested, backend, TaskSupport::Forbidden → None (no error leak).
    #[tokio::test]
    async fn gate_rejects_forbidden_no_error_leak() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Forbidden), true, None, None)
            .await;
        assert!(out.is_none(), "Forbidden must yield None, never an error");
    }

    /// task_requested, backend, TaskSupport::None → None.
    #[tokio::test]
    async fn gate_rejects_no_task_support() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, None, true, None, None)
            .await;
        assert!(out.is_none(), "no task_support must yield None");
    }

    /// Required-with-backend, value missing taskId/status → None.
    #[tokio::test]
    async fn gate_rejects_non_task_shaped_value() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = serde_json::json!({ "foo": "bar" });
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None, None)
            .await;
        assert!(out.is_none(), "non-task-shaped value must yield None");
    }

    /// Assert the Some-case three-way store-minted-id invariant on an envelope.
    fn assert_store_minted(resp: &JSONRPCResponse) {
        let ResponsePayload::Result(value) = &resp.payload else {
            panic!("expected a success result envelope");
        };
        let wire_task_id = value
            .get("task")
            .and_then(|t| t.get("taskId"))
            .and_then(Value::as_str)
            .expect("task.taskId present");
        let meta_id = value
            .get("_meta")
            .and_then(|m| m.get(RELATED_TASK_META_KEY))
            .and_then(|r| r.get("taskId"))
            .and_then(Value::as_str)
            .expect("_meta.relatedTask.taskId present");
        assert_eq!(
            wire_task_id, meta_id,
            "three-way invariant: task.taskId == _meta.relatedTask.taskId"
        );
        assert_ne!(
            wire_task_id, "tool-fabricated",
            "wire id must be store-minted, not the tool-fabricated id"
        );
    }

    /// task_requested, backend, TaskSupport::Optional, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_optional_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Optional), true, None, None)
            .await;
        let resp = out.expect("Optional + task-shaped must yield Some");
        assert_store_minted(&resp);
    }

    /// task_requested, backend, TaskSupport::Required, task-shaped → Some + invariant.
    #[tokio::test]
    async fn gate_accepts_required_task_shaped() {
        let store = store_backend();
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider: false,
        };
        let value = task_shaped_value();
        let out = dispatch
            .maybe_build_task_created(id(), &value, Some(TaskSupport::Required), true, None, None)
            .await;
        let resp = out.expect("Required + task-shaped must yield Some");
        assert_store_minted(&resp);
    }
}

/// The era-aware owner binding (plan 114-09, TASK-05, D-07).
///
/// One test per ROW of the v2 identity table, each named for the row it proves,
/// plus the v1 freeze and its D-10 migration warn. These are the UNIT half; the
/// ordered refusal chain is measured over a real socket in
/// `tests/v2_tasks_owner_binding.rs`, and the cross-caller proof is 114-15.
#[cfg(test)]
mod owner_binding_tests {
    use super::*;
    use crate::server::auth::AuthContext;
    use crate::server::core::ANONYMOUS_PRINCIPAL;
    use crate::types::protocol::Era;

    /// Bind an owner at `era` with the given identity inputs.
    ///
    /// Deliberately backend-LESS: the v2 table reads only
    /// `authenticated_subject` and `has_auth_provider`, and the v1 arm's
    /// store/router branches are already covered by `gate_tests` and
    /// `era_gate_tests`. Keeping the fixture minimal is what makes each
    /// assertion below attributable to exactly one row.
    fn bind(subject: Option<&str>, has_auth_provider: bool, era: Option<Era>) -> OwnerBinding {
        let store = None;
        let router = None;
        let dispatch = TaskDispatch {
            task_store: &store,
            task_router: &router,
            has_auth_provider,
        };
        let auth = subject.map(AuthContext::new);
        dispatch.resolve_owner(auth.as_ref(), era)
    }

    /// Row 1: an authenticated subject IS the owner, on either auth posture.
    ///
    /// `has_auth_provider` is the "any" column of the table, so both values are
    /// asserted — a row-1 implementation that accidentally read the flag would
    /// pass a single-value test.
    #[test]
    fn v2_owner_is_the_authenticated_subject() {
        for has_auth_provider in [true, false] {
            assert_eq!(
                bind(Some("user-alice"), has_auth_provider, Some(Era::V2)),
                OwnerBinding::Owner("user-alice".to_string()),
                "row 1 must bind the OAuth subject verbatim, \
                 has_auth_provider={has_auth_provider}"
            );
        }
    }

    /// Row 2, the FAIL-CLOSED row: unauthenticated + an auth provider → refused.
    ///
    /// This is TASK-05's central control. If it ever returns an `Owner`, an
    /// anonymous caller can mint and read tasks on a server that expects
    /// authentication (T-114-37).
    #[test]
    fn v2_unauthenticated_with_auth_provider_is_refused() {
        assert_eq!(
            bind(None, true, Some(Era::V2)),
            OwnerBinding::Refused,
            "row 2 must refuse: no subject on an auth-configured server binds NO owner"
        );
    }

    /// Row 3: unauthenticated on a server with NO auth provider → the shared
    /// anonymous bucket, NOT a refusal.
    ///
    /// The counterweight to row 2: a fail-closed change must not be satisfiable
    /// by refusing everyone, which would break every stdio/dev server.
    #[test]
    fn v2_unauthenticated_without_auth_provider_is_anonymous() {
        assert_eq!(
            bind(None, false, Some(Era::V2)),
            OwnerBinding::Owner(ANONYMOUS_PRINCIPAL.to_string()),
            "row 3 must bind the NAMED anonymous principal, not refuse"
        );
    }

    /// The v2 anonymous bucket and the v1 `"local"` bucket are DISJOINT keys.
    ///
    /// `GenericTaskStore::is_anonymous_owner` treats the two identically for the
    /// `allow_anonymous` decision, but `make_key` prefixes every record by owner
    /// — so these two facts are separate and easy to conflate. Asserting the
    /// inequality here stops a future "simplify the two fallbacks" change from
    /// silently merging two key spaces.
    #[test]
    fn the_v1_and_v2_unauthenticated_buckets_are_different_keys() {
        assert_ne!(
            ANONYMOUS_PRINCIPAL, V1_UNAUTHENTICATED_OWNER,
            "the v1 and v2 unauthenticated owners must remain distinct key prefixes"
        );
    }

    /// The v1 arm is FROZEN: an unauthenticated caller still binds `"local"`,
    /// on an explicit v1 era AND on a request carrying no era code at all.
    #[test]
    fn v1_unauthenticated_owner_is_still_local() {
        for era in [Some(Era::V1), None] {
            for has_auth_provider in [true, false] {
                assert_eq!(
                    bind(None, has_auth_provider, era),
                    OwnerBinding::Owner(V1_UNAUTHENTICATED_OWNER.to_string()),
                    "v1 owner binding is frozen and NEVER refuses: \
                     era={era:?}, has_auth_provider={has_auth_provider}"
                );
            }
        }
    }

    /// v1 with a subject still binds that subject — the freeze is not "always
    /// local".
    #[test]
    fn v1_authenticated_owner_is_still_the_subject() {
        assert_eq!(
            bind(Some("user-bob"), true, Some(Era::V1)),
            OwnerBinding::Owner("user-bob".to_string()),
            "v1 store-path owner binding is the OAuth subject, unchanged"
        );
    }

    /// D-10's migration warn fires EXACTLY once per unauthenticated v1
    /// resolution, and NOT for an authenticated one.
    ///
    /// Counted with a hand-rolled `tracing::Subscriber` rather than
    /// `tracing-subscriber`, which is an OPTIONAL dependency behind the
    /// `logging` feature — this assertion must hold under every feature set the
    /// gate builds.
    #[test]
    fn the_v1_migration_warn_fires_once_per_unauthenticated_resolution() {
        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);

        tracing::subscriber::with_default(counter, || {
            assert_eq!(
                bind(None, false, Some(Era::V1)),
                OwnerBinding::Owner(V1_UNAUTHENTICATED_OWNER.to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "exactly one migration warn per unauthenticated v1 owner resolution"
        );

        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);
        tracing::subscriber::with_default(counter, || {
            assert_eq!(
                bind(Some("user-carol"), false, Some(Era::V1)),
                OwnerBinding::Owner("user-carol".to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "an AUTHENTICATED v1 caller is not in the shared bucket and must not be warned about"
        );
    }

    /// A v2 resolution never emits the v1 migration warn — the two arms are
    /// genuinely separate, not one arm with a flag.
    #[test]
    fn the_migration_warn_is_v1_only() {
        let counter = WarnCounter::default();
        let counts = Arc::clone(&counter.warnings);
        tracing::subscriber::with_default(counter, || {
            assert_eq!(bind(None, true, Some(Era::V2)), OwnerBinding::Refused);
            assert_eq!(
                bind(None, false, Some(Era::V2)),
                OwnerBinding::Owner(ANONYMOUS_PRINCIPAL.to_string())
            );
        });
        assert_eq!(
            counts.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the D-10 migration warn is about v1's shared bucket and must not fire on v2"
        );
    }

    /// Counts WARN-level events, and nothing else.
    ///
    /// Hand-rolled against `tracing`'s core `Subscriber` trait so the assertion
    /// has no optional-dependency footprint (see the test above).
    #[derive(Default)]
    struct WarnCounter {
        warnings: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl tracing::Subscriber for WarnCounter {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() == tracing::Level::WARN
        }
        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::Id {
            tracing::Id::from_u64(1)
        }
        fn record(&self, _span: &tracing::Id, _values: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _span: &tracing::Id, _follows: &tracing::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            if *event.metadata().level() == tracing::Level::WARN {
                self.warnings
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        fn enter(&self, _span: &tracing::Id) {}
        fn exit(&self, _span: &tracing::Id) {}
    }
}

/// The v2 retirement of `tasks/list` and `tasks/result` (plan 114-08, TASK-03).
///
/// One `#[tokio::test]` per row of the per-method era matrix, each named for the
/// row it proves — the shape `gate_tests` above established. The live-socket
/// half, with a negative control per gate, is `tests/v2_tasks_era_gates.rs`.
#[cfg(test)]
// Why: `store_backend()` always returns `Some` BY DESIGN, so each caller reads
// as a backend-present row (clippy::unnecessary_wraps); and `route()` takes
// `&Option<Arc<dyn TaskStore>>` because that is the type
// `TaskDispatch::task_store` borrows — a helper taking `Option<&T>` could not
// construct the production struct at all (clippy::ref_option). Both are the same
// truth-table-test noise `gate_tests` above already allows.
#[allow(clippy::unnecessary_wraps, clippy::ref_option)]
mod era_gate_tests {
    use super::*;
    use crate::server::task_store::InMemoryTaskStore;
    use crate::types::protocol::error_codes::{METHOD_NOT_FOUND, V1_TASK_PENDING};
    use crate::types::protocol::Era;
    use crate::types::RequestId;

    /// Every era value a request can carry, in truth-table order.
    const ERAS: [(Option<Era>, bool); 3] =
        [(Some(Era::V1), true), (None, true), (Some(Era::V2), false)];

    fn id() -> RequestId {
        RequestId::from(1i64)
    }

    fn store_backend() -> Option<Arc<dyn TaskStore>> {
        Some(Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>)
    }

    fn list_request() -> ClientRequest {
        ClientRequest::TasksList(crate::types::tasks::ListTasksRequest { cursor: None })
    }

    fn result_request() -> ClientRequest {
        ClientRequest::TasksResult(crate::types::tasks::GetTaskPayloadRequest {
            task_id: "absent".to_string(),
        })
    }

    /// A [`ClientCapabilities`] that DECLARES the tasks extension, spelled
    /// through the shared key constant.
    fn tasks_declaring_capabilities() -> crate::types::ClientCapabilities {
        let mut extensions = HashMap::new();
        extensions.insert(TASKS_EXTENSION_KEY.to_string(), tasks_extension_value());
        crate::types::ClientCapabilities {
            extensions: Some(extensions),
            ..crate::types::ClientCapabilities::default()
        }
    }

    /// The already-resolved [`ProtocolContext`] for `era`, DECLARING the tasks
    /// extension.
    ///
    /// The declaration is deliberate: this module measures the ERA gates, so
    /// every fixture must clear the extension-declaration gate
    /// ([`missing_tasks_declaration_refusal`]) or a `-32021` would masquerade as
    /// a retirement. `None` reproduces the "no era code at all" row.
    fn context_for(era: Era) -> crate::types::protocol::ProtocolContext {
        let version = match era {
            Era::V2 => crate::types::protocol::PROTOCOL_VERSION_2026_07_28,
            Era::V1 => crate::LATEST_PROTOCOL_VERSION,
        };
        crate::types::protocol::ProtocolContext::new(
            era,
            crate::types::ProtocolVersion(version.to_string()),
        )
        .with_client_capabilities(tasks_declaring_capabilities())
    }

    /// Drive one `tasks/*` request through the real router at one era.
    async fn route(
        store: &Option<Arc<dyn TaskStore>>,
        request: &ClientRequest,
        era: Option<Era>,
    ) -> JSONRPCResponse {
        let router = None;
        let dispatch = TaskDispatch {
            task_store: store,
            task_router: &router,
            has_auth_provider: false,
        };
        let context = era.map(context_for);
        dispatch
            .route_tasks_endpoint(id(), request, None, context.as_ref())
            .await
    }

    /// The `(code, message)` of an error response, or `None` for a success.
    fn error_of(response: &JSONRPCResponse) -> Option<(i32, String)> {
        match &response.payload {
            ResponsePayload::Error(error) => Some((error.code, error.message.clone())),
            ResponsePayload::Result(_) => None,
        }
    }

    /// `tasks/list` serves on v1 and on an era-less request, and not on v2.
    #[test]
    fn tasks_list_era_truth_table() {
        for (era, expected) in ERAS {
            assert_eq!(
                tasks_list_serves_on_era(era),
                expected,
                "tasks/list serving decision for era {era:?}"
            );
        }
    }

    /// `tasks/result` serves on v1 and on an era-less request, and not on v2.
    #[test]
    fn tasks_result_era_truth_table() {
        for (era, expected) in ERAS {
            assert_eq!(
                tasks_result_serves_on_era(era),
                expected,
                "tasks/result serving decision for era {era:?}"
            );
        }
    }

    /// A v2 `tasks/list` is `-32601` with the RETIRED message and enumerates
    /// nothing.
    #[tokio::test]
    async fn v2_tasks_list_is_retired() {
        let store = store_backend();
        let response = route(&store, &list_request(), Some(Era::V2)).await;

        let (code, message) = error_of(&response).expect("a v2 tasks/list must be refused");
        assert_eq!(code, METHOD_NOT_FOUND, "message was {message}");
        assert!(
            message.starts_with(TASKS_LIST_METHOD) && message.contains(V2_TASKS_METHOD_RETIRED),
            "the refusal must name the method AND the retirement: {message}"
        );
    }

    /// A v2 `tasks/result` is `-32601` with the RETIRED message and never the
    /// spec-prohibited `-32002`.
    #[tokio::test]
    async fn v2_tasks_result_is_retired() {
        let store = store_backend();
        let response = route(&store, &result_request(), Some(Era::V2)).await;

        let (code, message) = error_of(&response).expect("a v2 tasks/result must be refused");
        assert_eq!(code, METHOD_NOT_FOUND, "message was {message}");
        assert_ne!(
            code, V1_TASK_PENDING,
            "protocol version 2026-07-28 MUST NOT emit -32002: {message}"
        );
        assert!(
            message.starts_with(TASKS_RESULT_METHOD) && message.contains(V2_TASKS_METHOD_RETIRED),
            "the refusal must name the method AND the retirement: {message}"
        );
    }

    /// The v1 side of the same two gates is untouched: `tasks/list` still
    /// enumerates and `tasks/result` still emits the FROZEN `-32002` with its
    /// existing message.
    #[tokio::test]
    async fn v1_list_and_result_are_unchanged() {
        let store = store_backend();

        let listed = route(&store, &list_request(), Some(Era::V1)).await;
        let ResponsePayload::Result(value) = &listed.payload else {
            panic!("a v1 tasks/list must still serve: {:?}", listed.payload);
        };
        assert!(
            value.get("tasks").is_some_and(Value::is_array),
            "a v1 tasks/list result still carries the tasks array: {value}"
        );

        let pending = route(&store, &result_request(), Some(Era::V1)).await;
        assert_eq!(
            error_of(&pending),
            Some((
                V1_TASK_PENDING,
                "task result not available: task not completed".to_string()
            )),
            "the v1 pending refusal is FROZEN, code and message"
        );
    }

    /// A server with NO backend keeps its "not enabled" / "not supported"
    /// answers on v2, and they are DIFFERENT strings from the RETIRED message.
    ///
    /// This is what makes the two-message split observable rather than
    /// cosmetic: a caller that hits the no-backend answer must not be told a
    /// method was retired, because the fix is to configure a backend.
    #[tokio::test]
    async fn a_backendless_v2_server_is_not_told_the_methods_were_retired() {
        let store = None;

        let listed = route(&store, &list_request(), Some(Era::V2)).await;
        let (list_code, list_message) = error_of(&listed).expect("no backend refuses tasks/list");
        assert_eq!(list_code, METHOD_NOT_FOUND, "message was {list_message}");
        assert_eq!(list_message, "Tasks not enabled");

        let resulted = route(&store, &result_request(), Some(Era::V2)).await;
        let (result_code, result_message) =
            error_of(&resulted).expect("no backend refuses tasks/result");
        assert_eq!(
            result_code, METHOD_NOT_FOUND,
            "message was {result_message}"
        );
        assert_eq!(result_message, "tasks/result not supported");

        for message in [&list_message, &result_message] {
            assert!(
                !message.contains(V2_TASKS_METHOD_RETIRED),
                "a no-backend refusal must not claim a retirement: {message}"
            );
        }
        assert_ne!(
            list_message, result_message,
            "the two no-backend refusals are themselves distinguishable"
        );
    }
}

/// The v2 arm of [`apply_tasks_capability_rule`] (plan 114-05, D-01/D-03).
///
/// One test per row of the additive-only truth table, each named for the row it
/// proves. Every test uses NO tools, so the `TaskSupport::Required` validation
/// branch is out of the way and each assertion is about the capability writes
/// alone.
#[cfg(test)]
mod capability_rule_tests {
    use super::*;

    fn no_tools() -> HashMap<String, ToolInfo> {
        HashMap::new()
    }

    /// Read the tasks-extension entry, if any.
    fn tasks_entry(capabilities: &ServerCapabilities) -> Option<&Value> {
        capabilities
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(TASKS_EXTENSION_KEY))
    }

    /// A backend-configured server gains the extension entry, and its value is
    /// EXACTLY `{}` — not merely present.
    ///
    /// Equality with `{}` rather than `is_some()` is the assertion that fails if
    /// a future change starts projecting `default_tasks_capability()`'s
    /// `list`/`cancel`/`requests` flags into the extension value: advertising
    /// `list: true` on an era where `tasks/list` answers `-32601` is the
    /// capability lie D-03 forbids.
    #[test]
    fn capability_rule_advertises_the_tasks_extension_when_a_backend_exists() {
        let mut capabilities = ServerCapabilities::default();
        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        assert_eq!(
            tasks_entry(&capabilities),
            Some(&serde_json::json!({})),
            "a backend-configured server must advertise the tasks extension as \
             the EMPTY OBJECT (D-03): {capabilities:?}"
        );
        // The v1 arm is unchanged by the v2 arm — one knob, two advertisements.
        assert!(
            capabilities.tasks.is_some(),
            "the v1 tasks capability must still be auto-advertised: {capabilities:?}"
        );
    }

    /// An EXPLICITLY configured extension value survives the rule byte-unchanged.
    ///
    /// This is the extensions-map twin of the `capabilities.tasks.is_none()`
    /// guard: an operator-supplied value is the operator's, and silently
    /// rewriting it would be worse than serving it.
    #[test]
    fn capability_rule_preserves_an_explicitly_configured_tasks_extension_value() {
        let explicit = serde_json::json!({ "io.example/nonconformant": true });
        let mut capabilities = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(TASKS_EXTENSION_KEY.to_string(), explicit.clone());
        capabilities.extensions = Some(extensions);

        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        assert_eq!(
            serde_json::to_string(tasks_entry(&capabilities).expect("entry present")).unwrap(),
            serde_json::to_string(&explicit).unwrap(),
            "an explicitly configured extension value must survive the rule \
             byte-unchanged: {capabilities:?}"
        );
    }

    /// A server with NO task backend gains neither the v1 capability nor the v2
    /// extension entry.
    ///
    /// The endpoint-backed rule's whole point: presence of the key is a promise
    /// that `tasks/*` works, so a backend-less server must make no such promise
    /// on either era.
    #[test]
    fn capability_rule_advertises_nothing_without_a_backend() {
        let mut capabilities = ServerCapabilities::default();
        apply_tasks_capability_rule(&mut capabilities, &no_tools(), false).unwrap();

        assert!(
            capabilities.tasks.is_none(),
            "no backend must mean no v1 tasks capability: {capabilities:?}"
        );
        assert_eq!(
            tasks_entry(&capabilities),
            None,
            "no backend must mean no v2 extension entry: {capabilities:?}"
        );
        assert!(
            capabilities.extensions.is_none(),
            "and the rule must not manufacture an empty extensions map: {capabilities:?}"
        );
    }

    /// An unrelated pre-existing extensions key is still present afterwards.
    ///
    /// The insert is alongside, never a replacement of the map.
    #[test]
    fn capability_rule_leaves_an_unrelated_extensions_key_intact() {
        let mut capabilities = ServerCapabilities::default();
        let mut extensions = HashMap::new();
        extensions.insert(
            "io.example/experimental".to_string(),
            serde_json::json!({ "enabled": true }),
        );
        capabilities.extensions = Some(extensions);

        apply_tasks_capability_rule(&mut capabilities, &no_tools(), true).unwrap();

        let extensions = capabilities.extensions.as_ref().expect("map present");
        assert_eq!(
            extensions.get("io.example/experimental"),
            Some(&serde_json::json!({ "enabled": true })),
            "an unrelated extensions key must survive: {extensions:?}"
        );
        assert_eq!(
            extensions.get(TASKS_EXTENSION_KEY),
            Some(&serde_json::json!({})),
            "and the tasks entry lands alongside it: {extensions:?}"
        );
    }
}
