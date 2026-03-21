---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: CLI DX Overhaul
status: in-progress
stopped_at: Completed 56-01-PLAN.md
last_updated: "2026-03-21T02:40:25Z"
last_activity: 2026-03-21 -- Phase 56 Plan 01 complete (Tower layers for DNS rebinding + security headers)
progress:
  total_phases: 34
  completed_phases: 24
  total_plans: 57
  completed_plans: 57
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-03)

**Core value:** Production-grade Rust MCP SDK with enterprise security, streamable HTTP focus, and Tasks with polling as the primary async pattern.
**Current focus:** v2.0 Protocol Modernization -- Phase 56 Plan 01 complete. Tower layers built. Plan 02 (Axum router integration) next.

## Current Position

Phase: 56 (tower-middleware-dns-rebinding-protection)
Plan: 1 of 2 complete
Status: Plan 01 complete. AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer implemented with 21 tests. 975 tests passing.
Last activity: 2026-03-21 -- Phase 56 Plan 01 complete (Tower layers for DNS rebinding + security headers)

Progress: [##########] 100%

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v1.5 | Cloud Load Testing Upload | 25-26 | 2026-03-01 |

## Performance Metrics

**Velocity:**

- Total plans completed: 73 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 8)
- Total phases completed: 29

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v1.6 decisions:

