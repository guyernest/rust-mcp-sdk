---
phase: 113
reviewers: [gemini, codex]
reviewed_at: 2026-07-25T01:01:45Z
plans_reviewed: [113-01-PLAN.md, 113-02-PLAN.md, 113-03-PLAN.md, 113-04-PLAN.md, 113-05-PLAN.md, 113-06-PLAN.md, 113-07-PLAN.md, 113-08-PLAN.md, 113-09-PLAN.md, 113-10-PLAN.md, 113-11-PLAN.md, 113-12-PLAN.md]
---

# Cross-AI Plan Review — Phase 113

## Gemini Review

# Plan Review: PMCP SDK Phase 113 — Stateless HTTP + Multi-Round-Trip Elicitation

## Executive Summary & Overall Verdict

**Verdict:** **APPROVED WITH HIGH CONFIDENCE**

Phase 113 presents a exceptionally well-researched, architecturally sound, and security-hardened implementation plan for extending the PMCP SDK with MCP 2026-07-28 (v2) Stateless HTTP transport and Multi-Round-Trip Requests (MRTR / SEP-2322).

### Key Highlights
- **Zero-Fork Era Gating:** Era-gating onto the existing `stateless()` streamable-HTTP branch using a unified `sessions_active(state, era)` predicate avoids transport code duplication and guarantees byte-identical behavior for v1 connections.
- **Cryptographically Robust `requestState` Token:** The AEAD token design (`ring::aead`, ChaCha20-Poly1305 / AES-256-GCM) incorporates principal, method, and salient parameter digest in AAD, mitigating cross-principal and cross-request replay attacks.
- **Conformance & DX Reconciliation:** The `key_id` prefix strategy cleverly resolves the tension between developer DX (fail-closed re-elicitation on fallback key mismatch across instances) and strict conformance testing (`sep-2322-reject-tampered-state` requiring JSON-RPC errors for tampered tokens).
- **Semver & Package Legitimacy Preservation:** Adapting MRTR parameters dynamically via `src/types/mrtr.rs` without modifying public request structs (`CallToolRequest`, etc.) preserves semver stability. Promoting `ring` and `zeroize` to explicit optional dependencies under `streamable-http` adds zero new crates to `Cargo.lock`.

---

## 1. Architectural & Requirement Coverage Matrix

| Requirement | Description | Plan Alignment & Implementation Strategy | Assessment |
| :--- | :--- | :--- | :--- |
| **HTTP-01** | Stateless HTTP (no `initialize` handshake, no `Mcp-Session-Id` header) | Wave 2 (`113-04`): Era-gated onto `stateless()` via `sessions_active(state, era)`. GET/DELETE yield 405 on v2 path. | **Pass** — Zero transport fork |
| **HTTP-02** | `input_required` disposition with `inputRequests` + AEAD `requestState` token | Wave 1 & 2 (`113-02`, `113-03`): Typed MRTR adapter; AEAD token authenticated with AAD binding (principal + method + salient param digest). | **Pass** — Replay-proof & tampered-state checked |
| **HTTP-03** | Client retry with `inputResponses` + `requestState` resumes operation | Wave 3 (`113-06`, `113-07`): Server ingress extracts & validates state; client auto-orchestrates bounded retry with a **fresh** JSON-RPC ID per round. | **Pass** — Fully satisfies SEP-2322 |
| **HTTP-04** | Change notifications via `subscriptions/listen` long-lived stream | Wave 5 (`113-10`): Opt-in SSE stream. Default server configuration advertises no subscription capabilities and returns `-32601` (404), matching official conformance skip rules. | **Pass** — Reconciles D-11 & D-13 |
| **HTTP-05** | No `Last-Event-ID` resumability on v2; response ID derived from live request | Wave 4 (`113-08`): Ignores `Last-Event-ID` on v2; forces response envelope serialization to always use the active request's ID (fixes discovery cache bug class). | **Pass** — Closes ID-replay vulnerability |
| **CLNT-01** | pmcp `Client` speaks v2 (`_meta`, `server/discover`, headers, no `initialize`) | Wave 2 (`113-05`): Explicit per-connection v2 builder opt-in emitting `MCP-Protocol-Version`, `Mcp-Method`, and `Mcp-Name` (with `=?base64?` sentinel). | **Pass** — Full spec header compliance |
| **CLNT-02** | pmcp `Client` fulfills MRTR requests via `ClientHostRegistry` | Wave 3 (`113-07`): 3-way fold across `elicitation`, `sampling`, and `roots` handlers; bounded gather-resend loop. | **Pass** — Reuses existing host registry |

---

## 2. Review of Wave 1 Plans (`113-01-PLAN.md` & `113-02-PLAN.md`)

