# Phase 113 — Deferred / Out-of-Scope Items

Discoveries made while executing Phase-113 plans that are **not** caused by the
current plan's changes and therefore were not auto-fixed (executor SCOPE BOUNDARY).

---

## D-113-A — Typed request structs rename `_meta` to `meta` on the wire

**Found during:** plan 02, task 3 (building the shared v2 HTTP harness)
**Severity:** HIGH — blocks a conformant v2 client from being detected as v2
**Owner:** plan 04 (HTTP-01)
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

`CallToolRequest`, `GetPromptRequest` and `ReadResourceRequest` all carry
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` **field**. Verified
by probing pmcp's own serialization:

```
CallToolRequest   { _meta: Some(..) }  ->  {"name":"x","arguments":{},"meta":{...}}
GetPromptRequest  { _meta: Some(..) }  ->  {"name":"p","arguments":{},"meta":{...}}
```

and by round-tripping deserialization:

```
{"name":"x","arguments":{},"_meta":{...}}  ->  _meta == None   (silently ignored)
{"name":"x","arguments":{},"meta":{...}}   ->  _meta == Some   (accepted)
```

The MCP spec spelling is `_meta`. Because Phase 112 routes the whole per-request era
signal through `extract_request_meta_value(request)` — which reads the TYPED
`req._meta` — a conformant client that sent `_meta` got **no** era detection at
all, and its `MCP-Protocol-Version: 2026-07-28` header was then rejected as
"header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04): `rename` + `alias`.** Each of the three
fields now carries `#[serde(rename = "_meta", alias = "meta", skip_serializing_if =
"Option::is_none", default)]` — conformant on egress, backward-compatible with
pre-113 pmcp peers on ingress. The repo already used this idiom at
`src/types/tools.rs:219` and `:556`; the three request types were simply missing it.

**Workaround retired.** `tests/common/v2.rs` no longer emits a dual spelling (it
exports `REQUEST_META_KEY = "_meta"`), and the tripwire
`forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling` was
INVERTED into the permanent regression guard
`tests/common_harness_smoke.rs::typed_requests_use_the_spec_meta_spelling`, which
proves both halves (egress `_meta`, ingress accepts both spellings).

**Cross-version note for plan 12 / release:** on egress pmcp now emits `_meta`. A
pmcp **server** older than this change accepts only `meta`, so a new pmcp client
talking to a ≤2.17 pmcp server loses the per-request `_meta` signal (progress
token, task id, namespaced keys). The alias fixes the ingress direction only.
That old server was already non-conformant with the spec spelling.

---

## D-113-B — `tools/list` (and every non-`_meta`-bearing method) cannot be a v2 request

**Found during:** plan 02, task 3
**Severity:** HIGH for HTTP-01 (stateless v2 has no handshake)
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

`extract_request_meta_value` enumerated exactly three `_meta`-bearing client
requests (`CallTool`, `GetPrompt`, `ReadResource`); every other variant returned
`None`. `ListToolsRequest` had no `_meta` field at all.

A stateless v2 server has no `initialize` handshake, so the per-request `_meta`
signal is the ONLY era channel — which meant `tools/list`, `prompts/list`,
`resources/list`, `completion/complete` and `subscriptions/listen` could not be
v2 requests. They were rejected 400 with
"MCP-Protocol-Version header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04, option 3 of D-113-D): read the RAW body.**

The first attempt added an optional `_meta` field to the five list-shaped request
types. That worked, but measurement showed it forced a MAJOR semver bump
(D-113-D), so it was reverted in `b2cc87fe` and replaced in `f6735c03` by a
raw-body read that needs **zero public API change**:

- `Server::resolve_discover_protocol_context` → `resolve_raw_meta_protocol_context`
  — same behavior, no longer discover-specific, now the era resolver for EVERY
  method on the HTTP path.
- `run_v2_header_gate` reads `raw_params_meta(body)` and absorbed the former
  `run_v2_header_gate_raw` / `finish_v2_gate` pair. The `server/discover` ingress
  is now simply the one caller that passes a `body_method_override`.
- `raw_params_meta` prefers the spec `_meta` and falls back to `meta`, mirroring
  the `#[serde(rename = "_meta", alias = "meta")]` ingress contract D-113-A put on
  the typed structs, so the two readers cannot disagree about what a `_meta`
  object IS.

