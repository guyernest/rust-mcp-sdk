---
phase: 112-version-plumbing-spine
verified: 2026-07-22T23:35:35Z
status: gaps_found
score: 2/5 roadmap success criteria verified (2/9 requirements BLOCKED, 1 additional requirement partially blocked)
overrides_applied: 0
gaps:
  - truth: "A v2 client calling server/discover receives a read-only projection of already-computed ServerCore capabilities, including the extensions map (VERS-04, VERS-08, ROADMAP SC#3)"
    status: failed
    reason: >
      server/discover is completely unreachable in production on every transport, for every era.
      The crate-private routing seam (parse_request_or_internal / IngressRequest::Internal /
      dispatch_internal_client_request / handle_discover) built in Plans 03/05 is exercised only
      by unit tests. Both production entry points — stdio (src/shared/transport.rs:138,
      parse_method_message) and the streamable-HTTP fast path (src/server/streamable_http_server.rs,
      via StdioTransport::parse_message) — call exclusively the PUBLIC
      crate::shared::parse_request(), which unconditionally maps IngressRequest::Internal to
      Error::method_not_found (-32601) regardless of the resolved era. Plan 05's own SUMMARY
      states live-transport wiring was "deferred to Plan 07 (dispatch) / Plan 08
      (streamable-http)" but the actual Plan 07/08 scope (per their PLAN frontmatter and
      SUMMARYs) was exclusively the error-code-literal migration — neither plan added a
      caller of dispatch_internal_client_request. handle_discover and
      dispatch_internal_client_request still carry #[allow(dead_code)] in the shipped code.
      REQUIREMENTS.md marks VERS-04 "[x] Complete" and the code-review classified this only as
      Info (IN-01) — both understate the severity: this is not a documentation/comment
      accuracy issue, it is the literal absence of the ROADMAP's success criterion #3.
    artifacts:
      - path: "src/shared/protocol_helpers.rs"
        issue: "parse_request() (the only production entry point transports call) discards IngressRequest::Internal -> -32601, always, regardless of era. parse_request_or_internal (which preserves the Internal variant) has no production caller."
      - path: "src/server/core.rs"
        issue: "dispatch_internal_client_request (:552) and handle_discover (:578) are #[allow(dead_code)], called only from #[cfg(test)] (lines 2110/2140/2152/2189)."
      - path: "src/shared/transport.rs"
        issue: "parse_method_message (:138), the real stdio/HTTP message-parsing entry point, calls the public parse_request — never parse_request_or_internal."
    missing:
      - "A live call site (in core.rs's handle_request/handle_request_internal or the transport layer) that, for an era==V2 request, routes through parse_request_or_internal/classify_internal_method and invokes dispatch_internal_client_request instead of falling through to method_not_found."
  - truth: "ProtocolContext resolved once at ingress is threaded through dispatch to EVERY handler (not just tools/call), and v2 requests self-describe via per-request _meta on any method (VERS-01, VERS-03, ROADMAP SC#1)"
    status: failed
    reason: >
      The per-request `_meta` extraction that feeds era resolution, and the RequestHandlerExtra
      protocol_context threading that makes it handler-visible, are BOTH wired for
      tools/call only — not for the general dispatch layer the goal/requirement describes.
      (1) src/server/core.rs:1750 extract_request_meta_value() — the single function both
      native ingress sites (core.rs, mod.rs) call to read the per-request `_meta` signal before
      resolving ProtocolContext — pattern-matches ONLY ClientRequest::CallTool; its own doc
      comment states "every other request yields None and resolves to the v1 fallback." Verified
      directly against GetPromptRequest/ReadResourceRequest, both of which DO carry a `_meta:
      Option<RequestMeta>` field (src/types/prompts.rs:283, src/types/resources.rs:167) that is
      silently ignored. (2) Independent of (1): handle_get_prompt / handle_read_resource in
      BOTH src/server/core.rs (:894, :962) and src/server/mod.rs (:1866, :1978) construct
      RequestHandlerExtra WITHOUT calling .with_protocol_context(...) or .with_request_meta(...)
      at all — only handle_call_tool does (core.rs:663-670, mod.rs:1639/1643). So even if (1)
      were fixed, a PromptHandler/ResourceHandler implementation still could not call
      extra.era()/.client_info()/.client_capabilities()/.trace_context() — those accessors
      always return None inside a prompt or resource handler today, regardless of what the v2
      client sends. This is a materially larger gap than the code review's WR-01 (which scoped
      the symptom to the HTTP header gate); it is a spine-completeness gap that also silently
      degrades the top-level resultType/serverInfo envelope (VERS-07) and W3C trace-context
      propagation (VERS-09) for the same two methods, on every transport (not just HTTP).
    artifacts:
      - path: "src/server/core.rs"
        issue: "extract_request_meta_value (:1750-1762) only reads _meta from ClientRequest::CallTool; handle_get_prompt (:894) and handle_read_resource (:962) never call .with_protocol_context()/.with_request_meta()."
      - path: "src/server/mod.rs"
        issue: "handle_get_prompt (:1866) and handle_read_resource (:1978) never call .with_protocol_context()/.with_request_meta() (only handle_call_tool at :1639/1643 does)."
    missing:
      - "extract_request_meta_value must read _meta from every ClientRequest variant that carries the per-request signal (at minimum GetPrompt/ReadResource), not just CallTool."
      - "handle_get_prompt/handle_read_resource at both native dispatch sites must thread the ingress-resolved protocol_context (and request_meta) into the RequestHandlerExtra passed to the handler, mirroring handle_call_tool."
    related_review_finding: "WR-01 in 112-REVIEW.md (scoped narrowly to the HTTP header gate; the root cause and blast radius are broader — see reason above)."
  - truth: "The required v2 HTTP headers are enforced correctly for all documented name-bearing methods (VERS-05, ROADMAP SC#4)"
    status: failed
    reason: >
      Direct consequence of the gap above. src/server/streamable_http_server.rs:470
      is_name_bearing_method() lists tools/call, prompts/get, and resources/read as
      v2-enforced (MCP-Name cross-checked for all three, per the header docs at
      src/shared/http_constants.rs:18). A compliant v2 client sending
      MCP-Protocol-Version:2026-07-28 on prompts/get or resources/read is rejected with a live
      400 because extract_request_meta_value never surfaces that method's _meta, so
      resolve_protocol_context falls back to v1, and classify_era_cell(header=V2, meta=V1) hits
      the fail-closed "REJECT — header claims v2 but _meta disagrees" cell. v2 is effectively
      unreachable for 2 of the 3 documented name-bearing methods; only tools/call works.
    artifacts:
      - path: "src/server/core.rs"
        issue: "Same extract_request_meta_value defect as the entry above; this is where the wire-visible symptom (400 on legitimate v2 requests) originates."
    missing:
      - "Fix extract_request_meta_value (see the entry above) so the header/_meta reconciliation has a correct _meta signal to reconcile against for prompts/get and resources/read."
    related_review_finding: "WR-01 in 112-REVIEW.md."
