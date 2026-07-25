---
phase: 113-stateless-http-multi-round-trip-elicitation
plan: 04
subsystem: streamable-http-transport
tags: [http-01, stateless, era-gate, session, status-mapping, mcp-name, meta-spelling, v2]
requires:
  - "113-01: the three v2 transport error codes (HEADER_MISMATCH, MISSING_REQUIRED_CLIENT_CAPABILITY, UNSUPPORTED_PROTOCOL_VERSION) + the Mcp-Name rule record"
  - "113-02: src/types/mrtr.rs (logical_name_key, decode/encode_header_value) + tests/common/v2.rs harness"
  - "112-06/112-10: the v2 header gate, ProtocolContext resolution seam and HttpIngress classification"
provides:
  - "sessions_active(state, era) — the ONE predicate every session decision routes through"
  - "active_session_generator + apply_session_header — the only readers/emitters of session state"
  - "v2_method_not_allowed — 405 on v2 GET/DELETE, before any session work"
  - "v2_status_for_code / status_for_error / v2_dispatch_response_status — the era-gated, code-driven HTTP status table"
  - "map_unparsed_body_for_v2 — raw-level 404+-32601 with the original id for a body that never typed-parses"
  - "V2GateOutcome::Reject { code, message, data } — structured rejection payloads (-32022 carries data.supported)"
  - "the spec `_meta` wire spelling on every _meta-bearing request type (D-113-A)"
  - "`_meta` on the five list-shaped request types + widened extract_request_meta_value (D-113-B)"
  - "tests/v2_stateless_http.rs — 15 live-HTTP assertions against a STATEFUL-config server"
affects:
  - "plan 05 (client): Mcp-Name emission must use the same mrtr codec; empty for name-less methods"
  - "plan 06 (MRTR params): owns the precise -32602 mapping for a KNOWN method with malformed params"
  - "plan 08 (id preservation): v2_string_id_preserved + raw_request_id are the foundations"
  - "plan 09 (dispatch): -32021 reaches HTTP 400 automatically via the code-driven mapper"
  - "plan 10 (subscriptions/listen): its new ClientRequest variant is a COMPILE ERROR in extract_request_meta_value until classified as _meta-bearing"
  - "plan 11 (conformance): a conformant client sending spec-spelled _meta is now detected as v2"
  - "plan 12 (semver gate): BLOCKED by D-113-D — the D-113-B field additions require a major bump"
tech-stack:
  added: []
  patterns:
    - "One era predicate over a server-wide config, not a transport fork"
    - "Code-driven status mapping (read the code about to reach the wire, not the call site)"
    - "Raw-level diagnosis for bodies that never produce a typed request"
    - "serde `rename` + `alias`: conformant on egress, backward-compatible on ingress"
    - "Forward tripwires INVERTED into permanent regression guards rather than deleted"
key-files:
  created:
    - "tests/v2_stateless_http.rs"
  modified:
    - "src/server/streamable_http_server.rs"
    - "src/server/mod.rs"
    - "src/server/core.rs"
    - "src/types/tools.rs"
    - "src/types/prompts.rs"
    - "src/types/resources.rs"
    - "src/types/protocol/mod.rs"
    - "src/client/mod.rs"
    - "tests/common/v2.rs"
    - "tests/common_harness_smoke.rs"
    - "tests/v2_required_headers.rs"
decisions:
  - "The v2 header gate MOVED above session resolution in both POST entrypoints — the era must be known before the first session decision"
  - "GET/DELETE evaluate sessions at era=None, which is exactly the pre-113 config-only read, because the 405 guard already removed v2 traffic"
  - "ListResourceTemplatesRequest was widened beyond the enumerated D-113-B list, since omitting it would leave resources/templates/list the sole un-v2-able list method"
  - "v1_unknown_method_still_200 asserts pmcp's TWO distinct v1 unknown-method paths (-32601@200 for server/discover, PARSE_ERROR@400 for an unparseable method string) rather than the plan's single assumed one"
  - "The Phase-112 baseline's expected error-code VALUES move with the codes this plan changes (-32600 -> -32020, -32602 -> -32022); statuses and v1 bytes are untouched"