**This also closes the "two ingress paths disagree" defect** that plan 02 flagged
alongside D-113-A. There is now ONE era-detection path in the HTTP transport,
reading the spec-spelled `_meta` from the raw body, instead of a typed path
(3 methods) and a raw path (discover only) that covered different method sets.

The tripwire `forward_tripwire_tools_list_cannot_be_a_v2_request` is now
`tests/common_harness_smoke.rs::tools_list_is_a_valid_v2_request` (asserts 200) and
passes **because of the raw route**, not because a field was added — it was
observed RED again after the revert and GREEN again after the raw gate landed.
`tests/v2_stateless_http.rs::v2_nameless_method_empty_mcp_name_accepted` exercises
the same path live, and
`v2_gate_accepts_every_method_from_the_raw_body` covers all five list-shaped
methods at the unit level.

**Accepted cost.** Handlers can no longer read `_meta` off a typed list-request
struct, because those structs have no such field. The supported way for a handler
to reach the per-request signal is the `ProtocolContext`-derived
`RequestHandlerExtra` accessors (`era()`, `client_info()`, `trace_context()`) that
Phase 112 wired — the HTTP layer resolves the context from the raw body and threads
that SAME value into dispatch. Plans 06/09/10 should not re-litigate this.

**Plan 10 note:** `subscriptions/listen` will be v2-capable for free — the raw
reader does not care whether a `ClientRequest` variant carries a `_meta` field. If
plan 10 also wants the TYPED extractor to see it (for the non-HTTP transports), it
must add the variant to `extract_request_meta_value`, whose wildcard-free match
makes that a compile-time decision point.

---

## D-113-C — Stateful (`::default()`) config still demands a session on v2

**Found during:** plan 02, task 3
**Severity:** expected — this IS requirement HTTP-01
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `2baf265f` (test `2d16e23b`)

`StreamableHttpServerConfig::default()` keeps a live `session_id_generator`, so
`validate_non_init_session` rejected a v2 `tools/call` with 400 "Session ID required
for non-initialization requests". Plan 04 introduced the single
`sessions_active(state, era)` predicate and routed every session decision site
through it, making the ERA rather than the build-time config the decider.

The tripwire `forward_tripwire_stateful_config_still_demands_a_session_on_v2` is
now `tests/common_harness_smoke.rs::stateful_config_runs_v2_session_free`, and
`tests/v2_stateless_http.rs` carries fifteen live-HTTP assertions against a
`Default::default()` server.

---

## D-113-D — D-113-B's field additions require a MAJOR semver bump

**Found during:** plan 04 (measured immediately after the D-113-B fix landed)
**Severity:** HIGH — the v2.5 milestone is scoped as additive (2.x minor)
**Owner:** phase-level decision; plan 12 owns the authoritative
`cargo semver-checks` gate
**Status:** ✅ **RESOLVED** — owner chose **option 3**; reverted in `b2cc87fe`,
replaced by the raw-body read in `f6735c03`

**Rationale in one line:** the WIRE was always fine (`Option` + `default` +
`skip_serializing_if`, so an absent `_meta` emits no key); the break was purely
Rust SOURCE compatibility on five constructible `pub` structs — and reading
`params._meta` off the raw body at HTTP ingress achieves the same v2 coverage
with no public API surface at all.

**Proof the break is gone** (after `f6735c03`):

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
     Checked [   0.148s] 223 checks: 223 pass, 30 skip
     Summary no semver update required
```

versus the measurement that triggered the decision (below).

---

### The original measurement (kept for the record)

Measured, not inferred:

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
Checked 223 checks: 222 pass, 1 fail, 0 warn, 30 skip

--- failure constructible_struct_adds_field: externally-constructible struct adds field ---
  field ListToolsRequest._meta              (src/types/tools.rs)
  field ListPromptsRequest._meta            (src/types/prompts.rs)
  field ListResourcesRequest._meta          (src/types/resources.rs)
  field ListResourceTemplatesRequest._meta  (src/types/resources.rs)
  field CompleteRequest._meta               (src/types/protocol/mod.rs)

Summary semver requires new major version: 1 major and 0 minor checks failed
```

