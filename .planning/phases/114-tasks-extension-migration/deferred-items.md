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