metrics:
  duration: 118min
  tasks: 4
  files: 11
  completed: 2026-07-25
---

# Phase 113 Plan 04: Stateless v2 HTTP (HTTP-01) Summary

Made the ERA, not the build-time config, decide whether a streamable-HTTP request has a
session — through one `sessions_active` predicate rather than a transport fork — and added the
v2 status mappings (405 / 404 / 400 + structured `data`) the spec requires. Along the way it
closed all three findings plan 02 surfaced, including the one that meant **no conformant v2
client could ever be detected as v2 at all**.

## What Was Built

### Task 1 — the `_meta` wire contract (D-113-A + D-113-B) · `73a24cf1` RED, `47eaad68` GREEN

These were scope expansions authorized by the owner after plan 02 surfaced them, and they are
prerequisites for everything else: without them a v2 `tools/list` cannot exist and a
spec-conformant client is invisible.

**D-113-A.** `CallToolRequest` / `GetPromptRequest` / `ReadResourceRequest` carry a struct-level
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` **field** to `meta`. pmcp
therefore emitted a spelling the MCP spec does not define, and silently DROPPED a spec-spelled
`_meta` on ingress — after which Phase 112's fail-closed matrix rejected the request with
"header claims v2 but `_meta` protocolVersion disagrees". Each field is now pinned with
`#[serde(rename = "_meta", alias = "meta", skip_serializing_if = "Option::is_none", default)]`:
conformant on egress, backward-compatible with pre-113 pmcp peers on ingress. The repo already
used this exact idiom two lines apart in the same file (`src/types/tools.rs:219`, `:556`) — the
three request types were simply missing it.

**D-113-B.** A stateless v2 server runs no `initialize` handshake, so the per-request `_meta`
object is the ONLY era channel — yet `ListToolsRequest` and its siblings had no `_meta` field.
An optional field was added to `ListToolsRequest`, `ListPromptsRequest`, `ListResourcesRequest`,
`ListResourceTemplatesRequest` and `CompleteRequest`, and `extract_request_meta_value` widened to
read all five. Its match stays wildcard-free, so plan 10's `subscriptions/listen` variant is a
compile error there until classified.

An absent `_meta` still emits no key at all, so **v1 wire bytes are unchanged** — pinned by
`absent_meta_emits_no_key_on_any_request_type`.

### Task 2 — `sessions_active`, the one session predicate · `2d16e23b` RED, `2baf265f` GREEN

`stateless()` is a BUILD-TIME config. A dual-version server is built with `Default::default()`,
which keeps a live `session_id_generator` — so every session decision keyed off the CONFIG and
v2 requests got session ids minted, demanded and echoed (RESEARCH Pitfall 1).

```rust
const fn sessions_active_for(cfg_has_generator: bool, era: Option<Era>) -> bool {
    !matches!(era, Some(Era::V2)) && cfg_has_generator
}
```

Every session decision now routes through it. `sessions_active` and `active_session_generator`
are the **only** readers of `config.session_id_generator`; `apply_session_header` is the **only**
emitter of `Mcp-Session-Id` (and it replaced three `.parse().unwrap()` panics with a
non-panicking insert). The two bodyless verbs evaluate at `era = None`, which is exactly the
pre-113 config-only read — justified in code by the fact that the 405 guard already removed all
v2 traffic before they run.

The structural change that makes this possible: **the v2 header gate moved above session
resolution** in both POST entrypoints. The era has to be known before the first session decision.
For v1 and non-opted-in servers the gate is a pure passthrough, so nothing observable changes.

### Task 3 — v2 status mappings, structured rejects, the locked `Mcp-Name` rule · `7ab9b205`