The D-113-A half is clean — it is serde attributes only, and `CallToolRequest` is
already `#[non_exhaustive]`. Only the five D-113-B structs are flagged: they are
`pub` with all-`pub` fields and NOT `#[non_exhaustive]`, so a downstream
`ListToolsRequest { cursor: None }` stops compiling.

**The WIRE is unaffected.** Every added field is `Option`, `default`ed and
`skip_serializing_if = "Option::is_none"`, so an absent `_meta` emits no key and
v1 bytes are byte-identical (pinned by
`src/types/protocol/mod.rs::absent_meta_emits_no_key_on_any_request_type`). This is
purely a Rust-API source-compatibility break.

There is no way to add a field to a constructible struct without this break —
`#[non_exhaustive]` and a private field are both flagged major by the same tool.

**Options put to the phase owner:**

1. **Accept the major bump** and ship this milestone as 3.0. Contradicts the
   ROADMAP's "milestone stays additive (2.x minor)" and research Pitfall 10
   ("accidental 3.0").
2. **Mark the five structs `#[non_exhaustive]` as well** while taking the break,
   so this is the LAST time a `_meta`-style addition breaks these types. Same
   major bump, better long-run shape.
3. ✅ **CHOSEN — revert the five field additions and resolve D-113-B from the RAW
   body instead.** The HTTP layer already has the body bytes and already resolved a
   raw `params._meta` for `server/discover`. Generalizing that read to every method
   makes all methods v2-able with ZERO public API change, at the cost of handlers
   no longer being able to read `_meta` off the typed list-request struct (the
   `ProtocolContext`-derived `RequestHandlerExtra` accessors still work).

### What shipping option 3 actually took

| Commit | What |
|--------|------|
| `b2cc87fe` | Pure revert — the five fields, the five `extract_request_meta_value` arms, and every mechanical `_meta: None` initializer restored **byte-identically** to the pre-plan baseline (verified with `git diff 73a24cf1~1`). Left `tools_list_is_a_valid_v2_request` and `v2_nameless_method_empty_mcp_name_accepted` RED with the original era-disagreement rejection. |
| `f6735c03` | The raw-body gate — `resolve_raw_meta_protocol_context`, `raw_params_meta`, one merged `run_v2_header_gate`, `SERVER_DISCOVER_METHOD`. Both tests GREEN again **via the raw route**. |

D-113-A was untouched by the revert: it is serde-attributes-only and semver-clean.

The 15 live tests in `tests/v2_stateless_http.rs`, the 25 Phase-112 baseline tests
and the 7 harness smoke tests all pass under the shipped design.

---

## D-113-E — the v2 client cannot read a structured JSON-RPC error off a 4xx

**Found during:** plan 05 (recorded in `113-05-SUMMARY.md`, not here)
**Severity:** HIGH — blocks any client-side dispatch on `error.code`
**Owner:** plan 07
**Status:** ✅ **RESOLVED** in plan 07, commit `cec054d4`

`StreamableHttpTransport::post_body` turned ANY non-2xx into
`Err(TransportError::Request("Request failed with status: …"))`, discarding the
JSON-RPC envelope. Plan 04 deliberately maps the v2 error codes onto 4xx statuses
(`-32601` at 404; `-32020`/`-32021`/`-32022` at 400) and plan 06 answers a
tampered or expired `requestState` with `-32602` at 400, so a v2 client saw all of
them as one opaque transport error.

