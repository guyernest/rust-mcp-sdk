# Features Research: rmcp Comparison Upgrades

**Domain:** Developer experience and documentation quality gap analysis
**Researched:** 2026-04-10
**Confidence:** HIGH (all claims verified by reading actual source files in both repos)

## Table Stakes

Features developers expect from a credible Rust MCP SDK. Missing = SDK feels amateur or abandoned.

| Feature | Why Expected | Complexity | PMCP Status | Notes |
|---------|-------------|------------|-------------|-------|
| Accurate examples/README.md | First thing devs check after crate docs | Low | BROKEN - contains Spin framework README | Critical credibility issue: the file at `examples/README.md` is literally the Spin WebAssembly framework README, not PMCP content |
| Consistent example numbering | Shows professional maintenance | Low | BROKEN - duplicates (08, 11, 12, 32), gaps (21, 25-26, 38-46, 62) | Numbered examples have collisions and missing ranges, suggesting organic growth without cleanup |
| Protocol version accuracy in README | Badge/table must match actual code | Low | DRIFTED - badge says `2025-03-26`, code says `2025-11-25` | The `MCP Compatible` badge, the v2.0 release notes, and compatibility table all reference `2025-03-26` while `lib.rs` exports `LATEST_PROTOCOL_VERSION = "2025-11-25"` |
| Macros README matches actual API | Devs read macros README to learn patterns | Med | DRIFTED - lists `#[tool]`/`#[tool_router]` as primary, but code deprecates them in favor of `#[mcp_tool]`/`#[mcp_server]`/`#[mcp_prompt]` | README says "Prompts and resources coming soon" but `#[mcp_prompt]` and `#[mcp_resource]` already ship; "Future Plans" section is stale |
| Feature flag documentation | Users need to know what to enable | Med | MISSING - no feature flag table in README or lib.rs | PMCP has 20+ features but no tabular reference; rmcp has clean tables in crate README |
| Working code examples in README | Copy-paste must compile | Low | PARTIAL - `Server::builder()` in lib.rs doesn't match actual `ServerBuilder::new()` API | The lib.rs doctest uses `Server::builder()` which is not the actual API surface |
| Crate README separate from repo README | docs.rs shows crate README | Low | WEAK - uses repo README as crate README | 682-line marketing README on docs.rs drowns API docs with emoji, Toyota Way content, AI agent setup |

## Differentiators

Features that would set PMCP apart if well-executed. Not expected, but valued.

| Feature | Value Proposition | Complexity | PMCP Status | Notes |
|---------|-------------------|------------|-------------|-------|
| Per-capability code examples in README | rmcp has full working server/client code for each MCP capability (tools, resources, prompts, sampling, roots, logging, completions, subscriptions, notifications) inline in README | High | MISSING - PMCP README has one weather server example, everything else is link-out | rmcp's README is 991 lines of mostly runnable code covering every MCP capability; PMCP's is 682 lines mostly ecosystem marketing |
| Spec links per capability | rmcp links to exact MCP spec section for each capability | Low | MISSING | rmcp links `https://modelcontextprotocol.io/specification/2025-11-25/server/tools` etc. for every section |
| `doc(cfg)` annotations on feature-gated items | Shows users which feature enables what on docs.rs | Med | PARTIAL - 7 annotations across 3 files | PMCP has some `cfg_attr(docsrs, doc(cfg(...)))` but only for `composition`, `streamable-http`, and `mcp-apps`; many feature-gated re-exports in lib.rs lack annotations (e.g., `websocket`, `http`, `schema-generation`, `validation`) |
| Explicit docs.rs feature list vs `all-features = true` | rmcp lists exactly which features to enable on docs.rs (22 specific features); PMCP uses `all-features = true` | Low | SUBOPTIMAL | `all-features = true` pulls in everything including `simd`, `unstable`, `rayon`, `test-helpers`, example features -- polluting docs.rs with internal/unstable APIs |
| Migration guide for breaking changes | rmcp links migration guide prominently for 1.x | Low | MISSING from README | rmcp has `> Migrating to 1.x?` callout at the top; PMCP v2.0 was a breaking change but no migration guide linked in README |
| Community showcase ("Built with") | rmcp lists 15+ projects built with it | Low | MISSING | Credibility signal: real projects using the SDK |
| Transport table with type links | rmcp has clean 2x2 transport matrix (client/server x stdio/HTTP) with hyperlinked types | Low | MISSING | PMCP lists transports as bullet points without linking to actual types |
| Separate crate README for docs.rs | rmcp's `crates/rmcp/README.md` is 66 lines: feature flag tables, transport matrix, license. Clean API-focused content. The repo root README is the marketing document. | Med | MISSING | PMCP's `Cargo.toml` uses `readme = "README.md"` (the 682-line repo README) as the crate README |

