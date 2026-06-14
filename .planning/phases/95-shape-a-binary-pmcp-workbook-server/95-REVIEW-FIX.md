---
phase: 95-shape-a-binary-pmcp-workbook-server
fixed_at: 2026-06-14T00:00:00Z
review_path: .planning/phases/95-shape-a-binary-pmcp-workbook-server/95-REVIEW.md
iteration: 1
findings_in_scope: 1
fixed: 1
skipped: 0
status: all_fixed
---

# Phase 95: Code Review Fix Report

**Fixed at:** 2026-06-14
**Source review:** .planning/phases/95-shape-a-binary-pmcp-workbook-server/95-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 1 (Critical + Warning; the 2 Info findings IN-01/IN-02 are out of scope for `fix_scope=critical_warning`)
- Fixed: 1
- Skipped: 0

## Fixed Issues

### WR-01: `RunError::Serve` is overloaded — a `Server::build()` failure is reported as a transport-start error

**Files modified:** `crates/pmcp-workbook-server/src/lib.rs`, `crates/pmcp-workbook-server/src/assemble.rs`
**Commit:** 1e349cc8
**Applied fix:**

- Added a distinct `Build(#[source] pmcp::Error)` variant to `RunError` in
  `lib.rs`, with an `#[error("workbook server build failed: {0}")]` message and
  rustdoc that explicitly contrasts it with `Serve` (build = in-process
  assembly before any listener is bound; serve = transport startup). The
  transport `Serve` variant was left untouched.
- Re-mapped the final `.build()` failure in `assemble.rs` (the
  `Server::builder()...build().map_err(...)` chain) from `RunError::Serve` to
  `RunError::Build`, so an in-process assembly fault no longer surfaces as
  "streamable-HTTP server failed to start".
- Reconciled the contradicting rustdoc: the `build_server` `# Errors` section
  (which previously claimed `RunError::Serve` for a build failure) now names
  `RunError::Build`, matching the variant's own doc. `serve()`'s
  `http.start().await.map_err(RunError::Serve)` was intentionally left as-is —
  that path genuinely is transport startup.
- Added a unit test `run_error_build_display_names_the_build_phase` asserting
  the `Build` variant's Display names the build phase and does NOT borrow the
  transport-start wording, locking in the fix against future re-skin
  regressions.

**Verification (scoped to `-p pmcp-workbook-server` per the orthogonal
`pmcp-toolkit-mysql` pre-existing failure note):**
- `cargo fmt -p pmcp-workbook-server` — applied.
- `cargo build -p pmcp-workbook-server` — exit 0.
- `cargo test -p pmcp-workbook-server` — 24 passed across 7 suites (lib unit +
  integration + doctests), including the new `Build` Display test.
- `cargo clippy -p pmcp-workbook-server --all-targets` — clean for this crate.
  The single remaining `redundant guard` warning is in the `pmcp-server-toolkit`
  dependency (`crates/pmcp-server-toolkit/src/http/auth.rs:538`), which the
  REVIEW.md summary already classifies as pre-existing and out of scope.

---

_Fixed: 2026-06-14_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
