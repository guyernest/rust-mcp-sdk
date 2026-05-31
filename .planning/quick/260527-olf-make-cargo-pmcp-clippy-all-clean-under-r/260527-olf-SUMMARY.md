---
quick_id: 260527-olf
status: complete
date: 2026-05-27
---

# Quick Task 260527-olf — cargo-pmcp clippy::all-clean under rust 1.95

## Outcome

`cargo clippy -p cargo-pmcp --all-targets --all-features -- -D clippy::all` now
exits **0** (was 22 errors). fmt clean; **all cargo-pmcp test suites pass**
(422 + 600 + the rest, 0 failed); `make lint` (root pmcp, the real CI gate) still
**✓ No lint issues**.

## Scope correction (important context)

This task was requested on the premise (from a prior turn) that rust-1.95 clippy
debt "blocked the quality gate." **That premise was wrong** — verified here:

- The real gate is `make lint` (Makefile:146) = `cargo clippy --features full --lib
  --tests` over the **root `pmcp` crate only**, with a large `-A` allow-list
  (incl. `uninlined_format_args`, `option_if_let_else`). CI (`ci.yml:63`) is the
  same shape. Both **already pass**. `cargo-pmcp` / `pmcp-widget-utils` / toolkit
  crates are **not** clippy-gated.
- So this sweep is **latent-debt hygiene**, not a gate unblock. See memory
  `project_rust195_clippy_gate_debt.md` (corrected).

User chose "small hygiene fix only" = the genuine `-D clippy::all` findings (not
the ~331-item full pedantic+nursery sweep, which stays declined).

## What it took (cascade)

Errors surfaced in waves because clippy can't lint a target whose dependency
target failed to compile: fixing the 14 lib/lib-test errors unmasked 8 more in
the bin + integration-test targets; fixing the `&PathBuf`→`&Path` ripple unmasked
3 caller `to_path_buf` sites. Final total: **22 fixes across 13 files**, all
mechanical and behavior-preserving (manual_contains, unnecessary_cast,
vec_init_then_push, type_complexity→type aliases, doc_lazy_continuation,
collapsible match/if, manual_checked_ops→checked_div, manual_map, ptr_arg,
unnecessary to_string).

Two touched files (`commands/configure/resolver.rs`, `configure/show.rs`) were
also touched by the Phase 260527-n51 GCR port; their default-clippy violations
had been masked by the pre-existing lib errors, so this also cleans up latent
port debt.

## Verification
- `cargo clippy -p cargo-pmcp --all-targets --all-features -- -D clippy::all` → 0 errors
- `cargo fmt -p cargo-pmcp --check` → clean
- `cargo test -p cargo-pmcp --all-features -- --test-threads=1` → all suites ok, 0 failed
- `make lint` → ✓ No lint issues

## Commit
- `c70045a2` fix(260527-olf): make cargo-pmcp clippy::all-clean under rust 1.95

## Not done (out of scope by user choice)
- The ~324 pedantic/nursery WARNINGS in cargo-pmcp (mostly `doc_markdown`
  backticks) — the full sweep was declined.
- Adding cargo-pmcp to the CI clippy gate (would be a phase).
- `pmcp-widget-utils` rust-1.95 lints (separate, also ungated).