## Anti-Features

Features to explicitly NOT build during this upgrade.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Copying rmcp's trait-based architecture docs | Different SDK architecture; copying structure would be misleading | Document PMCP's builder pattern + standalone function approach authentically |
| Adding "Related Projects" with fake entries | Padding the community section with unverified projects damages trust | Only add real projects using PMCP when they exist |
| Removing the book/course/AI-agent ecosystem from README | These are genuine PMCP differentiators rmcp lacks | Keep but move to a focused "Ecosystem" section, separate from the crate-level API docs |
| Duplicating rmcp's per-capability README sections | Would make README 2000+ lines | Instead, write focused crate-level docs in a separate `crates-README.md` and link to examples per capability |
| Over-annotating with `doc(cfg)` on internal modules | Noise for users who only see pub API | Only annotate public re-exports and public module declarations |

## Feature Dependencies

```
Fix examples/README.md ----> Example numbering cleanup
                              |
Protocol version fix -------> README content audit (badge, table, release notes section)
                              |
Macros README rewrite ------> Crate-level docs rewrite (lib.rs doc comments reference macros)
                              |
Feature flag table ----------> docs.rs configuration fix (move from all-features to explicit list)
                              |
Separate crate README ------> docs.rs presentation (what users see first on docs.rs)
                              |
doc(cfg) annotations -------> Feature flag table (users need both: table tells what exists, annotation tells where)
```

## Detailed Comparison Evidence

### 1. README Quality

**rmcp README (991 lines):**
- Clean header with 4 badges (crates.io, docs.rs, CI, license)
- Immediately links to crate README and macros README
- Table of contents linking to every MCP capability
- Each capability gets: spec link, server-side code, client-side code, notification handling, example link
- Code examples are near-complete and reference actual example files
- Related projects section with 15+ real community projects
- Development section with contributor tips link

**PMCP README (682 lines):**
- 12+ badges including duplicate quality gate badges (one failing, one passing)
- Marketing-heavy: "16x faster", "Toyota Way", emoji-heavy sections
- Three "paths" for getting started (AI-assisted, cargo-pmcp, SDK directly)
- Only one actual code example (WeatherArgs server)
- Large ecosystem section about book, course, AI agents, MCP Apps
- Performance benchmarks table (good differentiator, keep)
- No per-capability documentation

**Verdict:** rmcp is developer-reference oriented; PMCP is marketing-oriented. For a crate README (what docs.rs shows), developer-reference wins.

### 2. Examples Organization

**rmcp:**
- Organized in subdirectories: `servers/`, `clients/`, `transport/`, `simple-chat-client/`, `wasi/`
- Each directory has its own README with description per example
- Examples named descriptively: `counter_stdio.rs`, `sampling_stdio.rs`
- Common code extracted to `common/` module
- Clean separation of concerns