### Plan 113-01: Foundations & Error Codes
- **Strengths:**
  - Establishes `113-SPEC-RECHECK.md` as an explicit checkpoint to guard against last-minute spec drift before final publication.
  - Promotes `ring` (0.17) and `zeroize` (1.8) to explicit optional dependencies under `streamable-http`. `cargo tree` criteria verify reachability while ensuring zero new entries land in `Cargo.lock`.
  - Defines missing v2 error codes (`HEADER_MISMATCH` -32020, `MISSING_REQUIRED_CLIENT_CAPABILITY` -32021, `UNSUPPORTED_PROTOCOL_VERSION` -32022) with locking unit tests.
- **Verification:** The gated dependency promotion includes appropriate checks (`cargo tree -p pmcp -e normal --features streamable-http --depth 1`) to guarantee direct reachability without dragging `ring` onto `wasm32-unknown-unknown` builds.

### Plan 113-02: MRTR Protocol-Type Layer & Test Harness
- **Strengths:**
  - `src/types/mrtr.rs` serves as a clean, centralized adapter for MRTR wire types (`InputRequests`, `InputResponses`, `InputRequest`, `InputResponse`).
  - Implements `encode_header_value` / `decode_header_value` for non-ASCII `Mcp-Name` values using the spec-mandated `=?base64?...?=` sentinel format.
  - Implements `salient_param_digest` using a strict whitelist approach (canonicalizing parameter keys to make digests order-independent while ignoring `_meta`, `inputResponses`, and `requestState`).
  - Fixes `ElicitRequestParams` deserialization in `src/types/elicitation.rs` so form elicitations omitting `mode` default to `"form"`, resolving compatibility with v2 spec forms.
  - Lifts the test harness into `tests/common/v2.rs`, providing a unified helper (`v2_body`, `v2_headers`, `spawn_default_config`) across integration test suites.

---

## 3. Critical Technical Edge Cases & Recommendations

### 1. Distinguishing Unknown Key IDs from Tampered Tokens (D-15 & Conformance)
- **Context:** In multi-instance deployments where `PMCP_REQUEST_STATE_KEY` is unset, instances fall back to random per-process keys (D-04). Retrying against a different instance fails decryption.
- **Review Finding:** A naive decryption failure handler that re-elicits would fail official conformance check `sep-2322-reject-tampered-state`, which demands a JSON-RPC error when a token is tampered.
- **Recommendation:** Ensure `src/server/request_state.rs` strictly adheres to the planned layout: `base64url(key_id || nonce || ciphertext)`.
  - **Unknown `key_id`:** Instance cannot verify token $\rightarrow$ return fresh `InputRequiredResult` (re-elicit, degraded multi-instance fallback).
  - **Known `key_id` + Tag/Auth Failure:** Token modified or forged $\rightarrow$ return JSON-RPC error `-32602` (conformance compliant).

### 2. Result Envelope Placement for `serverInfo` (Pitfall 6 Fix)
- **Context:** Phase 112 injected `serverInfo` as a top-level property on `Result`. The 2026-07-28 spec places it inside `result._meta["io.modelcontextprotocol/serverInfo"]`.
- **Recommendation:** Verify during Wave 4 (`113-09`) that `inject_v2_result_envelope` constructs `_meta` if absent and places `serverInfo` inside `_meta`.

### 3. Client Capability Derivation (`_meta.clientCapabilities`)
- **Context:** Spec obligation 7 prohibits servers from returning `inputRequests` for client capabilities that were not declared during request negotiation.
- **Recommendation:** In Wave 2 (`113-05`), the client builder should automatically derive `_meta.clientCapabilities` from registered handlers in `ClientHostRegistry` (`elicitation`, `sampling`, `roots`), preventing mismatch errors (`-32021`).

### 4. Trace Context Preservation across MRTR Rounds
- **Context:** Requests carry W3C trace context (`traceparent`, `tracestate`) in `_meta`.
- **Recommendation:** In Wave 3 (`113-07`), ensure the client MRTR loop preserves `_meta` trace context headers across retry rounds so distributed tracing spans remain linked.

---

## 4. Final Summary

The execution roadmap (Waves 1 through 6) is well-ordered and defensively designed. 

- **Wave 1:** Core types, dependencies, error codes, and shared test harness.
- **Wave 2:** AEAD token state engine (`113-03`), HTTP era gate (`113-04`), and client transport headers (`113-05`).
- **Wave 3:** Server MRTR ingress (`113-06`) and client gather-resend loop (`113-07`).
- **Wave 4:** Resumability suppression (`113-08`) and MRTR server egress (`113-09`).
- **Wave 5:** `subscriptions/listen` stream (`113-10`) and E2E conformance harness (`113-11`).
- **Wave 6:** Feature matrix verification and quality gate sweep (`113-12`).

Proceed with executing Wave 1 (`113-01-PLAN.md` and `113-02-PLAN.md`).

---

## Codex Review

# Phase 113 Plan Review

> External-review note: I attempted the available Gemini, Cursor, Claude, and separate Codex CLIs. Sandbox restrictions blocked Gemini/Codex, Cursor’s agent crashed while updating, and Claude was unauthenticated. Therefore this is a direct Codex review, not a multi-model consensus. No repository files were changed.

