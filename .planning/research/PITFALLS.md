# Pitfalls Research: rmcp Upgrades

**Domain:** Documentation quality and developer experience upgrades for an established Rust SDK crate
**Researched:** 2026-04-10
**Overall Confidence:** HIGH (findings based on direct codebase audit, not just web research)

---

## Critical Pitfalls

Mistakes that cause user confusion, broken builds, or lost credibility. Ordered by severity.

### Pitfall 1: Examples README Is a Completely Wrong File (EXISTING - CONFIRMED)

**What goes wrong:** The file `examples/README.md` is the Spin framework README -- an entirely unrelated project. It describes WebAssembly microservices, links to Spin documentation, and has zero relationship to PMCP.

**Evidence:** Direct file read confirms `examples/README.md` starts with "Spin is a framework for building, deploying, and running fast, secure, and composable cloud microservices with WebAssembly" and includes Spin badges, Spin install instructions, and a Spin language support table.

**Consequences:** Any user browsing the GitHub examples/ directory sees a completely wrong README. This is the single most damaging credibility issue -- it signals the project does not maintain its own documentation.

**Prevention:** Replace with accurate example index before any other documentation work. This is the "stop the bleeding" item.

**Detection:** `head -3 examples/README.md` -- if it mentions Spin, it is wrong.

**Phase:** Must be Phase 1 (Examples README Fix). This is table-stakes credibility.

---

### Pitfall 2: README Protocol Version Badge Drift (EXISTING - CONFIRMED)

**What goes wrong:** The root README badge says `MCP-v2025-03-26` but the actual `LATEST_PROTOCOL_VERSION` constant in `src/types/protocol/version.rs` is `"2025-11-25"`. The SDK supports protocol version 2025-11-25 but the README advertises the older version.

**Evidence:** Badge markup: `[![MCP Compatible](https://img.shields.io/badge/MCP-v2025--03--26-blue.svg)]` vs code: `pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";`

**Consequences:** Users evaluating the SDK see an outdated protocol version and may choose rmcp instead, thinking PMCP has not been updated. This directly undermines the v2.0 work that upgraded protocol support.

**Prevention:** Add a CI check or `version-sync` style test that verifies the README badge matches the code constant. Alternatively, generate the badge dynamically.

**Detection:** `grep 'MCP-v' README.md` should match `grep LATEST_PROTOCOL_VERSION src/types/protocol/version.rs`.

**Phase:** Phase 1 alongside examples README fix (quick, high-impact).

---

### Pitfall 3: Macros README Documents Deprecated API, Ignores Current API (EXISTING - CONFIRMED)

**What goes wrong:** The `pmcp-macros/README.md` documents `#[tool]` and `#[tool_router]` as the primary macros. In reality, `#[tool]` has been deprecated since v0.3.0 in favor of `#[mcp_tool]`. The README also:
- Claims `pmcp = { version = "1.1" }` when the current version is 2.2.0
- Lists "Future Plans" including `#[prompt]` and `#[resource]` macros that already ship as `#[mcp_prompt]` and `#[mcp_resource]`
- Says "Currently only supports tools (prompts and resources coming soon)" -- but all three are implemented
- Does not mention `#[mcp_server]`, `#[mcp_prompt]`, or `#[mcp_resource]` at all

**Evidence:** `pmcp-macros/src/lib.rs` line 97: `#[deprecated(since = "0.3.0", note = "Use #[mcp_tool] instead")]` on the `#[tool]` macro. The lib.rs exports `mcp_tool`, `mcp_server`, `mcp_prompt`, `mcp_resource` -- none of which appear in the README.

**Consequences:** Users following the README will use deprecated APIs and get deprecation warnings. Users looking for prompt/resource macros will think they do not exist and write boilerplate by hand. This is exactly the kind of DX gap that rmcp can exploit.

**Prevention:** Rewrite the README to document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` as the primary macros. Add a migration section for `#[tool]` -> `#[mcp_tool]`.

**Phase:** Phase 2 (Macros Documentation). Critical for DX credibility but slightly less urgent than the examples README.