| Part | What landed |
|------|-------------|
| (a) | The `Mcp-Name` rule stated verbatim on `require_three_headers`, with tests in BOTH directions: `tools/list` + `Mcp-Name: ""` is `EnforceOk`; `tools/list` with the header ABSENT is a rejection. DRIFT-1 stays locked — pmcp is deliberately stricter than the draft spec. |
| (b) | `v2_method_not_allowed` answers 405 on a v2 GET/DELETE **before** header and session validation, so a v2 GET never touches session state or the event store (T-113-18). Routes stay registered; pmcp is dual-version. |
| (c) | `map_unparsed_body_for_v2` diagnoses an unknown method at the RAW level — resolving the era from `params._meta` and recovering the id from the body bytes — because an unknown method string never produces a typed `ClientRequest` for a typed-response mapper to inspect. `v2_dispatch_response_status` then re-maps handler-produced errors from the CODE about to reach the wire, which is the only way plan 09's `-32021` can ever be mapped. |
| (d) | `V2GateOutcome::Reject { code, message, data }` — `-32022` now carries `error.data.supported` (the server accept-list) so a client can retry with a mutually supported version instead of probing (T-113-51). |
| (e) | `Mcp-Name` is decoded through the SHARED `mrtr` sentinel codec before the body cross-check, so a conformant `=?base64?…?=` non-ASCII name is accepted and a malformed sentinel is a `HEADER_MISMATCH`. The module-local `logical_name_key` is deleted — client and server read one table. |

`grep -vE '^\s*(//|///|//!)' … | grep -cE '\-32[0-9]{3}'` is **0**.

### Task 4 — `tests/v2_stateless_http.rs` (15 tests) · `860cb269`

Every server spawned with `StreamableHttpServerConfig::default()`. The file header records why
in full: a build-time-stateless spawn would make every assertion vacuous, because the session
machinery would already be gone and the per-request era gate would never be under test.
`grep -c` for the other spawn helper returns **0**, including in prose.

## Key Decisions

**The gate moved above session resolution, rather than resolving the era twice.** The
alternative — a second, cheaper era read just for the session decision — is exactly research
Pitfall 2 (dual negotiation). One resolution, threaded.

**`ListResourceTemplatesRequest` was widened beyond the enumerated list.** The owner decision
named `tools/list`, `prompts/list`, `resources/list` and `completion/complete`. Omitting
`resources/templates/list` would have left it as the single remaining un-v2-able list method — an
obvious gap for plan 11's conformance run. Treated as Rule 2.

**`v1_unknown_method_still_200` asserts pmcp's two ACTUAL v1 paths.** The plan assumed one
("v1 unknown method → 200 + -32601"). Measurement shows pmcp has two: `server/discover` is
classified at ingress and reaches dispatch, answering `-32601` at HTTP **200**; an arbitrary
unknown method string fails the transport parse and answers `PARSE_ERROR` at HTTP **400** with
`id: null`. The test keeps the planned name for traceability and pins both, with the load-bearing
assertion being that **neither is 404**.

**The Phase-112 baseline's expected VALUES move with the codes.** Plan 04 is the plan that
routes the header-gate cells onto the spec-allocated codes, so `tests/v2_required_headers.rs`
now expects `HEADER_MISMATCH` where it expected `-32600` and `UNSUPPORTED_PROTOCOL_VERSION` where
it expected `-32602`. Every **status** in that file is unchanged, and the unsupported-version test
gained an assertion that `data.supported` is an array. All 25 pass.

## Deviations from Plan

### Authorized scope expansion (owner decision, recorded in the execution prompt)

D-113-A and D-113-B were implemented as Task 1 of this plan, adding
`src/types/{tools,prompts,resources,protocol/mod}.rs`, `src/server/core.rs`, `src/client/mod.rs`
and several test files to `files_modified`. Both tripwires were retired as instructed — inverted
into permanent regression guards, not deleted — and the dual-spelling workaround in
`tests/common/v2.rs` was removed.

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Two clippy pedantic failures after the era threading**

- **Found during:** Task 2, `make lint`
- **Issue:** `needless_lifetimes` on `active_session_generator`; `too_many_arguments` (8/7) on
  `assemble_discover_response_with_middleware`
- **Fix:** elided the lifetimes; introduced `DiscoverResponseShape<'_>` bundling
  `(response_session_id, v2_outbound, sessions_on)` — which also guarantees the fast and
  middleware discover paths can never drift on session-header gating
- **Commit:** `2baf265f`

**2. [Rule 3 — Blocking] Two more clippy failures after the status mapping**