## Overall assessment

The plans show unusually strong traceability, security thinking, v1 regression coverage, semver awareness, and validation intent. However, they are not ready to execute as written. Several cross-plan contradictions make core outcomes impossible or unsafe: coding against a pre-final draft, an unusable client “return input_required” API, a process-global crypto codec, impossible unknown-key re-elicitation, server-controlled fields that handlers may spoof, incomplete `subscriptions/listen` client support, and incorrect dependency ordering.

Overall risk: **HIGH**. The phase goals are achievable, but the plan set needs an architectural correction pass before implementation.

## Blocking findings

1. **Final-spec gate is not actually blocking.** Plan 01 permits using the draft on July 24 and then hard-coding values, contradicting the requirement that wire-exact values come only from the published July 28 schema.
2. **The client cannot return an unfulfilled `input_required` result.** Existing methods return `CallToolResult`, `GetPromptResult`, or `ReadResourceResult`; deserialization discards `inputRequests`, `requestState`, and `resultType`.
3. **Unknown-key/expired re-elicitation is underspecified or impossible.** With no decrypted continuation, the server cannot mint a meaningful fresh state-only token while also refusing to invoke the handler.
4. **The crypto codec cannot be process-global.** A `OnceLock` conflicts with builder overrides, multiple servers, integration tests, key rotation, and reliable startup validation.
5. **Handler-controlled `resultType` defeats the eligibility tripwire.** Preserving a handler-provided reserved field lets a handler emit `input_required` on any method.
6. **The internal MRTR signal leaks on v1 and unsupported methods.** Plan 09 explicitly leaves signal-bearing v1 results untouched, exposing continuation state in plaintext.
7. **HTTP-04 lacks the pmcp Client half.** The plans implement a raw server SSE route but no `Client::subscriptions_listen` stream API, and existing v2 client subscribe methods still use retired RPCs.
8. **Mandatory project workflow is not followed.** Contract updates are deferred until Plan 12 rather than preceding implementation; PDMT todo generation and PMAT quality-proxy writes are absent.

---

## Plan 01 — Foundations

[113-01-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-01-PLAN.md)

**Summary:** Good foundational scope, but the schema checkpoint does not enforce the project’s final-spec-only rule.

**Strengths**

- Centralizes the three new error codes with locking tests.
- Carefully validates that `ring` and `zeroize` add no new packages.
- Preserves optional/wasm feature boundaries.
- The human dependency-legitimacy checkpoint is explicit and evidence-based.

**Concerns**

- **HIGH:** Task 1 falls back to `schema/draft/schema.ts`, after which Task 3 still lands wire constants. That violates VERS-06 and the stated out-of-scope rule against pre-final hard-coding.
- **MEDIUM:** `Cargo.lock` is omitted from `files_modified`; promoting direct dependencies normally changes the root package’s dependency list in the lockfile.
- **MEDIUM:** The zeroize-rejection fallback contradicts unconditional must-haves and acceptance criteria requiring `zeroize`.
- **MEDIUM:** The checkpoint records the schema commit but does not re-pin the official conformance-suite commit.
- **LOW:** A blocking human gate in Wave 1 makes the phase non-autonomous, which should be reflected at the phase level.

**Suggestions**

- Make publication of `schema/2026-07-28` a hard prerequisite. If absent, stop after recording `PENDING`, not `CONFIRMED`.
- Re-pin and record the conformance-suite commit in the same checkpoint.
- Add `Cargo.lock` to modified files.
- Remove the zeroize fallback or make all must-haves and acceptance criteria conditional.

**Risk assessment:** **HIGH**, because every downstream wire decision depends on this gate being authoritative.

---

## Plan 02 — MRTR types and shared harness

[113-02-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-02-PLAN.md)

**Summary:** A strong attempt at a single wire adapter, but parsing semantics and public-surface choices need tightening.

**Strengths**

- Correctly avoids adding fields to constructible public request structs.
- Correctly locates MRTR fields at top-level `params`.
- Centralizes eligible methods, logical names, header encoding, and salient-parameter hashing.
- Includes property tests and literal wire-key assertions.
- The stateful test-server helper is an important defense against vacuous tests.

**Concerns**

- **HIGH:** `extract_mrtr_params` conflates absent, malformed, oversized, and wrong-shaped values by returning `None`. A tampered or oversized `requestState` can therefore be treated as absent rather than rejected.
- **HIGH:** `splice_mrtr_params` only inserts present fields. Across rounds, absent fields must remove stale `inputResponses` and `requestState`; otherwise old round data can leak into later retries.
- **HIGH:** The planned `v2_body` helper mentions protocol version but not the required empty-or-populated `clientCapabilities`.
- **MEDIUM:** `InputResponse` as an untagged enum can misclassify overlapping or future result shapes. The corresponding request method/key should guide decoding.
- **MEDIUM:** `pub mod mrtr` plus broad flat re-exports exposes implementation-oriented wire types despite D-10 describing an internal adapter.
- **MEDIUM:** Header encoding rules are underspecified. “Printable ASCII excluding delimiters” needs to reproduce the final spec exactly.
- **LOW:** `common_harness_smoke.rs` is created but missing from `files_modified`.

