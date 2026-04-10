# Research Summary: rmcp Upgrades (v2.1 Documentation & DX)

**Project:** PMCP SDK — rmcp comparison / DX upgrade milestone
**Domain:** Rust crate documentation, docs.rs presentation, developer experience
**Researched:** 2026-04-10
**Confidence:** HIGH (all findings verified against actual source files in both repos)

---

## Executive Summary

PMCP is an established Rust MCP SDK with more raw documentation content than its main competitor rmcp, but it has a critical presentation gap: the right information exists in the wrong places, several files contain actively wrong content, and docs.rs surfaces internal/unstable APIs to users alongside production ones. This is a documentation architecture problem, not a documentation quantity problem — no new crates or runtime dependencies are needed to fix it.

The recommended approach is a three-phase upgrade: (1) stop the bleeding by replacing provably-wrong content (the examples/README.md is the Spin framework README, the protocol version badge is 6 months stale), (2) rewrite stale documentation to reflect the current API (the macros README documents deprecated `#[tool]` and claims `#[mcp_prompt]` doesn't exist — it does), and (3) fix the docs.rs rendering pipeline so feature-gated items display badges and users can see which features enable what. The entire upgrade is configuration changes, file rewrites, and targeted attribute additions — scope is well-bounded.

The key risk is drift: the examples/README.md became the Spin framework README because there was no enforcement mechanism. Every fix in this milestone must include a verification step (CI check, doctest, or count assertion) that prevents the same decay from happening again. The secondary risk is accidentally breaking the public API surface during doc-focused changes — run `cargo semver-checks` on every PR in this milestone.

---

## Key Findings

### Recommended Stack

No new dependencies are required for this milestone. All fixes are configuration and content changes. The relevant tooling already exists:

- `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` in `src/lib.rs` — single line that auto-generates feature badges for all 145 feature-gated items on docs.rs (currently only 6 have manual annotations, meaning 139 items show no badge)
- Explicit `[package.metadata.docs.rs]` feature list replacing `all-features = true` — prevents test helpers, wasm-only, and unstable APIs from surfacing in docs
- `#![doc = include_str!("../README.md")]` in `pmcp-macros/src/lib.rs` — appropriate for the macros crate (focused README); NOT recommended for the main pmcp crate (README contains deployment/CLI/book content)
- `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` added to CI — enforces zero doc warnings (currently 29 warnings including broken intra-doc links)

**Core changes (no new deps):**
- `src/lib.rs`: Add `doc_auto_cfg` attribute, expand feature flag table in doc comments
- `Cargo.toml`: Replace `all-features = true` with explicit 13-feature list, exclude internal features
- `pmcp-macros/README.md`: Complete rewrite (currently documents deprecated macros only)
- `examples/README.md`: Complete rewrite (currently contains Spin framework README — entirely wrong)
- `examples/` directory: Audit and resolve 17 orphan files, 4 duplicate number prefixes

### Expected Features (DX Capabilities)

**Must have — table stakes (credibility blockers if absent):**
- Accurate `examples/README.md` with correct PMCP content — first thing devs check
- Correct protocol version badge (`2025-11-25`, not `2025-03-26`) — active misinformation
- Macros README documenting `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` — current README steers users to deprecated `#[tool]`
- Consistent example numbering without collisions — examples 08, 11, 12, 32 each have two files
- Feature flag documentation table — 20+ features, none documented in any user-facing location

**Should have — competitive differentiators:**
- Feature badges on all 145 feature-gated docs.rs items (via `doc_auto_cfg` — one line)
- Transport matrix table in lib.rs docs linking to actual types
- Migration callout for `#[tool]` -> `#[mcp_tool]` (rmcp has migration guides prominently; PMCP v2.0 was a breaking change with no linked guide)
- Zero-warning doc build enforced in CI
- Ghost feature flags (`wasi-http`, `unstable`, `simd`) documented with status/intent

**Defer to later milestone:**
- Per-capability code examples in README (book/course fill this role for PMCP)
- Community showcase ("Built with") — cannot fabricate; add when real projects verified
- Subdirectory reorganization of examples (flat numbering works; reorganization is high churn for low gain)
- `document-features` crate (adds build dep for something a manual table does equally well)

### Architecture Integration Points

The documentation build pipeline has a specific dependency order that must be respected. Changes in the wrong order produce wasted rework:

**Major components affected:**

1. **`Cargo.toml` docs.rs metadata** — upstream gate; all feature-badge rendering depends on this being an explicit list, not `all-features = true`. This must change first.

2. **`src/lib.rs` crate attributes + doc comments** — add `doc_auto_cfg`, expand feature flag table. Depends on knowing the explicit feature list from step 1.

3. **`pmcp-macros/README.md` + `pmcp-macros/src/lib.rs`** — standalone rewrite; documents the current proc-macro API. No dependencies on other changes, but logically part of the same content pass.

4. **`examples/README.md` rewrite + orphan audit** — depends on knowing the complete feature list (step 2) to accurately list `required-features` per example. Must come after content pass so example descriptions reference the right macro names.

5. **CI enforcement gates** — doc-check Makefile target, example count assertion, semver-checks on every PR. These are the drift-prevention layer and should be added in the same phase as the content they guard.

**Dependency graph (short form):**
```
Cargo.toml (explicit features)
  -> lib.rs (doc_auto_cfg + feature table)
       -> macros README (current API names)
             -> examples/README.md (complete index)
                   -> CI gates (doc-check, count assertion, semver-checks)
```

### Critical Pitfalls

1. **examples/README.md is the Spin framework README** — replace immediately; this is the single highest-credibility damage. A dev browsing the examples/ directory on GitHub sees WebAssembly microservice content with zero relation to PMCP. Detection: `head -3 examples/README.md`.

2. **Macros README actively misleads** — documents `#[tool]` (deprecated since 0.3.0) as the primary macro, claims prompts/resources are "coming soon" when they shipped. New users will adopt the deprecated API and get deprecation warnings on first compile. Fix: complete README rewrite leading with `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]`.

3. **17 orphan example files not in Cargo.toml** — users cannot run them with `cargo run --example`, they do not compile in CI, and they silently rot. The number collisions (4 pairs of files sharing a prefix number) make any README index ambiguous. Fix: audit each — register with correct `required-features` or delete.

4. **`all-features = true` exposes internal APIs on docs.rs** — `test-helpers`, `unstable`, `simd`, and example-only feature flags appear on docs.rs as production APIs. Users see `authentication_example` as a feature they can enable. Fix: explicit 13-feature list in `[package.metadata.docs.rs]`, excluding all internal/test/example flags.

5. **Documentation changes that accidentally break public API** — doc cleanup is when `pub use` re-exports get moved or removed without noticing it's a semver break. The deprecated `#[tool]` macro must stay until a major version bump. Fix: `cargo semver-checks check-release` as a required check on every PR in this milestone.

---

## Implications for Roadmap

Based on combined research, the suggested phase structure follows the architecture dependency order above. Phases are scoped to be completable independently with verifiable exit criteria.

### Phase 1: Examples Cleanup and Credibility Fixes
**Rationale:** Two confirmed-broken files are actively damaging credibility right now — the wrong examples/README.md and the stale protocol version badge. These are the lowest-complexity, highest-impact items and unblock everything that comes after.
**Delivers:** Accurate examples/README.md, correct protocol badge, resolved orphan files, no duplicate example numbers, no `.disabled` files
**Addresses:** Table-stakes features (examples/README.md accuracy, consistent numbering)
**Avoids:** Pitfall 1 (Spin README), Pitfall 4 (orphan examples), Pitfall 2 (version badge), Pitfall 16 (duplicate numbers)
**Exit criteria:**
- `head -3 examples/README.md` shows PMCP content
- `grep 'MCP-v' README.md` version matches `LATEST_PROTOCOL_VERSION` in code
- Count of `*.rs` files in `examples/` equals count of `[[example]]` entries in Cargo.toml
- `ls examples/*.rs | awk -F_ '{print $1}' | sort | uniq -d` returns empty

### Phase 2: Macros Documentation Rewrite
**Rationale:** The macros README is the second-highest credibility damage. It documents a deprecated API as primary, claims shipped features are missing, and cites a stale version number. This must be fixed before the docs.rs pipeline work because the feature flag table will reference these macro names.
**Delivers:** Rewritten `pmcp-macros/README.md` covering current macros, added `include_str!` in macros lib.rs, updated version references, migration section for `#[tool]` -> `#[mcp_tool]`, additional compile-fail UI tests for `mcp_server` and `mcp_resource`
**Addresses:** Must-have (macros README accuracy), differentiator (migration callout)
**Avoids:** Pitfall 3 (deprecated macros as primary API), Pitfall 14 (missing compile-fail tests), Pitfall 15 (rust,ignore doctests)
**Exit criteria:**
- `grep -c 'mcp_tool\|mcp_server\|mcp_prompt\|mcp_resource' pmcp-macros/README.md` > 0
- `grep -c '^pmcp = { version = "1' pmcp-macros/README.md` == 0
- Compile-fail test for `mcp_server` and `mcp_resource` added to `tests/ui/`

### Phase 3: docs.rs Pipeline and Feature Flag Polish
**Rationale:** Once content is accurate (Phases 1-2), fix the rendering pipeline that determines what users see on docs.rs. The `doc_auto_cfg` one-liner is the highest-leverage change in the entire milestone (139 items gain badges). The explicit feature list prevents internal APIs from surfacing.
**Delivers:** `doc_auto_cfg` in lib.rs, explicit feature list in Cargo.toml docs.rs metadata, feature flag table in lib.rs doc comments, 10 missing `cfg_attr(docsrs, doc(cfg(...)))` annotations, 29 rustdoc warnings resolved, ghost feature flags documented, `make doc-check` CI target
**Addresses:** Should-have differentiators (feature badges, zero-warning docs), ghost feature flag documentation
**Avoids:** Pitfall 5 (29 doc warnings), Pitfall 6 (missing doc(cfg) annotations), Pitfall 7 (ghost feature flags), Pitfall 10 (all-features conflicts)
**Exit criteria:**
- `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits 0
- `src/lib.rs` contains `cfg_attr(docsrs, feature(doc_auto_cfg))`
- `Cargo.toml` `[package.metadata.docs.rs]` uses explicit feature list, not `all-features = true`

### Phase 4: General Documentation Polish
**Rationale:** Lower-urgency improvements that are valuable but not blocking. Lib.rs doctests show the legacy `ToolHandler` pattern; they compile but showcase suboptimal DX. This phase also adds the enforcement CI gates that prevent future drift.
**Delivers:** Updated lib.rs doctests showing `TypedTool`/`TypedToolWithOutput` pattern, transport matrix table in lib.rs docs, CI example-count assertion to prevent future orphan accumulation, `cargo semver-checks` added to PR gate
**Addresses:** Minor pitfall (stale crate-level doc examples), drift prevention (Pitfall 11)
**Avoids:** Pitfall 8 (stale lib.rs examples), Pitfall 9 (accidental API breakage), Pitfall 11 (README drifting again)
**Exit criteria:**
- lib.rs crate-level doc examples compile and show `TypedToolWithOutput` pattern
- CI check counts `[[example]]` entries vs `.rs` files, fails on mismatch
- `cargo semver-checks check-release` runs in CI

### Phase Ordering Rationale

- **Phases 1-2 before 3:** The feature flag table in Phase 3 must reference accurate macro names (Phase 2) and will be cross-linked from the examples README (Phase 1). Doing the pipeline fix first would produce technically correct badges on inaccurate content.
- **Phase 3 before 4:** The polish in Phase 4 should reference the feature names that come out of the Phase 3 explicit feature list. Running semver-checks (Phase 4 gate) is also only meaningful once the doc changes (Phases 1-3) are stable.
- **Phases 1 and 2 are partially parallel:** The examples cleanup and macros rewrite do not depend on each other. A team with two people could work them simultaneously.

### Research Flags

Phases where standard patterns apply (no additional research needed):
- **Phase 1** — file replacements and Cargo.toml audits; purely mechanical
- **Phase 2** — macro API is fully documented in `pmcp-macros/src/lib.rs`; rewrite follows that ground truth
- **Phase 3** — `doc_auto_cfg` is well-documented and the explicit feature list content is known

Phases that may need targeted investigation during implementation:
- **Phase 3 (rustdoc warnings)** — 9 of the 29 warnings are unresolved links that likely reference `pmcp-tasks` crate types. The fix may require adding a dependency or changing link format; inspect each warning before deciding approach.
- **Phase 4 (TypedToolWithOutput doctest)** — the doctest must compile with specific feature flags; verify which features are required before writing the doctest.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Zero new dependencies; changes are attribute lines and config values verified in both repos |
| Features | HIGH | All feature gaps confirmed by direct file reads; broken README content confirmed line-by-line |
| Architecture | HIGH | Dependency order derived from actual build pipeline mechanics, not speculation |
| Pitfalls | HIGH | All "critical" pitfalls are confirmed-existing bugs, not risks — the wrong README is literally there right now |

**Overall confidence:** HIGH

### Gaps to Address

- **Rustdoc warning origins** — 9 unresolved link warnings likely from `pmcp-tasks` types; the correct fix (intra-doc link fix vs. cross-crate link vs. removal) must be determined during Phase 3 implementation by inspecting each warning.
- **Orphan example viability** — 17 unregistered example files; each needs a compile check before deciding register-vs-delete. Some (like `32_simd_parsing_performance.rs`) need the `simd` feature which may have other constraints.
- **Ghost feature `wasi-http` disposition** — currently an empty feature flag with a "future" comment. The decision to document-as-planned vs. remove vs. implement is a product decision that research cannot make; flag for explicit decision during Phase 3.
- **docs.rs explicit feature list validation** — the 13-feature list proposed in ARCHITECTURE.md should be validated against a local `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc` run to confirm no build failures before merging.

---

## Sources

### Primary (HIGH confidence)
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/` — direct codebase audit (all PMCP findings)
- `/Users/guy/Development/mcp/sdk/rust-sdk/` — direct codebase audit (all rmcp comparison findings)
- `https://docs.rs/about/metadata` — docs.rs `[package.metadata.docs.rs]` configuration reference
- `https://doc.rust-lang.org/stable/unstable-book/language-features/doc-auto-cfg.html` — `doc_auto_cfg` specification

### Secondary (MEDIUM confidence)
- `https://vadosware.io/post/getting-features-to-show-up-in-your-rust-docs` — `doc_auto_cfg` usage guide
- `https://users.rust-lang.org/t/best-practice-for-documenting-crates-readme-md-vs-documentation-comments/124254` — README drift discussion
- `https://docs.rs/document-features/latest/document_features/` — evaluated and rejected (adds build dep for no gain over manual table)

### Tertiary (MEDIUM confidence)
- `https://github.com/rust-osdev/uefi-rs/pull/487` — real-world `doc_auto_cfg` adoption example

---

*Research completed: 2026-04-10*
*Ready for roadmap: yes*
