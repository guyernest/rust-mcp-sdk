# Architecture Research: rmcp Documentation/DX Upgrades

**Domain:** Rust crate documentation, docs.rs presentation, developer experience
**Researched:** 2026-04-10
**Overall confidence:** HIGH (direct codebase comparison, no external sources needed)

## Executive Summary

PMCP has significant documentation architecture gaps compared to rmcp. The issues fall into six categories: (1) docs.rs feature metadata uses `all-features = true` which hides feature gate information from users, (2) the examples/README.md is literally a copy of the Spin framework README -- completely bogus, (3) the pmcp-macros README documents deprecated `#[tool]`/`#[tool_router]` but not the current `#[mcp_tool]`/`#[mcp_server]`/`#[mcp_prompt]`/`#[mcp_resource]` macros, (4) only 7 out of ~131 feature-gated items have `cfg_attr(docsrs, doc(cfg(...)))` annotations, (5) lib.rs crate docs use inline doc comments instead of the README-as-docs pattern, and (6) these changes have a specific dependency order that must be followed.

## Recommended Architecture

### Component 1: docs.rs Feature Metadata (Cargo.toml)

**Current state (PMCP):**
```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

**rmcp's approach:**
```toml
[package.metadata.docs.rs]
features = [
  "auth", "client", "macros", "server",
  "transport-io", "transport-child-process",
  "transport-streamable-http-server",
  # ... explicit list of every feature
]
rustdoc-args = ["--cfg", "docsrs"]
```

**Problem with `all-features = true`:** It enables every feature including test-helpers, unstable, simd, authentication_example, cancellation_example, progress_example, wasm, wasi-http, etc. This means:
1. Test scaffolding and example-only features leak into docs
2. Mutually exclusive features (wasm vs non-wasm) may conflict
3. Users cannot see which items require which features -- everything appears unconditionally available

**Recommendation:** Replace `all-features = true` with an explicit feature list. Include all user-facing features, exclude internal/test/example features.

**Explicit feature list for PMCP:**
```toml
[package.metadata.docs.rs]
features = [
  "websocket",
  "http",
  "streamable-http",
  "sse",
  "validation",
  "resource-watcher",
  "schema-generation",
  "jwt-auth",
  "composition",
  "mcp-apps",
  "http-client",
  "logging",
  "macros",
  "oauth",
]
rustdoc-args = ["--cfg", "docsrs"]
```

Excluded: `wasm`, `websocket-wasm`, `wasm-tokio`, `wasi-http` (platform-specific, not buildable together with native), `full` (redundant -- list individual features), `test-helpers`, `unstable`, `simd`, `rayon`, `authentication_example`, `cancellation_example`, `progress_example`.

### Component 2: Feature Gate Annotations (cfg_attr docsrs)

**Current state (PMCP):** 7 annotations across 3 files (lib.rs, server/mod.rs, types/mod.rs).

**Current state (rmcp):** Only 1 annotation (lib.rs preamble). rmcp relies on `cfg_attr(docsrs, feature(doc_cfg))` in lib.rs but does NOT systematically annotate individual items -- they rely on Rust nightly `doc_cfg` auto-detection instead.

**Key insight:** The `#![cfg_attr(docsrs, feature(doc_cfg))]` attribute in lib.rs enables the unstable `doc_cfg` feature on docs.rs nightly builds. When combined with `--cfg docsrs`, this causes rustdoc to **automatically** display feature requirements for items behind `#[cfg(feature = "...")]`. The manual `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]` annotations are only needed when the auto-detection does not work correctly (e.g., complex conditional compilation with `all()`, `any()`, or platform-specific gates).

**However**, PMCP has many items gated on compound conditions like `#[cfg(all(not(target_arch = "wasm32"), feature = "mcp-apps"))]` where auto-detection would show the full compound condition. Manual annotation with just `doc(cfg(feature = "mcp-apps"))` provides a cleaner user-facing label.

**Recommendation:** Add `cfg_attr(docsrs, doc(cfg(...)))` annotations to:
1. All public modules gated on features in lib.rs, server/mod.rs, shared/mod.rs, types/mod.rs
2. All public re-exports gated on features in lib.rs
3. Feature-gated public items in server builder methods
4. Skip internal/private items -- they are not visible in docs anyway

**Priority targets (public-facing modules):**