**Resolution.** A new `jsonrpc_error_envelope` reader: on the **v2 path only**, a
non-2xx whose body is a well-formed JSON-RPC 2.0 frame carrying an `error` member
is fed through the normal response channel and surfaces as `Error::Protocol`
(hence `error_code()`, and plan 09's `-32021` becomes actionable). It is
deliberately strict about `jsonrpc == "2.0"` **and** the presence of `error`, so a
proxy's HTML page or JSON error document is never laundered into what a caller
reads as a server-authored protocol error — those still fail on the status. v1 is
gated out by the `v2_mode` latch and is byte-identical to prior releases.

Pinned by three unit tests in `src/shared/streamable_http.rs`
(`v2_error_envelope::{v2_surfaces_a_jsonrpc_error_carried_on_a_400,
v2_falls_back_to_the_status_error_for_a_non_envelope_body,
v1_still_errors_on_the_status_alone}`), driven against a `mockito` server.

---

## D-113-F — `Client::send_notification` never received plan 05's v2 branch

**Found during:** plan 07 (confirmed, not caused, by this plan)
**Severity:** MEDIUM — every client→server message that is NOT a request is
rejected at HTTP 400 by pmcp's own v2 gate
**Owner:** plan 13 (client subscriptions/listen) — the next plan whose work sits
on that path
**Status:** OPEN

Plan 05 put the v2 branch in `Client::dispatch_request`: on v2 the client
assembles the JSON-RPC frame, splices the reserved `params._meta` era keys, and
calls `Transport::send_raw`. `Client::send_notification` was not given the same
treatment — it still builds a typed `TransportMessage::Notification` and calls
`Transport::send`. The transport then emits `MCP-Protocol-Version: 2026-07-28`
plus the routing headers (they are derived from the body, which does carry a
`method`) while the body carries **no `_meta` era key**, which pmcp's own
`classify_v2_request` matrix classifies as `-32020 HEADER_MISMATCH` at HTTP 400.

Affected outbound messages on a v2 client:

| Path | Caller |
|------|--------|
| `Client::cancel_request` | `notifications/cancelled` |
| `Client::send_progress` | `notifications/progress` |
| `Client::notify_roots_list_changed` / `send_roots_list_changed` | `notifications/roots/list_changed` |
| the host-reply `send` inside `Client::dispatch_request` | client→server RESPONSES to a server-initiated `sampling`/`elicitation` request |

**Why plan 07 did NOT fix it.** The MRTR loop sends nothing that is not a request.
`inputRequests` are answered **locally** from the host registry and the answers
travel back as `params.inputResponses` on the next `tools/call` / `prompts/get` /
`resources/read` **request**, which goes through `dispatch_request`'s v2 raw-frame
path and is fully conformant. The fourth row above (host replies) is also
unreachable on a conformant v2 connection, because the spec forbids a v2 server
sending independent requests — MRTR replaces that direction entirely. So the gap
never blocked this plan, and a broad refactor here would have been scope creep
(executor SCOPE BOUNDARY).

**Fix shape for the owner.** Give `send_notification` the same `is_v2()` branch
`dispatch_request` has: build `JSONRPCNotification` via `create_notification`,
splice the reserved `_meta` with the existing `splice_v2_meta`, serialize, and
`send_raw`. The host-reply `send` needs the same treatment if v1-style
server-initiated requests are ever to be answered on a v2 connection. Note
`send_raw`'s `is_notification` flag is currently hard-coded `false`, which
suppresses the 202-Accepted/SSE-start behavior — a notification path needs that
parameter threaded through.

---

## D-113-F — Two PRE-EXISTING cog-25 violations in `streamable_http_server.rs`

**Found during:** plan 12, task 2 (the mandatory PMAT complexity budget)
**Severity:** MEDIUM — the PR-blocking CI gate is red, but it was red BEFORE this phase
**Owner:** unassigned — needs its own refactor slice
**Status:** ⏸️ **DEFERRED** (executor SCOPE BOUNDARY — not caused by Phase 113)

`pmat quality-gate --fail-on-violation --checks complexity` (the exact PR-blocking
invocation CLAUDE.md pins in `.github/workflows/ci.yml`) reports **3** violations at
Phase-113 HEAD. One of them — `src/client/subscriptions.rs::sse_payload_stream`, cog 26 —
WAS a Phase-113 regression and was fixed in place (commit `14fc8d64`, P1 extract-method).

The other two are **pre-existing and were measurably WORSE before this phase**:

| Function | Baseline `0c598639` (pre-113) | Phase-113 HEAD | Delta |
|----------|-------------------------------|----------------|-------|
| `handle_post_fast_path` | cognitive **35** | cognitive **30** | **−5** |
| `handle_post_with_middleware` | cognitive **36** | cognitive **31** | **−5** |

Measured by extracting `src/server/streamable_http_server.rs` at commit `0c598639`
(the last Phase-112 commit) into a scratch tree and running the identical
`pmat analyze complexity --max-cognitive 25`.

**Why not fixed here.** They are not caused by this plan's changes, and Phase 113 already
moved both in the right direction by 5 points each while adding the whole v2 header gate,
session gate, status mapper and MRTR ingress to the same two functions. Decomposing them
properly means restructuring the two POST entrypoints, which is a design change with real
regression surface across every transport test — a refactor slice of its own, not a
close-out task.

**Fix shape for the owner.** Both are the same shape: a long linear sequence of
gate → resolve → dispatch → assemble steps with early returns. P2 (extract the
gate/pipeline stages into named `fn`s returning a small decision enum) is the natural
technique, mirroring how `sessions_active` / `resumability_active` / `classify_v2_request`
were already pulled out of these functions during Phase 113. The hard cap is 50 and both
are well under it, so the `// Why:`-annotated `#[allow(clippy::cognitive_complexity)]`
escape hatch is NOT justified here — they are reducible.

**Do not** "fix" this by weakening the gate: CLAUDE.md forbids disabling, weakening or
removing it without explicit Phase-level approval.

---

## D-113-G — `make quality-gate`'s fuzz stage never fuzzes anything

**Found during:** plan 12, task 2 (the mandatory quality gate)
**Severity:** HIGH — a MANDATORY CLAUDE.md requirement reports green while doing nothing
**Owner:** unassigned — needs an owner policy decision (deviation Rule 4)
**Status:** ⏸️ **DEFERRED** (pre-existing; not caused by Phase 113)

`Makefile:10` sets `CARGO = cargo`. `Makefile:234-244`'s `test-fuzz` then runs:

```make
cd fuzz && $(CARGO) fuzz list | while read target; do \
    timeout 30s $(CARGO) fuzz run $$target || echo "Fuzz target $$target completed"; \
done
```

`cargo fuzz` requires **nightly** — it passes `-Zsanitizer=address`. The default toolchain in
this workspace is stable, so every one of the **17** fuzz targets fails to build:

```
error: the option `Z` is only accepted on the nightly compiler
error: 1 nightly option were parsed
Error: failed to build fuzz script: … -Zsanitizer=address … --bin app_widget_scanner
```

The `|| echo` swallows all 17 failures. `test-fuzz` then prints `✓ Fuzz testing completed`,
`validate-always` prints `✅ ALL ALWAYS requirements validated!`, and `make quality-gate`
exits **0** having fuzzed nothing.

Confirmed on a quality-gate run with **no** concurrent cargo process, so it is not lock
contention — it is unconditional. CLAUDE.md lists FUZZ testing as an ALWAYS requirement for
every new feature; this stage has not been meeting it.

**Why not fixed here.** Making the stage real is a policy call, not a bug fix (deviation
Rule 4). It requires choosing:
1. a toolchain pin (`RUSTUP_TOOLCHAIN=nightly` or `cargo +nightly fuzz`), and whether CI has
   nightly available;
2. a per-target budget — 17 targets at the current `timeout 30s` adds ~8.5 minutes to *every*
   `make quality-gate`, which is the repo's pre-commit gate. A `-runs=N` bound is probably
   better than a wall-clock timeout because it is deterministic;
3. whether a fuzz *failure* should be fatal (today even a genuine crash would be swallowed by
   the `|| echo`, which is the more serious half of this defect).

A phase executor should not unilaterally change the pre-commit gate's runtime for the whole
repo.

**Compensating control actually exercised in Phase 113.** Plan 12 ran the campaign explicitly:

```bash
RUSTUP_TOOLCHAIN=nightly cargo fuzz run fuzz_request_state -- -runs=20000
# exit 0 — #20000 DONE cov: 570 ft: 803 corp: 79/1905b — zero crash artifacts
```

This is precisely why the plan mandated an explicit 20k-run invocation instead of trusting
`cargo fuzz build` or the gate. Evidence is recorded in `113-FEATURE-MATRIX.md` § Fuzz.

**Fix shape for the owner.** Pin the toolchain in the recipe, replace the wall-clock timeout
with a deterministic run bound, and drop the `|| echo` so a crash is fatal:

```make
cd fuzz && RUSTUP_TOOLCHAIN=nightly $(CARGO) fuzz list | while read target; do \
    RUSTUP_TOOLCHAIN=nightly $(CARGO) fuzz run $$target -- -runs=$(FUZZ_RUNS) || exit 1; \
done
```

with `FUZZ_RUNS ?= 2000` locally and a larger value in the nightly CI job.

**Update (plan 16).** Gap item 5 is now closed for `subscription_listen_frames`
the same way: the campaign was invoked directly, bypassing the gate, and recorded
in `113-FUZZ-EVIDENCE.md`. D-113-G itself remains open and unowned — plan 16's
scope fence explicitly forbade editing the Makefile — and was re-confirmed during
that plan: the gate log carries 17 `failed to build fuzz script ... -Zsanitizer=address`
errors and still exits 0.

---

## D-113-H — Pre-existing crash artifact for the `auth_flows` fuzz target

**Found during:** plan 16, task 2 (proving `fuzz/artifacts/` empty after the
`subscription_listen_frames` campaign)
**Severity:** UNKNOWN — never triaged; possibly stale, possibly a live defect
**Owner:** none (needs one)
**Status:** open, NOT touched by plan 16

```console
$ /usr/bin/find fuzz/artifacts -type f -exec /bin/ls -la {} \;
-rw-r--r--@ 1 guy staff 8 Sep 12 2025 fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89
```

An 8-byte crash artifact dated **2025-09-12** — ten months before Phase 113 —
sits in the working tree for the `auth_flows` target. `fuzz/.gitignore` ignores
`artifacts`, so it has never been committed and no CI job has ever seen it; it
survives only because this checkout is long-lived. D-113-G is why nothing has
flagged it since: the gate builds zero targets and swallows the result.

Out of plan 16's scope fence ("running campaigns against targets other than
`subscription_listen_frames` … is not this gap"), so it was recorded rather than
investigated.

**Next step for the owner:** replay it and find out whether it still reproduces —

```bash
cargo +nightly fuzz run auth_flows \
  fuzz/artifacts/auth_flows/crash-e29e9da4b8b23e9e48def2fd1201ea339341fc89
```

If it no longer reproduces, delete the artifact. If it does, it is a live
untriaged crash in an AUTH code path and deserves its own phase item.

---

## D-113-I — `with_native_roots()` panics the SDK when the OS trust store hiccups

**Found during:** the phase-113 post-merge test gate (orchestrator, after plan 16)
**Severity:** MEDIUM — turns a transient OS condition into 14 hard test failures,
and would panic a real client the same way
**Owner:** unassigned — pre-existing, untouched by any Phase-113 plan

`StreamableHttpTransport`'s constructor does

```rust
hyper_rustls::HttpsConnectorBuilder::new()
    .with_native_roots()
    .expect("Failed to load native root certificates")
```

at `src/shared/streamable_http.rs:374`. When macOS Keychain returns
`Os { code: -36 }` (`ioErr`) for user/admin/system trust settings — which it does
under full-suite concurrency in one particular long-lived checkout — that
`.expect()` **panics** instead of surfacing a `Result`. Observed failure text:

```
Failed to load native root certificates: Custom { kind: NotFound, error:
"no native root CA certificates found (errors: [ ... failed to load user trust
settings, kind: Os(Error { code: -36, message: \"I/O error.\" }) ... ])" }
```

**Proven NOT a Phase-113 regression.** Complete 2x2, same machine, minutes apart:

| source tree | main checkout | clean worktree |
|---|---|---|
| plan-16 reverted (`9e685d04` content) | FAIL 14 | PASS 2158/2158 |
| HEAD `84fe84cc` | FAIL 14 | PASS 2161/2161 |

Reverting plan 16's only compiled change does not fix it; the identical source
passes in a different directory. The same main checkout passed these very tests
in the wave-1 gate earlier the same day, so the checkout degraded partway
through the session. Failure is load-dependent: the 23-test group alone passes,
the 2161-test suite fails reliably.

**Two separable follow-ups for the owner:**

1. *Environmental* — why can this working directory no longer read macOS trust
   settings under concurrent load? Not reproducible from `/private/tmp`.
2. *Code robustness* — `.expect()` on OS trust-store loading is the wrong
   failure mode for a library. Returning `Error` would degrade gracefully
   instead of panicking. Out of scope for every Phase-113 plan (none of them
   declare `src/shared/streamable_http.rs`), so recorded rather than fixed.