**Suggestions**

- Return `Result<MrtrRequestParams, MrtrParseError>` with distinct absent and invalid states.
- Have `splice_mrtr_params` remove both keys before inserting current values.
- Include `clientCapabilities: {}` in every shared v2 test request.
- Keep parsing helpers crate-private; expose only deliberate authoring/result types.
- Decode responses using the original `InputRequest` kind rather than an untagged guess.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 03 — `requestState` AEAD

[113-03-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-03-PLAN.md)

**Summary:** Cryptographic primitives and bindings are well selected, but codec ownership and configuration are architecturally wrong.

**Strengths**

- Uses a standard AEAD with random nonces and AAD.
- Binds principal, method, and salient parameters.
- Distinguishes unknown key IDs from authentication failure.
- Includes property and fuzz testing.
- Avoids secret-bearing `Debug` output.

**Concerns**

- **HIGH:** A process-global `OnceLock<RequestStateCodec>` prevents per-server builder configuration, multiple differently configured servers, deterministic integration tests, and clean key rotation.
- **HIGH:** Malformed configured keys are logged and replaced with random fallback keys. D-04 permits fallback only when unset; malformed production configuration should fail server construction.
- **HIGH:** `codec()` initializes on first request, not at server startup, so the required startup warning and startup validation are not reliable.
- **MEDIUM:** D-05 calls for env/builder TTL configuration, but only an env override is planned.
- **MEDIUM:** Expiry tests lack an injected clock; “already elapsed” tokens otherwise require races, sleep, or hand-crafted ciphertext.
- **MEDIUM:** Only decoded key bytes are scrubbed; the environment `String` and intermediate buffers remain.
- **MEDIUM:** An eight-byte key ID can collide, and the accepting set’s collision behavior is unspecified.
- **LOW:** A hidden public fuzz-support seam permanently expands the public API.

**Suggestions**

- Construct `Arc<RequestStateCodec>` during `Server`/`ServerCore` build and store it in server state.
- Add builder methods for current key, previous keys, TTL, and an injectable clock.
- Fail server build on malformed configured keys; use fallback only when the variable is absent.
- Return `Expired(Continuation)` if expiry must permit clean re-elicitation.
- Use a fuzz-only feature or internal harness instead of a permanent public seam.

**Risk assessment:** **HIGH**.

---

## Plan 04 — Stateless HTTP era gate

[113-04-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-04-PLAN.md)

**Summary:** The single-predicate session design is correct, but header rules and rejection plumbing remain inconsistent.

**Strengths**

- Tests against `Default::default()` rather than the already-stateless config.
- One `sessions_active` predicate minimizes v1/v2 drift.
- Explicitly covers inbound bogus session IDs, response suppression, GET/DELETE, and status mappings.
- Keeps v1 behavior separately asserted.

**Concerns**

- **HIGH:** The existing server gate requires all three headers on all v2 requests, while Plan 05 says name-less methods emit no `Mcp-Name`. `server/discover` and list requests cannot satisfy both plans.
- **MEDIUM:** `V2GateOutcome::Reject(code, message)` cannot carry the required `supported` data for `-32022` without an interface change not described here.
- **MEDIUM:** Unknown-method 404 must be applied before or during typed `ClientRequest` deserialization; mapping only an already-built response may miss raw unknown methods.
- **LOW:** Grep-based “no numeric literals” and exact test-count checks are brittle.

**Suggestions**

- Lock the final `Mcp-Name` matrix before implementation: always required with a sentinel/empty representation, or optional for name-less methods. Apply the same rule in Plans 02, 04, and 05.
- Make rejection carry structured `data`.
- Add raw-body tests for unknown methods, malformed JSON, unsupported versions, and string IDs.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 05 — v2 Client transport

[113-05-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-05-PLAN.md)

**Summary:** It identifies the correct client responsibilities, but does not explain how protocol mode reaches the generic transport or how capabilities work without initialization.

**Strengths**

- Explicit per-connection opt-in honors D-08.
- Capabilities derive from registered handlers.
- Headers derive from the outgoing body rather than duplicate caller input.
- Tests cover URI names, Unicode sentinel encoding, no handshake, and no session.

**Concerns**

- **HIGH:** `ClientBuilder` stores protocol mode, but `StreamableHttpTransport` is responsible for HTTP headers. The plan does not define how a generic `Client<T>` propagates mode into `T`.
- **HIGH:** Existing client operations call `assert_capability` using capabilities learned during `initialize`. With no initialize and no automatic discover, normal v2 calls may fail locally before sending.
- **HIGH:** Name-less requests omit `Mcp-Name`, contradicting Plan 04’s three-header gate.
- **HIGH:** The non-ASCII live test needs Plan 04’s server decoder, but Plan 05 does not depend on Plan 04.
- **MEDIUM:** `with_protocol_version` is available for all transports, although stdio-v2 behavior is explicitly out of scope.
- **MEDIUM:** Arbitrary protocol-version inputs need validation.

