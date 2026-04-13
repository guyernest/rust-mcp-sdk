# Stack Research: rmcp Documentation & DX Upgrades (v2.1)

**Domain:** Documentation quality, docs.rs presentation, README accuracy, macro documentation
**Researched:** 2026-04-10
**Confidence:** HIGH (all findings verified against actual codebase files in both repos)
**Mode:** Subsequent milestone -- only documentation/DX gaps benchmarked against rmcp

## Executive Assessment

PMCP has a **documentation presentation gap, not a documentation quantity gap**. PMCP actually has MORE documentation content than rmcp (682-line README, 252-line macros README, extensive rustdoc comments), but rmcp presents its documentation more effectively through three specific patterns that PMCP should adopt:

1. **Crate-level README as docs.rs landing page** via `include_str!`
2. **`doc_auto_cfg`** for automatic feature-flag badges on every gated item
3. **Focused crate READMEs** that work well in both GitHub and docs.rs contexts

No new crates or dependencies are needed. This is entirely configuration and content restructuring.

## Gap Analysis: PMCP vs rmcp

### 1. docs.rs Feature Coverage Annotations

**rmcp approach:**
```toml
# crates/rmcp/Cargo.toml
[package.metadata.docs.rs]
features = ["auth", "client", "macros", "server", "transport-io", ...]  # explicit list of 22 features
rustdoc-args = ["--cfg", "docsrs"]
```
```rust
// lib.rs
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
```
rmcp explicitly lists 22 features in `[package.metadata.docs.rs]` but does NOT use `#[doc(cfg(...))]` on individual items. It relies on `feature(doc_cfg)` which on nightly (docs.rs uses nightly) enables automatic `doc_auto_cfg` behavior -- items behind `#[cfg(feature = "...")]` get feature badges automatically.

**PMCP approach:**
```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```
```rust
#![cfg_attr(docsrs, feature(doc_cfg))]
```
PMCP uses `all-features = true` and has 6 manual `#[cfg_attr(docsrs, doc(cfg(...)))]` annotations across 145 feature-gated items.

**Gap:** PMCP's 6/145 coverage means 139 feature-gated items show NO feature badge on docs.rs. Users cannot tell which items require which feature flags.

**Fix:** Add `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` to `src/lib.rs`. This nightly-only feature (available on docs.rs since docs.rs uses nightly) automatically generates `doc(cfg(...))` for ALL `#[cfg(feature = "...")]` items. Zero manual annotation needed. The existing 6 manual annotations can be removed.

```rust
// src/lib.rs - add this line:
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
```

**Why not explicit feature list like rmcp?** PMCP has more features (20+ in the `full` flag alone). `all-features = true` is simpler and correct. rmcp's explicit list is actually worse -- it requires manual maintenance when features are added.

**Confidence:** HIGH -- verified both Cargo.toml files, counted actual annotations.

### 2. Crate-Level README as docs.rs Landing Page

**rmcp approach:**
```rust
// crates/rmcp/src/lib.rs
#![doc = include_str!("../README.md")]
```
rmcp has a focused 66-line crate README (`crates/rmcp/README.md`) that:
- Uses `<style>.rustdoc-hidden { display: none; }</style>` to hide GitHub badges from docs.rs
- Documents feature flags in a clean table
- Documents transport options
- Links to the main repo README for full docs

The MAIN repo README (991 lines) is separate and contains the full usage guide with tools, resources, prompts, sampling, roots, logging, completions, subscriptions, and examples.

**rmcp-macros approach:**
```rust
// crates/rmcp-macros/src/lib.rs
#![doc = include_str!("../README.md")]
```
81-line focused README with macro table, quick example, and link to main README.

**PMCP approach:**
- `src/lib.rs` has inline `//! # MCP SDK for Rust` doc comments (61 lines of module docs)
- Does NOT use `include_str!`
- The main README (682 lines) is specified as `readme = "README.md"` in Cargo.toml but is NOT rendered as the docs.rs landing page
- `pmcp-macros/src/lib.rs` has inline `//!` doc comments (53 lines)

**Gap:** PMCP's docs.rs landing page is the 61-line inline doc comment, not the README. The README content (feature flags, transports, quick start) is not visible on docs.rs. Meanwhile, rmcp users see feature flag tables and transport docs immediately on the docs.rs landing page.

**Fix:** Create a focused `DOCS_RS.md` or rename approach -- but the simplest and best approach is:

1. Create a concise crate-level README file (like rmcp's 66 lines) specifically for the docs.rs landing page
2. Use `#![doc = include_str!("../crate-doc.md")]` in lib.rs
3. Keep the main README.md for GitHub (repository landing page)

OR (simpler, recommended):

1. Add feature flag table and transport summary to the existing inline docs in lib.rs
2. Expand the `//!` module doc from 61 lines to ~100-120 lines with feature flags table

**Recommendation:** Use the `include_str!` approach with a focused crate doc file. This keeps documentation in markdown (easier to edit) and automatically stays in sync. The file should contain:
- Feature flags table (like rmcp)
- Transport summary
- Quick start code snippet
- Link to full README and docs

**Confidence:** HIGH -- verified both lib.rs files and README content.

### 3. Macro Documentation: README vs Inline Docs

**rmcp-macros approach:**
- 81-line README with `<style>.rustdoc-hidden { display: none; }</style>` for GitHub/docs.rs dual rendering
- Concise table of all 7 macros with docs.rs links
- Two quick examples (simple + advanced)
- Uses `#![doc = include_str!("../README.md")]` so this IS the docs.rs landing
- Each macro has detailed `///` doc comments in lib.rs (30-50 lines each with tables and examples)

**pmcp-macros approach:**
- 252-line README with detailed examples but several inaccuracies:
  - States "Currently only supports tools (prompts and resources coming soon)" -- but `#[mcp_prompt]` and `#[mcp_resource]` already ship
  - Version reference `pmcp = { version = "1.1" }` is outdated (current: 2.2.0)
  - Does not document `#[mcp_tool]`, `#[mcp_prompt]`, `#[mcp_resource]`, `#[mcp_server]` macros
  - Only documents the deprecated `#[tool]` and `#[tool_router]` macros
- Does NOT use `include_str!` -- lib.rs has its own 53-line inline docs
- Each macro has detailed `///` doc comments in lib.rs (good coverage, 10-40 lines each)

**Gap:** The pmcp-macros README is fundamentally stale. It documents deprecated macros and claims features are missing that actually exist. The inline lib.rs docs are accurate but the README (what users see on crates.io and GitHub) is misleading.

**Fix:**
1. Rewrite `pmcp-macros/README.md` to match rmcp's pattern:
   - Table of ALL current macros: `#[mcp_tool]`, `#[mcp_prompt]`, `#[mcp_resource]`, `#[mcp_server]`, plus legacy `#[tool]`, `#[tool_router]`
   - Use `rustdoc-hidden` CSS trick for GitHub-only badges
   - Quick example using CURRENT macros (not deprecated ones)
   - Link to main README for full docs
2. Add `#![doc = include_str!("../README.md")]` to pmcp-macros/src/lib.rs
3. Remove inline `//!` module docs from lib.rs (README replaces them)

**Confidence:** HIGH -- verified README content against actual lib.rs macro exports.

### 4. Example Indexing Patterns

**rmcp approach:**
- Examples organized into subdirectories: `examples/servers/`, `examples/clients/`, `examples/transport/`, `examples/wasi/`
- Each subdirectory has its own README with:
  - Per-example descriptions (3-5 lines each)
  - Run commands
  - Dependencies list
  - Common module documentation
- Top-level `examples/README.md` is a quick-start guide, not an index

**PMCP approach:**
- All 60+ examples in flat `examples/` directory with numeric prefixes: `01_`, `02_`, etc.
- `examples/README.md` exists but was not verified as current in this research
- Examples registered in `Cargo.toml` with `[[example]]` entries and `required-features`
- Some standalone examples in subdirectories: `examples/mcp-apps-chess/`, `examples/mcp-apps-map/`, etc.

**Gap:** PMCP's flat directory works fine for discoverability (numeric prefixes are good). The gap is README accuracy -- the README must list every example with:
- Correct name
- What it demonstrates
- Required features
- Run command

**Fix:** Rewrite `examples/README.md` with:
- Category groupings (Core, Transport, Security, Workflows, MCP Apps, Macros)
- Per-example: name, one-line description, required features, run command
- Generate from `Cargo.toml` `[[example]]` entries if desired (but manual is fine for 60 examples)

**Confidence:** HIGH -- verified actual example files against Cargo.toml entries.

### 5. Crate-Level Doc Attributes

**rmcp lib.rs attributes:**
```rust
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![doc = include_str!("../README.md")]
```

**PMCP lib.rs attributes:**
```rust
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::result_large_err)]
```

**Gap:** PMCP actually has STRICTER lint attributes than rmcp (good). The only missing piece is `doc_auto_cfg` (covered in item 1 above) and `include_str!` (covered in item 2).

**Fix:** Add `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` -- that is the only attribute change needed.

## Recommended Stack Changes

### Configuration Changes (Zero Dependencies)

| Change | File | What | Why |
|--------|------|------|-----|
| Add `doc_auto_cfg` | `src/lib.rs` | `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` | Auto-generates feature badges for all 145 gated items on docs.rs |
| Add `include_str!` | `src/lib.rs` | `#![doc = include_str!("../crate-doc.md")]` | Makes docs.rs landing page useful with feature tables |
| Add `include_str!` | `pmcp-macros/src/lib.rs` | `#![doc = include_str!("../README.md")]` | Makes macros docs.rs page show the README |
| Remove manual `doc(cfg)` | 6 locations in `src/` | Delete `#[cfg_attr(docsrs, doc(cfg(...)))]` | Redundant once `doc_auto_cfg` is enabled |

### New Files (Zero Dependencies)

| File | Purpose | Size |
|------|---------|------|
| `crate-doc.md` | Focused docs.rs landing page (feature tables, transports, quick start) | ~80-100 lines |

### File Rewrites (Zero Dependencies)

| File | What Changes | Why |
|------|-------------|-----|
| `pmcp-macros/README.md` | Complete rewrite: document current macros, drop deprecated-only coverage | README is stale, documents only deprecated macros |
| `examples/README.md` | Comprehensive example index with categories, features, run commands | Current index may not match actual examples |

### Makefile/CI Integration

The existing `make doc` target already does the right thing:
```makefile
doc:
    RUSTDOCFLAGS="--cfg docsrs" $(CARGO) doc --all-features --no-deps
```

Add a verification target:
```makefile
doc-check:
    RUSTDOCFLAGS="--cfg docsrs -D warnings" $(CARGO) doc --all-features --no-deps
```

This catches broken doc links and missing docs before they reach docs.rs.

### Local Testing Command

```bash
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features --no-deps --open
```

This previews exactly what docs.rs will render, including feature badges.

## What NOT to Add

| Temptation | Why Avoid |
|-----------|-----------|
| `document-features` crate | Adds a build dependency just to extract Cargo.toml comments. Manual feature table in crate-doc.md is simpler and more flexible. |
| README generation tooling (cargo-readme, etc.) | Over-engineering. PMCP has 2 READMEs to maintain. Manual is fine. |
| Automated example index generation | The `[[example]]` entries in Cargo.toml are the source of truth. A script adds fragile tooling for a one-time rewrite task. |
| Sphinx/mdBook for API docs | docs.rs + rustdoc is the standard. PMCP already has mdBook for book/course. |
| Per-feature documentation pages | Feature badges on items are sufficient. Per-feature guide pages are the book/course's job. |
| Copying rmcp's explicit feature list in docs.rs metadata | PMCP's `all-features = true` is correct and lower-maintenance. rmcp's explicit list requires updates on every feature addition. |

## Implementation Priority

1. **`doc_auto_cfg` attribute** -- Single line change, massive docs.rs improvement (139 items gain badges)
2. **pmcp-macros README rewrite** -- Highest user-facing impact, current README actively misleads
3. **`include_str!` for crate-doc.md** -- Makes docs.rs landing page useful
4. **examples/README.md rewrite** -- Ensures example discoverability
5. **Remove manual `doc(cfg)` annotations** -- Cleanup after `doc_auto_cfg` does it automatically
6. **`make doc-check` target** -- CI enforcement

## Version Matrix

| Component | Current Version | Required Change |
|-----------|----------------|-----------------|
| pmcp | 2.2.0 | lib.rs attributes only |
| pmcp-macros | 0.4.1 | README rewrite + lib.rs `include_str!` |
| Rust (local) | 1.83+ | No change needed |
| docs.rs | nightly | Already compatible |
| Makefile | existing | Add `doc-check` target |

## Sources

All findings verified against actual files in:
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/` (PMCP codebase)
- `/Users/guy/Development/mcp/sdk/rust-sdk/` (rmcp codebase)

Key files examined:
- rmcp: `crates/rmcp/Cargo.toml` (docs.rs metadata, 22 explicit features)
- rmcp: `crates/rmcp/src/lib.rs` (3 crate attributes + `include_str!`)
- rmcp: `crates/rmcp/README.md` (66-line focused crate doc)
- rmcp: `crates/rmcp-macros/README.md` (81-line macro table + examples)
- rmcp: `crates/rmcp-macros/src/lib.rs` (`include_str!` + detailed `///` docs)
- pmcp: `Cargo.toml` (docs.rs metadata with `all-features = true`)
- pmcp: `src/lib.rs` (145 feature gates, 6 doc(cfg) annotations)
- pmcp: `pmcp-macros/README.md` (252 lines, stale content)
- pmcp: `pmcp-macros/src/lib.rs` (inline docs, no `include_str!`)
- rmcp docs.rs: https://docs.rs/rmcp
- Rust doc_auto_cfg: https://doc.rust-lang.org/stable/unstable-book/language-features/doc-auto-cfg.html
- docs.rs metadata: https://docs.rs/about/metadata
