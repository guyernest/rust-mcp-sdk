---
phase: 112-version-plumbing-spine
reviewed: 2026-07-22T00:00:00Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - src/error/mod.rs
  - src/server/batch.rs
  - src/server/builder.rs
  - src/server/cancellation.rs
  - src/server/core.rs
  - src/server/mod.rs
  - src/server/streamable_http_server.rs
  - src/server/task_dispatch.rs
  - src/shared/http_constants.rs
  - src/shared/protocol_helpers.rs
  - src/types/jsonrpc.rs
  - src/types/protocol/context.rs
  - src/types/protocol/error_codes.rs
  - src/types/protocol/mod.rs
  - src/types/protocol/version.rs
  - src/utils/parallel_batch.rs
findings:
  critical: 0
  warning: 2
  info: 3
  total: 5
status: issues_found
---

# Phase 112: Code Review Report

**Reviewed:** 2026-07-22
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

Phase 112 adds v2 (`2026-07-28`) era-gating: `ProtocolContext` resolved once at ingress
and threaded through both native dispatch sites, a centralized version-gated error-code
table, required v2 HTTP headers with fail-closed header/`_meta` classification, and a
`server/discover` internal-dispatch seam.

The two areas I was asked to scrutinize most held up well:

- **v1 byte-identical output** — `inject_v2_result_envelope` and every ingress path are
  correctly gated behind `Era::V2` AND `is_v2_opted_in()`. A default (non-opted-in)
  server short-circuits to `Ok(None)` before any era detection, threads `protocol_context:
  None`, and every v2 injection/gate is a no-op. I could not find a path where a v1 or
  non-opted-in response gains or loses a byte.
- **error-code literal→constant migration** — I traced all migrated sites
  (`core.rs`, `mod.rs`, `streamable_http_server.rs`, `batch.rs`, `jsonrpc.rs`,
  `parallel_batch.rs`). Every literal maps to a constant of the identical numeric value,
  including the deliberately-preserved cases: `-32603` for the parallel-batch timeout
  (mapped to `INTERNAL_ERROR`, NOT `REQUEST_TIMEOUT`), and `-32002` server-not-initialized
  (mapped to `V1_TASK_PENDING`). The `error_codes` table is value-guarded by
  `error_code_surface_delegates_to_table`. No value drift found.

The defects below are all confined to the opt-in v2 path (v1 is unaffected). The most
substantive is an inconsistency between the v2 HTTP header gate and the `_meta` resolver
that makes two of the three explicitly-supported name-bearing methods un-serveable on v2.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: v2 header gate rejects legitimate `prompts/get` and `resources/read` requests (header/`_meta` resolver mismatch)

**File:** `src/server/core.rs:1782` (`extract_request_meta_value`), interacting with `src/server/streamable_http_server.rs:430` (`classify_era_cell`) and `:470` (`is_name_bearing_method`)

**Issue:** `extract_request_meta_value` only extracts the per-request `_meta` signal from
`ClientRequest::CallTool`:

```rust
Request::Client(boxed) => match boxed.as_ref() {
    ClientRequest::CallTool(req) => { /* reads req._meta */ },
    _ => None,
},
```

Every other method — including `prompts/get` (`GetPromptRequest._meta`) and
`resources/read` (`ReadResourceRequest._meta`), both of which DO carry a `_meta` field
(confirmed in `src/types/prompts.rs:283` and `src/types/resources.rs`) — yields `None`.
`resolve_protocol_context` therefore falls back to the first v1 version in the accept-list,
so `ProtocolContext.era` is always `Era::V1` for those methods, i.e. `meta_is_v2 == false`.

Meanwhile the HTTP gate's `is_name_bearing_method` explicitly lists `tools/call`,
`prompts/get`, and `resources/read` as v2-enforced, and the `MCP_NAME` header doc
(`src/shared/http_constants.rs:18`) says its value is cross-checked for all three. When a
v2 client sends the spec-required `MCP-Protocol-Version: 2026-07-28` header on a
`prompts/get`/`resources/read`, `classify_era_cell(V2, false)` hits the `(true, false)`
cell and returns `V2Classification::Reject("MCP-Protocol-Version header claims v2 but
_meta protocolVersion disagrees")` → a live `400`. So v2 is effectively unreachable for
two of the three name-bearing methods, and clients that follow the header contract are
actively rejected. This is a wired, production HTTP path on any opted-in server (not
dead code).

**Fix:** Extend `extract_request_meta_value` to read `_meta` from every request variant
that carries the per-request protocol signal, at minimum the name-bearing methods:

```rust
pub(crate) fn extract_request_meta_value(request: &Request) -> Option<serde_json::Value> {
    match request {
        Request::Client(boxed) => match boxed.as_ref() {
            ClientRequest::CallTool(req) => req._meta.as_ref().and_then(|m| serde_json::to_value(m).ok()),
            ClientRequest::GetPrompt(req) => req._meta.as_ref().and_then(|m| serde_json::to_value(m).ok()),
            ClientRequest::ReadResource(req) => req._meta.as_ref().and_then(|m| serde_json::to_value(m).ok()),
            _ => None,
        },
        Request::Server(_) => None,
    }
}
```

