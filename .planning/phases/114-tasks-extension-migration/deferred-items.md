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