| File | Module/Item | Feature | Has annotation? |
|------|------------|---------|-----------------|
| lib.rs | `pub mod composition` | composition | YES |
| lib.rs | `pub mod axum` | streamable-http | YES |
| lib.rs | `pub use tower_layers::*` | streamable-http | NO |
| lib.rs | `pub use pmcp_macros::*` | macros | NO |
| lib.rs | `pub use WebSocketTransport` | websocket | NO |
| lib.rs | `pub use HttpTransport` | http | NO |
| server/mod.rs | `pub mod axum_router` | streamable-http | YES |
| server/mod.rs | `pub mod tower_layers` | streamable-http | YES |
| server/mod.rs | `pub mod mcp_apps` | mcp-apps | YES |
| server/mod.rs | `pub mod schema_utils` | schema-generation | NO |
| server/mod.rs | `pub mod streamable_http_server` | streamable-http | NO |
| server/mod.rs | `pub mod resource_watcher` | resource-watcher | NO |
| shared/mod.rs | `pub mod websocket` | websocket | NO |
| shared/mod.rs | `pub mod http` | http | NO |
| shared/mod.rs | `pub mod sse_optimized` | sse | NO |
| shared/mod.rs | `pub mod streamable_http` | streamable-http | NO |
| types/mod.rs | `pub mod mcp_apps` | mcp-apps | YES |

**Missing count:** ~10 public modules/re-exports need annotation.

### Component 3: lib.rs Documentation Strategy

**Current state (PMCP):** Inline `//!` doc comments with Quick Start examples (Client + Server). 62 lines of doc comments. Does NOT use `include_str!("../README.md")`.

**rmcp's approach:** `#![doc = include_str!("../README.md")]` -- README.md serves as both the GitHub/crates.io landing page AND the docs.rs crate-level documentation. The README uses `<div class="rustdoc-hidden">` to hide GitHub-specific content (badges) from rustdoc.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Inline `//!` (PMCP current) | Separate control, can include compilable doctests | Drifts from README, duplicated content |
| `include_str!` (rmcp) | Single source of truth, always in sync | Markdown only (no compilable doctests in lib.rs), need `rustdoc-hidden` CSS hack |
| Hybrid | Best of both worlds | Two places to maintain |

**Recommendation:** Keep inline `//!` doc comments in lib.rs but rewrite them to be comprehensive. Do NOT adopt the `include_str!("../README.md")` pattern because:
1. PMCP's README contains deployment/CLI/book content that is irrelevant to crate-level docs
2. The doctests in lib.rs serve as compile-time validation of the API
3. The `rustdoc-hidden` CSS hack is fragile

Instead, the lib.rs doc comments should be expanded to include:
- Feature flag table (matching README)
- Transport overview table
- Quick start examples (already present but need accuracy check)
- Links to key modules

### Component 4: examples/README.md

**Current state (PMCP):** The file contains the Spin framework README -- entirely wrong content. It is 100% bogus.

**rmcp's approach:** A focused README with:
1. Quick Start with Claude Desktop (build command + config JSON)
2. Links to sub-directories: `clients/README.md`, `servers/README.md`
3. Transport examples section
4. Integration examples section
5. WASI section
6. MCP Inspector usage

**Recommendation:** Complete rewrite of `examples/README.md`. Structure:

```
# PMCP Examples

## Quick Start
[Build + run the simplest example]

## Example Index

### Getting Started (01-12)
| # | Name | Transport | Features | Description |
|---|------|-----------|----------|-------------|

### Transport Examples (13-24)
| # | Name | Transport | Features | Description |
|---|------|-----------|----------|-------------|

### Advanced Patterns (27-37)
| # | Name | Transport | Features | Description |
|---|------|-----------|----------|-------------|

### Workflow & Integration (49-64)
| # | Name | Transport | Features | Description |
|---|------|-----------|----------|-------------|

### MCP Apps
[Links to standalone examples in subdirectories]

## Running Examples
[cargo run --example NAME -- flags]

## Testing with MCP Inspector
```

Key difference from rmcp: PMCP examples are flat-file numbered, not sub-crate based. The README must provide the organizational structure that the file naming alone does not.

**Content audit needed:** Some examples exist on disk but are not in Cargo.toml `[[example]]` entries (e.g., `08_server_resources.rs`, `11_progress_countdown.rs`, `12_prompt_workflow_progress.rs`, `32_simd_parsing_performance.rs`, `40_middleware_demo.rs`, `47_multiple_clients_parallel.rs`, `48_structured_output_schema.rs`, `54_hybrid_workflow_execution.rs`, `58_oauth_transport_to_tools.rs`, `59_dynamic_resource_workflow.rs`, `60_resource_only_steps.rs`, `61_observability_middleware.rs`, `client.rs`, `currency_server.rs`). These are orphaned examples that should either be registered in Cargo.toml or removed.

### Component 5: pmcp-macros README and Documentation

**Current state (PMCP):**
- README.md documents `#[tool]` and `#[tool_router]` (deprecated since 0.3.0)
- Does NOT document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]`
- lib.rs crate docs mention `#[tool]`, `#[tool_router]`, `#[prompt]`, `#[resource]` in the feature list
- lib.rs does have comprehensive doc comments on each proc macro
- README.md does NOT use `include_str!` pattern
- Version references say `pmcp = { version = "1.1" }` (stale -- current is 2.2.0)