**PMCP:**
- All examples in flat directory with numbered prefix: `01_client_initialize.rs` through `64_mcp_prompt_macro.rs`
- Duplicate numbers: `08_logging.rs` + `08_server_resources.rs`, `11_progress_countdown.rs` + `11_request_cancellation.rs`, `12_error_handling.rs` + `12_prompt_workflow_progress.rs`, `32_simd_parsing_performance.rs` + `32_typed_tools.rs`
- Gaps: 21 (disabled), 25-26 (subdirectories), 38-39, 41-46, 62
- Unnumbered examples mixed in: `client.rs`, `server.rs`, `currency_server.rs`, `hotel_gallery.rs`, `conference_venue_map.rs`, `refactored_server_example.rs`
- `examples/README.md` is the Spin framework README (completely wrong content)
- 10+ examples not listed in `Cargo.toml` `[[example]]` sections

**Verdict:** PMCP numbering scheme is good in principle but has degraded through organic growth. The wrong README is a critical bug.

### 3. Macro Documentation

**rmcp-macros:**
- README (81 lines): Clean table of all 7 macros with docs.rs links
- Two quick examples showing the two patterns (server_handler shortcut vs separate handler)
- Links to full docs.rs documentation for detailed usage
- Macros cover: `#[tool]`, `#[tool_router]`, `#[tool_handler]`, `#[prompt]`, `#[prompt_router]`, `#[prompt_handler]`, `#[task_handler]`

**pmcp-macros:**
- README (252 lines): Documents `#[tool]` and `#[tool_router]` extensively
- Says "Currently only supports tools (prompts and resources coming soon)" -- FALSE, `#[mcp_prompt]` and `#[mcp_resource]` already ship
- "Future Plans" lists features that already exist (`#[prompt]` macro, `#[resource]` macro)
- Does not mention `#[mcp_tool]`, `#[mcp_server]`, or `#[mcp_prompt]` (the current recommended API)
- The deprecated `#[tool]` is documented as the primary API
- `lib.rs` has good inline documentation for `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` but the README doesn't reflect this

**Verdict:** pmcp-macros README is severely stale. The actual macro API (visible in lib.rs) is well-documented but the README tells a completely different story. rmcp-macros README is accurate and concise.

### 4. Feature Flag Documentation

**rmcp (Cargo.toml + crate README):**
- Crate README has two clean tables: core features (6 entries with default column) and transport features (6 entries)
- Additional TLS backend options table (3 entries)
- Cargo.toml `[package.metadata.docs.rs]` lists 22 specific features
- Features are logically named: `transport-io`, `transport-child-process`, `transport-streamable-http-client`, etc.

**PMCP (Cargo.toml):**
- 20+ features with no documentation table anywhere
- Feature names inconsistent: some use hyphens (`streamable-http`), some use hyphens differently (`http-client`), some are single words (`websocket`, `validation`, `logging`)
- `full` feature pulls in everything including `rayon` and `composition`
- `all-features = true` in docs.rs metadata exposes internal features (`test-helpers`, `unstable`, `simd`, `authentication_example`, `cancellation_example`, `progress_example`)
- No default feature discussion (default is just `logging`)

**Verdict:** rmcp's feature documentation is a model of clarity. PMCP has no feature documentation at all, and the docs.rs configuration exposes internal implementation details.

### 5. docs.rs Presentation

**rmcp:**
- `package.metadata.docs.rs` explicitly lists 22 features to build docs with
- Excludes internal/dev features
- `rustdoc-args = ["--cfg", "docsrs"]` enables conditional feature badges
- `lib.rs` uses `include_str!("../README.md")` -- shows the focused crate README (66 lines)
- `cfg_attr(docsrs, feature(doc_cfg))` + `cfg_attr(docsrs, allow(unused_attributes))`

**PMCP:**
- `all-features = true` -- builds docs with every feature including `test-helpers`, `unstable`, `simd`, example features
- `rustdoc-args = ["--cfg", "docsrs"]` is set
- `lib.rs` has inline doc comments (not include_str) -- shows ~60 lines of crate docs
- Only 7 `cfg_attr(docsrs, doc(cfg(...)))` annotations across the codebase
- Many feature-gated items in `lib.rs` re-exports lack `doc(cfg)` annotations:
  - `websocket` items: no annotation
  - `http` items: no annotation
  - `schema-generation` items: no annotation
  - `macros` items: no annotation
  - `wasm` items: no annotation

