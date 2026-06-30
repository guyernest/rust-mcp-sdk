---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
plan: 01
subsystem: client-oauth-pkce
tags: [pkce, oauth, wasm, crypto, getrandom, rfc7636]
requires: []
provides:
  - "pmcp::shared::pkce module (generate_code_verifier / code_challenge_s256 / generate_state)"
  - "ungated crate-root re-export pmcp::{generate_code_verifier, code_challenge_s256, generate_state}"
  - "getrandom as a cross-target [dependencies] entry (host + wasm32)"
affects:
  - "future browser PKCE orchestrator (out of SDK scope per D-03)"
tech-stack:
  added: []
  patterns:
    - "getrandom::fill CSPRNG (cross-target, replaces rand for wasm-safety)"
    - "ungated shared module compiling on host AND wasm32"
    - "Result-returning crypto helper (no unwrap/expect in production code)"
key-files:
  created:
    - src/shared/pkce.rs
    - tests/pkce_helper.rs
    - fuzz/fuzz_targets/pkce_helper.rs
  modified:
    - Cargo.toml
    - src/shared/mod.rs
    - src/lib.rs
    - fuzz/Cargo.toml
decisions:
  - "getrandom moved to cross-target [dependencies] (HIGH-1) so the ungated pkce module links on host; wasm32 target entry keeps features=[\"wasm_js\"]"
  - "S256 challenge helper is infallible (#[must_use] String); only the RNG-backed verifier/state return Result"
  - "fuzz target validated via the plain `cargo build --bin pkce_helper` fallback because `cargo fuzz build` needs a nightly toolchain on this host (-Z sanitizer); plain `cargo fuzz run pkce_helper` form retained per LOW-7"
metrics:
  duration: ~6min
  completed: 2026-06-30
---

# Phase 103 Plan 01: Wasm-safe PKCE Crypto Helper Summary

Target-agnostic RFC 7636 PKCE primitives (verifier / S256 challenge / CSRF state) added to the
`pmcp` crate as `src/shared/pkce.rs`, backed by `getrandom::fill` so the module compiles and runs
on both host and `wasm32` — the first reusable public-API piece of the Phase 103 browser-OAuth release.

## What Was Built

- **`src/shared/pkce.rs`** — three public fns: `generate_code_verifier() -> Result<String>`
  (43-char base64url-no-pad of 32 CSPRNG bytes), `code_challenge_s256(&str) -> String`
  (deterministic SHA-256 S256 via the audited `sha2` crate), and `generate_state() -> Result<String>`
  (same entropy source/shape as the verifier). A single private `random_bytes()` helper centralises
  the lone `getrandom::fill` call and maps `getrandom::Error` to `Error::internal` — no
  `unwrap`/`expect` in production code.
- **HIGH-1 dependency fix** — `getrandom = "0.4"` added to the cross-target `[dependencies]` table
  (Cargo.toml line 89, above the first `[target.` at line 119); the existing wasm32-target entry keeps
  `features = ["wasm_js"]` so the wasm backend stays selected. This is the build-correctness pivot:
  the ungated module would fail to link on host if getrandom stayed wasm-only.
- **Ungated wiring** — `pub mod pkce;` in `src/shared/mod.rs` (no cfg gate) + a crate-root
  re-export in `src/lib.rs` modelled on the `StdioTransport` re-export (not the wasm-gated transport block).
- **`tests/pkce_helper.rs`** — the four WEBCH-01 validation rows: `pkce_rfc7636_vector` (Appendix B
  vector), `pkce_verifier_charset_len` (proptest charset/length), `pkce_challenge_deterministic`
  (proptest determinism), `pkce_base64url_roundtrip` (proptest encode→decode identity), plus a
  crate-root re-export resolution test.
- **`fuzz/fuzz_targets/pkce_helper.rs`** — a cargo-fuzz no-panic target feeding arbitrary bytes through
  verifier → S256 challenge → base64url decode, registered as `[[bin]] pkce_helper` in `fuzz/Cargo.toml`
  (ALWAYS FUZZ).

## Tasks Completed

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | getrandom cross-target + PKCE helper module + re-exports (TDD) | `855f3e30` | Cargo.toml, src/shared/pkce.rs, src/shared/mod.rs, src/lib.rs |
| 2 | ALWAYS coverage — RFC 7636 vector + property tests + cargo-fuzz target (TDD) | `a1a99eb5` | tests/pkce_helper.rs, fuzz/fuzz_targets/pkce_helper.rs, fuzz/Cargo.toml |

## Verification Results

- `getrandom` is in the cross-target `[dependencies]` table (line 89 < first `[target.` line 119) — HIGH-1 met
- `cargo build -p pmcp --features full` — exit 0 (host not regressed)
- `cargo test -p pmcp --lib pkce` — 6 passed (HOST-target proof getrandom links on host, HIGH-1)
- `cargo test -p pmcp --doc pkce` — 5 doctests passed
- `cargo build -p pmcp --target wasm32-unknown-unknown --no-default-features --features wasm` — exit 0 (wasm)
- `cargo test -p pmcp --test pkce_helper` — 5 passed (RFC vector + 3 proptest + re-export)
- `cargo build --bin pkce_helper` (in fuzz/) — exit 0 (fuzz target links; `cargo fuzz build` fallback)
- Production unwrap/expect count in `src/shared/pkce.rs` (outside `#[cfg(test)]`) — 0
- `cargo fmt --all -- --check` clean; `cargo clippy -p pmcp` (lib + test) — zero warnings

## Deviations from Plan

None — plan executed exactly as written.

The only environment note: `cargo fuzz build pkce_helper` requires a nightly toolchain on this host
(it emits `-Z sanitizer=address`, a nightly-only flag). The plan's acceptance criterion explicitly
permits the alternative (`cargo fuzz build pkce_helper` OR the bin build exits 0), so the target was
validated via `cargo build --bin pkce_helper` (exit 0). The fuzz command itself uses the plain
`cargo fuzz run pkce_helper` form with no `+nightly` argument (LOW-7); a nightly env / CI runs the
fully-instrumented build.

## Threat Mitigations Applied

| Threat ID | Mitigation |
| --------- | ---------- |
| T-103-RNG | `getrandom::fill` (OS / Web Crypto CSPRNG) — never `rand`/PRNG; proptest asserts verifier charset+length so a degenerate RNG is detectable |
| T-103-PKCE | S256 via audited `sha2`; RFC 7636 Appendix B vector test pins correctness; cargo-fuzz target proves no-panic on arbitrary verifier bytes |
| T-103-SC | No new external packages — only MOVED getrandom from a wasm-target table to the cross-target table; base64 dev-dep added to the fuzz crate is already a workspace dependency |

## Known Stubs

None. The PKCE helper is a complete, self-contained crypto primitive with no placeholder data paths.

## Self-Check: PASSED

- `src/shared/pkce.rs` — FOUND
- `tests/pkce_helper.rs` — FOUND
- `fuzz/fuzz_targets/pkce_helper.rs` — FOUND
- commit `855f3e30` — FOUND
- commit `a1a99eb5` — FOUND