**rmcp's approach:**
- README.md uses `rustdoc-hidden` for badges, `include_str!("../README.md")` in lib.rs
- README.md has a clean macro table linking to docs.rs for each macro
- README.md provides Quick Example showing the most concise usage pattern
- lib.rs doc comments on each macro are comprehensive with tables

**Recommendation:**
1. Rewrite pmcp-macros README.md to document current macros (`#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]`)
2. Mark deprecated macros (`#[tool]`, `#[tool_router]`, `#[prompt]`, `#[resource]`) as legacy
3. Update version references to 2.2.0
4. Update lib.rs crate-level doc comment feature list to lead with current macros
5. Consider `include_str!("../README.md")` for the macro crate (simpler crate, README is focused)

### Component 6: Feature Flag Documentation Strategy

**Two approaches observed:**

**rmcp's approach:** Feature flag table in README.md (which becomes crate docs via `include_str!`). Divided into three categories: core features, transport features, TLS backend options. Clean tables with description and default indicator.

**PMCP's current state:** Feature flags are listed in Cargo.toml but not documented in any user-facing location. No feature table in README, no feature table in lib.rs docs.

**Recommendation:** Add a feature flag table to lib.rs doc comments. Structure:

```rust
//! ## Feature Flags
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `logging` | Enable tracing-subscriber for structured logging | Yes |
//! | `macros` | Proc macros: `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` | |
//! | `schema-generation` | JSON Schema generation via `schemars` | |
//!
//! ### Transport Features
//! | Feature | Description |
//! |---------|-------------|
//! | `websocket` | WebSocket transport (tokio-tungstenite) |
//! | `http` | HTTP transport (hyper) |
//! | `streamable-http` | Streamable HTTP with axum/tower |
//! | `sse` | Server-Sent Events transport |
//!
//! ### Extension Features
//! | Feature | Description |
//! |---------|-------------|
//! | `mcp-apps` | MCP Apps interactive UI (ChatGPT Apps, MCP-UI) |
//! | `composition` | Server composition / proxy |
//! | `jwt-auth` | JWT authentication support |
//! | `oauth` | OAuth2 client helper |
//! | `validation` | JSON Schema + garde validation |
//! | `resource-watcher` | File system resource watching |
```

## Integration Points

### New Components (to create)

| Component | Type | Location | Purpose |
|-----------|------|----------|---------|
| Feature flag table | Doc comments | `src/lib.rs` | Centralized feature documentation |
| Examples README | Markdown | `examples/README.md` | Accurate example index |

### Modified Components (existing)

| Component | Change Type | Location | What Changes |
|-----------|------------|----------|--------------|
| docs.rs metadata | Config | `Cargo.toml` | `all-features = true` -> explicit list |
| lib.rs preamble | Doc comments | `src/lib.rs` | Add feature flag table, improve module docs |
| Feature gate annotations | Attributes | Multiple `mod.rs` files | Add ~10 `cfg_attr(docsrs, ...)` annotations |
| pmcp-macros README | Rewrite | `pmcp-macros/README.md` | Document current macros, deprecate old |
| pmcp-macros lib.rs | Doc comments | `pmcp-macros/src/lib.rs` | Update feature list in crate-level docs |

### Data Flow

Documentation build pipeline:
```
Cargo.toml [package.metadata.docs.rs]
  |
  v
rustdoc (with --cfg docsrs + explicit feature list)
  |
  v
lib.rs #![cfg_attr(docsrs, feature(doc_cfg))]
  |                                |
  v                                v
Inline //! doc comments      Per-item #[cfg_attr(docsrs, doc(cfg(...)))]
(feature table, examples)    (shows "Available on feature X only" badges)
  |                                |
  v                                v
              docs.rs rendered output
```

## Build Order for Documentation Changes

These changes have dependencies. The correct build order:

### Phase 1: Foundation (no dependencies)
**Can be done in parallel:**

1. **Cargo.toml docs.rs metadata** -- Replace `all-features = true` with explicit feature list. This is the single highest-impact change: it fixes the docs.rs build configuration that everything else depends on.

2. **Feature gate annotations** -- Add `cfg_attr(docsrs, doc(cfg(...)))` to ~10 public modules/re-exports. These are mechanical changes across `src/lib.rs`, `src/server/mod.rs`, `src/shared/mod.rs`.

**Rationale:** These two changes fix the rendering pipeline. Without them, the feature badges will not appear on docs.rs regardless of what documentation text says.

### Phase 2: Content (depends on Phase 1)