**Verdict:** PMCP's docs.rs will show internal test helpers and unstable APIs alongside production APIs. Users cannot tell which features enable which types.

### 6. Crate-Level Documentation (lib.rs)

**rmcp lib.rs:**
- `include_str!("../README.md")` -- delegates to the focused 66-line crate README
- Clean re-exports with `cfg` guards
- Minimal, lets the README do the talking

**PMCP lib.rs:**
- 357 lines of inline documentation
- Good doctest examples for `ToolResult`, protocol versions, timeout constants
- Missing: feature flag overview, "getting started" that matches actual API
- The `Server::builder()` example in the doc comments does not match the actual `ServerBuilder::new()` API
- The `Client::new(transport)` example doesn't match the actual `ClientBuilder` pattern
- Extensive re-exports section is good for discoverability

**Verdict:** PMCP's lib.rs doctests have API drift. rmcp delegates cleanly. PMCP's extensive re-exports are a genuine DX advantage that rmcp lacks.

## MVP Recommendation

Prioritize these in order:

1. **Fix examples/README.md** - Replace Spin content with actual PMCP example index. Low complexity, critical credibility fix.
2. **Fix protocol version drift** - Update badge, compatibility table, and v2.0 release notes to `2025-11-25`. Low complexity, accuracy fix.
3. **Rewrite pmcp-macros README** - Document `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, `#[mcp_resource]` as primary APIs. Remove stale "Future Plans". Medium complexity.
4. **Add feature flag table** - Either in README or in a focused crate-level README. Document all 20+ features with defaults and descriptions. Medium complexity.
5. **Fix docs.rs configuration** - Replace `all-features = true` with explicit feature list excluding internal features. Low complexity.
6. **Add `doc(cfg)` annotations** - Annotate all feature-gated public re-exports in `lib.rs`. Medium complexity.
7. **Clean up example numbering** - Resolve duplicates, fill or acknowledge gaps, remove disabled files. Medium complexity.
8. **Fix lib.rs doctest examples** - Ensure `Server::builder()` and `Client::new()` examples match actual API. Low complexity.

Defer:
- **Separate crate README** - Medium complexity, dependent on README content audit first. Can ship later.
- **Per-capability code examples** - High complexity, not blocking. Book/course fill this role for PMCP.
- **Community showcase** - Cannot fabricate; add when real projects exist.

## Sources

All findings verified by direct file reads:
- rmcp README: `/Users/guy/Development/mcp/sdk/rust-sdk/README.md` (991 lines)
- rmcp crate README: `/Users/guy/Development/mcp/sdk/rust-sdk/crates/rmcp/README.md` (66 lines)
- rmcp-macros README: `/Users/guy/Development/mcp/sdk/rust-sdk/crates/rmcp-macros/README.md` (81 lines)
- rmcp lib.rs: `/Users/guy/Development/mcp/sdk/rust-sdk/crates/rmcp/src/lib.rs` (43 lines)
- rmcp Cargo.toml: `/Users/guy/Development/mcp/sdk/rust-sdk/crates/rmcp/Cargo.toml` (342 lines, 22 explicit docs.rs features)
- rmcp examples README: `/Users/guy/Development/mcp/sdk/rust-sdk/examples/README.md` (83 lines)
- rmcp servers README: `/Users/guy/Development/mcp/sdk/rust-sdk/examples/servers/README.md` (146 lines)
- rmcp clients README: `/Users/guy/Development/mcp/sdk/rust-sdk/examples/clients/README.md` (107 lines)
- PMCP README: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/README.md` (682 lines)
- PMCP lib.rs: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/lib.rs` (357 lines)
- PMCP Cargo.toml: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml` (`all-features = true`, 20+ features)
- PMCP examples/README.md: contains Spin framework content (confirmed via `head -5`)
- PMCP pmcp-macros README: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/pmcp-macros/README.md` (252 lines, stale)
- PMCP pmcp-macros lib.rs: `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/pmcp-macros/src/lib.rs` (accurate inline docs for actual macros)
