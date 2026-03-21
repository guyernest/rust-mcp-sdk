# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- ✅ **v1.2 Pluggable Storage Backends** — Phases 9-13 (shipped 2026-02-24)
- ✅ **v1.3 MCP Apps Developer Experience** — Phases 14-19 (shipped 2026-02-26)
- ✅ **v1.4 Book & Course Update** — Phases 20-24 (shipped 2026-02-28)
- ✅ **v1.5 Cloud Load Testing Upload** — Phases 25-26 (shipped 2026-03-01)
- **v1.6 CLI DX Overhaul** — Phases 27-32 (in progress)
- ✅ **v1.7 SDK Maturation** — Phases 52-53 (shipped 2026-03-20)
- **v2.0 Protocol Modernization** — Phases 54-59 (in progress)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.2 Pluggable Storage Backends (Phases 9-13) — SHIPPED 2026-02-24</summary>

- [x] Phase 9: Storage Abstraction Layer (2/2 plans) — completed 2026-02-24
- [x] Phase 10: InMemory Backend Refactor (2/2 plans) — completed 2026-02-24
- [x] Phase 11: DynamoDB Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 12: Redis Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 13: Feature Flag Verification (1/1 plans) — completed 2026-02-24

See: `.planning/milestones/v1.2-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.3 MCP Apps Developer Experience (Phases 14-19) — SHIPPED 2026-02-26</summary>

- [x] Phase 14: Preview Bridge Infrastructure (2/2 plans) — completed 2026-02-24
- [x] Phase 15: WASM Widget Bridge (2/2 plans) — completed 2026-02-25
- [x] Phase 16: Shared Bridge Library (2/2 plans) — completed 2026-02-26
- [x] Phase 17: Widget Authoring DX and Scaffolding (2/2 plans) — completed 2026-02-26
- [x] Phase 18: Publishing Pipeline (2/2 plans) — completed 2026-02-26
- [x] Phase 19: Ship Examples and Playwright E2E (2/2 plans) — completed 2026-02-26

See: `.planning/milestones/v1.3-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.4 Book & Course Update (Phases 20-24) — SHIPPED 2026-02-28</summary>

- [x] Phase 20: Book Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 21: Book MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 22: Course Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 23: Course MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 24: Course Quizzes & Exercises (2/2 plans) — completed 2026-02-28