- **Found during:** Task 3, `make lint`
- **Issue:** `needless_pass_by_value` on `create_error_response_with_id(id: Value)` (the `json!`
  macro borrows, so `id` was never consumed); `large_futures` (16608 bytes) on the fast-path
  dispatch call once the status mapping was threaded through it
- **Fix:** built the envelope through a `serde_json::Map` so `id` is consumed; `Box::pin`ned the
  dispatch future, matching the treatment the two POST entrypoints already had
- **Commit:** `7ab9b205`

**3. [Rule 3 — Blocking] A `_meta`-addition script mangled a doctest**

- **Found during:** Task 1, `cargo test --doc`
- **Issue:** the mechanical initializer update collapsed a `CompleteRequest { … }` inside a
  rustdoc example onto one line, producing "unexpected closing delimiter"
- **Fix:** rewrote the example by hand; a repo-wide grep confirmed no other doc block was touched
- **Commit:** `47eaad68`

**4. [Rule 3 — Blocking] `v1_unknown_method_still_200` hit the v1 session gate**

- **Found during:** Task 4, first run
- **Issue:** on a stateful-config server, a v1 non-init request is rejected for a missing session
  id BEFORE the method is ever routed, so the test never reached the behavior it was asserting
- **Fix:** the test now mints a session via a real v1 `initialize` first — which is the correct v1
  flow anyway, and makes the assertion genuinely about method routing
- **Commit:** `860cb269`

### Plan Assumptions That Did Not Hold

**5. There is no single "v1 unknown method → 200" behavior.** See Key Decisions.

**6. `cargo test --test v2_required_headers` could NOT stay byte-for-byte unchanged.** The plan
asked both for the gate to emit `-32020`/`-32022` and for the Phase-112 suite to pass untouched.
Those are mutually exclusive: that suite pins the exact code values the plan changes. The
assertions were updated (values only — every status and every v1 byte is unchanged), which is the
reading consistent with the plan's own `<behavior>` block and its live `-32020` test.

**7. GET must not be driven to EOF in a test.** A v1 GET on a stateful config opens a real SSE
stream, and the harness reads the body to completion — so a naive `v1_get_delete_unchanged` would
hang forever. The test instead drives both verbs with an unknown `Mcp-Session-Id`, which produces
an immediate 404 on v1 and 405 on v2. That is a stronger assertion anyway: it proves the 405 guard
runs BEFORE session validation.

## Findings Surfaced

### D-113-D (HIGH, OPEN, needs a phase-level decision) — D-113-B requires a MAJOR semver bump

Measured immediately after the fix landed:

```
cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
  223 checks: 222 pass, 1 fail, 0 warn, 30 skip
  --- failure constructible_struct_adds_field ---
    ListToolsRequest._meta, ListPromptsRequest._meta, ListResourcesRequest._meta,
    ListResourceTemplatesRequest._meta, CompleteRequest._meta
  Summary: semver requires new major version
```

The D-113-A half is clean (serde attributes only; `CallToolRequest` is already
`#[non_exhaustive]`). Only the five D-113-B structs are flagged: they are `pub` with all-`pub`
fields and not `#[non_exhaustive]`, so a downstream `ListToolsRequest { cursor: None }` stops
compiling. **The WIRE is unaffected** — every added field is `Option` + `default` +
`skip_serializing_if`, and the "absent `_meta` emits no key" test pins that.

There is no way to add a field to a constructible struct without this break: `#[non_exhaustive]`
and a private field are both flagged major by the same tool. Three options (accept the major;
accept it *and* mark the structs `#[non_exhaustive]` so it is the last time; or revert the fields
and resolve D-113-B from the raw body instead, which is API-free) are written up in
`deferred-items.md`. **This cross-cuts plan 12's authoritative semver gate and the ROADMAP's
"milestone stays additive (2.x minor)" scope — it cannot be decided inside this plan.**

### Cross-version note for the release (D-113-A)

pmcp now emits `_meta` on egress. A pmcp **server** older than this change accepts only `meta`,
so a new pmcp client talking to a ≤2.17 pmcp server loses the per-request `_meta` payload
(progress token, task id, namespaced keys). The `alias` fixes the ingress direction only. That
older server was already non-conformant with the spec spelling.