(Note the two `_meta` field types differ — `Option<RequestMeta>` vs
`Option<serde_json::Map<..>>` — so the `to_value` bridge must handle both.) Alternatively,
if v2 is intended to cover only `tools/call` this phase, remove `prompts/get`/`resources/read`
from `is_name_bearing_method` and the header docs so the gate does not advertise coverage
it cannot deliver.

### WR-02: v2 envelope injection mutates handler-owned verbatim `ToolOutput::Result` envelopes

**File:** `src/server/core.rs:1224` (`inject_v2_result_envelope`), called at `:1308` and `src/server/mod.rs` twin site

**Issue:** `DispatchOutput::Verbatim` (a `ToolOutput::Result`) is contractually
send-to-wire-VERBATIM: per `task_dispatch.rs:154-169` the dispatcher must "BYPASS response
middleware, the create-path gate, and text-wrap / widget enrichment … the handler owns the
full envelope, including its own redaction" (D-04/D-04a, marked USER-APPROVED and LOCKED).
But `inject_v2_result_envelope` runs unconditionally at the `handle_request` boundary
AFTER dispatch resolves, and for a v2 request it inserts `resultType` and `serverInfo` into
the result object — including a verbatim `CallToolResult` the handler deliberately shaped.
This re-introduces `serverInfo` a handler may have intentionally omitted, contradicting the
verbatim/bypass guarantee. Impact is limited (v2-only; `serverInfo` is the server's own
non-sensitive `Implementation`, same data as `initialize`), which is why this is a WARNING
rather than a blocker — but it is a latent contract violation that will bite once v2 tools
rely on owning their envelope.

**Fix:** Either scope the envelope injection to exclude verbatim results (thread the
`Verbatim` disposition through so the injector can skip it), or explicitly document that the
v2 `resultType`/`serverInfo` envelope is an exception to the verbatim-bypass contract and
add a regression test asserting a verbatim v2 result still receives the envelope by design.
Do not leave the two contracts silently in tension.

## Info

### IN-01: `server/discover` internal dispatch is never reachable in production

**File:** `src/server/core.rs:552` (`dispatch_internal_client_request`), `:578` (`handle_discover`)

**Issue:** Both functions are `#[allow(dead_code)]` and are called ONLY from unit tests
(`core.rs:2110-2189`). The routing seam `parse_request_or_internal`
(`src/shared/protocol_helpers.rs:46`) is consumed exclusively by the public `parse_request`,
which maps `IngressRequest::Internal` straight to `Error::method_not_found`
(`protocol_helpers.rs:89`). No transport ever feeds an `IngressRequest::Internal` to
`dispatch_internal_client_request`, so `server/discover` returns `-32601` everywhere in
production. The `#[allow(dead_code)]` justification comments claim "production transport
caller lands in Plan 07/08," but per the commit history Plans 07/08 were the error-code
literal migrations — the promised caller never arrived. The scaffolding is fine as a staged
rollout, but the traceability comment is now inaccurate and should point at the real
follow-up phase (113+) so a future reader does not hunt for a wired caller that does not exist.

**Fix:** Update the `// Why:` comments on `dispatch_internal_client_request` /
`handle_discover` to reference the actual phase that wires the transport, and add a tracking
note that `server/discover` is intentionally inert until then.

### IN-02: `V1_TASK_PENDING` constant name is misleading at the "Server not initialized" call site

**File:** `src/server/core.rs:1461` (uses `error_codes::V1_TASK_PENDING` for a not-initialized error); constant at `src/types/protocol/error_codes.rs:85`

**Issue:** The `-32002` "Server not initialized. Call initialize first." rejection sources
its code from a constant named `V1_TASK_PENDING`. The value is byte-identical and correct,
and the collision is deliberately documented and test-locked
(`both_minus_32002_meanings_coexist`, `pending_tasks_result_preserves_minus_32002`), but a
constant literally named "task pending" reading out at a server-lifecycle error site is
semantically confusing and invites a future "cleanup" that would break the frozen wire value.

**Fix:** Consider adding a second same-value alias (e.g.
`SERVER_NOT_INITIALIZED: i32 = -32002;`) documented as sharing the number, so each call site
reads by its own semantic name while the frozen value stays single-sourced — or add an
inline comment at the `core.rs` call site clarifying why the task-pending constant is used
for a not-initialized error.

### IN-03: `negotiation_error_to_rejection` emits Debug-formatted version strings to clients

**File:** `src/server/core.rs` (`negotiation_error_to_rejection`, `UnsupportedVersion` arm)

**Issue:** `format!("Unsupported protocol version: {v:?}")` uses the Debug formatter on the
version `String`, so a client-facing message renders the value quoted and escaped
(e.g. `Unsupported protocol version: "1999-01-01"`) rather than as the plain token. Minor
cosmetic/consistency issue in a wire-visible error message.

**Fix:** Use Display formatting: `format!("Unsupported protocol version: {v}")`.

---

_Reviewed: 2026-07-22_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