**Suggestions**

- Define one shared `ProtocolMode` that is passed into transport send context, or constrain the method to the streamable-HTTP client builder.
- Decide whether explicit v2 mode automatically calls `server/discover` for capabilities or makes capability enforcement era-aware. This is not the prohibited era auto-probe.
- Add dependency on Plan 04, or defer the cross-server Unicode test.
- Test every name-less v2 method against the finalized header rule.

**Risk assessment:** **HIGH**.

---

## Plan 06 — MRTR server ingress

[113-06-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-06-PLAN.md)

**Summary:** The security verdict table is good, but its re-elicitation behavior cannot be implemented with the information retained.

**Strengths**

- Verifies before handler invocation.
- Uses authenticated subject rather than self-reported client info.
- Exposes trusted continuation separately from untrusted input responses.
- Covers exact tampering, principal mismatch, argument replay, and method replay.

**Concerns**

- **HIGH:** For `UnknownKey`, no continuation can be decrypted. The server cannot mint a meaningful fresh state-only token while also refusing to invoke the original handler.
- **HIGH:** `Expired` currently contains no continuation, producing the same problem unless the handler is rerun.
- **HIGH:** Malformed/oversized states can already have been converted to `None` by Plan 02, bypassing the verdict table.
- **HIGH:** Using `""` for every unauthenticated caller does not meaningfully satisfy “principal-bound”; tokens become transferable between anonymous callers.
- **HIGH:** No count or byte limits are imposed on `inputResponses`; cloning them through `ProtocolContext` and `RequestHandlerExtra` creates a memory/CPU DoS surface.
- **MEDIUM:** Process-global codec initialization makes environment-controlled integration tests order-dependent.

**Suggestions**

- Define re-elicitation precisely:

  - Unknown key: strip MRTR fields and re-run the original handler to recreate both request and continuation.
  - Expired authentic token: either retain `Expired(Continuation)` and reseal it, or rerun the handler.

- Reject malformed/oversized fields distinctly at ingress.
- Require an authenticated/stable principal for state-bearing continuation, or explicitly define a safe anonymous-binding mechanism.
- Add limits for response count, per-entry size, total serialized size, and nesting depth.

**Risk assessment:** **HIGH**.

---

## Plan 07 — Client MRTR loop

[113-07-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-07-PLAN.md)

**Summary:** The loop semantics are mostly sound, but the public return type and execution ordering are unresolved.

**Strengths**

- Bounded logical-operation loop with fresh IDs.
- Handles all three request kinds and state-only retries.
- Correctly treats future non-`input_required` result types as terminal.
- Uses the existing host registry and preserves server-assigned keys.

**Concerns**

- **HIGH:** Existing public methods return concrete result structs. If no handler exists, those structs discard `inputRequests`, `requestState`, and `resultType`, so “return the result to the caller” is not possible.
- **HIGH:** Live fixture handlers cannot emit `input_required` before Plan 09, yet Plan 07 does not depend on Plan 09.
- **HIGH:** Mutable retry params can retain stale MRTR fields unless Plan 02’s splice helper deletes old keys first.
- **MEDIUM:** Direct handler calls risk bypassing existing sampling approval, tool-choice handling, and result-review safeguards.
- **MEDIUM:** The fold invokes handlers before proving every requested kind is fulfillable. A later missing handler can leave earlier prompts or side effects wasted.
- **MEDIUM:** Handler errors are swallowed into “return original result” without a logging or observability requirement.
- **LOW:** `Error::Protocol.code` is an `ErrorCode`, not a bare `i32`; the proposed constructor needs the proper wrapper.

**Suggestions**

- Decide the public API first. Options include:

  - New additive `MrtrOutcome<T> { Complete(T), InputRequired(InputRequiredResult) }` methods.
  - Existing methods auto-orchestrate and return a structured client-local error containing the raw result when fulfillment is impossible.
  - A lower-level raw v2 request API for manual handling.

- Make Plan 07 unit/mock-transport only, then leave real server integration to Plan 11; otherwise move it after Plan 09.
- Preflight all requested kinds before invoking any handler.
- Reuse the complete existing host-dispatch pipeline, including approvals and review hooks.

**Risk assessment:** **HIGH**.

---

## Plan 08 — No resumability and live IDs

[113-08-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-08-PLAN.md)

**Summary:** Era-gating event replay is appropriate, but the stated ID invariant conflicts with preserved v1 replay behavior.

**Strengths**

- Separately gates event reads, replay, and storage.
- Preserves legacy event-store code.
- Includes sequential, concurrent, string-ID, and error-ID tests.
- The event-store bypass reduces retained v2 envelopes.

**Concerns**