### Known limitation handed to plan 06

A KNOWN method whose params fail to deserialize reaches the same `method_not_found` seam as a
genuinely unknown method, so on v2 it is reported as `-32601`/404 rather than `-32602`/400.
Distinguishing them needs a method-string table this layer does not own. Documented in code on
`map_unparsed_body_for_v2`; plan 06 owns the precise per-parameter mapping.

## Verification

| Check | Result |
|-------|--------|
| `cargo test --test v2_stateless_http --features full` | **15 passed** |
| `cargo test --test v2_required_headers --features full` | **25 passed** (Phase-112 baseline) |
| `cargo test --test common_harness_smoke --features full` | **7 passed** (all three tripwires flipped) |
| `cargo test --test server_subscriptions --features full` | **6 passed** (v1 subscribe path untouched) |
| `cargo test --lib --features full -- streamable_http_server` | **35 passed** (was 25) |
| `cargo test --features full` (whole workspace suite) | **0 failures** |
| `cargo test --doc --features full` | **382 passed** |
| `make lint` (unproxied, absolute path) | **exit 0** |
| **`make quality-gate`** (unproxied `/usr/bin/make`) | **ALL TOYOTA WAY QUALITY CHECKS PASSED, exit 0** |
| `git status --porcelain -- src/ tests/` after the gate | **empty** — the gate ran against COMMITTED source |
| `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` | **222/223 — 1 MAJOR failure, see D-113-D** |

### Acceptance greps

| Criterion | Result |
|-----------|--------|
| `streamable_http_server.rs` contains `fn sessions_active` | present (+ `sessions_active_for`, `active_session_generator`) |
| no bare `config.session_id_generator` read in the four session sites | 0 — only `sessions_active` and `active_session_generator` read it |
| `StatusCode::METHOD_NOT_ALLOWED` / `StatusCode::NOT_FOUND` in the v2 paths | present |
| `error_codes::HEADER_MISMATCH` + `error_codes::UNSUPPORTED_PROTOCOL_VERSION` | present |
| `Reject {` with a `data` field + the literal `"supported"` | present |
| `decode_header_value` | present |
| `grep -c 'fn logical_name_key' streamable_http_server.rs` | **0** |
| `grep -vE '^\s*(//\|///\|//!)' … \| grep -cE '\-32[0-9]{3}'` | **0** |
| `tests/v2_stateless_http.rs` contains `spawn_default_config` | 17 occurrences |
| `grep -c 'spawn_stateless_config' tests/v2_stateless_http.rs` | **0** |
| all fifteen planned test fn names present | 15/15 |
| `v2_unknown_method_404` uses `post_raw` | yes |
| `v2_unsupported_version_400_with_supported` asserts `.is_array()` | yes |

## TDD Gate Compliance

Both `tdd="true"` tasks completed a real RED→GREEN cycle with the failure OBSERVED, not assumed.

| Task | RED commit | RED evidence | GREEN commit |
|------|-----------|--------------|--------------|
| 1 (D-113-A/B) | `73a24cf1` `test(113-04)` | 3 failing tests, e.g. `left: None, right: Some({"ns/key":"v"})` — the spec-spelled `_meta` was being dropped, and `tools/list` surfaced no signal | `47eaad68` `fix(113-04)` |
| 2 (`sessions_active`) | `2d16e23b` `test(113-04)` | `left: 400, right: 200` with body `{"error":{"code":-32600,"message":"Session ID required for non-initialization requests"}}` | `2baf265f` `feat(113-04)` |

The first RED was initially written against Rust fields that did not exist, which is a compile
error rather than an observable failure; it was restructured to drive everything from JSON so the
RED state COMPILES and FAILS. No REFACTOR commit was needed.

## Threat Model Coverage

