# Phase 114 — Deferred Items

Out-of-scope discoveries logged during execution. These were **measured, attributed and
NOT fixed** — each is either pre-existing and unrelated to the plan that found it, or owned
by a later plan.

---

## D-114-A — `native root certificates` keychain flake in `shared::streamable_http::tests`

**Found by:** 114-04 (Task 2 verification)
**Status:** open, unowned, pre-existing, environment-caused
**Severity:** low (intermittent CI/local noise, not a product defect)

`shared::streamable_http::tests::v2_error_envelope::v1_still_errors_on_the_status_alone`
FAILED once during a broad nextest filter run, panicking at the **pre-existing** `.expect`
in `src/shared/streamable_http.rs:458`:

```
Failed to load native root certificates: Custom { kind: NotFound, error:
"no native root CA certificates found (errors: [Error { context: \"failed to load user
trust settings\", kind: Os(Error { code: -36, message: \"I/O error.\" }) }, ...])" }
```

**Measured, not assumed:**

- Re-run **in isolation: PASS** (`-E 'test(v1_still_errors_on_the_status_alone)'`, 1 passed).
- Re-run of the **same broad filter: 136/136 passed.**
- `df -h /` at the time of failure: **19 GiB available** — so this is **NOT** the known
  disk-exhaustion mode recorded in memory (`project_disk_exhaustion_fake_test_failures`),
  which presents with the same `ioErr -36` signature on a full volume. It fires here under
  concurrent-load contention on the macOS keychain instead.
- `make quality-gate` subsequently ran **exit 0** with this test green.

**Why not fixed here:** 114-04 changed only `src/server/task_store.rs` and
`src/server/tasks.rs`. The panicking `.expect` is in the transport layer, is pre-existing,
and hardening it (fall back to webpki roots, or surface a typed error) is a transport
decision with its own blast radius. Fixing it inside a task-store seam plan would bury
that change.

**Suggested owner:** whichever later plan next touches `src/shared/streamable_http.rs`,
or a standalone hardening plan. The narrow fix is to stop `.expect`-ing on
platform-verifier root loading in a test-reachable path.

**Addendum, 114-12 (2026-07-28) — two REPRODUCIBLE triggers, not just "intermittent".**
114-12 hit this hard enough to be worth pinning down, because it presents as 14 code
regressions in files the plan never touched:

1. **A SANDBOXED shell reproduces it 100% of the time.** Every run of
   `cargo nextest run --features full -E 'binary_id(pmcp) and test(/collected_body_cap/)'`
   under the default sandboxed Bash failed **8/8** with the `ioErr -36` signature; the
   identical command with the sandbox disabled passed **8/8**. The keychain read is simply
   denied there. `df -h /` showed 39 GiB free, so again NOT the disk-exhaustion mode.
2. **Unsandboxed, it is PARALLELISM-sensitive.** The default `-j <ncpu>` produced 14 then 4
   failures across two broad runs; `-j 4` produced **1845/1845 green** and `-j 2` re-ran the
   4 stragglers green. The affected tests are always the same set in
   `shared::streamable_http::tests`.