---

### Pitfall 4: 17 Orphan Example Files Not Registered in Cargo.toml (EXISTING - CONFIRMED)

**What goes wrong:** There are 63 `.rs` files in `examples/` but only 46 `[[example]]` entries in `Cargo.toml`. The 17 unregistered files include substantive examples like `47_multiple_clients_parallel.rs`, `48_structured_output_schema.rs`, `54_hybrid_workflow_execution.rs`, and `61_observability_middleware.rs`.

**Evidence:** Direct comparison of `ls examples/*.rs` (63 files) vs `grep '^\[\[example\]\]' Cargo.toml` (46 entries). Unregistered files: `08_server_resources.rs`, `11_progress_countdown.rs`, `12_prompt_workflow_progress.rs`, `32_simd_parsing_performance.rs`, `40_middleware_demo.rs`, `47_multiple_clients_parallel.rs`, `48_structured_output_schema.rs`, `54_hybrid_workflow_execution.rs`, `58_oauth_transport_to_tools.rs`, `59_dynamic_resource_workflow.rs`, `60_resource_only_steps.rs`, `61_observability_middleware.rs`, `client.rs`, `currency_server.rs`, `refactored_server_example.rs`, `server.rs`, `test_currency_server.rs`.

**Consequences:**
1. Users cannot run these examples with `cargo run --example` (they get "no example target" errors)
2. These files do not get compiled during CI, so they may silently rot and reference removed APIs
3. The example numbering has collisions (08, 11, 12, 32 each have two files with the same prefix)

**Why it happens:** Examples accumulate organically. When a new example is added, the author may forget to add the `[[example]]` entry, or may have intended it as a draft that never got finalized.

**Prevention:**
- Add a CI check: count `.rs` files in examples/ vs `[[example]]` entries in Cargo.toml; fail if they differ
- Either register orphan examples or delete/archive them
- Resolve number collisions by renaming one of each pair

**Phase:** Phase 1 (Examples README Fix should audit and register/remove orphans simultaneously).

---

### Pitfall 5: 29 Rustdoc Warnings Including Broken Links (EXISTING - CONFIRMED)

**What goes wrong:** `cargo doc --all-features --no-deps` produces 29 warnings:
- 9x "unresolved link to [REDACTED]" (likely referencing types from pmcp-tasks crate that are not in scope)
- 2x "unresolved link to TaskStore"
- 2x "unclosed HTML tag `str`"
- 3x "public documentation links to private item" (PauseReason, StepStatus, insert_legacy_resource_uri_key)
- 1x "redundant explicit link target"
- Various unresolved links to `Task`, `TaskRouter`, `router`, `router_with_config`, etc.

**Consequences:** docs.rs renders these as broken links. Users clicking through to understand types hit dead ends. This signals neglect.

**Prevention:** Zero-warning doc build as a CI gate. Run `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` in CI.

**Detection:** `cargo doc --all-features --no-deps 2>&1 | grep "warning:" | grep -v "generated" | wc -l` should be 0.

**Phase:** Phase 3 (docs.rs and Feature Flag Polish). Can be done incrementally.

---

## Moderate Pitfalls

### Pitfall 6: Feature-Gated Items Missing doc(cfg) Annotations -- Systematic Gap

**What goes wrong:** In `src/lib.rs`, 3 of 5 feature-gated items lack `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]` annotations: `simd`, `macros`, `streamable-http` (the second re-export). In `src/server/mod.rs`, 17 of 29 feature-gated items lack doc(cfg) annotations, particularly `schema-generation` (6 missing) and `mcp-apps` (4 missing).

**Evidence:** Only 3 `cfg_attr(docsrs` annotations vs 29 `#[cfg(feature` occurrences in `server/mod.rs`. Specifically missing: all `schema-generation` gated builder methods, several `mcp-apps` gated items, multiple `streamable-http` internal items.

**Consequences:** On docs.rs, users see these items but cannot tell which feature flag enables them. When they try to use them without the right feature, they get cryptic "not found" compile errors instead of a clear "this requires feature X" message.

