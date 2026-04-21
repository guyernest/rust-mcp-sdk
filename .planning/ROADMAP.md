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
- **v2.1 rmcp Upgrades** — Phases 65-68 (in progress)

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

<details>
<summary>v1.6 CLI DX Overhaul (In Progress — paused for v2.0)</summary>

**Milestone Goal:** Normalize the cargo pmcp CLI for consistency and developer experience ahead of course recording -- fix flag inconsistencies, propagate auth to all server-facing commands, surface mcp-tester via `cargo pmcp test`, and add doctor/completions commands.

- [x] **Phase 27: Global Flag Infrastructure** - Add --no-color and --quiet as global flags available on all commands (completed 2026-03-04)
- [x] **Phase 28: Flag Normalization** - Rename and normalize all per-command flags for consistency (positional URL, --server, --verbose, --yes, -o, --format, #[arg()]) (completed 2026-03-12)
- [x] **Phase 29: Auth Flag Propagation** - Add shared OAuth and API-key flag structs to all server-facing commands (completed 2026-03-13)
- [ ] **Phase 30: Tester CLI Integration** - Surface mcp-tester subcommands through cargo pmcp test with aligned flags
- [ ] **Phase 31: New Commands** - Add cargo pmcp doctor and cargo pmcp completions commands
- [ ] **Phase 32: Help Text Polish** - Consistent help text format with descriptions and usage examples across all commands

See phase details in `.planning/phases/27-*` through `.planning/phases/32-*`

</details>

<details>
<summary>v1.7 SDK Maturation — SHIPPED 2026-03-20</summary>

**Milestone Goal:** Reduce dependency footprint and produce gap analysis against TypeScript SDK v2.

- [x] **Phase 52: Reduce transitive dependencies** - Feature-gate reqwest and tracing-subscriber, slim tokio/hyper/chrono (completed 2026-03-18)
- [x] **Phase 53: Review TypeScript SDK Updates** - Gap analysis comparing TypeScript v2 against Rust SDK (completed 2026-03-20)

See phase details in `.planning/phases/52-*` and `.planning/phases/53-*`

</details>

### v2.0 Protocol Modernization (In Progress)

**Milestone Goal:** Upgrade to MCP protocol 2025-11-25 with massive type cleanup, add Tasks with polling, Tower middleware with DNS rebinding protection, and conformance testing. Focus on streamable HTTP and stateless calls. SSE, elicitations, and notifications are de-prioritized — Tasks with status polling is the primary async pattern. This is a semver major bump enabling breaking changes for a cleaner API surface.

- [x] **Phase 54: Protocol Version 2025-11-25 + Type Cleanup** - Add all 2025-11-25 types (TaskSchema, IconSchema, AudioContent, ResourceLink), expanded capabilities, version negotiation for latest 3 versions. Breaking change: clean up legacy type aliases and deprecated fields. (completed 2026-03-20)
- [x] **Phase 54.1: Protocol Type Construction DX** - Default impls, builders, and constructors for all protocol types. Fix inconsistent construction patterns that break downstream on every upgrade. (INSERTED) (completed 2026-03-20)
- [x] **Phase 55: Tasks with Polling** - Task capability negotiation, TaskStore trait, in-memory + DynamoDB backends, task status polling via streamable HTTP. No SSE-based notifications — polling is the pattern. (completed 2026-03-21)
- [ ] **Phase 55.1: Fix MCP Tasks support** - Add execution/taskSupport to TypedTool API, wire task detection in ServerCore so standard task_store path returns CreateTaskResult instead of CallToolResult text. (INSERTED)
- [x] **Phase 56: Tower Middleware + DNS Rebinding Protection** - Tower Layer for MCP protocol concerns (host validation, DNS rebinding protection, session management, JSON-RPC routing). Axum convenience adapter. Enterprise security focus.
- [x] **Phase 57: Conformance Test Suite** - mcp-tester conformance command with core protocol, tools, resources, prompts, and tasks scenarios. Validates any MCP server against the spec. (completed 2026-03-21)
- [ ] **Phase 58: #[mcp_tool] Proc Macro** - Eliminate Box::pin(async move {}) boilerplate on every tool definition. Expand pmcp-macros crate with #[mcp_tool] attribute that accepts async fn directly, handles Arc state injection, and auto-derives input/output schema. Also addresses the foundation Arc cloning ceremony.
- [ ] **Phase 59: TypedPrompt with Auto-Deserialization** - Typed prompt equivalent of TypedToolWithOutput. Prompt arguments deserialize from HashMap<String, String> into a typed struct automatically via JsonSchema + serde, matching the tool DX pattern.

## Phase Details

<details>
<summary>Phases 27-53 (v1.6 + v1.7 — prior milestones)</summary>

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
**Plans:** 6 plans
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [x] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 31: New Commands
**Goal**: Users have workspace diagnostics and shell completion generation built into the CLI
**Depends on**: Phase 28
**Requirements**: CMD-01, CMD-02
**Success Criteria** (what must be TRUE):
  1. User can run `cargo pmcp doctor` and see validation results for workspace structure, Rust toolchain, config files, and optionally server connectivity
  2. User can run `cargo pmcp completions bash` (or zsh/fish/powershell) and pipe the output to the appropriate shell config file
  3. Both commands follow all established flag conventions (global flags, --format, help text patterns)
**Plans:** 6 plans
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [x] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 32: Help Text Polish
**Goal**: Every cargo pmcp command has professional, consistent help output ready for course recording
**Depends on**: Phase 31
**Requirements**: HELP-01, HELP-02
**Success Criteria** (what must be TRUE):
  1. Every command's `--help` output includes a description, grouped options (by category: connection, auth, output, etc.), and a usage examples section via `after_help`
  2. All help text follows the same structural pattern: synopsis line, categorized options, examples section
  3. Running `cargo pmcp --help` shows a clean top-level overview with all subcommands and their one-line descriptions
**Plans:** 6 plans
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

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
| 30. Tester CLI Integration | v1.6 | 0/? | Complete    | 2026-03-28 |
| 31. New Commands | v1.6 | 0/? | Complete    | 2026-03-28 |
| 32. Help Text Polish | v1.6 | 0/? | Complete    | 2026-03-28 |

### Phase 33: Fix mcp-tester compatibility failure

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
**Plans:** 3 plans (2 complete, 1 gap closure)

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
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 39-01-PLAN.md — Add deep_merge function in ui.rs and ToolInfo::with_meta_entry builder method
- [ ] 39-02-PLAN.md — Update TypedTool, TypedSyncTool, TypedToolWithOutput, WasmTypedTool metadata() to use deep_merge; add with_ui() to TypedToolWithOutput

### Phase 40: Review ChatGPT Compatibility for Apps

**Goal:** Align SDK metadata emission with official ext-apps spec: add legacy flat key ui/resourceUri to build_meta_map, dual-emit nested ui.csp/ui.domain in WidgetMeta, add ui.visibility array format, and add ModelOnly visibility variant
**Requirements**: COMPAT-01, COMPAT-02, COMPAT-03, COMPAT-04
**Depends on:** Phase 39
**Plans:** 3 plans (2 complete, 1 gap closure)

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
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 42-01-PLAN.md — Core types migration: ToolAnnotations cleanup, ToolInfo field + builder, TypedToolWithOutput rewire, macro codegen
- [ ] 42-02-PLAN.md — Consumers: cargo-pmcp schema structs, tests, example, docs update

### Phase 43: ChatGPT MCP Apps alignment

**Goal:** Fix 4 protocol gaps preventing ChatGPT from rendering MCP Apps widgets -- add _meta to ResourceInfo, filter tools/call _meta to invocation keys only, merge descriptor keys into resources/read _meta, and build URI-to-tool-meta index for auto-propagation
**Requirements**: None (hotfix-style phase)
**Depends on:** Phase 42
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [ ] 43-01-PLAN.md — Add _meta field to ResourceInfo, filter with_widget_enrichment to openai/toolInvocation/*, build URI-to-tool-meta index on ServerCore, update all struct literals
- [ ] 43-02-PLAN.md — Post-process handle_list_resources and handle_read_resource to propagate tool _meta to resource responses

### Phase 44: Improving mcp-preview to support ChatGPT version

**Goal:** Add --mode chatgpt flag to mcp-preview enabling strict ChatGPT protocol validation, postMessage emulation with window.openai stub, and a Protocol diagnostics tab in DevTools
**Requirements**: P44-MODE, P44-CONFIG, P44-RESOURCEMETA, P44-PROTOCOL-TAB, P44-CHATGPT-EMULATION, P44-BADGE
**Depends on:** Phase 43
**Plans:** 3 plans (2 complete, 1 gap closure)

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
**Plans:** 3 plans (2 complete, 1 gap closure)

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
**Plans:** 3 plans (2 complete, 1 gap closure)

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
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 53-01-PLAN.md — Deep verification of TypeScript vs Rust SDK source differences across 6 domains
- [x] 53-02-PLAN.md — Gap analysis report with prioritized recommendations and proposed implementation phases

</details>

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

### Phase 55.1: Fix MCP Tasks support (INSERTED)

**Goal:** Fix three SDK-side gaps that prevent the standard task_store path from returning proper CreateTaskResult wire format. Add execution/taskSupport to all TypedTool variants, wire task detection in ServerCore handle_call_tool, and add _meta with io.modelcontextprotocol/related-task to CreateTaskResult responses.
**Requirements**: D-01, D-02, D-03, D-04, D-05, D-06, D-07, D-08, D-09
**Depends on:** Phase 55
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 55.1-01-PLAN.md — Add execution field and with_execution() to all TypedTool variants
- [x] 55.1-02-PLAN.md — Wire task detection in core.rs, return CreateTaskResult with _meta

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
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 57-01-PLAN.md — Conformance module with ConformanceRunner orchestrator and 5 domain scenario groups (Core, Tools, Resources, Prompts, Tasks) (completed 2026-03-21)
- [x] 57-02-PLAN.md — CLI integration: replace Compliance with Conformance in mcp-tester, add cargo pmcp test conformance (completed 2026-03-21)

### Phase 58: #[mcp_tool] Proc Macro

**Goal:** Expand pmcp-macros crate with `#[mcp_tool]` attribute macro that eliminates `Box::pin(async move {})` boilerplate on tool definitions. Accepts `async fn(input: T, extra: RequestHandlerExtra) -> Result<Output>` directly. Handles Arc state injection for composition scenarios (eliminates the foundation cloning ceremony). Auto-derives input/output JSON schema from types.
**Requirements**: TOOL-MACRO, STATE-INJECTION
**Depends on:** Phase 54
**Plans:** 3/3 plans complete

Plans:
- [x] 58-01-PLAN.md — State<T> type, parameter classification, standalone #[mcp_tool] macro
- [x] 58-02-PLAN.md — #[mcp_server] impl-block macro with McpServer trait and builder extension
- [x] 58-03-PLAN.md — Integration tests, compile-fail tests, and example 63

### Phase 59: TypedPrompt with Auto-Deserialization

**Goal:** Add `TypedPrompt` analogous to `TypedToolWithOutput` for prompts. Prompt arguments deserialize from `HashMap<String, String>` into a typed struct via JsonSchema + serde, eliminating the manual `args.get("x").ok_or()?.parse()?` pattern on every prompt. Builder-friendly registration via `.prompt("name", TypedPrompt::new(handler))`.
**Requirements**: TYPED-PROMPT, PROMPT-SCHEMA
**Depends on:** Phase 54
**Plans:** 3 plans (2 complete, 1 gap closure)

Plans:
- [x] 59-01-PLAN.md — TypedPrompt runtime type and standalone #[mcp_prompt] attribute macro
- [x] 59-02-PLAN.md — #[mcp_server] prompt extension, integration tests, compile-fail tests, and example 64

### Phase 60: Clean up mcp-preview side tabs

**Goal:** Clean up the mcp-preview DevTools side panel: remove the Console tab, make the panel resizable and collapsible with a draggable left boundary and header toggle button, and add a global Clear All button.
**Requirements**: D-01, D-02, D-03, D-04, D-05, D-06, D-07, D-08, D-09, D-10, D-11, D-12, D-13
**Depends on:** Phase 59
**Plans:** 1/1 plans complete

Plans:
- [x] 60-01-PLAN.md — Remove Console tab, add resizable/collapsible panel with toggle button and global Clear All

### Phase 61: Add OAuth support to mcp-preview

**Goal:** Add browser-based OAuth PKCE authentication to mcp-preview so developers can test MCP Apps against OAuth-protected servers on pmcp.run, with dynamic auth header updates, login modal, and CLI flag wiring.
**Requirements**: TBD
**Depends on:** Phase 60
**Plans:** 3/3 plans complete

Plans:
- [x] 61-01-PLAN.md -- Server-side OAuth infrastructure (RwLock proxy, auth handlers, callback page, config exposure, 401/403 propagation)
- [x] 61-02-PLAN.md -- Browser-side OAuth popup flow (OAuthManager, PKCE, login modal) and CLI OAuth flag wiring
- [x] 61-03-PLAN.md -- Gap closure: fix forward_raw/forward_mcp 401/403 propagation for WASM bridge path

### Phase 62: mcp-pen-test

**Goal:** Add automated penetration testing for MCP server endpoints via `cargo pmcp pentest <url>` -- probes for prompt injection, tool poisoning, and session security vulnerabilities with severity classification, rate limiting, and SARIF output for CI integration.
**Requirements**: None (new feature, not tracked in REQUIREMENTS.md)
**Depends on:** Phase 61
**Plans:** 3/3 plans complete

Plans:
- [x] 62-01-PLAN.md -- Foundation: types, config, rate limiter, report (JSON/SARIF), discovery, payload library, CLI command skeleton
- [ ] 62-02-PLAN.md -- Prompt injection (PI-01..PI-07) and tool poisoning (TP-01..TP-06) attack runners
- [x] 62-03-PLAN.md -- Session security (SS-01..SS-06) attack runner and final integration verification

### Phase 63: advanced-pentest-attack-modules

**Goal:** Extend pentest with 4 new attack categories (transport, auth, data exfiltration, protocol abuse), --profile quick/deep flag, and deep fuzzing mutations -- 13 new attacks (TR-01..03, AF-01..03, DE-01..03, PA-01..04) across 32 total attack IDs.
**Requirements**: None (new feature, not tracked in REQUIREMENTS.md)
**Depends on:** Phase 62
**Plans:** 3/3 plans complete

Plans:
- [x] 63-01-PLAN.md -- Foundation: extend AttackCategory enum, add PentestProfile, --profile flag, 4 attack module stubs, SARIF rules, engine dispatch
- [ ] 63-02-PLAN.md -- Transport security (TR-01..03) and auth flow (AF-01..03) attack runners
- [x] 63-03-PLAN.md -- Data exfiltration (DE-01..03), protocol abuse (PA-01..04) attack runners, deep fuzzing mode

### Phase 64: secrets-deployment-integration

**Goal:** Wire `cargo pmcp secret` into deployment targets so secrets are injected as environment variables at deploy time. Five workstreams: (1) AWS Lambda — resolve secrets from configured provider and inject as Lambda env vars in CDK context during `cargo pmcp deploy --target aws-lambda`. (2) pmcp.run — ensure `cargo pmcp secret set --server <id>` sends server ID for backend-side env var trigger, and `cargo pmcp deploy --target pmcp-run` transmits secret requirements to the backend. (3) SDK support — add thin `pmcp::secrets` module with `get`/`require` helpers that read env vars with helpful error messages pointing to `cargo pmcp secret set`. (4) Local dev — `cargo pmcp dev` reads local secrets and sets them as env vars for the child server process. (5) Documentation — update cargo-pmcp README, secret command help text, deployment docs, and add SDK-level rustdoc examples.
**Requirements**: D-01 through D-17 (from CONTEXT.md)
**Depends on:** Phase 63
**Plans:** 3/3 plans complete

Plans:
- [x] 64-01-PLAN.md -- Secret resolution logic + deploy pipeline integration (dotenvy, resolve_secrets, CDK env passthrough)
- [x] 64-02-PLAN.md -- SDK pmcp::secrets thin reader module (get/require helpers, SecretError)
- [ ] 64-03-PLAN.md -- Dev command .env loading + documentation (dev.rs injection, README, CLI help)

### v2.1 rmcp Upgrades (In Progress)

**Milestone Goal:** Close the credibility and developer-experience gaps where the official Rust MCP SDK (rmcp) outshines PMCP -- documentation accuracy, feature gate presentation, macro documentation, example index, and repo hygiene. No new runtime dependencies; all fixes are configuration changes, file rewrites, and targeted attribute additions.

- [x] **Phase 65: Examples Cleanup and Protocol Accuracy** - Replace broken examples/README.md, fix protocol badge, resolve 17 orphan example files and 4 duplicate number prefixes (completed 2026-04-10)
- [x] **Phase 66: Macros Documentation Rewrite** - Rewrite pmcp-macros README to document current #[mcp_tool]/#[mcp_server]/#[mcp_prompt]/#[mcp_resource] API with migration guide (completed 2026-04-11)
- [x] **Phase 67: docs.rs Pipeline and Feature Flags** - Enable doc_auto_cfg for automatic feature badges, explicit feature list in docs.rs metadata, feature flag table, zero rustdoc warnings (completed 2026-04-12)
- [ ] **Phase 68: General Documentation Polish** - Update lib.rs doctests to TypedToolWithOutput pattern, add transport matrix, CI enforcement gates for drift prevention

## Phase Details — Current Milestone

### Phase 65: Examples Cleanup and Protocol Accuracy
**Goal**: Developers browsing the examples/ directory and README see accurate PMCP content with correct protocol version, every example file is runnable, and no numbering collisions exist
**Depends on**: Phase 64
**Requirements**: EXMP-01, EXMP-02, EXMP-03, PROT-01
**Success Criteria** (what must be TRUE):
  1. `examples/README.md` contains a PMCP example index organized by category (transport, tools, resources, prompts, tasks, apps) with required features and run commands for each example
  2. Every `.rs` file in `examples/` has a corresponding `[[example]]` entry in `Cargo.toml` with correct `required-features`, and `cargo run --example <name>` works for each
  3. No two example files share the same numbered prefix -- `ls examples/*.rs | awk -F_ '{print $1}' | sort | uniq -d` returns empty
  4. The README.md MCP-Compatible badge and compatibility table display protocol version `2025-11-25`, matching `LATEST_PROTOCOL_VERSION` in source code
**Plans:** 3/3 plans complete
Plans:
- [x] 65-01-PLAN.md — Audit orphan examples + fix protocol badge (EXMP-02, PROT-01)
- [x] 65-02-PLAN.md — Renumber all examples with role-prefix scheme (EXMP-03)
- [x] 65-03-PLAN.md — Write examples/README.md index (EXMP-01)

### Phase 66: Macros Documentation Rewrite
**Goal**: A developer reading pmcp-macros documentation (on docs.rs or GitHub) sees accurate documentation of #[mcp_tool], #[mcp_server], #[mcp_prompt], and #[mcp_resource] as the primary API, with a clear migration path from deprecated macros
**Depends on**: Phase 65
**Requirements**: MACR-01, MACR-02, MACR-03
**Success Criteria** (what must be TRUE):
  1. `pmcp-macros/README.md` documents `#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`, and `#[mcp_resource]` as the primary API with working code examples that compile
  2. A migration section guides users from deprecated `#[tool]`/`#[tool_router]` to `#[mcp_tool]`/`#[mcp_server]` with before/after code comparisons
  3. `pmcp-macros/src/lib.rs` uses `include_str!("../README.md")` so that `docs.rs/pmcp-macros` renders the rewritten README as the crate-level documentation
  4. No references to stale version numbers (e.g., `pmcp = { version = "1.*" }`) appear in the macros README
**Plans:** 6 plans
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [x] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [x] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 67: docs.rs Pipeline and Feature Flags
**Goal**: docs.rs renders PMCP with automatic feature badges on all feature-gated items, an explicit feature list preventing internal APIs from surfacing, a documented feature flag table, and zero rustdoc warnings
**Depends on**: Phase 66
**Requirements**: DRSD-01, DRSD-02, DRSD-03, DRSD-04
**Success Criteria** (what must be TRUE):
  1. `src/lib.rs` contains `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` and all ~145 feature-gated items on docs.rs display automatic feature availability badges
  2. `Cargo.toml` `[package.metadata.docs.rs]` uses an explicit feature list (~13 user-facing features) instead of `all-features = true`, preventing test helpers and internal features from surfacing
  3. A feature flag table in `lib.rs` doc comments documents all user-facing features with descriptions and what they enable
  4. `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` exits with zero warnings -- all broken intra-doc links and unclosed HTML tags resolved
  5. CI includes a `make doc-check` target that enforces zero rustdoc warnings on every PR
**Plans:** 6 plans
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [x] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 67.1: Code Mode Support (INSERTED)

**Goal:** External MCP server developers can add Code Mode (validate → approve → execute) to their servers using PMCP SDK crates, with a `#[derive(CodeMode)]` proc macro, pluggable `PolicyEvaluator` + `CodeExecutor` traits, zeroizing token secrets, and a complete worked example — unblocking an imminent MCP server launch that depends on this capability.
**Depends on:** Phase 67
**Requirements**: CMSUP-01, CMSUP-02, CMSUP-03, CMSUP-04, CMSUP-05, CMSUP-06
**Success Criteria** (what must be TRUE):
  1. `crates/pmcp-code-mode/` exists in the rust-mcp-sdk workspace containing the moved + hardened Code Mode core (validation pipeline, `PolicyEvaluator`, `CedarPolicyEvaluator`, `NoopPolicyEvaluator`, new `CodeExecutor` trait, new `TokenSecret` newtype with zeroization) and all existing tests pass
  2. `crates/pmcp-code-mode-derive/` exists and provides a working `#[derive(CodeMode)]` proc macro that emits a `register_code_mode_tools(builder)` method, enforces `Send + Sync` at compile time, and has `trybuild` compile-pass + compile-fail snapshot coverage
  3. `pmcp-code-mode/src/lib.rs` re-exports `async_trait` (`pub use async_trait::async_trait;`) and generated derive output uses `#[pmcp_code_mode::async_trait]` to avoid version conflicts
  4. A complete worked example in `examples/` (e.g. `XX_code_mode_graphql.rs`) demonstrates: struct annotation → `register_code_mode_tools` → `validate_code` → approval token → `execute_code` round trip using `NoopPolicyEvaluator`
  5. Contract YAMLs for `pmcp-code-mode` and `pmcp-code-mode-derive` exist under `../provable-contracts/contracts/` and `pmat comply check` passes on both
  6. `make quality-gate` passes workspace-wide (zero clippy warnings, zero SATD, all tests green, format clean) and both new crates are positioned in the publishing order documented in CLAUDE.md, ready for the next release phase
**Plans:** 6/6 plans complete

Plans:
- [x] 67.1-01-PLAN.md — Crate scaffolding + source move into workspace
- [x] 67.1-02-PLAN.md — Security hardening (TokenSecret, NoopPolicyEvaluator, async_trait re-export)
- [x] 67.1-03-PLAN.md — CodeExecutor high-level trait
- [x] 67.1-04-PLAN.md — pmcp-code-mode-derive proc macro (#[derive(CodeMode)] + trybuild)
- [x] 67.1-05-PLAN.md — Property tests + fuzz targets
- [x] 67.1-06-PLAN.md — End-to-end example + CRATE-READMEs + SECURITY.md + quality-gate

### Phase 67.2: Code Mode Derive Hardening (INSERTED)

**Goal:** Fix critical derive macro issues from pmcp.run team review (policy_evaluator not called, static ValidationContext, hardcoded "graphql"), address review warnings, and resolve high-priority performance/quality issues from IMPROVEMENTS.md.
**Depends on:** Phase 67.1
**Requirements**: CMSUP-07, CMSUP-08, CMSUP-09, CMSUP-10
**Success Criteria** (what must be TRUE):
  **Derive macro — critical (pmcp.run review items 1-3):**
  1. Generated `ValidateCodeHandler` calls `policy_evaluator.evaluate_operation()` between pipeline validation and token generation — the security contract is enforced
  2. `#[code_mode(context_from = "method_name")]` attribute extracts real `ValidationContext` from `RequestHandlerExtra` or a struct method, replacing hardcoded placeholders
  3. `#[code_mode(language = "graphql"|"javascript"|"sql")]` attribute parameterizes tool metadata — SQL/OpenAPI servers get correct tool schemas
  **Derive macro — warnings (pmcp.run review items 4-8):**
  4. `HmacTokenGenerator::new` returns `Result` instead of panicking on short secrets
  5. Trybuild compile-fail tests cover `token_secret` and `code_executor` absent fields
  6. Generated handlers share a single `Arc<ValidationPipeline>` instead of constructing two
  **Performance (IMPROVEMENTS.md P-01, P-03):**
  7. eval.rs array methods use scope-chain/push-pop instead of cloning entire HashMap per element (P-01)
  8. Async GraphQL validation fallback reuses parsed `query_info` instead of re-parsing (P-03)
  *Deferred: P-02 (double SWC parse) requires new `ValidatedCode` type threading AST across javascript.rs/executor.rs — deferred to a future phase*
  **Code quality (IMPROVEMENTS.md Q-01 through Q-04, R-01):**
  9. `json_to_string` / `value_to_string` unified into one function (Q-01)
  10. `LoopContinue`/`LoopBreak` moved to internal `StepOutcome` enum, removed from public `ExecutionError` (Q-04)
  11. `ValidationResponse` wraps `ValidationResult` instead of duplicating all fields (R-01)
  **Baseline:**
  12. All existing tests pass, `cargo test -p pmcp-code-mode -p pmcp-code-mode-derive` green
  13. Clippy suppressions reduced (trivially fixable: `useless_format`, `derivable_impls`, etc.)
**Plans:** 6/6 plans complete
Plans:
- [x] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [x] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [ ] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

### Phase 68: General Documentation Polish
**Goal**: Crate-level documentation showcases current best practices (TypedToolWithOutput, proc macros), transport types are discoverable, and CI gates prevent future documentation drift
**Depends on**: Phase 67
**Requirements**: PLSH-01, PLSH-02, PLSH-03
**Success Criteria** (what must be TRUE):
  1. `lib.rs` crate-level doc examples compile and demonstrate the `TypedToolWithOutput` pattern and current builder APIs (not legacy `Server::builder()` or `ToolHandler`)
  2. A transport matrix table in `lib.rs` doc comments lists all supported transports (stdio, streamable HTTP, SSE) with links to their actual module/type paths
  3. CI enforces that the count of `[[example]]` entries in `Cargo.toml` matches the count of `.rs` files in `examples/`, failing the build on mismatch
  4. `cargo semver-checks check-release` runs in CI on every PR to prevent accidental API breakage during documentation changes
**Plans:** 6 plans
Plans:
- [ ] 67.2-01-PLAN.md — Wire policy_evaluator into generated handlers + switch to async validation
- [ ] 67.2-02-PLAN.md — Add context_from and language darling attributes to derive macro
- [ ] 67.2-03-PLAN.md — HmacTokenGenerator::new returns Result + trybuild compile-fail tests
- [ ] 67.2-04-PLAN.md — eval.rs scope-chain optimization for array methods (P-01)
- [ ] 67.2-05-PLAN.md — Async GraphQL double-parse elimination (P-03)
- [ ] 67.2-06-PLAN.md — json_to_string unification + StepOutcome refactor + ValidationResponse wrapping + clippy cleanup

## Progress — Current Milestone

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 65. Examples Cleanup + Protocol Accuracy | v2.1 | 3/3 | Complete    | 2026-04-10 |
| 66. Macros Documentation Rewrite | v2.1 | 5/5 | Complete    | 2026-04-11 |
| 67. docs.rs Pipeline + Feature Flags | v2.1 | 6/6 | Complete    | 2026-04-12 |
| 68. General Documentation Polish | v2.1 | 0/? | Not started | - |

## Backlog

Parking lot for unsequenced ideas. Items here aren't scheduled — promote with `/gsd:review-backlog` when ready.

### Phase 999.1: Delete DEFAULT_PROTOCOL_VERSION constant and make callsites explicit (BACKLOG)

**Goal:** Remove the public `DEFAULT_PROTOCOL_VERSION` re-export and replace each of its ~15 callsites with an explicit choice — either `LATEST_PROTOCOL_VERSION` (where the code is advertising what this SDK supports) or a literal `"2025-03-26"` with a `// backward-compat fallback` comment (where the code genuinely wants the widest-compatible version for un-negotiated peers). The name `DEFAULT` is misleading: nothing is actually "default" about it — it's a specific compat choice that happens to be older than `LATEST`, and that distinction is invisible at every callsite.

**Scope:**
- Breaking API change (public re-export at `src/lib.rs:307` + `src/types/mod.rs:32`) — requires minor version bump and release note
- Callsites to audit and convert:
  - `src/types/protocol/mod.rs:32` — `impl Default for ProtocolVersion`
  - `src/server/streamable_http_server.rs` lines 560, 971, 979, 981, 985, 1225, 1231, 1233, 1236
  - `src/server/core.rs:1329` and `:1376`
  - `src/shared/event_store.rs:367`
  - `src/lib.rs:286` — public doctest asserting the value
  - `src/types/protocol/version.rs:51,65` — unit tests
  - `benches/comprehensive_benchmarks.rs:421`

**Why:** Phase 65 simplify review flagged the inconsistency — `LATEST_PROTOCOL_VERSION = "2025-11-25"` but `DEFAULT_PROTOCOL_VERSION = "2025-03-26"`. Mechanically bumping `DEFAULT` to match `LATEST` would be a silent behavior change for peers reaching the fallback path (they'd be assumed to speak the newer protocol before they've said so). The right fix is to delete the misleading abstraction, not flip its value.

**Requirements:** TBD

**Plans:** 6/6 plans complete

Plans:
- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 999.2: TOON data format feasibility and SDK integration (BACKLOG)

**Goal:** Investigate whether PMCP should add built-in support for TOON as an alternative wire format to JSON for MCP tool outputs and resource payloads, specifically to optimize performance of MCP Apps that often ship large JSON payloads. Deliver a spike report + recommendation (adopt / pilot / reject), and if adoption is recommended, a follow-up implementation plan for a feature-gated `toon` format with encoder/decoder integrated into `Content`, tool output serialization, and the MCP App bridge so servers and Apps can opt in with a single flag.

**Motivation:**
- MCP Apps frequently serialize large structured payloads (tables, datasets, chart data) — the dataviz, hotel gallery, and venue map examples are all 10–100 KB of JSON per response
- Both the LLM context window and the UI widget render path benefit from smaller payloads: fewer tokens consumed by tool output, faster postMessage bridge transfer, faster widget mount
- TOON (Token-Oriented Object Notation) is designed explicitly for this use case — schema-aware compression that encodes repeated keys and types once, yielding ~30–60% size reductions on tabular data compared to JSON
- If adoption works, MCP servers would flip a flag per-tool or per-resource to switch output format, and MCP Apps would transparently decode on the bridge side

**Research questions (spike scope, not implementation):**
1. Maturity of TOON — is the spec stable enough to commit a feature-gated SDK integration? Is there a Rust encoder/decoder crate, or would PMCP need to author one?
2. Compatibility — can TOON payloads ride over existing MCP `Content` variants (probably via a new `TextContent` MIME type or a new `Content::Toon` variant), or does it need protocol changes that break v2025-11-25 compat?
3. Measurement — what are the realistic size/token savings on representative MCP App payloads (dataviz, gallery, map from existing examples)? Do LLMs tokenize TOON efficiently, or does the token savings on the wire get lost when Claude/GPT re-tokenize the decoded content?
4. Widget-side decoder — can the MCP App bridge (TypeScript) decode TOON in-browser without a heavy dependency, or is this a non-starter for the WASM/iframe sandbox?
5. Opt-in UX — what does `#[mcp_tool(output_format = "toon")]` or equivalent look like on the server side? What's the per-call server-side negotiation story?

**Why backlog (not an active phase):**
- The user's framing is explicitly exploratory ("let's investigate if we can add it as a built-in support")
- TOON is a newer format — the feasibility spike should happen before committing a phase slot
- The v2.1 rmcp Upgrades milestone is scoped to documentation polish; runtime data-format work doesn't belong there
- Natural home after spike: either seed of a "v2.2 Payload Optimization" milestone or early phase of a milestone focused on MCP Apps performance

**Promotion path:** Run `/gsd:discuss-phase 999.2` to gather context, then `/gsd:research-phase 999.2` for the spike, then promote via `/gsd:review-backlog` into an active milestone with a concrete Phase N number.

**Requirements:** TBD (depends on spike outcome)

**Plans:** 6 plans

Plans:
- [ ] TBD (promote with `/gsd:review-backlog` when ready)

### Phase 69: rmcp parity research — ergonomics gap analysis + follow-on phase proposals

**Goal:** Produce a rigorous, evidence-backed gap matrix comparing pmcp vs rmcp on *ergonomics* (macro DX, builder APIs, typed wrappers, handler shapes, state/extra patterns) and use it to propose 2–4 concrete follow-on phases to close the credibility/DX gap. Transports, examples polish, and docs coverage are intentionally out of scope — Phase 68 handles those surfaces at the polish layer.

**Deliverables:**
- `69-RESEARCH.md` — gap matrix (per-feature: rmcp approach, pmcp approach, gap severity, evidence citations)
- `69-PROPOSALS.md` — 2–4 follow-on phase proposals with goals, scope, and rough success criteria, ready to slot into v2.1 or seed v2.2

**Requirements**: TBD (derived from research findings; expected to seed new v2.1/v2.2 requirement IDs)
**Depends on:** Phase 68
**Plans:** 3 plans

Plans:
- [x] 69-01-PLAN.md — Produce the rmcp vs pmcp ergonomics gap matrix (69-RESEARCH.md) across 6 surfaces
- [x] 69-02-PLAN.md — Derive follow-on phase proposals from High-severity gaps (69-PROPOSALS.md)
- [x] 69-03-PLAN.md — Quality gate + land PARITY-* requirement IDs + update STATE/PROJECT

### Phase 70: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01)

**Goal:** Extend `RequestHandlerExtra` with two drop-in additive capabilities — a typed-key `Extensions` map (HANDLER-02) for request-scoped user data crossing middleware/handler boundaries, and an optional `PeerHandle` back-channel (HANDLER-05) exposing `sample` / `list_roots` / `progress_notify` from inside tool/prompt/resource handlers — without breaking any existing `::new(...)` or `::with_session(...)` call site. Restructured from 3 plans to 4 plans after cross-AI review (70-REVIEWS.md) + codebase verification (70-REVIEW-VERIFICATION.md) confirmed 5 of Codex's HIGH findings: the original plan set assumed an outbound `ServerRequest` transport + response-correlation layer that does not exist in the live codebase. Plan 02 (NEW) builds that foundational plumbing before Plan 03 wires the peer.
**Requirements**: PARITY-HANDLER-01
**Depends on:** Phase 69
**Plans:** 4 plans

Plans:
- [x] 70-01-PLAN.md — Extensions typemap on both RequestHandlerExtra structs + #[non_exhaustive] + accessor parity + 5 proptests + refactor 12 struct-literal test sites (Wave 1)
- [x] 70-02-PLAN.md — ServerRequestDispatcher (outbound ServerRequest + response correlation) + Server::run drain-to-transport + route TransportMessage::Response through dispatcher (NEW plan from reviews replan — addresses Codex Findings 2+3) (Wave 2)
- [x] 70-03-PLAN.md — PeerHandle trait + DispatchPeerHandle delegating to Plan 02 dispatcher + conditional .with_peer(...) at 9 dispatch sites + dispatch-path round-trip integration test (Wave 3)
- [x] 70-04-PLAN.md — Examples s42 + s43 (s43 uses real ToolHandler per Codex Finding 5) + fuzz target + rustdoc migration prose with explicit semver posture + make quality-gate (Wave 4)

### Phase 71: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01)

**Goal:** Enable `#[mcp_tool]` to harvest the attached function's rustdoc as the tool description when the `description = "..."` attribute is omitted — eliminating forced duplication where a well-documented tool fn must repeat its description in both the rustdoc block and the macro attribute. Preserves precedence (explicit attribute wins over rustdoc), fails with a clear error when neither is present, and remains backwards-compatible with all existing call sites. Derived from 69-PROPOSALS.md Proposal 3 (MACRO-02, High severity).
**Requirements**: PARITY-MACRO-01
**Depends on:** Phase 70
**Plans:** 4/4 plans complete

Plans:
- [x] 71-01-PLAN.md — Create new sibling crate `crates/pmcp-macros-support/` (non-proc-macro) holding the pure `extract_doc_description` normalization helper with unit tests + proptest invariants — resolves HIGH-1 via Option A so proc-macro crate API restrictions don't block property/fuzz consumers (Wave 1)
- [x] 71-02-PLAN.md — `pmcp-macros` adds path dep on `pmcp-macros-support` + single shared `resolve_tool_args` resolver in `mcp_common.rs`; both parse sites (`mcp_tool.rs` standalone + `mcp_server.rs::parse_mcp_tool_attr` impl-block) delegate to it; integration tests lock symmetry (MEDIUM-1) (Wave 2)
- [x] 71-03-PLAN.md — 4 trybuild compile-fail snapshots (existing regenerated + new empty-args + new non-empty-args + regenerated multi-args) + README migration section with Limitations subsection + mixed-shape fuzz target `rustdoc_normalize.rs` (MEDIUM-2 + MEDIUM-3 + LOW-3) (Wave 3)
- [x] 71-04-PLAN.md — Workspace `pmcp`-dependency ripple audit + version bumps (pmcp 2.3.0→2.4.0 MINOR per MEDIUM-4, pmcp-macros 0.5.0→0.6.0, new pmcp-macros-support 0.1.0, concurrent downstream patch bumps cargo-pmcp 0.6.0→0.6.1 + mcp-tester 0.5.0→0.5.1 per CLAUDE.md §"Version Bump Rules") + CHANGELOG entry + REQUIREMENTS.md closure + `make quality-gate` (HIGH-2 + MEDIUM-4) (Wave 4)

### Phase 72: Investigate rmcp as foundations for pmcp - evaluate using rmcp for protocol level while focusing pmcp on pragmatic batteries-included SDK for enterprise use cases ✓ COMPLETE

**Status:** COMPLETE (2026-04-19) — **Recommendation: D** (Maintain pmcp as authoritative Rust MCP SDK; do not migrate onto rmcp). 7/9 decision thresholds resolved; T6/T7 remain UNKNOWN per 72-CONTEXT.md. Slice 1 spike executed — serde `params: null` round-trip fails against rmcp 1.5.0, downgrading inventory row 1 from EXACT to compatible-via-adapter. Phase 69's parity phases (70, 71, CLIENT-02) remain the forward path. See `.planning/phases/72-investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp/72-RECOMMENDATION.md`.

**Goal:** Produce a research/decision-only recommendation on whether pmcp's protocol layer should be refactored to sit on top of rmcp 1.5.0 — repositioning pmcp + mcp-tester + mcp-preview + cargo-pmcp as a pragmatic, batteries-included, enterprise-focused SDK built *on top of* rmcp rather than alongside it. Deliverables are 7 markdown documents (CONTEXT, inventory, strategy matrix, PoC proposal, PoC results, decision rubric, final recommendation). If the recommendation is adopt (A/B/C1/C2), migration itself is scoped as a separate future v3.0 phase; if stay (D), Phase 69's parity phases remain the path forward.
**Requirements**: RMCP-EVAL-01, RMCP-EVAL-02, RMCP-EVAL-03, RMCP-EVAL-04, RMCP-EVAL-05
**Depends on:** Phase 71
**Plans:** 3/3 plans complete

Plans:
- [x] 72-01-PLAN.md — Seed RMCP-EVAL-01..05 in REQUIREMENTS.md; produce 72-INVENTORY.md (inversion inventory, >=15 pmcp module families with file:line + rmcp evidence) and 72-STRATEGY-MATRIX.md (5 options x 5 criteria = 25 cells, no TBD) (Wave 1)
- [x] 72-02-PLAN.md — Produce 72-POC-PROPOSAL.md (3 slices, each <=500 LOC, at least one <=3 days, with LOC/Files/Pass/Fail/Time-box fields) and 72-DECISION-RUBRIC.md (>=5 falsifiable thresholds, each followed by Data source) (Wave 2)
- [x] 72-03-PLAN.md — Produce 72-RECOMMENDATION.md (RMCP-EVAL-05) — opens with `**Recommendation:** <A|B|C|D|E>`, contains 5 per-criterion justification subsections citing T-IDs + inventory/matrix rows, lists UNRESOLVED thresholds, and names the next-phase handoff (Wave 3)

### Phase 72.1: Finalize landing support (INSERTED)

**Goal:** Ship CR-03 rev-2 — replace build-time `NEXT_PUBLIC_*` env vars in the landing Next.js template with a runtime `fetch('/landing-config')` via a new required shared hook `useLandingConfig`, fix 3 stale rustdoc references in `cargo-pmcp/src/landing/config.rs`, and bump `cargo-pmcp` 0.8.0 -> 0.8.1 (patch, additive). Unblocks pmcp.run Phase 71 UAT Test 7 and Cost Coach production launch.
**Requirements**: LAND-CR03-01
**Depends on:** Phase 72
**Plans:** 1/1 plans complete

Plans:
- [x] 72.1-01-PLAN.md — Create `lib/useLandingConfig.ts` hook; rewrite 4 consumers (signup, callback, connect [server->client flip], Header [conditional button]); fix 3 rustdoc comments in `src/landing/config.rs`; bump `Cargo.toml` 0.8.0 -> 0.8.1; run `make quality-gate` + `cargo doc` + template `tsc`/`next build` + grep guardrails G1..G6 + manual AC-11 offline gate (Wave 1)

### Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 72.1
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 74 to break down)

### Phase 73: Typed client helpers + list_all pagination (PARITY-CLIENT-01)

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 74
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 73 to break down)