- **HIGH:** “Every response ID equals the live request ID on both eras” conflicts with v1 replay, whose historical events legitimately retain their original IDs. Direct responses and replayed events must be distinguished.
- **MEDIUM:** Sequential `tools/list` calls do not test the documented cached-envelope bug if the server never caches that response.
- **MEDIUM:** A source-code audit comment and `debug_assert!` are weaker than a design that structurally requires a live ID when enveloping a cached payload.
- **LOW:** Counting `LAST_EVENT_ID` occurrences is brittle.

**Suggestions**

- Scope the invariant to direct responses to a live request; separately assert that v1 replay retains historical event identity.
- Add a fixture that deliberately reuses a cached result payload and verifies it is re-enveloped with each live ID.
- Use an event-store spy to prove zero v2 reads/writes rather than relying only on response behavior.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 09 — MRTR server egress

[113-09-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-09-PLAN.md)

**Summary:** This plan provides the missing handler authoring surface, but reserved-field ownership and signal stripping are unsafe.

**Strengths**

- Covers tools, prompts, and resources through one signal type.
- Mints continuation state at a centralized boundary.
- Includes an exhaustive eligible-method tripwire.
- Corrects `serverInfo` nesting.
- Checks declared client capabilities.

**Concerns**

- **HIGH:** On v1, signal-bearing results are “left untouched,” exposing `dev.pmcp/mrtr` and plaintext continuation state on the wire.
- **HIGH:** On a non-eligible v2 method, early return similarly leaves the internal signal key serialized.
- **HIGH:** `inject_v2_result_envelope` preserves handler-supplied `resultType`. A handler can set `"input_required"` on `tools/list`, bypassing the exhaustive tripwire.
- **HIGH:** Handler-supplied `serverInfo` is also preserved, allowing server identity spoofing.
- **MEDIUM:** Capability checking considers only presence of `elicitation`; it must distinguish form versus URL support and any relevant sampling sub-capabilities.
- **MEDIUM:** `ReadResourceResult._meta: Option<Value>` is less constrained than the map types used by tool and prompt results, complicating safe merge behavior.
- **MEDIUM:** Capability validation occurs after signal extraction/minting instead of before work is performed.
- **LOW:** `MrtrSignal::into_meta_entry` is declared infallible even though `serde_json::to_value` returns a result.

**Suggestions**

- Treat `resultType`, server-info metadata, request state, and the internal signal key as server-owned reserved fields: overwrite or reject collisions.
- Always remove the internal signal before serialization. On v1 or an unsupported method, return a structured internal/invalid-state error rather than leaking it.
- Prefer an explicit internal `HandlerOutcome::InputRequired` over smuggling control flow through result metadata.
- Validate capability submodes before minting.
- Use consistent map-shaped result metadata.

**Risk assessment:** **HIGH**.

---

## Plan 10 — `subscriptions/listen`

[113-10-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-10-PLAN.md)

**Summary:** The capability-gated server concept matches D-13, but the planned stream registry is incomplete for production and does not fulfill the client-facing requirement.

**Strengths**

- Correctly supports both conformant configurations.
- Encodes advertise-implies-serve as a tripwire.
- Requires ack-first delivery and filter intersection.
- Retires legacy subscription RPCs only on v2.
- Recognizes connection limits and the enterprise tradeoff.

**Concerns**

- **HIGH:** No pmcp Client `subscriptions/listen` streaming API is planned. Raw `reqwest` tests do not prove that v2 pmcp clients can receive change notifications.
- **HIGH:** Existing client `resources/subscribe`/`unsubscribe` methods remain callable on v2.
- **HIGH:** Registry keys use the JSON-RPC ID alone; different principals/connections commonly reuse IDs such as `1`, causing collisions.
- **HIGH:** An unbounded channel allows an arbitrarily slow subscriber to accumulate unbounded notifications.
- **HIGH:** Disconnect-safe unregistering is not designed. A dropped SSE response can leak registry entries and concurrency permits.
- **HIGH:** Per-principal limits require an authenticated principal, but this raw transport interception does not explain how `AuthContext` reaches the listen registry.
- **MEDIUM:** In-memory streams are instance-local. Advertising the capability behind a load balancer can silently miss notifications generated on another instance.
- **MEDIUM:** The final SSE result may bypass normal `resultType`/`serverInfo` envelope injection and required outbound headers.
- **MEDIUM:** `src/server/core.rs` and client files may need changes but are absent from `files_modified`.
- **MEDIUM:** No trigger for graceful server-side closure is defined.
- **MEDIUM:** Exact field types for `resourceSubscriptions` and the `notifications` wrapper need to be locked from the final schema.

**Suggestions**

- Add an additive client streaming API and era-gate legacy client methods.
- Key entries by `(principal-or-connection-id, RequestId)`.
- Use bounded channels with a documented lag policy.
- Use an RAII/drop guard or cancellation token to unregister and release permits.
- Advertise stream capabilities only when the configured notification backend can reach all serving instances, or clearly restrict opt-in mode to single-instance/sticky deployments.
- Route stream acknowledgements and final results through shared v2 envelope/header helpers.