deferred: []
human_verification: []
---

# Phase 112: Version Plumbing Spine Verification Report

**Phase Goal:** ProtocolContext resolved once at ingress + threaded through dispatch; 2026-07-28 as explicit opt-in (LATEST stays 2025-11-25); server/discover, extensions map, required v2 headers, resultType envelope, W3C trace-context, centralized version-gated error-code table. v1 wire output must remain byte-identical.
**Verified:** 2026-07-22T23:35:35Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | v2-opt-in server resolves `ProtocolContext` once at ingress from per-request `_meta`; a handler reads it via typed accessors on `RequestHandlerExtra`; v2 results carry `serverInfo` (VERS-01, VERS-03) | ✗ FAILED | True ONLY for `tools/call`. `extract_request_meta_value` (core.rs:1750) reads `_meta` from `ClientRequest::CallTool` exclusively (own doc comment: "every other request yields `None`"); `handle_get_prompt`/`handle_read_resource` at both native dispatch sites (core.rs:894/962, mod.rs:1866/1978) never call `.with_protocol_context()`/`.with_request_meta()`. `extra.era()`/`.client_info()`/`.trace_context()` always return `None` inside a prompt/resource handler. |
| 2 | An existing v1 client negotiates exactly as before — `LATEST_PROTOCOL_VERSION` stays pinned to `2025-11-25`; `2026-07-28` reached only via explicit opt-in (VERS-02) | ✓ VERIFIED | `src/types/protocol/version.rs:4` unchanged (`LATEST_PROTOCOL_VERSION = "2025-11-25"`); `SUPPORTED_PROTOCOL_VERSIONS` len 4, does NOT contain `2026-07-28` (tests `latest_version_is_2025_11_25`, line 134 assertion); `.with_supported_protocol_versions()` unset ⇒ v1-only default (builder.rs tests); non-opted-in ingress short-circuits to `Ok(None)` before any era detection (core.rs:483-491); `cargo semver-checks check-release` (v0.49.0, baseline 2.17.0) → `223 checks: 223 pass, 30 skip`, **no semver update required** (re-run independently, confirms Plan 08 SUMMARY claim). |
| 3 | A v2 client calling `server/discover` receives a read-only projection of already-computed `ServerCore` capabilities, including the `extensions` map (VERS-04, VERS-08) | ✗ FAILED | `server/discover` is unreachable in production on every transport and every era. Confirmed by tracing both production entry points (`src/shared/transport.rs:138` stdio, and the streamable-HTTP fast path which reuses the same `StdioTransport::parse_message`) — both call only the PUBLIC `crate::shared::parse_request()`, which unconditionally discards `IngressRequest::Internal` → `-32601`. `dispatch_internal_client_request`/`handle_discover` carry `#[allow(dead_code)]` and are called only from `#[cfg(test)]`. |
| 4 | On the v2 HTTP path, required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound (VERS-05) | ✗ FAILED | Enforcement machinery (classification matrix, cog-25-safe helpers, outbound emission) is well-built and works correctly for `tools/call` (10/10 `v2_required_headers` integration tests pass). But it is broken for 2 of the 3 documented name-bearing methods (`prompts/get`, `resources/read`): a compliant v2 client is rejected 400 because the era resolver never sees `_meta` for those methods (same root cause as Truth #1) — `is_name_bearing_method` (streamable_http_server.rs:470) advertises coverage the resolver cannot deliver. |
| 5 | Every result carries the `resultType` envelope discriminator defaulting to `complete`; W3C trace-context keys are surfaced via typed accessors and propagated through dispatch; all error codes resolve from one centralized version-gated table (VERS-06, VERS-07, VERS-09) | ⚠ PARTIAL (VERS-06 verified; VERS-07/09 fail outside `tools/call`) | **VERS-06 error-code centralization: VERIFIED** — `error_codes.rs` holds the full standard + pmcp `-320xx` family + frozen `V1_TASK_PENDING`, no SATD, no v2 numeric constant; `error::ErrorCode`'s 11 consts delegate via `Self(error_codes::NAME)` (confirmed in `src/error/mod.rs`); repo-wide audit (independently spot-checked) shows all compiled production emission sites migrated; frozen `-32002`/`-32601` byte-identical, locking test untouched and green. **VERS-07/VERS-09: same root-cause failure as Truth #1** — `resultType`/`serverInfo` injection and `trace_context()` both depend on the ingress-resolved `protocol_context`/`request_meta`, which is only correctly populated for `tools/call`; a v2 `prompts/get`/`resources/read` response silently gets NO envelope and the handler cannot see trace-context, even on a fully v2-negotiated connection. |

**Score:** 2/5 roadmap success criteria fully verified (Truth #2 clean; Truth #5 only its VERS-06 sub-claim clean). 3/5 FAILED.

### Requirements Coverage

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| VERS-01 | 01, 02, 04 | ProtocolContext resolved once at ingress, threaded through dispatch, handler-readable | ✗ BLOCKED | Threading exists only for `tools/call` (see Truth #1) |
| VERS-02 | 01, 04 | 2026-07-28 explicit opt-in; LATEST stays pinned | ✓ SATISFIED | Verified directly (Truth #2) |
| VERS-03 | 01, 02, 04, 05 | v2 self-describes via per-request `_meta`; v2 results carry serverInfo | ✗ BLOCKED | `_meta` extraction is CallTool-only (see Truth #1); serverInfo injection inherits the same gap for non-CallTool methods |
| VERS-04 | 03, 05 | `server/discover` read-only capability projection | ✗ BLOCKED | Unreachable in production on any transport/era (Truth #3) |
| VERS-05 | 06 | Required v2 headers enforced inbound/outbound | ✗ BLOCKED | Broken for `prompts/get`/`resources/read` (Truth #4) |
| VERS-06 | 03, 06, 07, 08 | Centralized version-gated error-code table | ✓ SATISFIED | Verified directly (Truth #5, VERS-06 sub-claim) |
| VERS-07 | 05 | `resultType` envelope, default `complete` | ✗ BLOCKED | Only fires for `tools/call` v2 responses (Truth #5) |
| VERS-08 | 04, 06 | `extensions` capability map supported in negotiation | ✓ SATISFIED | `.with_extension()` populates `ServerCapabilities.extensions` (builder.rs), which is surfaced via the pre-existing `initialize` handshake (`InitializeResult.capabilities`) — independent of the broken `server/discover` path; builder tests pass |
| VERS-09 | 01, 02 | W3C trace-context surfaced via typed accessors, propagated through dispatch | ✗ BLOCKED | `TraceContext::from_meta`/`extra.trace_context()` mechanism is solid in isolation (proptest + fuzz + unit tests all pass), but `request_meta` is only populated on the `tools/call` dispatch arm at both native sites — unavailable in prompt/resource handlers regardless of client signal |

No orphaned requirements — all 9 VERS-01..09 IDs appear in at least one plan's `requirements:` frontmatter and are traced to Phase 112 in REQUIREMENTS.md. **However, REQUIREMENTS.md currently marks all 9 as `[x]` Complete and the traceability table lists all as "Complete" — this verification found VERS-01, VERS-03, VERS-04, VERS-05, VERS-07, VERS-09 (6 of 9) to be BLOCKED against the live codebase.**

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `src/types/protocol/version.rs` | `PROTOCOL_VERSION_2026_07_28`, `Era`, `protocol_era()`, `LATEST_PROTOCOL_VERSION` unchanged | ✓ VERIFIED | All present; `SUPPORTED_PROTOCOL_VERSIONS` len 4, excludes 2026-07-28 |
| `src/types/protocol/context.rs` | `ProtocolContext`, `TraceContext::from_meta`, `MAX_TRACE_VALUE_LEN`, `resolve_protocol_context`, `ProtocolNegotiationError` | ✓ VERIFIED | All present, bounded-validated, proptest+fuzz covered |
| `src/server/cancellation.rs` | `protocol_context` field + `with_protocol_context` + `era()`/`client_info()`/`client_capabilities()`/`protocol_version()`/`trace_context()` accessors | ✓ VERIFIED (as a type/accessor surface) | All present and unit-tested. **NOTE:** the accessors are correct, but the field is only ever populated by the `tools/call` dispatch arm (see Truth #1) — the artifact is substantively built but its production callers under-wire it. |
| `src/types/protocol/error_codes.rs` | Centralized version-gated table | ✓ VERIFIED | Full standard + pmcp `-320xx` family + frozen `V1_TASK_PENDING`/`UNSUPPORTED_CAPABILITY` coexisting by name; zero SATD |
| `src/error/mod.rs` | `ErrorCode` consts delegate to `error_codes::` | ✓ VERIFIED | `grep` confirms `Self(crate::types::protocol::error_codes::NAME)` for all 11 consts |
| `src/server/builder.rs` | `.with_supported_protocol_versions()`, `.with_extension()` | ✓ VERIFIED | Present, tested (default/dual/v2-only/empty-fallback all covered) |
| `src/server/core.rs`, `src/server/mod.rs` | `resolve_protocol_context` threaded at both native sites; `handle_discover`; `inject_v2_result_envelope` | ⚠ ORPHANED (discover) / ⚠ HOLLOW (envelope+extra for non-CallTool) | `resolve_protocol_context`/envelope injection ARE wired generically at the top-level `handle_request` (correct machinery), but the *input* to that machinery (`extract_request_meta_value`) is CallTool-only, so the machinery silently produces v1 results for other methods even on v2 connections. `handle_discover`/`dispatch_internal_client_request` are present but have zero production callers (dead code). |
| `src/shared/http_constants.rs`, `src/server/streamable_http_server.rs` | `MCP_METHOD`/`MCP_NAME` constants; full v2 classification matrix; error_codes:: migration | ✓ VERIFIED (headers/matrix machinery) / ✗ affected by Truth #1's root cause for 2/3 name-bearing methods | 25/25 literal migration confirmed (`error_codes::` count matches); classifier machinery well-tested; functional correctness for `prompts/get`/`resources/read` fails per Truth #4 |
| `src/server/task_dispatch.rs`, `src/types/jsonrpc.rs` | Error-code literal migration | ✓ VERIFIED | `error_codes::V1_TASK_PENDING` present in both files; frozen locking test `pending_tasks_result_preserves_minus_32002` untouched and green |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `src/server/cancellation.rs` | `src/types/protocol/context.rs` | `protocol_context: Option<ProtocolContext>` field | ✓ WIRED | Confirmed by grep + tests |
| `src/error/mod.rs` | `src/types/protocol/error_codes.rs` | `Self(error_codes::NAME)` delegation | ✓ WIRED | Confirmed by grep on all 11 consts |
| `src/server/builder.rs` | `src/server/core.rs` | `supported_protocol_versions` stored on `ServerCore` | ✓ WIRED | Confirmed via builder tests + `is_v2_opted_in()` |
| `src/server/core.rs` | `src/types/protocol/context.rs` | `resolve_protocol_context(...)` called once at ingress | ✓ WIRED (but fed a defective `_meta` extraction for non-CallTool methods) | `resolve_ingress_protocol_context` calls `extract_request_meta_value` then `resolve_protocol_context` — the link itself is correct; the upstream input is incomplete (Truth #1) |
| `src/server/streamable_http_server.rs` | `src/server/core.rs` | Resolves `ProtocolContext` once, passes into `handle_request_internal` (pass-through, not re-resolved) | ✓ WIRED | Confirmed: HTTP layer resolves once, core.rs does not re-resolve; `resolve_protocol_context` call-count is 1 per request |
| `src/shared/protocol_helpers.rs` (`parse_request_or_internal`) | `src/server/core.rs` (`dispatch_internal_client_request`) | Live transport routing for `server/discover` | ✗ NOT_WIRED | No production caller anywhere in `src/` — confirmed by exhaustive grep across the transport layer (Truth #3) |
| `src/server/core.rs`/`mod.rs` (`handle_get_prompt`/`handle_read_resource`) | `src/server/cancellation.rs` (`with_protocol_context`) | Threading `protocol_context` into non-tool handlers | ✗ NOT_WIRED | Confirmed absent at both native dispatch sites (Truth #1) |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Library builds (native, full features) | `cargo build --lib --features full` | Clean, 0 warnings via targeted clippy | ✓ PASS |
| Library builds (wasm32) | `cargo build --lib --target wasm32-unknown-unknown` | Exit 0 (pre-existing unrelated warnings only) | ✓ PASS |
| Version/context/cancellation/builder unit tests | `cargo test --lib --features full -- server:: protocol:: cancellation` | 764 passed | ✓ PASS |
| v2 HTTP header integration tests | `cargo test --test '*' --features full -- v2_required_headers` | 10 passed | ✓ PASS (covers `tools/call` cells only; does not cover `prompts/get`/`resources/read` live requests, which is exactly the gap found) |
| `error::ErrorCode` delegates to `error_codes::` | `grep -n 'pub const .*Self(crate::types::protocol::error_codes' src/error/mod.rs` | 11 matches | ✓ PASS |
| No production caller of `dispatch_internal_client_request`/`parse_request_or_internal` | `grep -rn 'parse_request_or_internal\|dispatch_internal_client_request' src/` (excl. definitions/tests) | 0 matches outside `protocol_helpers.rs` definitions and `core.rs` `#[cfg(test)]` | ✗ FAIL (confirms Truth #3) |
| `extract_request_meta_value` handles `GetPrompt`/`ReadResource` | `grep -n 'ClientRequest::' -A2 src/server/core.rs \| sed -n '/extract_request_meta_value/,+12p'` | Only `ClientRequest::CallTool` matched; doc comment confirms "every other request yields None" | ✗ FAIL (confirms Truth #1/#4) |
| `cargo semver-checks check-release` (independent re-run) | `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | `223 checks: 223 pass, 30 skip` — no semver update required | ✓ PASS |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| — | — | No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` found in the 14 phase-modified files scanned | — | None — zero-SATD discipline held |
| `src/server/core.rs` | 551, 577 | `#[allow(dead_code)]` on `dispatch_internal_client_request`/`handle_discover` with a `// Why:` comment claiming "production transport caller lands in Plan 07/08" | 🛑 Blocker (traceability now false) | The comment names a specific follow-up (Plans 07/08) that did NOT deliver the wiring — this is a stale, now-incorrect justification, not a documented, tracked deferral. Matches code-review IN-01, but severity is elevated here because it also proves the SUMMARY's completion claim inaccurate. |

## Cross-Check of Reviewer Warnings (as requested)

**WR-01 (v2 header gate rejects `prompts/get`/`resources/read`):** CONFIRMED as real and reproducible, and the root cause is BROADER than the review scoped it. The review attributed the symptom to the HTTP header gate specifically; this verification traced the same `extract_request_meta_value` defect to the shared, transport-agnostic `resolve_ingress_protocol_context` used by both native dispatch sites (core.rs AND mod.rs, i.e. also the stdio path), and additionally found that `handle_get_prompt`/`handle_read_resource` never thread `protocol_context` into `RequestHandlerExtra` at all — a second, independent gap the review did not surface. This is **not a legitimate/documented deferral to Phase 113/114** — no SUMMARY, deferred-items.md, or Phase 113/114 ROADMAP goal text mentions fixing this; it is an unrecorded functional gap. **This blocks Phase-112 must-haves** (VERS-01/03/05/07/09 truths above) and is reflected as a BLOCKER-level gap in this report, not merely the review's advisory Warning.

**WR-02 (`inject_v2_result_envelope` mutates handler-owned verbatim `ToolOutput::Result` envelopes):** CONFIRMED present as described (`core.rs:1224`/`:1308` and the `mod.rs` twin). Verified this does **not** affect v1 output — the injection is unconditionally gated on `era == Era::V2` (confirmed by reading `inject_v2_result_envelope`'s guard clause and the golden v1-byte-identity fixtures, which pass). Confined to the opt-in v2 path; `serverInfo` is non-sensitive (server's own `Implementation`, same data already exposed via `initialize`). Consistent with the review's classification: **WARNING, not a Phase-112 must-have blocker** — the phase's explicit "v1 wire output must remain byte-identical" contract is intact; the tension is with a different phase's (Task-dispatch D-04/D-04a) verbatim-envelope guarantee, and is appropriately a forward-looking risk for Phase 113/114 tool authors who rely on `ToolOutput::Result` verbatim semantics on the v2 path. Recommend tracking as a follow-up rather than blocking Phase 112 sign-off.

## Human Verification Required

None — all findings in this report are directly observable and were confirmed by reading source, running the test suite, and re-running the semver gate; no visual/real-time/external-service behavior is in question.

## Gaps Summary

Phase 112 built solid, well-tested INFRASTRUCTURE for every deliverable in the phase goal — the `Era`/`ProtocolContext`/`TraceContext` types, the accept-list builder, the shared `resolve_protocol_context` resolver, the centralized `error_codes::` table (fully wired, VERIFIED), the v2 HTTP header classification matrix, and the `resultType`/`serverInfo` envelope injection point. Unit and property tests for each of these pieces, in isolation, are thorough and pass.

However, goal-backward verification against the live codebase found the actual WIRING of that infrastructure into the request-dispatch path is materially incomplete in two ways that make several of the phase's headline claims false in production:

1. **`server/discover` (VERS-04) is 100% unreachable** — the crate-private internal-dispatch seam exists and is unit-tested, but no production transport (stdio or HTTP) ever calls it. Every `server/discover` request, on any era, returns `-32601`. The SUMMARY for Plan 05 explicitly deferred the live wiring to "Plan 07/08," but those plans' actual scope (confirmed by reading their PLAN frontmatter and SUMMARYs) was exclusively the error-code-literal migration — the wiring was never done by any plan in this phase.

2. **Per-request `_meta`/`ProtocolContext` threading (VERS-01, VERS-03, and by extension VERS-05, VERS-07, VERS-09) only works for `tools/call`** — `extract_request_meta_value` reads `_meta` from `ClientRequest::CallTool` only (its own doc comment says so), and `handle_get_prompt`/`handle_read_resource` at both native dispatch sites never thread `protocol_context`/`request_meta` into the handler's `RequestHandlerExtra` at all. A v2 client calling `prompts/get` or `resources/read` — two of the three methods the codebase's own `is_name_bearing_method` list explicitly documents as v2-enforced — gets silently downgraded to v1 response shape (stdio) or an outright 400 rejection (HTTP), and the handler itself cannot see era/clientInfo/trace-context regardless of what the client sends.

Both gaps are reproducible directly from source (multiple independent code reads: doc comments, match-arm exhaustiveness, absence of expected method calls, exhaustive grep for production callers) — this is not a UNCERTAIN/needs-human finding, it is directly falsifiable and falsified.

REQUIREMENTS.md currently marks VERS-01, VERS-03, VERS-04, VERS-05, VERS-07, and VERS-09 as `[x]` Complete; this verification found all six to be BLOCKED against the live codebase (VERS-02, VERS-06, VERS-08 are genuinely satisfied).

**Recommended fix scope for a closure plan:**
- Extend `extract_request_meta_value` (src/server/core.rs) to read `_meta` from `ClientRequest::GetPrompt` and `ClientRequest::ReadResource` (at minimum; ideally document the policy for every method going forward) — mirrored at the `mod.rs` twin site if it has its own copy.
- Thread `protocol_context`/`request_meta` into `RequestHandlerExtra` inside `handle_get_prompt`/`handle_read_resource` at both native dispatch sites (core.rs and mod.rs), mirroring the existing `handle_call_tool` pattern.
- Wire a live production caller of `parse_request_or_internal`/`dispatch_internal_client_request` for `server/discover` — likely inside `handle_request`/`handle_request_internal` before/around the existing `parse_request` conversion, gated on the resolved era, per Plan 05/06's own design intent.
- Re-run the full Phase-112 test suite plus the two golden-fixture / matrix tests extended to cover `prompts/get`/`resources/read`, and re-run this verification.

---
*Verified: 2026-07-22T23:35:35Z*
*Verifier: Claude (gsd-verifier)*