| Threat ID | Disposition | How this plan discharged it |
|-----------|-------------|------------------------------|
| T-113-06 | mitigate | `sessions_active` means the v2 path holds no session at all, so nothing downstream can treat a session id as identity. `v2_ignores_inbound_session_id` proves an attacker-supplied id is INERT (200, nothing echoed) rather than accepted |
| T-113-08 | mitigate | The Phase-112 cross-check is retained and now sentinel-aware; its reject cell emits `HEADER_MISMATCH` at HTTP 400, so an intermediary sees an unambiguous rejection status instead of a generic `-32600` |
| T-113-18 | mitigate | `v2_method_not_allowed` runs FIRST in both handlers — before header validation and before session validation. `v2_get_405` / `v2_delete_405` send a bogus session id that a v1 request answers 404 for, so the 405 proves session state was never consulted |
| T-113-19 | mitigate | The predicate returns `true` for `era == None` (not opted in) and `Era::V1`. `v1_session_unchanged` proves mint → validate → serve still works AND that a session-less v1 request is still rejected; the 25-test Phase-112 suite is the second guard |
| T-113-50 | mitigate | `map_unparsed_body_for_v2` runs at the RAW level on the body bytes; `v2_unknown_method_404` drives it with `"totally/unknown"`, a method string that cannot deserialize into any typed request, and asserts the original id survives |
| T-113-51 | mitigate | `V2GateOutcome::Reject` carries structured `data`; `-32022` always includes `data.supported`, asserted as an array both in a unit test and live over HTTP |

## Known Stubs

None. No `TODO`/`FIXME`/`unimplemented!()` was introduced; the `make quality-gate` zero-SATD
check passes. The one documented LIMITATION (a known method with malformed params reported as
`-32601` on v2) is a bounded, in-code-documented hand-off to plan 06, not a stub.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: status-surface | `src/server/streamable_http_server.rs` | v2 responses now emit HTTP 404 and 405 where the transport previously only emitted 200/400/401/404-for-session. Intermediaries, WAFs and load balancers that key on status may treat a 404 as "endpoint gone" and a 405 as a route misconfiguration. Intentional and spec-mandated; era-gated so v1 is unaffected |
| threat_flag: error-payload | `src/server/streamable_http_server.rs` | `-32022` rejections now disclose the server's full protocol accept-list in `error.data.supported`. This is spec-required (the client must be able to pick a mutually supported version) and reveals only version strings, but it is new outbound information at an unauthenticated boundary |
| threat_flag: schema-change | `src/types/{tools,prompts,resources,protocol}.rs` | Five public request types gained an `_meta` field, and three changed their `_meta` wire spelling from `meta` to `_meta` (accepting both on ingress). Trust-boundary relevant: `_meta` is the carrier for the era signal, client identity and W3C trace context, all of which Phase 112 documents as self-reported and NOT for authorization |

## Follow-ups

1. **Phase owner / plan 12 — D-113-D is BLOCKING.** Decide between accepting the major bump,
   accepting it plus `#[non_exhaustive]`, or reverting the fields for a raw-body `_meta` read.
   Options are written up in `deferred-items.md`.
2. **Plan 05** — the client must emit `Mcp-Name` through `mrtr::encode_header_value`, empty for
   name-less methods. The server half is now tested in both directions.
3. **Plan 06** — owns the precise `-32602` mapping for a known method with malformed params.
4. **Plan 09** — `-32021` reaches HTTP 400 for free; nothing to wire at the transport.
5. **Plan 10** — `subscriptions/listen`'s new `ClientRequest` variant is a compile error in
   `extract_request_meta_value` until classified as `_meta`-bearing. Classify it that way.
6. **Plan 12** — the 113-01 spec verdict is still PENDING under a recorded exception; NO Phase-113
   requirement (including HTTP-01) is marked complete by this plan.

## Self-Check: PASSED

- `tests/v2_stateless_http.rs` — FOUND (580 lines, 15 `#[tokio::test]`)
- `.planning/phases/113-.../deferred-items.md` — FOUND (D-113-A/B/C resolved, D-113-D added)
- `src/server/streamable_http_server.rs` — FOUND (`fn sessions_active`, `v2_method_not_allowed`,
  `map_unparsed_body_for_v2`, `v2_status_for_code` all present)
- Commit `73a24cf1` — FOUND
- Commit `47eaad68` — FOUND
- Commit `2d16e23b` — FOUND
- Commit `2baf265f` — FOUND
- Commit `7ab9b205` — FOUND
- Commit `860cb269` — FOUND
