---
phase: 92
slug: bundlesource-served-tool-toolkit-module
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-10
---

# Phase 92 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Populated from 92-RESEARCH.md "## Validation Architecture" — the planner
> fills the Per-Task Verification Map when PLAN.md tasks are finalized.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest (property) + cargo-fuzz |
| **Config file** | root `Cargo.toml` workspace; crate-level `Cargo.toml` per member |
| **Quick run command** | `cargo test -p pmcp-workbook-runtime -p pmcp-server-toolkit --features workbook` |
| **Full suite command** | `make quality-gate` (fmt + clippy pedantic/nursery + build + test + audit) |
| **Estimated runtime** | quick ~60s; full ~10min |

---

## Sampling Rate

- **After every task commit:** Run the quick run command
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green; `make purity-check` (incl. new toolkit `workbook` + `workbook-embedded` combos) must pass
- **Max feedback latency:** ~600 seconds (full gate)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| P01-T1 include_dir legitimacy gate | 92-01 | 1 | WBSV-09 | T-92-SC | Blocking human-verify before any include_dir install (author/license/repo + cargo audit) | checkpoint:human-verify | MISSING — blocking-human; `cargo audit` is the post-install automated gate (P01-T4) | N/A | ⬜ pending |
| P01-T2 BundleSource trait + Local/Embedded impls | 92-01 | 1 | WBSV-09 | T-92-03 | Dumb-byte source only (no parsed-bundle return) → integrity gate cannot be bypassed; object-safe Send+Sync | unit + compile-assert | `cargo test -p pmcp-workbook-runtime bundle_source && cargo test -p pmcp-workbook-runtime --features embedded bundle_source` | ✅ src/bundle_source.rs | ⬜ pending |
| P01-T3 BundleLoader fail-closed integrity + model scrub | 92-01 | 1 | WBSV-08, WBSV-02 | T-92-01/02/04 | Recompute BUNDLE.lock hash + verify_stamp_binding fail-closed (IntegrityMismatch/StampMismatch/Parse); bundle_id rename; additive annotations | unit | `cargo test -p pmcp-workbook-runtime bundle_loader && cargo test -p pmcp-workbook-runtime manifest_model && cargo test -p pmcp-workbook-runtime artifact_model` | ✅ src/bundle_loader.rs | ⬜ pending |
| P01-T4 Wire embedded feature + deps | 92-01 | 1 | WBSV-09 | T-92-SC | Reader-free with/without embedded; include_dir gated + audited | build + audit | `cargo build -p pmcp-workbook-runtime && cargo build -p pmcp-workbook-runtime --features embedded && cargo audit` | ✅ Cargo.toml | ⬜ pending |
| P02-T1 Synthetic tax-calc fixture generator | 92-02 | 2 | WBSV-08, WBSV-09 | T-92-06/07 | Deterministic (fixed serde config, no unordered iteration); lock via runtime build_bundle_lock; zero customer identifiers | unit (generator) | `cargo test -p pmcp-server-toolkit --test fixture_byte_stability generate 2>/dev/null \|\| cargo build -p pmcp-server-toolkit --tests` | ✅ tests/support/fixture_gen.rs | ⬜ pending |
| P02-T2 Commit golden + byte-stability + golden-loads | 92-02 | 2 | WBSV-08, WBSV-09 | T-92-05/06 | Golden regenerates byte-identical (CI check) AND passes BundleLoader boot gate | integration | `cargo test -p pmcp-server-toolkit --test fixture_byte_stability` | ✅ tests/fixture_byte_stability.rs + tests/fixtures/tax-calc@1.0.0/* | ⬜ pending |
| P02-T3 Tamper helpers (WBSV-06/08 negatives) | 92-02 | 2 | WBSV-08 | T-92-08 | Tamper-at-test-time only (tempdir copy); each helper provokes a distinct BundleLoadError; no committed corrupt fixtures | unit (smoke) | `cargo test -p pmcp-server-toolkit --test fixture_byte_stability tamper 2>/dev/null; cargo test -p pmcp-server-toolkit tamper` | ✅ tests/support/tamper.rs | ⬜ pending |
| P03-T1 isError envelope (error.rs) + schema projection (schema.rs) | 92-03 | 3 | WBSV-06, WBSV-07 | T-92-10/12 | to_iserror_result isError:true in structuredContent + ProvStamp(bundle_id); additionalProperties:false, non-empty outputSchema, S-1 headline dropped | tdd unit | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::error workbook::schema 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::error workbook::schema` | ✅ src/workbook/error.rs + schema.rs | ⬜ pending |
| P03-T2 Fail-closed input validation (input.rs) | 92-03 | 3 | WBSV-06 | T-92-09 | WR-05 no-role reject, WR-02 string-only enum, V4 strict-constant reject; proptest totality (never panics) | tdd unit + proptest | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::input 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::input` | ✅ src/workbook/input.rs | ⬜ pending |
| P03-T3a CalculateHandler + shared helpers (handler.rs) | 92-03 | 3 | WBSV-01 | T-92-10/11/12 | All-outputs {value,unit} + provenance, no headline; WR-06 finiteness; failure → isError in structuredContent (never protocol Err); cog ≤25 | tdd unit | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::handler 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::handler` | ✅ src/workbook/handler.rs | ⬜ pending |
| P03-T3b Explain/GetManifest/DiffVersion handlers (handler.rs) | 92-03 | 3 | WBSV-02, WBSV-03, WBSV-04 | T-92-10/13 | Generic manifest-declared annotations (S-2 coil_band removed); curated manifest projection; recorded changelog served; cog ≤25 | tdd unit | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::handler 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::handler` | ✅ src/workbook/handler.rs | ⬜ pending |
| P04-T1 workbook:// URI codec (render_uri.rs) + render_workbook handler | 92-04 | 4 | WBSV-05 | T-92-14/17 | Size-guard-first decode (MAX_ENCODED_URI_LEN before base64), total panic-free decode; pointer-not-bytes; proptest round-trip + decode-fuzz | tdd unit + proptest | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::render_uri 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::render_uri` | ✅ src/workbook/render_uri.rs | ⬜ pending |
| P04-T2 Stateless regen-on-read resource (render_resource.rs) | 92-04 | 4 | WBSV-05 | T-92-15/16/18 | Provenance-verify-before-render (spoofing guard), RE-VALIDATE decoded inputs (injection guard), stateless determinism (no session) | tdd unit | `cargo test -p pmcp-server-toolkit --features workbook,http workbook::render_resource 2>/dev/null \|\| cargo test -p pmcp-server-toolkit --features workbook workbook::render_resource` | ✅ src/workbook/render_resource.rs | ⬜ pending |
| P04-T3 Publish workbook:// URI public contract | 92-04 | 4 | WBSV-05 | — | Documented versioned public SDK contract (scheme, payload, size bound, stateless-regen, security properties); tax-domain examples only | doc-exists | `test -f docs/workbook-uri-spec.md && grep -q "workbook://" docs/workbook-uri-spec.md && grep -qi "MAX_ENCODED_URI_LEN\|size" docs/workbook-uri-spec.md && echo OK` | ✅ docs/workbook-uri-spec.md | ⬜ pending |
| P05-T1 WorkbookBuilderExt registration + lib re-exports + Cargo features | 92-05 | 5 | WBSV-01, WBSV-05, WBSV-08, WBSV-09 | T-92-20/21 | Fail-closed boot load via BundleLoader; full boot-surface re-export (D-11); workbook/workbook-embedded out of default (D-06/D-10) | build + unit | `cargo build -p pmcp-server-toolkit --features workbook && cargo build -p pmcp-server-toolkit --features workbook-embedded && cargo test -p pmcp-server-toolkit --features workbook workbook::mod 2>/dev/null; cargo build -p pmcp-server-toolkit` | ✅ src/workbook/mod.rs + lib.rs + Cargo.toml | ⬜ pending |
| P05-T2 Streamable-HTTP example + integration tests | 92-05 | 5 | WBSV-01, WBSV-08, WBSV-09 | T-92-20 | Example boots from EmbeddedSource (include_dir! over committed golden); build-and-assert 5 tools; tamper-fails-boot through builder | build + integration | `cargo build --example workbook_server_http -p pmcp-server-toolkit --features workbook-embedded,http && cargo test -p pmcp-server-toolkit --features workbook,http --test workbook_integration` | ✅ examples/workbook_server_http.rs + tests/workbook_integration.rs | ⬜ pending |
| P05-T3 Extend purity gate (justfile-first + Makefile parity) | 92-05 | 5 | WBSV-01 | T-92-19 | Distinct per-feature reader-absence assertions for BOTH workbook and workbook-embedded trees; fail-closed on non-zero cargo status; merge-blocking | gate | `just purity-check && grep -E "features workbook\b" justfile && grep -E "features workbook-embedded" justfile && grep -E "features workbook\b" Makefile && grep -E "features workbook-embedded" Makefile` | ✅ justfile + Makefile + ci.yml | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

> Wave 0 materials are fully planned in Plan 02 (Wave 2): the synthetic golden
> fixture generator, byte-stability check, tamper helpers, and the `workbook`
> feature wiring (Plan 01 Task 4 + Plan 05 Task 1). All WBSV tests have a
> defined automated command in the map above → `nyquist_compliant: true`.

- [x] Synthetic tax-calc fixture generator (test-support) + committed golden bundle under fixtures dir — every WBSV test depends on it (Plan 02 Task 1-2)
- [x] Byte-stability check: regenerating the golden is byte-identical (CI-checkable command) (Plan 02 Task 2 — `golden_regeneration_is_byte_identical`)
- [x] Tamper helpers (copy-to-tempdir + corrupt) for WBSV-08/WBSV-06 negative paths (Plan 02 Task 3 — `copy_golden_to_temp` / `flip_byte` / `delete_artifact` / `desync_lock_version`)
- [x] `workbook` feature wiring in pmcp-server-toolkit so `cargo test --features workbook` compiles (Plan 01 Task 4 runtime `embedded` + Plan 05 Task 1 toolkit `workbook`/`workbook-embedded`)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `include_dir 0.7.4` dependency vetting | D-06 | New external dep ([ASSUMED] in research) | checkpoint:human-verify before install (Plan 01 Task 1) — confirm crate name/version/maintenance on crates.io |
| Streamable-HTTP example end-to-end | D-12 | Real client against running server | `cargo run --example workbook_server_http --features workbook-embedded,http` then mcp-tester against the HTTP endpoint; confirm all five tools respond |
