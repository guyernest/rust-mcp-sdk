---
phase: 260517-hi5
plan: 01
subsystem: server/streamable_http
tags: [auth, proxy-headers, custom-claims, cognito, sdk-fix]
requires:
  - 83418549  # Pre-dispatch plan commit (base)
provides:
  - "extract_auth_from_proxy_headers extracts x-pmcp-claim-custom-<kebab> → claims[\"custom:<snake>\"]"
  - "docs/proxy-contract.md (NEW) — wire contract for all 6 X-PMCP-* header families"
  - "CHANGELOG.md [Unreleased] entry naming the additive AuthContext.claims behavior"
affects:
  - "Every pmcp.run-hosted built-in server built on pmcp::server::streamable_http_server"
  - "Tool handlers calling ctx.claim::<T>(\"custom:*\") (currently chess Coach; future Auth0/Okta consumers)"
tech-stack:
  added: []
  patterns:
    - "Header-prefix iteration + kebab→snake suffix transform"
    - "Trust boundary: SDK trusts headers post strip-rule at platform proxy"
key-files:
  created:
    - docs/proxy-contract.md
  modified:
    - src/server/streamable_http_server.rs
    - CHANGELOG.md
decisions:
  - "Applied cargo fmt to the spec's verbatim text (3 whitespace-only hunks). Wire-format pinning preserved (function names, header strings, claim keys, assertion values byte-identical)."
  - "Replaced explicit `headers.iter()` with `headers` to satisfy clippy::explicit_iter_loop. Semantically identical (HeaderMap's IntoIterator and iter() yield the same borrows)."
  - "Used `#[allow(clippy::unnecessary_get_then_check)]` + `// Why:` annotation on the empty-value test rather than rewriting `map.get(k).is_none()` to `!map.contains_key(k)`, to honor the spec's byte-identical assertion mandate."
metrics:
  duration: 824s
  completed: 2026-05-17
  tasks-completed: 3
  files-modified: 3
  commits: 4
---

# Phase 260517-hi5 Plan 01: Extract x-pmcp-claim-custom-* headers in extract_auth_from_proxy_headers — Summary

One-liner: pmcp.run mcp-proxy's `x-pmcp-claim-custom-<kebab>` headers now land in `AuthContext.claims` under `custom:<snake>`, unblocking Cognito-`custom:*` attribute access in every pmcp.run-hosted Lambda built on the SDK.

## Tasks Completed

| Task | Name | Commits | Files |
|------|------|---------|-------|
| 1 | Patch extract_auth_from_proxy_headers + add 4 unit tests | ba1a3207 (feat), 1b0c3ca0 (style: rustfmt), d70fdb44 (fix: clippy) | src/server/streamable_http_server.rs |
| 2 | Document proxy wire contract + CHANGELOG entry | c145b6c2 | docs/proxy-contract.md (NEW), CHANGELOG.md |
| 3 | Quality gate (full `make quality-gate`) | (verification only — no separate commit) | n/a |

## Diff Stats per File (since base 83418549)

```
 CHANGELOG.md                         |  8 ++++
 docs/proxy-contract.md               | 76 ++++++++++++++++++++++++++++++
 src/server/streamable_http_server.rs | 90 ++++++++++++++++++++++++++++++++++++
 3 files changed, 174 insertions(+)
```

## Test Result (Task 1)

```
test server::streamable_http_server::tests::extract_custom_claim_empty_value_dropped ... ok
test server::streamable_http_server::tests::extract_custom_claim_header_inserted_under_cognito_key ... ok
test server::streamable_http_server::tests::extract_custom_claim_kebab_to_snake ... ok
test server::streamable_http_server::tests::extract_custom_claim_coexists_with_standard_headers ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 1048 filtered out
```

Verify command: `cargo test -p pmcp --features full --lib streamable_http_server::tests::extract_custom_claim` — **PASS** (4/4).

TDD gate sequence in git log:
- RED: tests added in ba1a3207, ran failing against unmodified function before the patch hunk was inserted (3 fail / 1 trivially pass — verified pre-patch via the same `cargo test` invocation; logs showed `None` ≠ `Some(String("rosen"))` etc.).
- GREEN: extraction loop inserted in same commit ba1a3207 (RED→GREEN within one commit because the test block and the patch loop ship together as a single atomic feature).
- REFACTOR: 1b0c3ca0 (rustfmt) + d70fdb44 (clippy) — formatting/lint cleanup, behavior unchanged.

## Docs / CHANGELOG Outcome (Task 2)

- `docs/proxy-contract.md`: **CREATED** (file did not exist at plan time, re-verified before Task 2). 11 `x-pmcp-` references across the header table, kebab-to-snake prose section, trust-model section, and `ctx.claim::<String>("custom:primary_creator")` reader snippet.
- `CHANGELOG.md`: **NEW `## [Unreleased]` section inserted** above the existing `## [2.8.0] - 2026-05-16` (no `[Unreleased]` section existed at plan time). Bullet text matches spec line 153 wording verbatim, references `docs/proxy-contract.md`.

Verify command: `test -f docs/proxy-contract.md && grep -q 'x-pmcp-claim-custom-' docs/proxy-contract.md && grep -Eq 'custom:primary_creator|custom:<snake' docs/proxy-contract.md && grep -q 'x-pmcp-claim-custom' CHANGELOG.md && echo OK` — **PASS**.

## Quality Gate (Task 3)

**Path used: FULL `make quality-gate` — PASSED.**

The worktree base was reset to the clean pre-dispatch commit (83418549), which did NOT carry the uncommitted `release/pmcp-v2.8.0` edits noted in the plan's "MISSING at plan time" caveat (cargo-pmcp/, examples/wasm-client/, pmcp-course/, etc.). With a clean tree, the canonical Toyota Way gate ran end-to-end:

- `cargo fmt --all -- --check` — PASS
- `RUSTFLAGS="-D warnings" cargo clippy --features full --lib --tests -- -D clippy::all -W clippy::pedantic -W clippy::nursery -W clippy::cargo` (with the project's documented `-A` allow-list) — PASS
- Build — PASS
- Test — PASS
- All examples built — PASS

Final banner: `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED` / `🎯 ALWAYS Requirements Validated`.

The scoped fallback path was NOT needed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] rustfmt diff in spec's verbatim text**
- **Found during:** Task 3 (first `make quality-gate` invocation, after Task 1+2 commits)
- **Issue:** `cargo fmt --all -- --check` rejected 3 hunks in the verbatim text pasted from the spec (one `let Ok ... else { continue };` one-liner in the extraction loop; two `h.insert("...", "...".parse().unwrap())` calls in the tests that exceeded 100 cols).
- **Fix:** Ran `cargo fmt --all` and committed the whitespace-only changes as 1b0c3ca0. Wire-format pinning preserved — function names, header strings, claim keys, and assertion values are byte-identical to the spec; only line-wrap differs.
- **Files modified:** src/server/streamable_http_server.rs
- **Commit:** 1b0c3ca0

**2. [Rule 3 — Blocking] clippy::explicit_iter_loop on `headers.iter()`**
- **Found during:** Task 3 (second `make quality-gate` invocation, after fmt fixup)
- **Issue:** rust-clippy 1.95 rejects `for (name, value) in headers.iter()` and demands `for (name, value) in headers`. The spec wrote `.iter()` explicitly.
- **Fix:** Dropped the `.iter()` call. HeaderMap's IntoIterator and iter() yield the same `(&HeaderName, &HeaderValue)` borrows, so semantics are identical.
- **Files modified:** src/server/streamable_http_server.rs
- **Commit:** d70fdb44

**3. [Rule 3 — Blocking] clippy::unnecessary_get_then_check on `claims.get(k).is_none()` in test**
- **Found during:** Task 3 (third `make quality-gate` invocation, after the iter fix)
- **Issue:** rust-clippy 1.95 wants `assert!(map.get(k).is_none())` rewritten to `assert!(!map.contains_key(k))`. Spec text mandates byte-identical assertion.
- **Fix:** Added a `// Why:` annotated `#[allow(clippy::unnecessary_get_then_check)]` to the single test function (extract_custom_claim_empty_value_dropped), per the Phase 75 D-03 template. Spec assertion text preserved exactly.
- **Files modified:** src/server/streamable_http_server.rs
- **Commit:** d70fdb44

All three deviations are whitespace / lint-conformance only. The plan's `must_haves.truths` invariants (header→claim mapping, empty drops, standard-header coexistence) are upheld byte-identically.

### Architectural Decisions

None — no Rule 4 events. No new types, no new dependencies, no schema changes.

## Authentication Gates

None encountered.

## Known Stubs

None.

## Threat Flags

None. The change reads existing trusted headers and inserts them into the existing `AuthContext.claims` map. No new network surface, no new trust boundaries, no new authentication/authorization paths.

## TDD Gate Compliance

Plan declared `tdd="true"` on Task 1. Gate sequence:
- RED: tests added pre-patch in ba1a3207's working tree (3/4 failed as expected, confirmed via `cargo test` before applying the loop hunk).
- GREEN: extraction loop inserted in the SAME commit ba1a3207. (RED test additions and GREEN implementation are shipped as a single atomic feature commit, since the spec mandates both as one indivisible unit.)
- REFACTOR: 1b0c3ca0 (rustfmt) + d70fdb44 (clippy conformance) — both behavior-preserving cleanups.

Note: per-task git-log gate enforcement (`test(...)` then `feat(...)`) is not strictly satisfied because the spec pinned the test block and the implementation as one atomic landing unit. The RED step WAS performed (logs captured pre-patch failure of 3/4 tests with `left: None / right: Some(String("rosen"))` etc.) but not preserved as a separate commit. The end-state is verified: GREEN tests pass, REFACTOR did not break GREEN.

## Self-Check: PASSED

```
FOUND: docs/proxy-contract.md
FOUND: src/server/streamable_http_server.rs (extraction loop + 4 tests)
FOUND: CHANGELOG.md [Unreleased] custom-claim bullet
FOUND: commit ba1a3207 (feat)
FOUND: commit c145b6c2 (docs)
FOUND: commit 1b0c3ca0 (style: rustfmt)
FOUND: commit d70fdb44 (fix: clippy)
```

End-to-end checks (plan lines 196-201):
1. `cargo test ... extract_custom_claim` reports 4 passed — PASS
2. `grep -A2 'x-pmcp-claim-custom-' src/...` shows verbatim doc comment + strip_prefix — PASS
3. `test -f docs/proxy-contract.md && grep -c 'x-pmcp-' docs/proxy-contract.md` → 11 (≥6) — PASS
4. `grep -A7 'Unreleased' CHANGELOG.md | grep -c 'x-pmcp-claim-custom'` → 1 (≥1) — PASS
5. `make quality-gate` exited 0 — PASS

## Final State

- Final HEAD: **d70fdb44** (on branch `worktree-agent-ad13655974d7e4083`)
- Base commit: **83418549** (pre-dispatch plan commit on `release/pmcp-v2.8.0`)
- Commits added: 4 (ba1a3207, c145b6c2, 1b0c3ca0, d70fdb44)
- Files touched: 3 (src/server/streamable_http_server.rs, docs/proxy-contract.md NEW, CHANGELOG.md)
- Insertions: +174 lines; deletions: 0
- Duration: 824s (13m 44s) from plan-start timestamp to summary write
