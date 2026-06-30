---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
plan: 06
subsystem: release
tags: [release, versioning, changelog, wasm, quality-gate, pkce]
requires: [103-01, 103-02, 103-04, 103-05]
provides:
  - "pmcp 2.11.0 (Cargo.toml version bump)"
  - "CHANGELOG.md 2.11.0 entry (PKCE helper Added + WasmHttpTransport Fixed)"
  - "recorded KEEP-PINS downstream decision"
  - "green BOTH-gates verification (quality-gate + wasm-build + wasm-release)"
affects:
  - "crates.io consumers (new additive public API becomes consumable)"
tech-stack:
  added: []
  patterns:
    - "Minor (additive) semver bump 2.10.0 -> 2.11.0"
    - "Path-pinned workspace crates build against in-tree source regardless of version string"
key-files:
  created:
    - .planning/phases/103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas/103-06-SUMMARY.md
  modified:
    - Cargo.toml
    - CHANGELOG.md
decisions:
  - "KEEP PINS: do NOT bump mcp-tester / cargo-pmcp pmcp pins this release (pure additive bump, downstream does not consume the new API)"
  - "Minor bump 2.10.0 -> 2.11.0 reflects purely additive public API (PKCE helper + WasmHttpTransport fix); no breaking change"
metrics:
  duration: "~12min (continuation)"
  completed: 2026-06-30
---

# Phase 103 Plan 06: pmcp 2.11.0 Release Cut + SC-4 BOTH-Gates Verification Summary

Cut pmcp **2.11.0** (minor, additive) for the two reusable browser-channel public-API pieces — the wasm-safe OAuth PKCE crypto helper (D-02/D-03) and the now-functional `WasmHttpTransport` (D-08) — with a CHANGELOG entry, an explicitly recorded KEEP-PINS downstream decision, and a green BOTH-gates verification proving the WASM boundary holds with no non-wasm regression. No tag/publish performed (separate release-branch action per CLAUDE.md).

## What Shipped

- **Cargo.toml** — root pmcp `version` bumped `2.10.0 -> 2.11.0` (Task 1, commit `10caa41a`).
- **CHANGELOG.md** — `## [2.11.0] - 2026-06-30` entry above 2.10.0 (Task 1, commit `10caa41a`):
  - **Added** — `pmcp::shared::pkce::{generate_code_verifier, code_challenge_s256, generate_state}` (RFC 7636, getrandom/sha2/base64-backed, host + wasm32; S256 helper infallible, RNG-backed fns return `Result`, no unwrap/expect); plus the new `examples/web-channel-client` reference (client/ wasm cdylib + server/ native split).
  - **Fixed** — `WasmHttpTransport` now correlates `send()`/`receive()` via a one-slot pending buffer with a double-send guard, so the high-level `Client` + typed task helpers work over browser Fetch.
  - **Changed** — `getrandom` relocated into the cross-target `[dependencies]` table (HIGH-1 dependency hygiene); no new external dependency.

## Downstream-Pin Verdict (Task 2 checkpoint) — KEEP PINS

The blocking human-verify checkpoint forced an explicit, recorded decision on whether the downstream crates that pin `pmcp` need their pins/versions bumped before the release branch is cut. **Verdict: KEEP all pins — no downstream edits this release.**

**Evidence gathered for the developer (two-question test from `<how-to-verify>`):**

1. **Current downstream pins** — both pin `pmcp` via a `path` override, so they already build against the in-tree 2.11.0 source regardless of the version string:
   - `crates/mcp-tester/Cargo.toml`: `pmcp = { version = "2.8.1", path = "../../", features = [...] }`
   - `cargo-pmcp/Cargo.toml`: `pmcp = { version = "2.9.0", path = "..", features = [...] }`
2. **Do they consume the new 2.11.0 API in this release?** — **No.** Neither `mcp-tester` nor `cargo-pmcp` is updated this release to call the new PKCE helper (`pmcp::shared::pkce::*`) or the now-functional `WasmHttpTransport`. The 2.10.0 -> 2.11.0 bump is purely additive.

Per CLAUDE.md Release & Publish Workflow ("downstream crates that pin a bumped dependency must also be bumped **IF** the pin is updated to consume the new API"), a pure additive release that downstream does not yet consume does **not** require a downstream pin/version bump. The human accepted this evidence and chose **KEEP PINS**.

**Action taken:** No edits to `crates/mcp-tester/Cargo.toml` or `cargo-pmcp/Cargo.toml` (`git diff --stat` on both = empty, confirmed). The decision is recorded here so the release-branch step acts on an explicit, non-implicit call.

## SC-4 Gate Results (Task 3) — BOTH GATES GREEN

The phase verification requires BOTH the native quality gate AND the wasm build/release (Pitfall 5: `make quality-gate` does NOT build wasm). All three exited 0:

| Gate | Command | Exit code |
|------|---------|-----------|
| Native quality gate | `make quality-gate` | **0** |
| WASM dev build | `make wasm-build` | **0** |
| WASM release build | `make wasm-release` | **0** |

- `make quality-gate` ran its full composition (fmt-check, lint with pedantic+nursery, build, test-all, audit, unused-deps, check-todos, check-unwraps, validate-always, purity-check). Each step is an `@$(MAKE)` sub-target, so the 0 exit confirms every gate passed — including check-unwraps (no unwrap/expect in the new `src/shared/pkce.rs`), SATD, and purity.
- `make wasm-build` and `make wasm-release` both compiled the `wasm32-unknown-unknown` target (`--no-default-features --features wasm`) with the new PKCE module + fixed transport — proving the WASM boundary is preserved and non-wasm is not regressed.

**Note on `cargo fuzz build`:** the `validate-always` step's fuzz-build sub-step emits `failed to run rustc to learn about target-specific information` on this host (cargo-fuzz needs a nightly + sanitizer toolchain not installed locally — a pre-existing, documented host limitation, see 103-01 decision). This is NOT one of the quality-gate hard-fail composed targets; `make quality-gate` still exited 0. The `pkce_helper` fuzz target builds via the bin-build fallback per the 103-01 LOW-7 note.

## LOCKED Fences Verified

- Downstream pins unchanged: `git diff --stat crates/mcp-tester/Cargo.toml cargo-pmcp/Cargo.toml` = empty.
- Frozen tasks/* contract files unchanged: `git diff --stat tests/tool_as_task_lifecycle_http.rs src/server/task_dispatch.rs` = empty.

## Deviations from Plan

None — plan executed exactly as written. Task 1 landed in the prior session (commit `10caa41a`); this continuation recorded the KEEP-PINS verdict and ran the SC-4 gate. No source code changed in this continuation (no auto-fixes needed; all gates green on first run).

## Commits

- `10caa41a` — `chore(103-06): bump pmcp 2.10.0 -> 2.11.0 + CHANGELOG entry` (Task 1; Cargo.toml + CHANGELOG.md)
- Closing docs commit — this SUMMARY + STATE.md + ROADMAP.md (final metadata commit)

## Self-Check: PASSED