**Prevention:**
- Use `doc_auto_cfg` (nightly-only, but docs.rs uses nightly). Add `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` to `lib.rs` -- this automatically generates doc(cfg) annotations for ALL feature-gated items without manual tagging.
- This single line replaces all 20+ manual `cfg_attr(docsrs, doc(cfg(...)))` annotations and catches future additions automatically.

**Phase:** Phase 3 (docs.rs Polish). One-line fix with massive impact.

**Confidence:** HIGH -- `doc_auto_cfg` is well-established for docs.rs builds. See [rust-osdev/uefi-rs PR #487](https://github.com/rust-osdev/uefi-rs/pull/487) and [VADOSWARE guide](https://vadosware.io/post/getting-features-to-show-up-in-your-rust-docs).

---

### Pitfall 7: Ghost Feature Flags (Declared but Empty or Misleading)

**What goes wrong:** Several feature flags in Cargo.toml have no or minimal implementation:
- `wasi-http = []` -- comment says "Future: WASI HTTP support (not yet implemented)" but it is a published, visible feature
- `unstable = []` -- empty feature flag, no code references it
- `simd = []` -- empty feature flag, separate from `rayon` which does actual SIMD work
- `authentication_example`, `cancellation_example`, `progress_example` -- these are example-only features that clutter the public feature list

**Evidence:** `grep -rn 'wasi-http\|wasi_http' src/` finds only 2 lines in `server/wasi_adapter.rs`, gated behind `target_arch = "wasm32"`. The `unstable` and `simd` features enable no dependencies and no conditional code.

**Consequences:** Users may enable features expecting functionality. `wasi-http` is particularly misleading -- a user wanting WASI HTTP support would enable it, find nothing works, and lose trust.

**Prevention:**
- Document each feature in a Feature Reference table in the crate-level docs
- Add `[!WARNING]` annotations for features that are placeholders
- Consider removing `wasi-http` until it is implemented, or at minimum adding doc comments explaining its status
- Use the `document-features` crate to auto-generate feature documentation from Cargo.toml comments

**Phase:** Phase 3 (Feature Flag Documentation).

---

### Pitfall 8: Lib.rs Crate-Level Doc Examples May Be Stale

**What goes wrong:** The crate-level doc examples in `src/lib.rs` show a simplified API that may not match the recommended pattern:
- Client example: `Client::new(transport)` then `client.initialize(ClientCapabilities::default()).await?` -- this pattern exists but the builder pattern (`ClientBuilder`) is the recommended API
- Server example: `Server::builder().name().version().capabilities().tool("my-tool", MyTool).build()?` -- uses `ToolHandler` trait which exists but the `TypedTool` pattern is the modern recommended approach
- Neither example mentions feature flags needed (the server example would need `schema-generation` for typed tools)

**Consequences:** New users copy these examples and end up using the legacy API pattern instead of the recommended modern pattern. The examples compile (they are doctests) but do not showcase the best DX.

**Prevention:** Update crate-level docs to show the modern `TypedTool`/`TypedToolWithOutput` pattern and the builder with `.tool_typed()` method. Keep the simple `ToolHandler` example but label it as the manual approach.

**Phase:** Phase 4 (General Documentation Polish). Lower urgency since the existing examples do compile.

---

### Pitfall 9: Accidentally Breaking Public API During Doc Changes

**What goes wrong:** When reorganizing documentation, it is easy to accidentally:
- Change `pub use` re-exports (removing or renaming a type from the public API)
- Move a `#[cfg(feature = "...")]` gate to cover more or fewer items
- Change the feature flag a type is gated behind
- Remove a deprecated item before the deprecation period ends

**Evidence:** The `#[deprecated]` on `#[tool]` macro (since 0.3.0) is correct practice, but if someone removes the deprecated macro during the doc cleanup, it breaks all users still on the old API.

**Consequences:** Semver violation. Users' builds break on a "documentation" update.

**Prevention:**
- Run `cargo semver-checks` before merging any doc PR to verify no public API changes
- Add `cargo public-api` snapshot test
- Review diffs for `pub use`, `pub mod`, `pub fn`, `pub struct`, `pub trait`, `pub enum` changes

**Detection:** `cargo semver-checks check-release` should pass with zero findings.

**Phase:** Every phase -- this is a process gate, not a single fix.

---

### Pitfall 10: docs.rs all-features=true vs Feature Conflicts

**What goes wrong:** PMCP uses `all-features = true` for docs.rs, which enables every feature including `wasm`, `oauth`, `wasi-http`, etc. Features designed for WASM targets (`wasm`, `websocket-wasm`) could conflict with native features on docs.rs's x86_64 build environment.

**Evidence:** The `wasm` feature enables `dep:futures-channel` and `dep:futures-locks` which are platform-agnostic, while the actual WASM-only code is `#[cfg(target_arch = "wasm32")]` gated. Current build succeeds (verified locally). However, the `full` feature does NOT include `wasm`, `oauth`, `wasi-http`, `unstable`, `simd`, or `test-helpers` -- meaning docs.rs shows more than what `full` users see.

**Consequences:** Currently benign, but fragile. Any future code that uses `cfg(feature = "wasm")` without also checking `target_arch = "wasm32"` will break the docs.rs build. Also, users see features on docs.rs that are not part of the recommended `full` feature set, creating confusion.

**Prevention:**
- Consider using `features = ["full"]` instead of `all-features = true` in docs.rs metadata, to show exactly what most users will see
- OR keep `all-features = true` but add `default-target = "x86_64-unknown-linux-gnu"` and `targets = ["x86_64-unknown-linux-gnu", "wasm32-unknown-unknown"]` to docs.rs metadata for cross-platform docs
- Add a CI job that runs `cargo doc --all-features` to catch breakage early

**Phase:** Phase 3 (docs.rs Polish). Decision point: full vs all-features.

---

### Pitfall 11: Writing an Example Index That Drifts Again Immediately

**What goes wrong:** Replacing the wrong examples/README.md with a correct one solves the immediate problem but creates a new maintenance burden. The README lists example names and descriptions manually; as new examples are added or removed, the README drifts again within one or two milestones. This is the core problem documented in the [Rust Forum discussion on README drift](https://users.rust-lang.org/t/best-practice-for-documenting-crates-readme-md-vs-documentation-comments/124254).

**Why it happens:** Manual documentation always drifts from code unless there is an enforcement mechanism.

**Prevention:**
- Generate the example table from `Cargo.toml` `[[example]]` entries using a script or CI step
- Add a CI check that parses `[[example]]` entries and verifies each has a corresponding entry in examples/README.md
- OR use the first `//!` doc comment line from each example file as its description in the README, generated automatically

**Phase:** Phase 1 (Examples README). Build the drift-prevention mechanism alongside the initial fix.

---

## Minor Pitfalls

### Pitfall 12: Disabled Example File Sitting in Directory

**What goes wrong:** `examples/21_macro_tools.rs.disabled` exists as a renamed-but-not-deleted file. The `[[example]]` entry for it is commented out in Cargo.toml. This suggests an example that broke and was disabled rather than fixed.

**Prevention:** Either fix and re-enable, or delete entirely. `.disabled` files in a public repo look unprofessional.

**Phase:** Phase 1 (Examples cleanup).

---

### Pitfall 13: Example Files with No Clear Purpose

**What goes wrong:** Several unnumbered example files exist: `client.rs`, `server.rs`, `refactored_server_example.rs`, `test_currency_server.rs`. These are not registered in Cargo.toml and lack the numbered prefix convention.

**Prevention:** Either integrate into the numbered system or move to `tests/` if they are test files, or delete if they are obsolete drafts.

**Phase:** Phase 1 (Examples cleanup).

---

### Pitfall 14: Compile-Fail Test Coverage Gaps for Macros

**What goes wrong:** Only 4 compile-fail UI tests exist for macros:
- `mcp_tool_missing_description.rs`
- `mcp_tool_multiple_args.rs`
- `tool_missing_description.rs`
- `mcp_prompt_missing_description.rs`

Missing: `mcp_server` compile-fail tests, `mcp_resource` compile-fail tests, `tool_router` compile-fail tests. When documentation says "this will produce a compile error if you do X", there should be a test verifying that.

**Prevention:** Add compile-fail tests for each macro's common error paths before documenting error messages. Users trust documented error messages only if they are tested.

**Phase:** Phase 2 (Macros Documentation). Add compile-fail tests alongside macro doc improvements.

---

### Pitfall 15: Over-Documenting While Under-Testing

**What goes wrong:** When improving documentation quality, teams often write beautiful docs with examples that are not tested. Rust doctests help, but `rust,ignore` annotations (used in several places in `pmcp-macros/src/lib.rs`) mean those examples do not compile-check.

**Evidence:** Multiple doc examples in `pmcp-macros/src/lib.rs` use `rust,ignore` instead of actual compilable doctests.

**Prevention:** Convert `rust,ignore` to actual compilable doctests where possible. For proc-macro crates, use integration test files that exercise the examples.

**Phase:** Phase 2 (Macros Documentation).

---

### Pitfall 16: Duplicate Example Number Prefixes Cause User Confusion

**What goes wrong:** Four number prefixes are used by two different example files:
- 08: `08_logging.rs` AND `08_server_resources.rs`
- 11: `11_request_cancellation.rs` AND `11_progress_countdown.rs`
- 12: `12_error_handling.rs` AND `12_prompt_workflow_progress.rs`
- 32: `32_typed_tools.rs` AND `32_simd_parsing_performance.rs`

**Consequences:** The numbering system implies ordering and uniqueness. Duplicates break both: users cannot reference "example 11" unambiguously, and the progression from 1 to N loses coherence.

**Prevention:** Renumber one of each pair. The registered (Cargo.toml) version keeps its number; the orphan gets a new unique number or is removed.

**Phase:** Phase 1 (Examples cleanup).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Phase 1: Examples README | Writing a README that drifts again immediately | Generate part of it from Cargo.toml `[[example]]` entries via CI or script |
| Phase 1: Examples README | Registering orphan examples that do not compile with current features | Run `cargo check --example NAME` for each before registering |
| Phase 1: Examples README | Renumbering examples breaks existing references in docs/course | Search all .md files for old example names before renaming |
| Phase 2: Macros docs | Documenting macro usage patterns that do not match what the macro actually generates | Add integration tests that compile the documented patterns |
| Phase 2: Macros docs | Forgetting to deprecation-warn users of `#[tool]` -> `#[mcp_tool]` migration | Add explicit migration section, keep deprecated macro with clear pointer |
| Phase 2: Macros docs | Removing the deprecated `#[tool]` and `#[tool_router]` macros | These must stay until a major version bump; only update docs to steer users away |
| Phase 3: docs.rs polish | Adding `doc_auto_cfg` then finding it generates confusing badges for internal features | Audit which features should be user-facing vs internal |
| Phase 3: docs.rs polish | Fixing doc warnings by removing cross-crate links instead of adding proper `use` imports | Prefer `[`Type`](crate::path::Type)` intra-doc links over deletion |
| Phase 3: Feature flags | Removing a feature flag that users depend on (even if it is a no-op) | Check crates.io download stats or just deprecate with doc comment |
| Phase 3: Feature flags | Documenting features that do not exist yet (wasi-http) without clear "planned" marking | Use crates.io alert blocks: `[!WARNING] This feature is planned but not yet implemented` |
| Phase 4: General polish | Updating README examples to use the newest API while breaking copy-paste for existing users | Keep both old and new patterns, clearly label which is recommended |
| All phases | Making doc changes that accidentally alter public API surface | Run `cargo semver-checks` as part of every PR in this milestone |

## Summary of Existing Issues Found

| Issue | Severity | File(s) | Phase |
|-------|----------|---------|-------|
| Examples README is Spin framework README | Critical | `examples/README.md` | 1 |
| README MCP version badge shows 2025-03-26, code is 2025-11-25 | Critical | `README.md` | 1 |
| Macros README documents deprecated `#[tool]`, ignores `#[mcp_tool]` et al | Critical | `pmcp-macros/README.md` | 2 |
| Macros README claims pmcp version 1.1, current is 2.2.0 | Critical | `pmcp-macros/README.md` | 2 |
| 17 orphan `.rs` files not in Cargo.toml | Critical | `examples/*.rs`, `Cargo.toml` | 1 |
| 4 duplicate example number prefixes (08, 11, 12, 32) | Moderate | `examples/` | 1 |
| 29 rustdoc warnings (broken links, private item links) | Moderate | Various `src/` | 3 |
| 17 of 29 feature-gated items in server/mod.rs lack doc(cfg) | Moderate | `src/server/mod.rs` | 3 |
| 3 of 5 feature-gated items in lib.rs lack doc(cfg) | Moderate | `src/lib.rs` | 3 |
| Ghost feature flags (wasi-http, unstable, simd) | Moderate | `Cargo.toml` | 3 |
| `21_macro_tools.rs.disabled` exists as dead file | Minor | `examples/` | 1 |
| Unnumbered orphan examples (client.rs, server.rs, etc.) | Minor | `examples/` | 1 |
| Only 4 compile-fail UI tests for 5 proc macros | Minor | `pmcp-macros/tests/ui/` | 2 |
| Crate-level doc examples show legacy pattern | Minor | `src/lib.rs` | 4 |
| Macros README "Future Plans" lists already-implemented features | Minor | `pmcp-macros/README.md` | 2 |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Wrong examples README | Phase 1 | `head -3 examples/README.md` mentions PMCP, not Spin |
| Protocol version badge drift | Phase 1 | CI test: badge version == LATEST_PROTOCOL_VERSION |
| Orphan example files | Phase 1 | Count of `.rs` files == count of `[[example]]` entries |
| Duplicate number prefixes | Phase 1 | `ls examples/*.rs \| awk -F_ '{print $1}' \| sort \| uniq -d` returns empty |
| Deprecated macros in README | Phase 2 | README contains `#[mcp_tool]`, not `#[tool]` as primary |
| Missing compile-fail tests | Phase 2 | One `.stderr` file per macro in `tests/ui/` |
| doc(cfg) annotation gaps | Phase 3 | `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` in lib.rs |
| Rustdoc warnings | Phase 3 | `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits 0 |
| Ghost feature flags | Phase 3 | Feature table in crate docs lists all features with status |
| API breakage from doc changes | All phases | `cargo semver-checks check-release` passes |

## Sources

- Direct codebase audit of `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/` -- all "EXISTING - CONFIRMED" findings
- [docs.rs Builds documentation](https://docs.rs/about/builds) -- HIGH confidence
- [docs.rs Metadata documentation](https://docs.rs/about/metadata) -- HIGH confidence
- [VADOSWARE: Getting Features to Show Up in Your Rust Docs](https://vadosware.io/post/getting-features-to-show-up-in-your-rust-docs) -- MEDIUM confidence
- [rust-osdev/uefi-rs PR #487: Use doc_auto_cfg](https://github.com/rust-osdev/uefi-rs/pull/487) -- HIGH confidence
- [Cargo Features Reference](https://doc.rust-lang.org/cargo/reference/features.html) -- HIGH confidence
- [Rust Forum: Best practice for documenting crates](https://users.rust-lang.org/t/best-practice-for-documenting-crates-readme-md-vs-documentation-comments/124254) -- MEDIUM confidence
- [readme-sync crate](https://docs.rs/readme-sync) -- MEDIUM confidence
- [document-features crate](https://docs.rs/document-features/latest/document_features/) -- HIGH confidence
- [apache/opendal issue #6655: doc_auto_cfg removal](https://github.com/apache/opendal/issues/6655) -- MEDIUM confidence (watch for doc_auto_cfg stability changes)