- 6 phases derived from 5 requirement categories: global flags, flag normalization, auth propagation, tester integration, new commands, help polish
- Phase 31 (New Commands) depends on Phase 28 (not 30) since doctor/completions don't need tester or auth
- Help polish is last phase since it touches every command and benefits from all prior changes being stable
- GlobalFlags defined in commands/mod.rs (not main.rs) to avoid circular imports
- no_color stores resolved effective value (CLI flag OR NO_COLOR env OR non-TTY)
- should_output() guard pattern for direct global_flags access; PMCP_QUIET env var for nested functions
- Secret module merges local --quiet with global --quiet via effective_quiet parameter
- Verbose field kept with allow(dead_code) -- used in precedence logic, not yet by individual commands
- [Phase 27]: Threaded not_quiet bool through validate.rs private functions rather than re-checking PMCP_QUIET env var in each function
- [Phase 34]: Axum 0.8 wildcard routes use {*path} syntax; mcp-preview bumped to 0.1.2
- [Phase 34-01]: Nested _meta.ui.resourceUri format with openai/outputTemplate for ChatGPT; HtmlMcpApp MIME type; dual-emit WidgetMeta prefersBorder
- [Phase 36]: Used explicit match arms (no wildcards) in From/TryFrom bridge for compile-time exhaustiveness
- [Phase 37]: Mirrored TypedTool::with_ui() exactly for TypedSyncTool and WasmTypedTool; WasmTypedTool tests wasm32-only gated
- [Phase 38]: Cache is sole source of truth for metadata; no fallback to handler.metadata() in hot paths; prompt_workflow() caches directly
- [Phase 39]: deep_merge in ui.rs for recursive JSON object merging; with_meta_entry on ToolInfo for composable _meta; arrays replaced not concatenated
- [Phase 39-02]: TypedToolWithOutput::with_ui() mirrors TypedTool::with_ui() for API consistency; all four tool types use identical deep_merge pattern
- [Phase 40-02]: redirect_domains excluded from nested ui.csp (ChatGPT-specific); nested csp uses spec camelCase field names; ModelOnly variant added to ToolVisibility
- [Phase 40-01]: Added legacy flat "ui/resourceUri" key to build_meta_map() matching official ext-apps dual-emit behavior
- [Phase 41-02]: AppBridge class in widget-runtime.mjs (not index.html); fall-through switch for backward compat; ui/notifications/initialized via setTimeout(0)
- [Phase 41]: Used field name meta with serde rename to _meta since leading underscores not idiomatic Rust
- [Phase 41-03]: Used TypedSyncTool::new().with_ui() in scaffold instead of tool_typed_sync_with_description() to enable tool-to-widget linking
- [Phase 42-01]: outputSchema is top-level on ToolInfo (MCP spec 2025-06-18); pmcp:outputTypeName remains in annotations as PMCP codegen extension
- [Phase 42-02]: cargo-pmcp local ToolSchema mirrors SDK ToolInfo with top-level output_schema; all docs and course content updated
- [Phase 43-01]: ResourceInfo._meta field with serde rename; URI-to-tool-meta index on ServerCore; with_widget_enrichment filtered to openai/toolInvocation/* only
- [Phase 43-02]: Post-process resources/list with clone and resources/read with deep_merge for _meta propagation from uri_to_tool_meta index
- [Phase 44-01]: Hard-coded ChatGPT descriptor/invocation keys in api.rs (mcp-preview doesn't depend on pmcp crate); derive(Default) with #[default] for PreviewMode
- [Phase 44-02]: AppBridge remains active in ChatGPT mode (postMessage is supplemental); skip iframe reload when same widget URI already loaded to preserve widget state
- [Phase 45-01]: Standard-only metadata emission by default; build_meta_map returns only ui.resourceUri nested key; host layer enrichment at build time; build_uri_to_tool_meta indexes by standard key; ChatGptAdapter always emits openai/outputTemplate from URI
- [Phase 45-02]: McpBridge refactored with extensions namespace; ChatGptExtensions isolates ChatGPT methods under extensions.chatgpt; Window intersection type for backward compat; buildChatGptExtensions() delegates to window.openai; legacy flat methods preserved with deprecation
- [Phase 45-03]: mcp-preview enriches tool/resource _meta with ChatGPT keys in ChatGPT mode; enrich_meta_for_chatgpt derives openai/* from standard ui.resourceUri; pre-existing widget issues documented not fixed
- [Phase 46-01]: Static lookup map for method name normalization in App class; McpApps bridge _onToolResult properties with getter/setter pairs; normalization in both widget-runtime and injected bridge scripts
- [Phase 46-02]: mcp-preview deliverToolResult emits dual ui/toolResult (primary) + ui/notifications/tool-result (fallback); readiness signal replaces 300ms setTimeout with ui/notifications/initialized listener + 3s fallback
- [Phase 47-01]: Resource URI cross-reference mismatch produces Warning not Failure; ChatGPT key absence is Warning; AppValidator applies strict mode internally
- [Phase 47-02]: Apps subcommand follows check.rs pattern for UX consistency; resources listing failure non-fatal (empty vec) since cross-reference is advisory
- [Phase 48-03]: THEME_PALETTES placed as module-level constant before PreviewRuntime class; THEME_PALETTES[this.theme] || {} for safe palette lookup
- [Phase 48]: THEME_PALETTES placed as module-level constant before PreviewRuntime class; safe palette lookup with || {}
- [Phase 48-01]: Used GUIDE.md as authoritative source for ch12-5 rewrite; eliminated ChatGptAdapter -- standard SDK APIs are primary documented pattern
- [Phase 48]: Eliminated ChatGptAdapter, WidgetDir, window.mcpBridge from course -- standard SDK APIs (ToolInfo::with_ui, ext-apps App class) are primary
- [Phase 28-01]: Retained #[allow(dead_code)] on GlobalFlags.verbose until Plans 02/03 add readers; ServerFlags makes both url and server optional for flexible flatten usage
- [Phase 28]: Retained #[allow(dead_code)] on GlobalFlags.verbose until Plans 02/03 add readers
- [Phase 28-02]: Removed #[allow(dead_code)] from GlobalFlags.verbose (now read by check, apps, run, validate, deploy); download format yaml->json default; schema diff url positional at index 2
- [Phase 28-03]: Landing deploy handler parameter server_id kept as internal API name; CLI field renamed to server
- [Phase 29-01]: allow(dead_code) on AuthMethod/resolve()/resolve_auth_middleware()/resolve_api_key() until Plans 02/03 add consumers; AuthMethod derives PartialEq for test assertions
- [Phase 29-02]: Middleware-only auth for ServerTester (None for api_key, middleware for chain) to avoid double headers; warning approach for run/generate library functions without auth passthrough
- [Phase 29-03]: McpProxy uses auth_header string (not middleware chain) since it uses raw reqwest; OAuth acquires token once at startup via get_access_token(); connect_inspector ignores auth; schema diff auth deferred
- [Phase 49-01]: Use oauth2::reqwest::Client for oauth2 token exchange (oauth2 5.0 re-exports reqwest 0.12); MSRV bumped 1.82->1.83 for jsonschema 0.45; accept dual reqwest in lockfile
- [Phase 50-01]: Rust target triples for asset naming; per-binary .sha256 files; macos-15-intel for x86_64, macos-14 for aarch64; ubuntu-24.04-arm for ARM Linux; fail-fast: false
- [Phase 50]: Rust target triples for asset naming; per-binary .sha256 files; macos-15-intel for x86_64, macos-14 for aarch64; ubuntu-24.04-arm for ARM Linux
- [Phase 50-02]: POSIX /bin/sh for install.sh; explicit repo URL in binstall pkg-url; pkg-fmt = bin for bare binaries; v{ version } prefix for tag convention
- [Phase 50]: POSIX /bin/sh for install.sh; explicit repo URL in binstall pkg-url; pkg-fmt = bin for bare binaries; v{ version } prefix for tag convention
- [Phase 51-01]: Used pmcp::server::Server (not ServerCore) as builder returns Server type; inserted pmcp-server after mcp-preview in workspace members
- [Phase 51-02]: AppValidationMode "all" implemented by iterating Standard+ChatGpt+ClaudeDesktop; "claude" accepted as alias for "claude-desktop"; strict mode applies inline on Vec<TestResult>
- [Phase 51-03]: Templates as const &str with {name} placeholder substitution; added get_server_version() to ServerTester; schema_export Rust codegen maps JSON Schema types to Rust types with Value fallback
- [Phase 51-04]: Used Content::Resource variant for ReadResourceResult to include URI and MIME type per MCP spec
- [Phase 51-04]: Const DOC_RESOURCES lookup table for URI routing avoids duplication between list() and read()
- [Phase 51-04]: One struct per prompt handler for cleaner PromptHandler trait impl and independent metadata
- [Phase 51-05]: Omitted explicit capabilities() since builder auto-sets on handler registration; publish order widget-utils->pmcp->mcp-tester->mcp-preview->pmcp-server->cargo-pmcp
- [Phase 53]: [Phase 53-01]: Verified Rust missing 2025-11-25 protocol version (20+ new types including TaskSchema, IconSchema, AudioContent, ResourceLink, expanded capabilities)
- [Phase 53]: [Phase 53-01]: Confirmed Rust ahead in MCP Apps (full adapter stack) but behind in Tasks capability negotiation (no ServerCapabilities.tasks/ClientCapabilities.tasks)
- [Phase 53-02]: Proposed 4 follow-up phases: Protocol 2025-11-25 (P0), Conformance Tests (P1), Tower Middleware (P2), Advanced Conformance (P2)
- [Phase 53-02]: 35 gaps identified across 6 domains; 15 areas where Rust leads TypeScript
- [Phase 53-02]: Deferred WebSocket transport, WASM cross-runtime, auth conformance, TaskMessageQueue per CONTEXT.md
- [Phase 54-01]: Protocol/mod.rs re-exports all domain types preserving crate::types::protocol::X paths; types/mod.rs uses single pub use protocol::* for flat access
- [Phase 54-01]: negotiate_protocol_version returns LATEST_PROTOCOL_VERSION (not DEFAULT) for unsupported versions; 3-version rolling window drops 2024 versions
- [Phase 54-01]: Domain module split pattern: types split by MCP domain (tools, resources, prompts, content, sampling, notifications) with re-export chain
- [Phase 54-02]: Implementation::new(name, version) constructor for backward-compat; 25+ struct literal sites unchanged
- [Phase 54-02]: ElicitRequestParams uses per-variant serde rename_all (not enum-level) for correct internally-tagged serialization
- [Phase 54-02]: SamplingMessageContent consolidates SamplingResultContent -- single enum for SamplingMessage and CreateMessageResultWithTools
- [Phase 54-02]: LogLevel kept as deprecated type alias; LoggingLevel is canonical 8-value enum with Notice, Alert, Emergency
- [Phase 54-02]: TaskRouter trait kept Value params -- typed params converted at call sites to avoid breaking pmcp-tasks interface
- [Phase 54-03]: LogLevel kept as deprecated alias (not removed) per Plan 02 decision for v2.0 backward compat
- [Phase 54-03]: types-internal IconInfo references use super::protocol::IconInfo for clean module-local paths
- [Phase 54-03]: ClientRequest enum variants now use canonical names (ListToolsRequest, CallToolRequest, etc.)
- [Phase 54-03]: PromptMessage.content field type changed from MessageContent to Content (canonical name)
- [Phase 54]: Used Implementation::new() across all test files instead of adding 4 optional fields to each struct literal
- [Phase 54]: Rewrote elicitation example 19 using spec-compliant ElicitRequestParams instead of disabling
- [Phase 54]: Added task type re-exports to protocol/mod.rs for flat pmcp::types:: access
- [Phase 54.1]: Content enum variants keep struct literal syntax; #[non_exhaustive] only on structs per D-08
- [Phase 54.1]: PromptMessage does not derive Default (both role and content required); uses ::new(role, content) + convenience ::user() ::assistant() ::system()
- [Phase 54.1-02]: TaskStatus Default variant is Working (most common initial state for task creation)
- [Phase 54.1-02]: Task::new() sets timestamps to empty strings -- .with_timestamps() sets both at once
- [Phase 54.1-02]: ToolChoice uses static factory methods (auto/required/none) not ::new() since it wraps single enum field
- [Phase 54.1-02]: CreateMessageParams has no Default (messages required) -- ::new(messages) only
- [Phase 54.1]: Content enum variant syntax preserved per D-08 but replaced with Content::text()/resource() helpers where available for consistency
- [Phase 55-01]: TTL serialization fixed -- removed skip_serializing_if from Task.ttl and TaskCreationParams.ttl so None serializes as null per MCP spec
- [Phase 55-01]: TaskStatus utility methods (is_terminal, can_transition_to) replicate pmcp-tasks behavior for SDK canonical source of truth
- [Phase 55-02]: Simplified TaskStore trait vs pmcp-tasks (no variables/result/request_method); returns Task wire type; TTL clamped not rejected; Instant-based expiration
- [Phase 55-03]: TaskStore checked before TaskRouter in dispatch for tasks/get, tasks/list, tasks/cancel; tasks/result remains TaskRouter-only (PMCP extension)
- [Phase 55-03]: ServerCapabilities.tasks (standard field) used for TaskStore path; experimental.tasks kept for TaskRouter backward compat
- [Phase 56-01]: AllowedOrigins auto-detects localhost/127.0.0.1/[::1] for loopback and unspecified bind addresses
- [Phase 56-01]: Missing Origin header permitted (non-browser clients like curl omit it); present but disallowed Origin returns 403
- [Phase 56-01]: No HSTS header per D-12 (transport-layer concern for reverse proxies)
- [Phase 56-01]: tower and tower-http gated behind existing streamable-http feature (no new feature flags)

### Roadmap Evolution

- Phase 33 added: Fix mcp-tester failure with v1.12.0
- Phase 34 added: Fix MCP Apps ChatGPT compatibility
- Phases 35-39 added: MCP Apps code quality improvements (meta key constants, MIME type unification, TypedSyncTool UI, ToolInfo caching, ui meta merge)
- Phase 40 added: Review ChatGPT Compatibility for Apps
- Phase 41 added: ChatGPT MCP Apps Upgraded Version
- Phase 42 added: Add outputSchema top level support
- Phase 43 added: ChatGPT MCP Apps alignment
- Phase 44 added: Improving mcp-preview to support ChatGPT version
- Phase 45 added: Extend MCP Apps Support to Claude Desktop
- Phase 46 added: MCP Bridge Review and Fixes
- Phase 47 added: Add MCP App support to mcp-tester
- Phase 48 added: MCP Apps Documentation and Education Refresh
- Phase 49 added: Bump dependencies (reqwest 0.13, jsonschema 0.45)
- Phase 50 added: Improve Binary Release
- Phase 51 added: PMCP MCP Server
- Phase 52 added: Reduce transitive dependencies
- Phase 53 added: Review TypeScript SDK Updates
- Phases 54-57 added: Protocol 2025-11-25 Support, Conformance Test Infrastructure, Tower Middleware, Conformance Extension (from Phase 53 gap analysis)
- Phase 58 added: #[mcp_tool] proc macro (from composition team DX review)
- Phase 59 added: TypedPrompt with auto-deserialization (from composition team DX review)
- Phase 54.1 inserted after Phase 54: Protocol Type Construction DX — Default impls, builders, and constructors for all protocol types (URGENT)

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-03-21T02:40:25Z
Stopped at: Completed 56-01-PLAN.md
Resume: Phase 56 Plan 01 complete. AllowedOrigins, DnsRebindingLayer, SecurityHeadersLayer created in src/server/tower_layers/. 21 new tests, 975 total passing. Plan 02 (Axum router integration) next.