**Risk assessment:** **HIGH**.

---

## Plan 11 — Conformance mirror and example

[113-11-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-11-PLAN.md)

**Summary:** Excellent verification intent, but the conformance inventory is static and inherits unresolved API and ordering problems.

**Strengths**

- Mirrors wire behavior with raw HTTP rather than only self-round-tripping pmcp types.
- Adds real Client↔server integration.
- Covers mixed input kinds, multiple rounds, under-supply, and exact IDs.
- Provides the required runnable example.

**Concerns**

- **HIGH:** Tests are derived from the July 24 research table, not a freshly pinned final conformance-suite commit.
- **HIGH:** Real client tests depend on the unresolved Plan 07 return-type problem.
- **MEDIUM:** “Every official check” spans multiple test files, but the plan lacks a manifest mapping each pinned official scenario to one exact local test.
- **MEDIUM:** The example demonstrates mainly the server side; a paired example client would better prove automatic fulfillment.
- **MEDIUM:** The actual 20k fuzz run is not included in this plan’s automated verification or the final phase gate.
- **LOW:** The server-process `timeout` verification is environment-sensitive, though this workspace currently has it.

**Suggestions**

- Generate and commit a scenario-to-test inventory from a pinned conformance commit.
- Fail if an official `sep-2322` scenario exists without a local mapping.
- Add a small real v2 client to the example or a companion example.
- Include the actual bounded fuzz run in phase verification.

**Risk assessment:** **MEDIUM-HIGH**.

---

## Plan 12 — Phase gate

[113-12-PLAN.md](/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-12-PLAN.md)

**Summary:** Strong final validation scope, but it places mandatory contract work too late and omits required development workflow controls.

**Strengths**

- Explicitly addresses feature-unification false greens.
- Includes no-default, wasm, examples, semver, public API, docs, and full quality gates.
- Reconciles HTTP-04 wording and records SEP-2243 rather than silently absorbing it.
- Enumerates public API additions for human review.

**Concerns**

- **HIGH:** Contract-first work occurs after every implementation plan. Repository rules require contract update and `pmat comply check` before implementation.
- **HIGH:** The referenced `../provable-contracts/contracts/pmcp/` checkout is external to this repository and is not present in the current workspace.
- **HIGH:** Plans do not use mandatory PDMT todo generation or the PMAT quality-proxy for writes.
- **HIGH:** Requirements are marked complete even though the actual official conformance suite is deferred to Phase 118 and the final-spec gate may not have run against the published schema.
- **MEDIUM:** The dev-dependency-free all-features command is described vaguely rather than specified exactly.
- **MEDIUM:** The automated verification uses ordinary `cargo`, while the action requires an absolute rustup cargo path.
- **MEDIUM:** No coverage measurement demonstrates the mandated 80% target.
- **MEDIUM:** The final gate does not run the actual fuzz target.
- **LOW:** Updating roadmap, requirements, external contracts, and full validation in one final plan makes failure recovery cumbersome.

**Suggestions**

- Move contract updates and initial compliance checks into a new Wave 0 plan.
- Add the required PDMT and quality-proxy workflow to every implementation plan.
- Specify exact reproducible matrix commands.
- Add coverage and bounded fuzz execution to the phase gate.
- Mark requirements complete only after the published schema checkpoint and a pinned scenario inventory pass.

**Risk assessment:** **HIGH**.

---

## Cross-plan dependency assessment

The current graph needs these corrections:

| Plan | Problem | Recommended correction |
|---|---|---|
| 05 | Unicode live test requires Plan 04’s server decoder | Add dependency on 04, or move that test later |
| 07 | Scripted real server cannot emit MRTR until Plan 09 | Use a mock transport only, or move 07 after 09 |
| 09 | Reserved-field behavior depends on envelope ownership decisions | Resolve before client loop and E2E work |
| 10 | Needs client, core-envelope, auth, and notification-backend integration | Expand files/dependencies and add client work |
| 11 | Uses static research rather than final pinned suite | Depend on a real final-spec/conformance checkpoint |
| 12 | Contract-first task occurs last | Split contract work into Wave 0 |

A safer sequence would be:

1. Final schema and conformance pin; contract/PDMT setup.
2. Internal wire types and per-server codec configuration.
3. Stateless HTTP and v2 transport mode.
4. Server ingress and egress together.
5. Client outcome API and MRTR loop.
6. HTTP-05 invariants.
7. Subscription server and client surfaces.
8. Conformance mirror, example, feature matrix, and quality gate.

## Requirement coverage assessment

| Requirement | Assessment |
|---|---|
| HTTP-01 | Achievable after resolving the header contradiction |
| HTTP-02 | Partial; re-elicitation and reserved-field ownership are blockers |
| HTTP-03 | Partial; stale params and public client outcome API are blockers |
| HTTP-04 | Not fully covered; server route exists, pmcp Client support does not |
| HTTP-05 | Partial; direct-response and historical-replay ID semantics conflict |
| CLNT-01 | Partial; transport-mode propagation and capability bootstrap are missing |
| CLNT-02 | Not achieved for no-handler/decline paths with current return types |