See: `.planning/milestones/v1.4-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.5 Cloud Load Testing Upload (Phases 25-26) — SHIPPED 2026-03-01</summary>

- [x] Phase 25: Loadtest Config Upload (2/2 plans) — completed 2026-02-28
- [x] Phase 26: Add OAuth Support to Load Testing (4/4 plans) — completed 2026-03-01

See phase details in `.planning/phases/25-*` and `.planning/phases/26-*`

</details>

### v1.6 CLI DX Overhaul (In Progress)

**Milestone Goal:** Normalize the cargo pmcp CLI for consistency and developer experience ahead of course recording -- fix flag inconsistencies, propagate auth to all server-facing commands, surface mcp-tester via `cargo pmcp test`, and add doctor/completions commands.

- [x] **Phase 27: Global Flag Infrastructure** - Add --no-color and --quiet as global flags available on all commands (completed 2026-03-04)
- [x] **Phase 28: Flag Normalization** - Rename and normalize all per-command flags for consistency (positional URL, --server, --verbose, --yes, -o, --format, #[arg()]) (completed 2026-03-12)
- [x] **Phase 29: Auth Flag Propagation** - Add shared OAuth and API-key flag structs to all server-facing commands (completed 2026-03-13)
- [ ] **Phase 30: Tester CLI Integration** - Surface mcp-tester subcommands through cargo pmcp test with aligned flags
- [ ] **Phase 31: New Commands** - Add cargo pmcp doctor and cargo pmcp completions commands
- [ ] **Phase 32: Help Text Polish** - Consistent help text format with descriptions and usage examples across all commands

### v1.7 SDK Maturation (Complete)

**Milestone Goal:** Reduce dependency footprint and produce gap analysis against TypeScript SDK v2.

- [x] **Phase 52: Reduce transitive dependencies** - Feature-gate reqwest and tracing-subscriber, slim tokio/hyper/chrono (completed 2026-03-18)
- [x] **Phase 53: Review TypeScript SDK Updates** - Gap analysis comparing TypeScript v2 against Rust SDK (completed 2026-03-20)

### v2.0 Protocol Modernization (In Progress)

**Milestone Goal:** Upgrade to MCP protocol 2025-11-25 with massive type cleanup, add Tasks with polling, Tower middleware with DNS rebinding protection, and conformance testing. Focus on streamable HTTP and stateless calls. SSE, elicitations, and notifications are de-prioritized — Tasks with status polling is the primary async pattern. This is a semver major bump enabling breaking changes for a cleaner API surface.

- [x] **Phase 54: Protocol Version 2025-11-25 + Type Cleanup** - Add all 2025-11-25 types (TaskSchema, IconSchema, AudioContent, ResourceLink), expanded capabilities, version negotiation for latest 3 versions. Breaking change: clean up legacy type aliases and deprecated fields. (completed 2026-03-20)
- [x] **Phase 54.1: Protocol Type Construction DX** - Default impls, builders, and constructors for all protocol types. Fix inconsistent construction patterns that break downstream on every upgrade. (INSERTED) (completed 2026-03-20)
- [x] **Phase 55: Tasks with Polling** - Task capability negotiation, TaskStore trait, in-memory + DynamoDB backends, task status polling via streamable HTTP. No SSE-based notifications — polling is the pattern. (completed 2026-03-21)
- [x] **Phase 56: Tower Middleware + DNS Rebinding Protection** - Tower Layer for MCP protocol concerns (host validation, DNS rebinding protection, session management, JSON-RPC routing). Axum convenience adapter. Enterprise security focus.
- [ ] **Phase 57: Conformance Test Suite** - mcp-tester conformance command with core protocol, tools, resources, prompts, and tasks scenarios. Validates any MCP server against the spec.
- [ ] **Phase 58: #[mcp_tool] Proc Macro** - Eliminate Box::pin(async move {}) boilerplate on every tool definition. Expand pmcp-macros crate with #[mcp_tool] attribute that accepts async fn directly, handles Arc state injection, and auto-derives input/output schema. Also addresses the foundation Arc cloning ceremony.
- [ ] **Phase 59: TypedPrompt with Auto-Deserialization** - Typed prompt equivalent of TypedToolWithOutput. Prompt arguments deserialize from HashMap<String, String> into a typed struct automatically via JsonSchema + serde, matching the tool DX pattern.

## Phase Details

### Phase 27: Global Flag Infrastructure
**Goal**: Every cargo pmcp invocation supports --no-color and --quiet for scripting and CI use
**Depends on**: Phase 26 (v1.5 complete)
**Requirements**: FLAG-08, FLAG-09
**Success Criteria** (what must be TRUE):
  1. User can pass `--no-color` to any cargo pmcp command and all terminal output is plain text (no ANSI escape codes)
  2. User can pass `--quiet` to any cargo pmcp command and only errors and explicit requested output appear
  3. Both flags work when placed before or after the subcommand (global position)
**Plans**: 2 plans
Plans:
- [ ] 27-01-PLAN.md — GlobalFlags struct, --no-color/--quiet CLI args, wire through all command dispatch, global color suppression
- [ ] 27-02-PLAN.md — Quiet mode output filtering across all commands, verbose-wins-over-quiet precedence

### Phase 28: Flag Normalization
**Goal**: Every existing cargo pmcp command uses the same conventions for URLs, server references, verbosity, confirmations, output, and format values
**Depends on**: Phase 27
**Requirements**: FLAG-01, FLAG-02, FLAG-03, FLAG-04, FLAG-05, FLAG-06, FLAG-07
**Success Criteria** (what must be TRUE):
  1. User can pass a server URL as a positional argument to any command that connects to a server (no more `--url` or `--endpoint`)
  2. User can use `--server` consistently for pmcp.run server references (no more `--server-id`)
  3. User can use `--verbose` / `-v` for detailed output on any command (no more `--detailed`)
  4. User can use `--yes` to skip confirmations and `-o` as shorthand for `--output` on any command that supports them
  5. All `--format` flags accept `text` and `json` as values (no other human-readable format names)
**Plans**: 3 plans

Plans:
- [ ] 28-01-PLAN.md — Create shared flag structs (FormatValue, OutputFlags, FormatFlags), convert deploy #[clap()] to #[arg()], clean up dead code
- [ ] 28-02-PLAN.md — Normalize test/schema/preview/connect/validate/deploy flags: URL positional, verbose removal, format normalization
- [ ] 28-03-PLAN.md — Normalize app/secret/loadtest/landing flags: URL positional, --force to --yes, -o alias, --server-id to --server

### Phase 29: Auth Flag Propagation
**Goal**: Every command that connects to an MCP server accepts OAuth and API-key authentication flags
**Depends on**: Phase 28
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, AUTH-06
**Success Criteria** (what must be TRUE):
  1. User can pass `--api-key <key>` to test check/run/generate, preview, schema export, and connect commands
  2. User can pass OAuth flags (--oauth-issuer, --oauth-client-id, --oauth-scopes, --oauth-no-cache, --oauth-redirect-port) to any of those same commands
  3. Auth flags are defined in a shared struct (AuthFlags or similar) flattened into each command, not duplicated per command
  4. Commands that already had auth support (e.g., loadtest) continue to work unchanged
**Plans**: 3 plans

Plans:
- [ ] 29-01-PLAN.md — Define AuthFlags struct, AuthMethod enum, resolve() method in flags.rs; create shared auth.rs with resolve_auth_middleware()
- [ ] 29-02-PLAN.md — Flatten AuthFlags into test check/run/generate/apps, wire handlers; migrate loadtest inline auth to shared AuthFlags
- [ ] 29-03-PLAN.md — Add AuthFlags to preview/schema export/connect; extend McpProxy with auth_header; wire connect config generation

### Phase 30: Tester CLI Integration
**Goal**: Users can run all mcp-tester capabilities through cargo pmcp test subcommands with consistent flag conventions
**Depends on**: Phase 29
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-07, TEST-08
**Success Criteria** (what must be TRUE):
  1. User can run `cargo pmcp test compliance <url>`, `cargo pmcp test diagnose <url>`, and `cargo pmcp test compare <url1> <url2>` to validate MCP servers
  2. User can run `cargo pmcp test tools <url>`, `cargo pmcp test resources <url>`, `cargo pmcp test prompts <url>`, and `cargo pmcp test health <url>` to inspect server capabilities
  3. All `cargo pmcp test` subcommands accept the same auth flags (--api-key, OAuth) and global flags (--verbose, --no-color, --quiet) established in prior phases
  4. The standalone `mcp-tester` binary uses the same flag conventions as `cargo pmcp test` (positional URL, --verbose/-v, --yes)
**Plans**: TBD

### Phase 31: New Commands
**Goal**: Users have workspace diagnostics and shell completion generation built into the CLI
**Depends on**: Phase 28
**Requirements**: CMD-01, CMD-02
**Success Criteria** (what must be TRUE):
  1. User can run `cargo pmcp doctor` and see validation results for workspace structure, Rust toolchain, config files, and optionally server connectivity
  2. User can run `cargo pmcp completions bash` (or zsh/fish/powershell) and pipe the output to the appropriate shell config file
  3. Both commands follow all established flag conventions (global flags, --format, help text patterns)
**Plans**: TBD

### Phase 32: Help Text Polish
**Goal**: Every cargo pmcp command has professional, consistent help output ready for course recording
**Depends on**: Phase 31
**Requirements**: HELP-01, HELP-02
**Success Criteria** (what must be TRUE):
  1. Every command's `--help` output includes a description, grouped options (by category: connection, auth, output, etc.), and a usage examples section via `after_help`
  2. All help text follows the same structural pattern: synopsis line, categorized options, examples section
  3. Running `cargo pmcp --help` shows a clean top-level overview with all subcommands and their one-line descriptions
**Plans**: TBD

## Progress

**Execution Order:** Phase 27 next

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Handoff + Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction | v1.2 | 2/2 | Complete | 2026-02-24 |
| 10. InMemory Refactor | v1.2 | 2/2 | Complete | 2026-02-24 |
| 11. DynamoDB Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 12. Redis Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 13. Feature Flags | v1.2 | 1/1 | Complete | 2026-02-24 |
| 14. Preview Bridge | v1.3 | 2/2 | Complete | 2026-02-24 |
| 15. WASM Bridge | v1.3 | 2/2 | Complete | 2026-02-25 |
| 16. Shared Bridge Lib | v1.3 | 2/2 | Complete | 2026-02-26 |
| 17. Authoring DX | v1.3 | 2/2 | Complete | 2026-02-26 |
| 18. Publishing | v1.3 | 2/2 | Complete | 2026-02-26 |
| 19. Ship + E2E | v1.3 | 2/2 | Complete | 2026-02-26 |
| 20. Book Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 21. Book MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 22. Course Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 23. Course MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 24. Course Quizzes | v1.4 | 2/2 | Complete | 2026-02-28 |
| 25. Loadtest Upload | v1.5 | 2/2 | Complete | 2026-02-28 |
| 26. OAuth Load Testing | v1.5 | 4/4 | Complete | 2026-03-01 |
| 27. Global Flag Infrastructure | 3/3 | Complete   | 2026-03-04 | - |
| 28. Flag Normalization | 3/3 | Complete   | 2026-03-12 | - |
| 29. Auth Flag Propagation | 3/3 | Complete    | 2026-03-13 | - |
| 30. Tester CLI Integration | v1.6 | 0/? | Not started | - |
| 31. New Commands | v1.6 | 0/? | Not started | - |
| 32. Help Text Polish | v1.6 | 0/? | Not started | - |

### Phase 33: Fix mcp-tester failure with v1.12.0

**Goal:** Bump mcp-tester to 0.2.2 and cargo-pmcp to 0.3.4, publish both to crates.io so `cargo install cargo-pmcp` works without `--locked`
**Requirements**: None (hotfix)
**Depends on:** Phase 32
**Plans:** 3/3 plans complete

Plans:
- [ ] 33-01-PLAN.md — Version bumps and crates.io publish

### Phase 34: Fix MCP Apps ChatGPT compatibility

**Goal:** Fix SDK metadata format, MIME types, and mcp-preview routes to be compatible with ChatGPT's MCP Apps implementation
**Requirements**: CHATGPT-01, CHATGPT-02, CHATGPT-03, CHATGPT-04, CHATGPT-05, CHATGPT-06
**Depends on:** Phase 33
**Plans:** 2/2 plans complete

Plans:
- [x] 34-01-PLAN.md — Fix tool _meta format (nested ui.resourceUri + openai/outputTemplate), add MIME type variant, dual-emit WidgetMeta
- [ ] 34-02-PLAN.md — Fix mcp-preview axum 0.8 wildcard route panic

### Phase 35: Add meta key constants module for UI/MCP Apps strings

**Goal:** Align SDK types, bridge protocol, and scaffold template with ChatGPT's official MCP Apps protocol -- add _meta to Content::Resource, fix MIME type, update bridge method names, fix scaffold
**Requirements**: P41-01, P41-02, P41-03, P41-04, P41-05
**Depends on:** Phase 34
**Plans:** 4 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 35 to break down)

### Phase 36: Unify UIMimeType and ExtendedUIMimeType with From bridge

**Goal:** Add From/TryFrom conversion traits between UIMimeType and ExtendedUIMimeType so code can seamlessly convert across the feature-gate boundary
**Requirements**: MIME-BRIDGE-01
**Depends on:** Phase 35
**Plans:** 1/1 plans complete

Plans:
- [ ] 36-01-PLAN.md — TDD: From<UIMimeType> for ExtendedUIMimeType and TryFrom<ExtendedUIMimeType> for UIMimeType

### Phase 37: Add with_ui support to TypedSyncTool

**Goal:** Add with_ui() builder method to TypedSyncTool and WasmTypedTool for API parity with TypedTool, enabling sync and WASM tool authors to declare UI resource associations
**Requirements**: P37-01, P37-02, P37-03, P37-04
**Depends on:** Phase 36
**Plans:** 1/1 plans complete

Plans:
- [ ] 37-01-PLAN.md — Add ui_resource_uri field, with_ui() builder, and _meta emission to TypedSyncTool and WasmTypedTool

### Phase 38: Cache ToolInfo at registration to avoid per-request cloning

**Goal:** Cache ToolInfo and PromptInfo at builder registration time so handle_list_tools, handle_call_tool, handle_list_prompts, and task routing use cached metadata instead of calling handler.metadata() per request
**Requirements**: CACHE-01
**Depends on:** Phase 37
**Plans:** 1/1 plans complete

Plans:
- [ ] 38-01-PLAN.md — Add tool_infos/prompt_infos cache to builders, replace 6 per-request metadata() call sites with cache lookups

### Phase 39: Add deep-merge for ui meta key to prevent collision

**Goal:** Add deep_merge function for serde_json::Map and update all metadata() implementations to merge _meta instead of replacing, preventing data loss when multiple builder methods contribute to _meta. Also add with_ui() to TypedToolWithOutput and with_meta_entry() to ToolInfo.
**Requirements**: MERGE-01, MERGE-02
**Depends on:** Phase 38
**Plans:** 2/2 plans complete

Plans:
- [x] 39-01-PLAN.md — Add deep_merge function in ui.rs and ToolInfo::with_meta_entry builder method
- [ ] 39-02-PLAN.md — Update TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool metadata() to use deep_merge; add with_ui() to TypedToolWithOutput

### Phase 40: Review ChatGPT Compatibility for Apps

**Goal:** Align SDK metadata emission with official ext-apps spec: add legacy flat key ui/resourceUri to build_meta_map, dual-emit nested ui.csp/ui.domain in WidgetMeta, add ui.visibility array format, and add ModelOnly visibility variant
**Requirements**: COMPAT-01, COMPAT-02, COMPAT-03, COMPAT-04
**Depends on:** Phase 39
**Plans:** 2/2 plans complete

Plans:
- [ ] 40-01-PLAN.md — Add legacy flat key ui/resourceUri to build_meta_map() for ext-apps backward compat
- [ ] 40-02-PLAN.md — Dual-emit nested ui.csp/ui.domain in WidgetMeta, add ModelOnly to ToolVisibility, emit ui.visibility array in ChatGptToolMeta

### Phase 41: ChatGPT MCP Apps Upgraded Version

**Goal:** Align SDK types, bridge protocol, and scaffold template with ChatGPT's official MCP Apps protocol -- add _meta to Content::Resource, fix MIME type, update bridge method names, fix scaffold
**Requirements**: P41-01, P41-02, P41-03, P41-04, P41-05
**Depends on:** Phase 40
**Plans:** 3/3 plans complete

Plans:
- [ ] 41-01-PLAN.md — Add _meta to Content::Resource, fix ChatGptAdapter MIME type to HtmlMcpApp
- [ ] 41-02-PLAN.md — Update bridge protocol method names in widget-runtime.mjs and index.html
- [ ] 41-03-PLAN.md — Update scaffold template with correct MIME type, with_ui(), and resource _meta

### Phase 42: Add outputSchema top level support

**Goal:** Migrate output_schema from ToolAnnotations to a top-level field on ToolInfo, aligning with MCP spec 2025-06-18. Clean break -- remove from annotations, keep pmcp:outputTypeName as codegen extension.
**Requirements**: OS-01, OS-02, OS-03, OS-04, OS-05, OS-06
**Depends on:** Phase 41
**Plans:** 2/2 plans complete

Plans:
- [x] 42-01-PLAN.md — Core types migration: ToolAnnotations cleanup, ToolInfo field + builder, TypedToolWithOutput rewire, macro codegen
- [ ] 42-02-PLAN.md — Consumers: cargo-pmcp schema structs, tests, example, docs update

### Phase 43: ChatGPT MCP Apps alignment

**Goal:** Fix 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets -- add _meta to ResourceInfo, filter tools/call _meta to invocation keys only, merge descriptor keys into resources/read _meta, and build URI-to-tool-meta index for auto-propagation
**Requirements**: None (hotfix-style phase)
**Depends on:** Phase 42
**Plans:** 2/2 plans complete

Plans:
- [ ] 43-01-PLAN.md — Add _meta field to ResourceInfo, filter with_widget_enrichment to openai/toolInvocation/*, build URI-to-tool-meta index on ServerCore, update all struct literals
- [ ] 43-02-PLAN.md — Post-process handle_list_resources and handle_read_resource to propagate tool _meta to resource responses

### Phase 44: Improving mcp-preview to support ChatGPT version

**Goal:** Add --mode chatgpt flag to mcp-preview enabling strict ChatGPT protocol validation, postMessage emulation with window.openai stub, and a Protocol diagnostics tab in DevTools
**Requirements**: P44-MODE, P44-CONFIG, P44-RESOURCEMETA, P44-PROTOCOL-TAB, P44-CHATGPT-EMULATION, P44-BADGE
**Depends on:** Phase 43
**Plans:** 2/2 plans complete

Plans:
- [x] 44-01-PLAN.md — Rust-side mode plumbing: PreviewMode enum, CLI --mode flag, ConfigResponse with keys, ResourceInfo _meta, banner
- [x] 44-02-PLAN.md — Browser-side Protocol tab, ChatGPT postMessage emulation, window.openai stub, mode badge

### Phase 45: Extend MCP Apps Support to Claude Desktop

**Goal:** Refactor SDK metadata emission to standard-only default with opt-in host layers, normalize widget-runtime bridge with extensions namespace, and update mcp-preview standard mode -- enabling Claude Desktop and all standard MCP Apps hosts to work without ChatGPT-specific keys
**Requirements**: P45-STANDARD-DEFAULT, P45-HOST-LAYER, P45-URI-INDEX, P45-BRIDGE-NORMALIZE, P45-EXTENSIONS-NS, P45-PREVIEW-STANDARD, P45-EXAMPLES-VERIFY
**Depends on:** Phase 44
**Plans:** 3/3 plans complete

Plans:
- [ ] 45-01-PLAN.md — Refactor metadata emission to standard-only default + host layer enrichment pipeline on ServerCoreBuilder
- [ ] 45-02-PLAN.md — Normalize widget-runtime bridge with extensions namespace for ChatGPT-specific APIs
- [ ] 45-03-PLAN.md — Update mcp-preview standard mode default + verify examples render in both modes

### Phase 46: MCP Bridge Review and Fixes

**Goal:** Fix the mcpBridge data delivery pipeline so widgets receive structuredContent from tool responses across all MCP hosts, add method name normalization for cross-host compatibility, replace fragile setTimeout delivery with readiness signals, and add Bridge diagnostics tab to mcp-preview
**Requirements**: BRIDGE-01, BRIDGE-02, BRIDGE-03, BRIDGE-04, BRIDGE-05, BRIDGE-06, BRIDGE-07, BRIDGE-08
**Depends on:** Phase 45
**Success Criteria** (what must be TRUE):
  1. Widgets receive tool result data regardless of whether the host sends short-form (ui/toolResult) or long-form (ui/notifications/tool-result) method names
  2. McpApps adapter bridge provides onToolResult callback API on mcpBridge
  3. mcp-preview waits for widget readiness signal before delivering tool results (no setTimeout)
  4. Bridge diagnostics tab in mcp-preview shows PostMessage traffic log, handshake trace, and current mode
**Plans:** 2/3 plans executed

Plans:
- [ ] 46-01-PLAN.md — Fix bridge protocol method name mismatch in adapter.rs and App class normalization
- [ ] 46-02-PLAN.md — Fix mcp-preview tool result delivery with readiness signal and dual method emission
- [ ] 46-03-PLAN.md — Add Bridge diagnostics tab to mcp-preview and verify complete fix with real widget

### Phase 47: Add MCP App support to mcp-tester

**Goal:** Add MCP App protocol metadata validation to mcp-tester and cargo pmcp test, enabling CLI-based App compliance checks (metadata-only, no browser) with standard and host-specific modes
**Requirements**: APP-VAL-01, APP-VAL-02, APP-VAL-03, APP-VAL-04, APP-VAL-05
**Depends on:** Phase 46
**Success Criteria** (what must be TRUE):
  1. User can run `mcp-tester apps <url>` or `cargo pmcp test apps --url <url>` to validate App metadata on any MCP server
  2. Validation checks ui.resourceUri, MIME types, resource cross-references, and optionally ChatGPT-specific keys
  3. `cargo pmcp test check` shows hint when App-capable tools are detected
  4. --strict promotes warnings to failures, --tool filters to single tool, --mode selects host-specific checks
**Plans:** 2/2 plans complete

Plans:
- [ ] 47-01-PLAN.md -- AppValidator module, TestCategory::Apps, mcp-tester apps subcommand
- [ ] 47-02-PLAN.md -- cargo pmcp test apps subcommand, check command App hint

### Phase 48: MCP Apps Documentation and Education Refresh

**Goal:** Update all documentation, tooling READMEs, book chapters, and course materials to reflect the current MCP Apps capabilities including multi-host support (ChatGPT, Claude Desktop), mcp-tester apps validation, mcp-preview improvements, and the developer guide. Also fix mcp-preview theme support by sending CSS variable palettes in host context.
**Requirements**: DOCS-01, DOCS-02, DOCS-03, DOCS-04, PREVIEW-01
**Depends on:** Phase 47
**Success Criteria** (what must be TRUE):
  1. mcp-tester README documents the `apps` subcommand with usage examples and validation modes
  2. mcp-preview README describes current capabilities including multi-host preview, widget runtime, and DevTools
  3. pmcp-book MCP Apps chapters are updated with current tooling, host layer system, and developer guide content
  4. pmcp-course materials are aligned with book updates
  5. mcp-preview sends `styles.variables` CSS custom properties in host context so widgets respond to theme changes
**Plans:** 3/3 plans complete

Plans:
- [ ] 48-01-PLAN.md — Update mcp-tester/mcp-preview READMEs and rewrite book ch12-5 MCP Apps chapter with GUIDE.md content
- [ ] 48-02-PLAN.md — Update pmcp-course ch20 MCP Apps chapters and ch11-02 mcp-tester lesson to align with book
- [ ] 48-03-PLAN.md — Add theme CSS variable palettes to mcp-preview host context for ext-apps widget theming

### Phase 49: Bump dependencies (reqwest 0.13, jsonschema 0.45)

**Goal:** Upgrade reqwest from 0.12 to 0.13 and jsonschema from 0.38 to 0.45 across the workspace, updating feature flags, MSRV, deprecated methods, and template strings
**Requirements**: DEP-01
**Depends on:** Phase 48
**Success Criteria** (what must be TRUE):
  1. All four workspace Cargo.toml files reference reqwest 0.13 with correct feature names (rustls, form)
  2. jsonschema bumped to 0.45 with MSRV raised to 1.83.0
  3. Template strings in deploy/scaffold generate correct reqwest 0.13 lines for new projects
  4. `make quality-gate` passes with zero warnings
**Plans:** 1/1 plans complete

Plans:
- [ ] 49-01-PLAN.md — Update all Cargo.toml files, MSRV, deprecated methods, and template strings for reqwest 0.13 + jsonschema 0.45

### Phase 50: Improve Binary Release

**Goal:** Fix the broken binary release auto-trigger, add Apple Silicon and Linux ARM64 targets, create installer scripts, add cargo-binstall metadata, and generate SHA256 checksums for mcp-tester and mcp-preview
**Requirements**: TRIGGER, ARM-MAC, ARM-LIN, CHECKSUMS, INSTALL-SH, INSTALL-PS1, BINSTALL
**Depends on:** Phase 49
**Success Criteria** (what must be TRUE):
  1. Pushing a v* tag triggers binary builds for both mcp-tester and mcp-preview automatically
  2. Release includes binaries for 5 targets: x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows
  3. Each binary has a corresponding SHA256 checksum file on the release
  4. Users can install binaries via curl|sh (Linux/macOS) or PowerShell (Windows)
  5. cargo binstall metadata is present in both crate Cargo.toml files
**Plans:** 2/2 plans complete

Plans:
- [ ] 50-01-PLAN.md — Convert binary workflows to reusable workflow_call, fix runner labels, add ARM64 targets, add SHA256 checksums
- [ ] 50-02-PLAN.md — Create install.sh and install.ps1 installer scripts, add cargo-binstall metadata to Cargo.toml files

### Phase 51: PMCP MCP Server

**Goal:** Build a developer tools MCP server (crates/pmcp-server/) that provides protocol testing, scaffolding, schema export, documentation resources, and guided workflow prompts over streamable HTTP -- deployed at pmcp.run and released as cross-platform binary
**Requirements**: None (new feature)
**Depends on:** Phase 50
**Success Criteria** (what must be TRUE):
  1. Server binary starts and serves 5 tools (test_check, test_generate, test_apps, scaffold, schema_export) over streamable HTTP
  2. Server provides 9 documentation resources via pmcp:// URIs with embedded markdown content
  3. Server provides 7 guided workflow prompts (quickstart, create-mcp-server, add-tool, diagnose, setup-auth, debug-protocol-error, migrate)
  4. All content is statically embedded in the binary via include_str! -- no runtime file dependencies
  5. Release workflow builds pmcp-server binaries for 5 platform targets and publishes to crates.io
**Plans:** 5/5 plans complete

Plans:
- [ ] 51-01-PLAN.md — Crate scaffold, workspace integration, server skeleton, ScenarioGenerator API addition
- [ ] 51-02-PLAN.md — Testing tools: test_check, test_generate, test_apps wrapping mcp-tester library
- [ ] 51-03-PLAN.md — Build tools: scaffold (code templates) and schema_export (schema discovery)
- [ ] 51-04-PLAN.md — Embedded content, documentation resources handler, workflow prompt handlers
- [ ] 51-05-PLAN.md — Wire all tools/resources/prompts into server builder, CI workflow updates

### Phase 52: Reduce transitive dependencies

**Goal:** Reduce pmcp crate's transitive dependency count from ~249 to ~150-185 by removing unused deps, slimming feature flags, making reqwest optional behind `http-client` feature, and making tracing-subscriber optional behind `logging` feature
**Requirements**: DEP-REDUCE-01, DEP-REDUCE-02, DEP-REDUCE-03, DEP-REDUCE-04, DEP-REDUCE-05, DEP-REDUCE-06, DEP-REDUCE-07
**Depends on:** Phase 51
**Plans:** 2 plans — Complete (2026-03-18)

Plans:
- [x] 52-01-PLAN.md — Cargo.toml: remove unused deps, slim features, make reqwest/tracing-subscriber optional
- [x] 52-02-PLAN.md — Source code: cfg gates for optional deps, full feature matrix verification

### Phase 53: Review TypeScript SDK Updates

**Goal:** Compare TypeScript MCP SDK v2 against Rust SDK v1.20.0 to identify gaps worth adopting. Produce gap analysis with prioritized recommendations covering protocol negotiation, conformance testing, MCP Apps, Tasks, and framework adapters.
**Requirements**: GAP-ANALYSIS
**Depends on:** Phase 52
**Plans:** 2/2 plans complete

Plans:
- [x] 53-01-PLAN.md — Deep verification of TypeScript vs Rust SDK source differences across 6 domains
- [x] 53-02-PLAN.md — Gap analysis report with prioritized recommendations and proposed implementation phases

### Phase 54: Protocol Version 2025-11-25 + Type Cleanup

**Goal:** Upgrade Rust SDK to MCP protocol 2025-11-25 with version negotiation (latest 3 versions). Add 20+ new types (TaskSchema, IconSchema, AudioContent, ResourceLink, expanded ServerCapabilities/ClientCapabilities). Clean up legacy type aliases and deprecated fields. Breaking change — part of the v2.0.0 semver bump.
**Requirements**: PROTO-2025-11-25, VERSION-NEGOTIATION, TYPE-CLEANUP
**Depends on:** Phase 53
**Plans:** 4/4 plans complete

Plans:
- [x] 54-01-PLAN.md — Module split (protocol.rs -> 7 domain sub-modules) + version negotiation update to 2025-11-25
- [ ] 54-02-PLAN.md — Add 33 new types (task, content, sampling, elicitation, capabilities) + fix IncludeContext, LogLevel bugs
- [ ] 54-03-PLAN.md — Fix internal src/ imports, remove 11 legacy type aliases
- [ ] 54-04-PLAN.md — Fix external imports (examples/tests/workspace), write MIGRATION.md

### Phase 54.1: Protocol Type Construction DX (INSERTED)

**Goal:** Add Default impls, builder methods, and constructors for all protocol types so downstream users can construct types without specifying every Optional field. Fix the inconsistency where some types have constructors, some don't, and enum variants have neither. Prevents painful migration breaks when new fields are added.
**Requirements**: PROTO-TYPE-DX
**Depends on:** Phase 54
**Plans:** 3/3 plans complete

Plans:
- [x] 54.1-01-PLAN.md — Add constructors/Default/#[non_exhaustive]/.with_*() to resources.rs, prompts.rs, content.rs (Content enum helpers)
- [x] 54.1-02-PLAN.md — Add constructors/Default/#[non_exhaustive]/.with_*() to protocol/mod.rs, tasks.rs, sampling.rs, notifications.rs, capabilities.rs, tools.rs
- [x] 54.1-03-PLAN.md — Migrate all external consumers (src/, tests/, examples/, workspace crates) to constructors, update MIGRATION.md

### Phase 55: Tasks with Polling

**Goal:** Reconcile SDK task types as canonical source, add TaskStore trait with InMemoryTaskStore to SDK, wire into server builder and request dispatch with ServerCapabilities.tasks capability negotiation. Polling-only async pattern -- no SSE notifications.
**Requirements**: TASKS-POLLING, TASK-STORE, TASK-CAPABILITIES
**Depends on:** Phase 54.1
**Success Criteria** (what must be TRUE):
  1. SDK TaskStatus has is_terminal() and can_transition_to() utility methods matching pmcp-tasks
  2. Task.ttl serializes as null (not omitted) when None, per MCP spec
  3. SDK defines TaskStore trait with create/get/list/cancel/update_status/cleanup_expired
  4. InMemoryTaskStore provides dev/test implementation with owner isolation, state machine, TTL
  5. Builder.task_store() registers Arc<dyn TaskStore> and auto-configures ServerCapabilities.tasks
  6. Server dispatches tasks/get, tasks/list, tasks/cancel through TaskStore
**Plans:** 3/3 plans executed

Plans:
- [x] 55-01-PLAN.md — SDK task type reconciliation: add utility methods, fix TTL serialization
- [x] 55-02-PLAN.md — TaskStore trait + InMemoryTaskStore in SDK core
- [x] 55-03-PLAN.md — Server builder integration, core dispatch, capability negotiation, re-exports

### Phase 56: Tower Middleware + DNS Rebinding Protection

**Goal:** Build a Tower Layer stack for MCP server hosting: DNS rebinding protection (Host + Origin header validation against allowed origins), security response headers, and origin-locked CORS. Axum convenience adapter (`pmcp::axum::router()`) for the 90% case. Enterprise security focus -- fix CVE-pattern wildcard CORS and achieve MCP spec 2025-03-26 Origin validation compliance.
**Requirements**: TOWER-MIDDLEWARE, DNS-REBINDING, AXUM-ADAPTER
**Depends on:** Phase 54
**Success Criteria** (what must be TRUE):
  1. DnsRebindingLayer validates Host header (always) and Origin header (when present), returns 403 on mismatch
  2. SecurityHeadersLayer adds X-Content-Type-Options: nosniff, X-Frame-Options: DENY, Cache-Control: no-store
  3. `pmcp::axum::router(server)` returns axum::Router with DNS rebinding + security headers + origin-locked CORS
  4. StreamableHttpServer no longer uses wildcard `Access-Control-Allow-Origin: *`
  5. Example 55 (ServerHttpMiddleware) still compiles unchanged
**Plans:** 3/3 plans complete

Plans:
- [x] 56-01-PLAN.md -- Tower deps, AllowedOrigins config, DnsRebindingLayer, SecurityHeadersLayer with unit tests (completed 2026-03-21)
- [x] 56-02-PLAN.md -- Axum router convenience function, StreamableHttpServer CORS fix, lib.rs re-exports (completed 2026-03-21)
- [ ] 56-03-PLAN.md -- Gap closure: apply Tower layers in StreamableHttpServer::start(), delete add_cors_headers, pre-resolve AllowedOrigins in ServerState

### Phase 57: Conformance Test Suite

**Goal:** Add `mcp-tester conformance <url>` command that validates any MCP server against the protocol spec. Core scenarios: initialize handshake, tools CRUD, resources CRUD, prompts CRUD, task lifecycle. Modeled after TypeScript SDK's @modelcontextprotocol/conformance infrastructure.
**Requirements**: CONFORMANCE-CLI, CONFORMANCE-SCENARIOS
**Depends on:** Phase 55
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 57 to break down)

### Phase 58: #[mcp_tool] Proc Macro

**Goal:** Expand pmcp-macros crate with `#[mcp_tool]` attribute macro that eliminates `Box::pin(async move {})` boilerplate on tool definitions. Accepts `async fn(input: T, extra: RequestHandlerExtra) -> Result<Output>` directly. Handles Arc state injection for composition scenarios (eliminates the foundation cloning ceremony). Auto-derives input/output JSON schema from types.
**Requirements**: TOOL-MACRO, STATE-INJECTION
**Depends on:** Phase 54
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 58 to break down)

### Phase 59: TypedPrompt with Auto-Deserialization

**Goal:** Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Prompt arguments deserialize from `HashMap<String, String>` into a typed struct via JsonSchema + serde, eliminating the manual `args.get("x").ok_or()?.parse()?` pattern on every prompt. Builder-friendly registration via `.prompt("name", TypedPrompt::new(handler))`.
**Requirements**: TYPED-PROMPT, PROMPT-SCHEMA
**Depends on:** Phase 54
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 59 to break down)