**Practical guidance for later plans:** run test suites with the sandbox disabled and, for
broad selectors, `-j 4` (or `NEXTEST_TEST_THREADS=4` for `make quality-gate`, which is how
114-12's gate ran to exit 0). A failure whose stderr contains `no native root CA
certificates found` is THIS item, not the plan under test — check for that string before
bisecting anything.

---

## D-114-B — 1 ms-TTL setup races in `InMemoryTaskStore` expiry tests

**Found by:** 114-04 (Task 2)
**Status:** **FIXED for the two occurrences in `src/server/task_store.rs`**; the pattern may
exist elsewhere.

`cleanup_expired_drops_result` created a task with a **1 ms** TTL and then called
`set_result` on it. Every `InMemoryTaskStore` write runs through `Self::validate_access`,
which returns `TaskStoreError::Expired` — not a lost write — once the TTL has elapsed, so
under load the setup lost to the clock and the test failed at its `unwrap()` for a reason
unrelated to the property it asserts. **Observed firing** on run 1 of a 5-run repeat while
this plan was adding tests to the same binary.

Both occurrences in this file are now widened to 500 ms with the reason written at the site
(Rule 1, committed in `c3ff793e`). Recorded here because the same
`default_ttl_ms: Some(1)` + `sleep(10)` shape may appear in other expiry tests across the
tree; a sweep is out of scope for a seam plan.

---

## D-114-C — server-side `Mcp-Name` enforcement for `tasks/*` is deliberately OFF

**Found by:** 114-06 (Task 3) — a **scoped decision**, not a discovery
**Status:** open, owned by **Phase 118** (conformance hardening)
**Severity:** low today, rising once the ecosystem's clients are conformant

114-06 made pmcp's CLIENT emit the spec's `Mcp-Name: <params.taskId>` on `tasks/get`,
`tasks/update` and `tasks/cancel` (a spec **MUST**, inventory row 34). The SERVER half was
deliberately left unchanged: `is_name_bearing_method` in
`src/server/streamable_http_server.rs` still reads `logical_name_key`, which is derived from
`MRTR_METHODS` and therefore answers `None` for every `tasks/*` method, so
`cross_check_name` returns `Ok(())` for them before comparing anything.

**The tolerance this buys, stated plainly:** a pmcp v2 server accepts a conformant
`Mcp-Name: <taskId>` AND a legacy `Mcp-Name: ""`, and does **not** detect a header that
disagrees with `params.taskId`. That is what lets pre-existing clients keep working while
the ecosystem migrates.

**What turning it on would take:** point `is_name_bearing_method` at
`crate::types::mrtr::name_bearing_key` instead of `logical_name_key`. The routing-name half
already resolves through the combined lookup (`frame_routing_pair`), so the body value is
already available at the comparison site — one predicate is the whole change. It is a
BREAKING change for any client still sending the empty value, which is why it is a
separable decision rather than a line in a client plan.

The tradeoff is also recorded in the rustdoc on `TASK_NAME_BEARING_METHODS`, so a reader of
the table cannot miss it.

---

## D-114-D — an immediate `tasks/get` after create needs a strongly-consistent read

**Found by:** 114-07 (Task 3, `a_created_task_is_immediately_readable_from_its_returned_handle`)
**Status:** open, unowned — a BACKEND CONFIGURATION obligation, not a store defect
**Severity:** low today (no eventually-consistent backend is wired in CI), rising with the
first real DynamoDB deployment of the tasks extension

The tasks extension requires the handle a create returns to be resolvable straight away.
`GenericTaskStore::create` satisfies that structurally: it returns the record it just wrote
and issues **zero reads** of the new key — asserted by a get-counter on the backend double,
so "does not depend on read-after-write" is measured, not argued.

**What was measured, and it is the part worth carrying.** Driven through an
`EventuallyConsistentBackend` double whose `get` serves each key's PREVIOUS value once
before converging, a follow-up `store.get` on the freshly created key returns
`TaskError::NotFound`, and the next read succeeds. That is faithful to DynamoDB's default
eventually-consistent read, which may be served by a replica that has not yet received the
write. The record is durable throughout; only that one read is stale.

**A trap inside the trap.** The double got this wrong on the first attempt: it stored the
previous value as `Option<(bytes, version)>` and then `.flatten()`ed the lookup, so
`Some(None)` — "the key had no value before this write" — collapsed into "no staleness
recorded" and fell through to the converged value. The test PASSED for the wrong reason. A
staleness double must keep three cases distinct (`stale value` / `stale absence` /
`converged`); collapsing the middle one is exactly how it stops being faithful. The
corrected double is in `crates/pmcp-tasks/tests/input_delivery.rs` with the reason at the
site.

**What a deployment owes.** Either a strongly-consistent read on the `tasks/get` path
(DynamoDB's `ConsistentRead`, which is opt-in and costs double a read unit), or a client
retry on the first `NotFound` after a create. `DynamoDbBackend::get` in this crate does
**not** currently request a consistent read.

**Why not fixed here:** 114-07 is additive at the domain layer and changed no backend.
Flipping `DynamoDbBackend::get` to a consistent read is a cost and latency decision that
affects every existing v1 tasks deployment on that backend, not just the v2 input-delivery
path, so it does not belong inside a `tasks/update` plan.

**Suggested owner:** whichever plan next touches `crates/pmcp-tasks/src/store/dynamodb.rs`,
or the phase that first deploys the tasks extension against DynamoDB.

---

## D-114-E — `make test-feature-flags` is RED, and was already red before 114-07

**Found by:** 114-07 close-out (verification re-run)
**Status:** open, unowned, **pre-existing and PROVEN so**
**Severity:** medium — it is an acceptance criterion of 114-07 (and D-14 item 4) that no
plan in this phase can satisfy while it stays red, so it will keep being "failed" by
later plans that did not cause it

`make test-feature-flags` exits **2**. The failure is in row **1/4**, at its second
sub-command:

```
cargo clippy -p pmcp-tasks --no-default-features -- -D warnings   → exit 101
```

56 `dead_code` warnings in the **root `pmcp` lib** are promoted to errors by the target's
`-D warnings`. Building `pmcp` through `-p pmcp-tasks --no-default-features` selects a
reduced pmcp feature set in which those items have no caller.

**Attributed by file** (identical at HEAD and at the 114-07 base commit):

| File | Errors |
|------|--------|
| `src/types/mrtr.rs` | 42 |
| `src/server/subscriptions.rs` | 7 |
| `src/server/core.rs` | 4 |
| `src/shared/sse_parser.rs` | 2 |
| `src/server/mod.rs` | 1 |

**Zero** are in `crates/pmcp-tasks/` and **zero** are in `src/server/task_store.rs` — i.e.
none are on 114-07's surface. The named items (`write_canonical`, `salient_params`,
`salient_param_digest`, `project_capabilities_for_v2`, `EXPERIMENTAL_TASKS_KEY`, …) belong
to Phase 113 and to 114-05, both of which landed before 114-07.

**Measured, not argued.** A detached worktree was created at 114-07's base commit
`4327b246` (114-06's last commit) with its own `CARGO_TARGET_DIR`, and the same commands
were run there:

| Command | at base `4327b246` | at HEAD `9081be3b` |
|---------|--------------------|--------------------|
| `make test-feature-flags` | exit **2**, 56 errors | exit **2**, 56 errors |
| `cargo clippy -p pmcp-tasks --no-default-features -- -D warnings` | exit **101** | exit **101** |

Same exit codes, same error count, same per-file distribution. The worktree and its target
directory were removed afterwards (`git worktree list` back to its three pre-existing
entries).

**The dev-dep-free rows the plan actually cares about are GREEN.** 114-07's `verification`
block singles out "the `cargo check -p pmcp-tasks --features X` rows are the dev-dep-free
ones", and every one of those exits 0 at HEAD:

```
cargo check -p pmcp-tasks --no-default-features        exit 0
cargo check -p pmcp-tasks --features dynamodb          exit 0
cargo check -p pmcp-tasks --features redis             exit 0
cargo check -p pmcp-tasks --features dynamodb,redis    exit 0
cargo check -p pmcp-tasks                              exit 0
```

So the *compile* claim across the four feature rows holds; what is red is the root crate's
dead-code hygiene under a reduced feature set.

**Why not fixed here:** the fix lands in five root-`pmcp` files owned by other plans
(Phase 113's MRTR module and subscriptions; 114-05's capability projections). Adding
`#[cfg]` gates or `#[allow(dead_code)]` across them from inside a `pmcp-tasks` store plan
would bury a decision about the root crate's feature hygiene in a plan whose declared
`files_modified` is five files under `crates/pmcp-tasks/`. It is also NOT caught by
`make quality-gate` or CI, which lint the root crate with its own generous allow-list.

**Suggested owner:** a Phase 118 (conformance/hardening) item, or whichever plan next
touches `src/types/mrtr.rs`. The narrow fix is to gate or `allow` the reduced-feature dead
code at each site, then re-run `make test-feature-flags` to green.

---

## D-114-F — a known method with malformed params answers `-32601`, not `-32602`

**Found during:** `114-09` Task 3, while building the case-4-before-case-5 ordering probe.

**Measured (v2 Streamable HTTP, `tests/v2_tasks_owner_binding.rs`):** a `tasks/get`
carrying `{"taskId": 12345}` — a well-known method with a `taskId` of the wrong JSON
type — is answered:

```json
{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found: tasks/get"},"id":40}
```

`-32601 METHOD_NOT_FOUND`, not `-32602 INVALID_PARAMS`. The method plainly exists and is
served; only its params are malformed.

**Cause:** `ClientRequest` is deserialized at INGRESS, before any dispatch runs. The enum
is method-tagged, so a params-shape mismatch makes the `tasks/get` variant fail to match
and the request falls through to the not-a-known-method arm. Nothing in
`route_tasks_endpoint` is reached.

**Why this matters beyond tidiness:** it makes a whole class of ordering claims
unobservable from outside. `114-09`'s plan specified proving "the auth refusal fires
before the params parse" by sending malformed params with no credentials and asserting
`-32003` rather than `-32602`. That probe cannot work: the request never reaches the
refusal chain at all. `114-09` proved the ordering a different and stronger way instead
(identical well-formed params, differing only in the credential; the authenticated leg
reaches the store and hears "task not found", the unauthenticated leg hears `-32003` and
learns nothing) and pinned the measured caveat as leg C of
`an_unauthenticated_caller_is_refused_before_the_params_parse`.

**Why not fixed here:** the fix is in the transport/ingress request-deserialization path
(`ClientRequest`'s untagged fallthrough), not in `src/server/task_dispatch.rs`. It is
NOT specific to `tasks/*` — every method with typed params has the same behaviour — so
changing it from inside a tasks owner-binding plan would move a global wire behaviour
under a `files_modified` list of three server files, and would need its own golden-byte
review across every existing suite that may depend on the current code.

**Suggested owner:** a conformance/hardening plan, or whichever plan next touches
`ClientRequest`'s deserialization. The narrow fix is to match the method name FIRST and
only then attempt the params, so a known method with bad params yields `-32602`.

---

## D-114-G — `pmcp::testing` is now a wire-behaviour seam, not only a wire-SHAPE seam

**Found during:** 114-10 Task 1 (recorded 2026-07-28)
**Status:** open — needs an owner
**Severity:** low (documentation / module-charter drift, no wire impact)

`src/testing/mod.rs` opens with a charter: it exists so a `tasks/*` wire SHAPE is proven
against the real client deserialization types instead of an author-written fixture
(the Phase 101 tools-as-tasks incident).

114-10 added something structurally different to that module: `v2_result_envelope` /
`v1_result_envelope` do not check a shape against a type — they RUN the production
`inject_v2_result_envelope` and return the response bytes plus the `tracing` warnings it
emitted, so a test can distinguish "the key was never there" from "the registry removed
it". That is a behaviour-observation seam. It also carries a small hand-written
`tracing::Subscriber` (no `tracing-subscriber` dependency, so no feature gate beyond the
one `server::core` already has).

The seam is correct and its own rustdoc explains itself, but the MODULE-level doc comment
still describes only the shape-conformance charter, so a reader arriving at the top of the
file gets a description that no longer covers everything below it. Not fixed here because
rewriting the module charter of a file six Phase-113 plans build on is a wider edit than a
row-23 fix should carry, and the charter sentence is load-bearing for those plans.

**Suggested owner:** whichever plan next adds to `pmcp::testing`, or the phase's closing
documentation plan.

---

## D-114-H — `ReservedFieldOwner::TasksDispatch` has no PRODUCTION constructor yet

**Found during:** 114-10 Task 2 (recorded 2026-07-28)
**Status:** open — owned by **114-12**, tracked here so it cannot be forgotten
**Severity:** low (by design; the tripwire is already armed)

`ReservedFieldOwner::TasksDispatch` exists and is honoured by the registry, but its ONLY
constructor today is the `pmcp::testing` reserved-field seam. Nothing in `src/` dispatches
a v2 `tasks/get` that names it — 114-10 supplies the egress PERMISSION and dispatches
nothing, deliberately.

Measured, not assumed (all with `RUSTFLAGS="-D warnings" cargo clippy --lib`):

| feature selection | without the `allow` | with it |
|---|---|---|
| `--features full` | exit **0** (the testing seam constructs it) | exit 0 |
| `--no-default-features --features streamable-http` | exit **101**, exactly one error: `variant TasksDispatch is never constructed` | exit 0 |
| `--no-default-features` | exit 101, includes `variants Mrtr and TasksDispatch are never constructed` | (pre-existing D-114-E failures) |

The `allow` is therefore scoped `not(feature = "testing")` and NOT `not(test)`: `make lint`
runs `--lib --tests` with `full`, and the `--lib` half is a non-test build with `testing`
ON, so a `not(test)` scope would deactivate the lint for exactly that half. Under the
feature scope, BOTH halves of the gate lint the variant — deleting the seam before 114-12
wires production fails the gate rather than passing quietly.

**Closure condition:** 114-12 names `ReservedFieldOwner::TasksDispatch` at the v2
`tasks/get` egress; the `#[cfg_attr(not(feature = "testing"), allow(dead_code))]` can then
be deleted and the `--no-default-features --features streamable-http` row above becomes
exit 0 without it.

**CLOSED by 114-11 (2026-07-28) — one plan earlier than expected.**
`TaskDispatch::route_tasks_get` returns `DispatchEnvelopeClaim::TASKS_INPUT_REQUIRED` for an
`input_required` task, and that constant constructs `ReservedFieldOwner::TasksDispatch`.
`server::task_dispatch` is gated only on `not(target_arch = "wasm32")` and on **no feature**,
so the production constructor is present on every native build. Re-measured with
`RUSTFLAGS="-D warnings" cargo clippy --lib` before removing the `allow`: `--features full`
exit **0**; `--no-default-features --features streamable-http` reports **no** error naming
`TasksDispatch`; `--no-default-features` still reports its **55** pre-existing D-114-E
dead-code errors and **none** of them names `TasksDispatch` or the `Task` disposition variant.
Both `#[cfg_attr(…, allow(dead_code))]` guards — on `ReservedFieldOwner::TasksDispatch` and on
`ResponseDisposition::Task` — are therefore **deleted**, and the comments in their place record
the measurement so a future reader does not re-add them defensively.

**Note for 114-12:** the "supply the owner from the tasks route" item STATE.md assigned to you is
DONE. What remains yours is the v2 create TRIGGER. The claim plumbing you will find is:
`route_tasks_endpoint` and `maybe_build_task_created` / `build_task_created_response` return
`(JSONRPCResponse, DispatchEnvelopeClaim)`; `handle_request_internal` (core) and
`handle_client_request` / `process_client_request` / `handle_call_tool` (mod) carry a
`&mut DispatchEnvelopeClaim` out-param; both dispatch sites fold it with the MRTR egress's claim
through `DispatchEnvelopeClaim::or_egress`. Do NOT add a second path.

## D-114-I — `tests/common/v2.rs`'s tasks fixture now registers TWO tools

**Found during:** 114-11 Task 3 (recorded 2026-07-28)
**Status:** open — informational, no owner needed unless a later suite trips on it
**Severity:** low

`spawn_tasks_server` (and its new `spawn_tasks_server_with_store` primitive) registers
`long_task_tool` under `TASKS_TOOL_NAME` **and** `completing_error_task_tool` under
`COMPLETING_TOOL_NAME`. The second is required by `terminal_status_discipline`, which must
exercise the REAL create path for a synchronously-completing `isError: true` outcome rather
than poking the store into the answer it wants to assert.

Consequence a later plan should know about: any test against this shared fixture that asserts
on the **length or exact contents of `tools/list`** now sees two tools, not one. No suite does
today (verified: the eight tasks/MRTR binaries are green at 98/98), but a `tools/list`
cardinality assertion is a natural thing for 114-15's cross-caller matrix to write.

**Addendum, 114-12 (2026-07-28): the count is now THREE, not two.** `pausing_task_tool`
(`PAUSING_TOOL_NAME = "elicit_task"`) was added, because it is the first CLIENT-REACHABLE way
to produce an `input_required` task — before it, a suite had to reach past the wire and poke
`record_input_requests` on the store. 114-13 / 114-14 need a paused task the same way, which is
why it lives in the shared harness rather than in one suite. Re-verified after the addition:
`binary_id(pmcp) or <the eight tasks/MRTR binaries> or binary_id(pmcp::v2_tasks_create)` ran
**1858 passed / 4 failed**, and all four failures were D-114-A's keychain signature (green on
re-run at `-j 2`). Still no suite asserts `tools/list` cardinality.

## D-114-J — a v1 caller against the shared v2 harness must complete a real handshake

**Found during:** 114-11 Task 3 (recorded 2026-07-28)
**Status:** open — informational
**Severity:** low

`tests/v1_tasks_golden.rs` spawns with `spawn_stateless_config`, so its v1 requests need no
session. The shared harness's `spawn_tasks_server` uses `StreamableHttpServerConfig::default()`,
which is STATEFUL on purpose (RESEARCH Pitfall 1). A v1 request against it without a session id
answers **`-32600 "Session ID required for non-initialization requests"`** — which looks like a
tasks bug and is not one.

`tests/v2_tasks_shapes.rs::v1_session_headers` is the fix and the pattern: POST a real v1
`initialize`, read `Resp::mcp_session_id`, then carry
`pmcp::shared::http_constants::MCP_SESSION_ID` on every later v1 request. Any later plan writing a
v1 row against the shared harness should reuse that shape rather than rediscovering the `-32600`.

---

## D-114-K — a declaring v2 client now gets a task handle from ANY task-capable tool

**Found during:** 114-12 (recorded 2026-07-28)
**Status:** open — DELIBERATE, re-recorded from the plan's own scope decision, not a defect
**Severity:** medium (a UX / client-compatibility question, not a correctness one)
**Suggested owner:** a post-114 client-experience plan, or the v2.6 AI-Package milestone

114-12 made the v2 create trigger "the client declared `io.modelcontextprotocol/tasks` on this
request". The trigger is per-REQUEST and per-CLIENT, but it is **not** per-TOOL: once a client
declares the extension, EVERY tool on that server whose `TaskSupport` is `Required`/`Optional`
and which returns a task-shaped value will hand that client a `CreateTaskResult` instead of a
terminal result. That is what the extension's own text requires (the server is the sole decider,
and the declaration is its `MUST NOT` precondition), and it is what makes TASK-04 demonstrable at
all — DQ1, user-approved 2026-07-27.

What remains DEFERRED is the surrounding design the original CONTEXT.md deferral was plausibly
protecting:

- a client that declares the extension once, at the transport layer, has no way to say "not for
  THIS call" — there is no v2 equivalent of v1's per-request `task` field;
- a v2 client library therefore has to be prepared to handle a task handle back from any
  task-capable tool, and the ergonomics of that (auto-poll? surface the handle? opt out?) are
  unspecified here;
- there is no server-side per-tool or per-client policy knob for "advertise task support but
  only materialize a task when X".

None of this blocks the phase: the gate is conformant and the behaviour is the spec's. It is
recorded so a later reader does not mistake the absence of an opt-out for an oversight.

---

## D-114-L — the retired `tasks/*` methods can no longer be observed on the client's wire

**Found during:** 114-19 (Task 1, surfaced by the quality gate)
**Status:** open — DELIBERATE consequence, recorded so a later reader does not re-add a
control that cannot work
**Severity:** low (a testability narrowing, not a defect)
**Suggested owner:** whichever plan next wants to assert client-side header derivation

114-19 makes `Client::tasks_result` and `Client::tasks_list` fail fast LOCALLY on v2 with
zero bytes on the wire. `tasks/list` and `tasks/result` are also the ONLY two `tasks/*`
methods absent from `TASK_NAME_BEARING_METHODS`. Those two facts together mean:

> a pmcp v2 CLIENT can no longer emit a `tasks/*` method that is not name-bearing.

114-06's `tasks_list_emits_an_empty_mcp_name` used exactly that shape as its vehicle and
had to be re-pointed at `StreamableHttpTransport::send_raw`, which is the layer that owns
the derivation anyway. That repair is in-tree and green.

What is DEFERRED is the general problem it exposes: any future assertion about "how does
the client emit headers for method X" is only reachable through the client API while X
remains callable. As more methods are retired on v2, more such controls will have to drop
to the transport layer. There is no shared helper for "hand this transport a raw frame and
read back what the socket saw" — the repaired test hand-rolls it in ~10 lines. If a second
test needs it, lift it into `tests/common/`.

---

## D-114-A addendum 2 — one keychain-denied read survives `RUST_TEST_THREADS=1`

**Found during:** 114-19 (quality gate, four measured runs)

114-12's addendum recorded that the `no native root CA certificates found` failure is
100% reproducible in a sandboxed shell and parallelism-sensitive when unsandboxed, and
that `-j 4` cleared it. In the 114-19 session that was no longer enough: an UNSANDBOXED
`make quality-gate` with `RUST_TEST_THREADS=1` still lost exactly ONE test per full run,
and **the identity of that test MOVED between runs** — `session_validation_tests::
test_double_initialization_rejected` in one, `sse_middleware_integration::
test_middleware_modifies_request_headers` in the next — while the panic site stayed the
pre-existing `.expect` at `src/shared/streamable_http.rs:458`. Both files pass in
isolation (10/10 and 1/1). A regression does not relocate itself between runs.

Practical guidance for the next executor: `make quality-gate` may not reach exit 0 on this
machine at all. Run the legs individually, grep every failure for the CA string FIRST, and
re-run the single failing binary in isolation before treating it as a regression. The real
fix — replacing the `.expect` at `streamable_http.rs:458` with a fallible path, or pinning
`webpki-roots` for tests — is still unowned.

### Process trap re-confirmed while writing THIS entry

`/usr/bin/cat` does not exist on macOS (it is `/bin/cat`), so a
`/usr/bin/cat >> deferred-items.md <<'EOF'` heredoc silently wrote NOTHING while the
`echo appended` on the following line printed a reassuring "appended". This is the exact
failure 114-10 recorded for `/usr/bin/cp`. **Verify the mutation landed before trusting
the message that says it did.**

---

## D-114-M — `TaskRouter::handle_tasks_update`'s default returns `-32603` where `-32601` is arguably right

**Found by:** 114-13 (decide-now item carried in from the phase brief)
**Status:** open, owned by **114-14**
**Severity:** low today (unreachable), medium once a caller exists

`src/server/tasks.rs:91-95` — the defaulted `TaskRouter::handle_tasks_update` returns
`Error::internal("tasks/update not supported by this router")`, which reaches the wire as
`-32603`. At the protocol level, "this router does not implement this method" is `-32601`,
and that is what the sibling no-backend refusals in `task_dispatch.rs` already emit
(`TASKS_NOT_ENABLED` / `TASKS_RESULT_NOT_SUPPORTED`, both `-32601`).

**Why 114-13 recorded it instead of changing it — and the reason is a measurement, not a
preference:**

```
$ grep -rn handle_tasks_update src/
src/server/tasks.rs:91:    async fn handle_tasks_update(&self, _params: Value, _owner_id: &str) -> Result<Value> {
```

**Zero callers, anywhere in the tree.** 114-13 routes `tasks/update` but stops at the gates;
delivery — and therefore the store-vs-router split that would first invoke this default — is
114-14's. Changing an unreachable default's wire code would be **unverifiable by
construction**: no negative control could fail it, which is precisely the "a property no
control fails" pattern this phase has now hit five times. It would also be a wire change
landed by a plan that cannot demonstrate its effect.

**For 114-14:** the moment you add the router branch, this becomes decidable and testable in
one step. Prefer `-32601` with a message distinguishable from `TASKS_NOT_ENABLED` (a router
that exists but does not do updates is a different fact from a server with no task backend at
all — T-114-33's distinguishability rule), and pin it with a control.

---

## D-114-A addendum 3 (114-13) — `make test-unit` reads `RUST_TEST_THREADS`, not `NEXTEST_TEST_THREADS`

**Found by:** 114-13 (two full gate runs)

The phase brief's trap list says to use `NEXTEST_TEST_THREADS=4`. That is correct for
`cargo nextest` invocations and **does nothing for the leg that actually fails**:
`make test-unit` runs `cargo test --lib --features "full"`, which reads `RUST_TEST_THREADS`.

Measured, same tree, same commit:

| parallelism | result | `no native root CA` messages |
|-------------|--------|------------------------------|
| default (`-j ncpu`) | `1750 passed; 14 failed` | **14** |
| `RUST_TEST_THREADS=4` | `1760 passed; 4 failed` | **4** |
| `RUST_TEST_THREADS=1` | **`1764 passed; 0 failed`, exit 0** | **0** |

Failure count equals CA-message count exactly at every level, and the population shrinks
monotonically to zero as parallelism drops. A regression does not do that. The panic is in
`StreamableHttpTransport::new_internal` — the **client transport constructor**, which runs
before any request — so no server-side change can reach it.

**Run `RUST_TEST_THREADS=1 make test-unit`.** The underlying fix (making the
`.expect("Failed to load native root certificates")` fallible, or pinning `webpki-roots` for
tests) is still unowned.

---

## D-114-M — the PUBLISHED core `2026-07-28` schema is not vendored and has no provenance tripwire

**Found by:** the 2026-07-29 spec re-verification run (`114-SPEC-RECHECK.md` § `### Verdict
re-verification` → `#### 2026-07-29`)
**Status:** open, unowned, newly created by an upstream publication event
**Severity:** medium — it leaves authoritative values sourced by inference rather than by a pin

`114-01` vendored `modelcontextprotocol/ext-tasks` `schema/draft/` at
`2c1425d9a288b9b1f489430fe1e00bb392b47e48` with a SHA-256 + git-blob provenance tripwire, because
that was the only schema this phase read. On **2026-07-29** the core specification published
`modelcontextprotocol/modelcontextprotocol` `schema/2026-07-28/` (`schema.ts`, `schema.json`,
`schema.mdx`, `examples/`), declaring `LATEST_PROTOCOL_VERSION = "2026-07-28"`.

That published core schema now **governs** three groups of values this phase relies on:

| Group | Values | Inventory rows |
|---|---|---|
| The extension capability map | `extensions?: { [key: string]: JSONObject }` on `ClientCapabilities` **and** `ServerCapabilities` | 1-3 |
| Error codes | `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`, `INVALID_PARAMS = -32602`, and the **absence** of `-32002` | 29, 30, 32, 33 |
| The result discriminator | `resultType: ResultType`, `ResultType = "complete" \| "input_required" \| string` | 16-20 |

**Nothing in the repo pins any of them.** They are currently asserted from a one-off HTTP read
recorded in prose. If upstream renumbers a code or narrows `ResultType` again, no test fails.

**Why not fixed at discovery:** vendoring a second schema tree is `114-01`-shaped work — a
directory, a `PROVENANCE.md` with dual digests, and a tripwire test — and the discovery was made
during a read-only verification run with no shell available (the harness Bash classifier was down),
so nothing could be fetched, hashed or committed. It is also genuinely optional for *this* phase:
the D-18 hold is still engaged and no requirement flips, so the pin buys future-run cheapness and
drift detection, not present correctness.

**Suggested owner:** a small standalone plan mirroring `114-01` (vendor + `PROVENANCE.md` + tripwire
over `schema/vendored/core-2026-07-28/`), or `114-18` if it is willing to widen. **Note the
asymmetry deliberately:** vendoring the CORE schema is safe because it is published and immutable;
vendoring anything from `ext-tasks` beyond the existing `draft` pin is **not**, because that
repository has not published and `## Recorded Exception` forbids promoting draft to authoritative.

---

## D-114-N — `ext-tasks` publishing is now the SOLE remaining D-18 trigger, and nothing watches it

**Found by:** the 2026-07-29 spec re-verification run
**Status:** open, unowned
**Severity:** medium — six held requirements hinge on an event no mechanism detects

The DQ6 trigger required a versioned (non-`draft`) schema directory in **both** repositories. As of
2026-07-29 the core half is **satisfied** and the extension half is **not**:
`modelcontextprotocol/ext-tasks` still carries `schema/draft/` and `specification/draft/` only, with
**no tags and no releases**, 17 commits on `main`, and a README describing an experimental extension
*"under development"* toward SEP-2663. Its `schema/draft` has not changed since **2026-05-22**
(`29f83d5`).

So the condition that releases TASK-01…TASK-06 — and, separately, re-enters the Phase-114
contract-first waiver — has collapsed from a two-repository check to a **one-repository** check.
That is strictly cheaper to monitor, and correspondingly cheaper to forget.

**What is missing:** any detector. `114-01`'s provenance tripwire watches the **vendored bytes** for
local tampering; it cannot see upstream publishing a new directory. Nothing in CI or the repo polls
`ext-tasks`. Today the trigger is noticed only if a human happens to look — which is precisely the
*"a waiver that quietly becomes permanent"* failure mode (**T-114-107**) that
`114-CONTRACT-DECISION.md` § 2 observed in Phase 113 in the wild.

**Why not fixed at discovery:** a watcher is infrastructure (a scheduled CI job, or a documented
manual checkpoint), not a wire value, and the run that found it had no shell.

**Suggested owner:** a scheduled check — e.g. a low-frequency CI workflow asserting
`gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'` still yields only
`draft`, failing loudly when it does not. Cheap, and it converts "someone must remember" into "the
build tells us". Until then, the recorded fallback is `114-SPEC-RECHECK.md` § `## Procedure`, re-run
by hand.

---

## D-114-M — a `TaskRouter` serving `tasks/update` performs its OWN decode, unaided (114-14)

**Discovered:** 2026-07-31, while landing `tasks/update` delivery (plan 114-14).

`TaskDispatch::deliver_tasks_update` is store-first with a router fall-through, the same
precedence every other `tasks/*` route uses. The STORE leg reads the server-recorded kinds through
`TaskStore::task_input_snapshot` and types every value with `InputResponse::decode_for`, which is
the whole point of the route (D-113-O). The ROUTER leg cannot: a `TaskRouter` is out-of-tree code
holding its own record, and the trait has no snapshot accessor, so it receives `params` VERBATIM
and owns its own decode.

**What IS still guaranteed on that path:** the four `inputResponses` bounds have already fired
(they run at case 6, before the store/router split), so a router never sees an unbounded payload;
the owner is the identity table's and is passed as `owner_id`, never read from `params`; and the
trait rustdoc on `TaskRouter::handle_tasks_update` states both facts.

**What is NOT:** nothing stops an out-of-tree router from running the untagged decoder on those
values and reproducing D-113-O inside its own crate. The kind-direction property is enforced for
`TaskStore` implementations and only *documented* for `TaskRouter` ones.

**Why not fixed here:** closing it means widening the `TaskRouter` trait with a snapshot accessor
(or handing it a pre-typed `InputResponses`, which requires kinds this dispatcher does not have for
a router-backed task). Both are additive trait changes with real design questions about who owns
the record, and `TaskRouter` is the *legacy experimental* backend — `TaskStore` is the supported
one. Doing it inside a delivery plan would have been an architectural change smuggled in as a fix.

**Suggested owner:** whichever plan next revisits `TaskRouter`. If the answer is "routers are
deprecated", the honest closure is to say so rather than to widen the trait.

---

## D-114-N — a store that does not accept inputs answers `-32601 "Tasks not enabled"` (114-14)

**Discovered:** 2026-07-31, writing the router fall-through arm of `deliver_tasks_update`.

Reachable in exactly one configuration: a server with a `TaskStore` whose `supports_inputs()` is
`false`, and NO `TaskRouter`. Case 2 of the gate chain already answered for a server with no task
backend at all, so this arm is about a backend that exists and cannot do this one thing.

It reuses `TASKS_NOT_ENABLED` — the FROZEN sibling message its three `tasks/*` siblings emit —
rather than minting a fifth `-32601` sentence, deliberately: `the_minus_32601_conditions_are_mutually_distinct`
asserts the population of those messages is pairwise distinct, and a fifth would have to be added
to that population and justified. The reuse is slightly imprecise ("Tasks not enabled" on a server
that plainly has tasks) and strictly better than a fifth near-synonym nobody can tell apart.

**Why not fixed here:** the precise message is `tasks/update input delivery is not supported by
this server's task backend`, which is a NEW member of the distinguishability set and therefore a
change to a frozen contract, not a delivery detail. `InMemoryTaskStore` and `pmcp-tasks`'
`GenericTaskStore` both return `true`, so no in-tree configuration reaches this arm.

**Suggested owner:** a plan that is already touching the `-32601` message population.

## D-114-O — a `pmcp` test cannot exercise `pmcp-tasks` behaviour, so a cross-crate security claim splits in two (114-15)

**Discovered:** 2026-07-31, writing `v1_local_and_v2_anonymous_buckets_are_disjoint`.

`pmcp-tasks` is a workspace MEMBER but not a dependency of the root `pmcp` crate in any profile —
not `[dependencies]`, not `[dev-dependencies]`, not behind a feature. A `tests/*.rs` integration
test of `pmcp` therefore cannot call `pmcp_tasks::…` at all.

The concrete consequence for TASK-05: the plan asked one test to assert BOTH that the v1 `"local"`
bucket and the v2 anonymous bucket are disjoint AND that `pmcp-tasks`' `is_anonymous_owner` treats
`""` and `"local"` identically. The first half is behavioural and was measured over a live socket
plus directly against the in-crate `InMemoryTaskStore`. The second half could only be asserted at
the SOURCE — a substring tripwire over `crates/pmcp-tasks/src/store/generic.rs` and
`crates/pmcp-tasks/src/store/backend.rs`. That is a real assertion (it rots loudly if the predicate
is split or `make_key` drops its owner prefix) but it is a weaker instrument than execution, and a
reader should know which half is which.

**Why not fixed here:** the fix is `pmcp-tasks = { path = "crates/pmcp-tasks" }` under the root
`[dev-dependencies]`, which is a manifest change. 114-15 is a coverage-only plan whose own threat
register (T-114-SC) requires `Cargo.toml`/`Cargo.lock` to be byte-unchanged, and a dev-dependency
edge from the core crate onto an experimental one deserves its own decision rather than arriving as
a side effect of a test.

**Interim mitigation, already in place:** `crates/pmcp-tasks/tests/input_delivery.rs` (114-07) owns
the BEHAVIOURAL twin of the predicate claim from inside the crate that can execute it
(`anonymous_owner_is_refused_by_default_on_this_backend`), and 114-15's source tripwire names it.
The two suites together cover what one cannot.

**Suggested owner:** a plan that already needs `pmcp-tasks` reachable from a `pmcp` test — most
likely a future backend-parity suite.

---

## D-114-P — a `TaskRouter`-backed v2 server answers `-32603` for a task its router cannot find (114-16)

**Discovered:** 2026-07-31, enumerating every `-32603` emission in `src/server/task_dispatch.rs`
for the `NotFound`-must-not-be-`INTERNAL_ERROR` tripwire.

Three router fall-through legs render a `TaskRouter` error as `-32603`:

| function | line | method |
|---|---|---|
| `route_tasks_get` | 1818 | `tasks/get` |
| `route_tasks_cancel` | 1933 | `tasks/cancel` |
| `deliver_tasks_update` | 2409 | `tasks/update` |

The tasks extension's error-handling section makes `-32602` a **MUST** for a `tasks/get` naming an
invalid or nonexistent `taskId`, and a SHOULD for `tasks/cancel` and `tasks/update` (inventory row
29). So a router-backed v2 deployment is **non-conformant** on `tasks/get` and non-ideal on the
other two.

**This is a router-only gap.** Every STORE-backed path — which is every backend in this repository,
`InMemoryTaskStore` and `GenericTaskStore` alike — reaches `store_error_response`, which maps
`NotFound`/`Expired` onto the one oracle-free `-32602` that 114-11 landed and 114-15 measured over a
live socket. `tests/v2_tasks_tripwires.rs :: the_v2_store_not_found_arm_still_maps_to_minus_32602`
pins that arm positionally.

**Why not fixed here:** `TaskRouter::handle_tasks_*` returns `crate::error::Error`, which carries no
not-found discriminant the dispatch can read, so the dispatch cannot map it without either
inspecting an error STRING (which would be a new, fragile wire dependency) or widening the trait
with a typed not-found. Widening a legacy-experimental public trait is a semver and design decision
of its own, and 114-16 is a coverage-only plan that touches no production byte.

**Recorded rather than hidden:** the three legs are `Disposition::RouterLeg` entries in
`INTERNAL_ERROR_SITES`, each naming this deferral in its justification, and the count is pinned — so
a fourth router leg, or a second `-32603` inside one of these three, fails the tripwire.

**Related:** D-114-M (114-14) records the sibling shape — a `TaskRouter` serving `tasks/update`
performs its own decode unaided, so the kind-direction property is enforced for `TaskStore` and only
documented for `TaskRouter`. Both close together, in a plan that decides what `TaskRouter` owes a v2
caller.

**Suggested owner:** Phase 118, alongside the conformance run that would grade `tasks/get` on a
router-backed server.

---

## D-114-Q — `TASKS_UPDATE_METHOD` has a PROSE attribution, not a walkable one (114-16)

**Discovered:** 2026-07-31, writing the provenance tripwire over the wire values this phase
introduced.

The D-18 gate's mechanism is that every provisional wire value carries an attribution a reader can
WALK — a path to the vendored artifact, or the recheck record. Two of the three values this phase
introduced do:

| constant | attribution site | strength |
|---|---|---|
| `TASKS_EXTENSION_KEY` | itself | names `schema/vendored/ext-tasks/schema.ts`, the pinned commit, AND `114-SPEC-RECHECK.md` |
| `V2_TASKS_METHOD_RETIRED` | itself | names `schema/vendored/ext-tasks/schema.ts` |
| `TASKS_UPDATE_METHOD` | `TASK_NAME_BEARING_METHODS` | **prose only** |

`TASKS_UPDATE_METHOD`'s own rustdoc is one line — ``/// `tasks/update`. See [`TASKS_GET_METHOD`].``
— and following that link twice arrives at `TASK_NAME_BEARING_METHODS`, whose rustdoc cites "the
ext-tasks specification's § *Streamable HTTP: Routing Headers*" and names **no file**. Measured, not
assumed: neither walkable token appears in the constant's own doc block, and the table's block
contains `ext-tasks` but neither `schema/vendored/ext-tasks` nor `114-SPEC-RECHECK`.

**Why not fixed here:** the fix is one added rustdoc paragraph in `src/types/mrtr.rs`, which is a
production edit. 114-16's own threat register (T-114-SC) and its `<verification>` both require
`git diff --stat -- src/ crates/` to be EMPTY, and a coverage plan that edits the thing it is
measuring stops being evidence.

**The lock is two-directional, so this cannot rot in either direction:**
`every_wire_value_constant_this_phase_introduced_carries_an_attribution` fails if the prose citation
is ALSO lost, and it fails if the rustdoc GAINS a walkable artifact reference — the second failure
message says "promote the entry to `Attribution::Pinned` and close the deferral". So closing this is
a two-line change that the test itself demands.

**Suggested owner:** any plan already editing `src/types/mrtr.rs` — most naturally the one that
re-vendors the schema at the D-18 gate, since that is when every attribution gets walked anyway.