## Final risk assessment

**Overall risk: HIGH.**

The plans have excellent testing discipline and unusually good security traceability, but several issues are architectural rather than implementation details. Executing the current waves would likely produce mid-phase redesigns, cyclic dependencies, or superficially green tests that cannot support the promised public behavior. The most important next step is a focused replanning pass around:

1. the client result/outcome API;
2. per-server request-state codec ownership;
3. exact re-elicitation semantics;
4. server ownership of reserved envelope fields;
5. the full client-and-server subscription surface;
6. the final-spec and contract-first gates.

---

## Consensus Summary

The two reviewers reached **opposite overall verdicts** — Gemini: *approved with high confidence* (proceed to Wave 1); Codex: *overall risk HIGH, needs an architectural correction pass before execution*. Codex's review is substantially deeper (per-plan findings with severities); Gemini's is a requirement-coverage validation. Where they overlap, they agree on the strengths and on four technical pressure points.

### Agreed Strengths (both reviewers)

- **Zero-fork era gating** onto the existing `stateless()` branch via the single `sessions_active(state, era)` predicate — no transport duplication, v1 behavior isolated.
- **AEAD `requestState` design** (principal + method + salient-param digest in AAD) is cryptographically sound and replay-resistant; the **key-id prefix** cleverly reconciles D-04's fail-closed re-elicitation with the `sep-2322-reject-tampered-state` conformance check.
- **Semver-preserving MRTR adapter** (`src/types/mrtr.rs` splice/extract at the wire boundary instead of widening public request structs).
- **Dependency hygiene** — `ring`/`zeroize` promoted from transitive to explicit optional deps with zero new crates, verified by `cargo tree --depth 1` + lockfile assertions.
- **Testing discipline** — property/fuzz coverage, literal wire-key assertions, stateful test-server helpers guarding against vacuous tests.

### Agreed Concerns (raised by both, differing severity)

1. **Re-elicitation semantics for unknown-key/expired tokens** — Gemini prescribes the verdict table as the thing to "strictly adhere to"; Codex goes further (HIGH): with no decrypted continuation, the server *cannot* mint a meaningful fresh state-only token without re-running the handler — the exact mechanism must be defined (strip MRTR fields and re-run handler, or carry `Expired(Continuation)`).
2. **Client capability bootstrap without `initialize`** — Gemini recommends auto-deriving `_meta.clientCapabilities` from registered host handlers; Codex flags (HIGH) that existing `assert_capability` calls rely on initialize-learned capabilities and may fail locally on v2.
3. **`serverInfo` / result-envelope ownership** — Gemini says verify `_meta` placement in Wave 4; Codex flags (HIGH) that handler-supplied `resultType`/`serverInfo` are *preserved* by the envelope injector, enabling spoofing and tripwire bypass — reserved fields must be server-owned (overwrite or reject).
4. **Conformance fidelity** — both anchor correctness to the official `sep-2322` checks; Codex additionally demands a freshly pinned conformance-suite commit + scenario-to-test manifest rather than the July-24 research table.

### Divergent Views (worth investigating before execution)

- **Overall readiness**: Gemini = proceed; Codex = replan first. Codex's 8 "blocking findings" are the crux — most are architectural (client return-type for unfulfilled `input_required`, process-global `OnceLock` codec vs per-server builder config, HTTP-04 missing the pmcp-Client streaming half, v1 leakage of the internal MRTR signal, `subscriptions/listen` registry robustness: id-collision keys, unbounded channels, drop-guard unregistration).
- **HTTP-04 coverage**: Gemini scores it *Pass* (server route + conformant skip); Codex scores it *not fully covered* because no `Client::subscriptions_listen` streaming API is planned and legacy client subscribe methods stay callable on v2.
- **Final-spec gate**: Codex insists the July-28 published schema be a hard prerequisite for wire constants (draft fallback = violation of VERS-06); Gemini treats the SPEC-RECHECK checkpoint as adequate.
- **Project-workflow compliance**: only Codex flags contract-first ordering (contracts updated in Plan 12 instead of before implementation) and absent PDMT/PMAT-proxy usage.

### Top shared action items

1. Define exact re-elicitation mechanics for `UnknownKey`/`Expired` verdicts (the one concern both reviewers converge on hardest).
2. Decide the client-facing API for unfulfilled `input_required` (additive `MrtrOutcome`-style return vs structured error) before Wave 3.
3. Make reserved envelope fields (`resultType`, `serverInfo`, MRTR signal key) server-owned — strip/overwrite on every path, including v1 and non-eligible methods.
4. Resolve the `Mcp-Name` header matrix contradiction between Plans 04 and 05 (three-headers-always vs no-name-methods-omit) against the final spec.