3. **lib.rs documentation rewrite** -- Add feature flag table, transport overview, improve module-level docs. Must come after Phase 1 because the feature table should match the explicit feature list in Cargo.toml.

4. **pmcp-macros README rewrite** -- Document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]`. Independent of lib.rs changes but logically part of the same documentation pass.

5. **pmcp-macros lib.rs crate doc update** -- Update feature list to lead with current macros. Should follow the README rewrite to stay consistent.

### Phase 3: Examples (depends on Phase 2)

6. **examples/README.md rewrite** -- Complete rewrite with accurate example index. Must come last because:
   - Need to audit which examples are registered in Cargo.toml vs orphaned on disk
   - Feature requirements per example should match the feature table from Phase 2
   - May discover examples that need updates/removal during audit

7. **Orphaned example cleanup** -- Register or remove examples that exist on disk but are not in Cargo.toml `[[example]]` entries.

### Dependency Graph

```
Phase 1 (Foundation):
  [Cargo.toml docs.rs] ----+
  [Feature annotations] ----+
                            |
                            v
Phase 2 (Content):
  [lib.rs docs] ------------+
  [macros README] ----------+
  [macros lib.rs docs] -----+
                            |
                            v
Phase 3 (Examples):
  [examples/README.md] -----+
  [Orphaned example cleanup]-+
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: `all-features = true` for docs.rs
**What:** Enabling every feature flag for documentation builds.
**Why bad:** Includes test scaffolding, platform-conflicting features (wasm + native), and example-only features in docs. Users see items as unconditionally available when they are actually feature-gated. Also risks build failures if mutually exclusive features are combined.
**Instead:** Explicit feature list in `[package.metadata.docs.rs]`.

### Anti-Pattern 2: README-as-crate-docs for complex crates
**What:** Using `#![doc = include_str!("../README.md")]` when the README contains non-API content.
**Why bad:** PMCP's README includes deployment guides, CLI usage, book references -- none of which belong in crate-level API docs. Forces the `rustdoc-hidden` CSS hack for every non-docs section.
**Instead:** Keep inline `//!` doc comments focused on API usage. Reserve `include_str!` for leaf crates with focused READMEs (like pmcp-macros).

### Anti-Pattern 3: Annotating only some feature-gated items
**What:** Adding `cfg_attr(docsrs, ...)` to a few items but not others.
**Why bad:** Inconsistent UX on docs.rs -- some items show "Available on feature X" badges, others silently appear/disappear. Users cannot trust the badges.
**Instead:** Systematic annotation of ALL public feature-gated modules and re-exports.

### Anti-Pattern 4: Documenting deprecated macros prominently
**What:** README leading with `#[tool]` and `#[tool_router]` when they are deprecated since 0.3.0.
**Why bad:** New users adopt the deprecated API, then hit deprecation warnings. Creates confusion about which macros to use.
**Instead:** Lead with current macros, add a "Legacy Macros" section at the bottom.

## Orphaned Example Audit

Examples on disk NOT in Cargo.toml `[[example]]` entries:

| File | Status | Action |
|------|--------|--------|
| `08_server_resources.rs` | Duplicate of `04_server_resources.rs`? | Investigate, likely remove |
| `11_progress_countdown.rs` | Not registered | Register or remove |
| `12_prompt_workflow_progress.rs` | Not registered | Register or remove |
| `32_simd_parsing_performance.rs` | Not registered, needs `simd` feature | Register with `required-features` or remove |
| `40_middleware_demo.rs` | Not registered | Register or remove |
| `47_multiple_clients_parallel.rs` | Not registered | Register or remove |
| `48_structured_output_schema.rs` | Not registered | Register or remove |
| `54_hybrid_workflow_execution.rs` | Not registered | Register or remove |
| `58_oauth_transport_to_tools.rs` | Not registered | Register or remove |
| `59_dynamic_resource_workflow.rs` | Not registered | Register or remove |
| `60_resource_only_steps.rs` | Number conflicts with tasks examples | Renumber or remove |
| `61_observability_middleware.rs` | Not registered | Register or remove |
| `client.rs` | Unnumbered | Register or remove |
| `currency_server.rs` | Unnumbered | Register or remove |

This is 14 orphaned examples. Each needs a decision: register in Cargo.toml (with correct `required-features`) or remove.

## Sources

- Direct codebase comparison: PMCP at `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/`
- Direct codebase comparison: rmcp at `/Users/guy/Development/mcp/sdk/rust-sdk/`
- Rust docs.rs build system: `[package.metadata.docs.rs]` consumed by docs.rs to configure the rustdoc build
- Rust `doc_cfg` feature: nightly-only feature enabled by `#![cfg_attr(docsrs, feature(doc_cfg))]` that shows feature requirement badges
- Confidence: HIGH -- all findings based on direct source code inspection of both repos
