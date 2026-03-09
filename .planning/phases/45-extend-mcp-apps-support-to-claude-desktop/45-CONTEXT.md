# Phase 45: Extend MCP Apps Support to Claude Desktop - Context

**Gathered:** 2026-03-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Enable the same MCP server with MCP Apps to serve both ChatGPT and Claude Desktop (and future hosts) using the additive layering pattern. Default to standard MCP Apps spec, with host-specific extensions opted in via builder methods. Scope: SDK type/adapter changes, mcp-preview mode updates, widget-runtime bridge normalization, and example verification. Book/course content updates are deferred to a follow-up phase.

</domain>

<decisions>
## Implementation Decisions

### Claude Desktop protocol baseline
- Start with standard MCP Apps spec (ext-apps reference patterns) as Claude Desktop baseline
- HostType::Claude already exists returning HtmlMcp — assume correct unless research contradicts
- If Claude Desktop has its own protocol variations, add --mode claude-desktop to mcp-preview (don't force into standard mode)
- Research phase should compare ext-apps reference server output against SDK output to identify gaps

### Additive layering strategy
- Default SDK behavior: emit ONLY standard MCP spec keys (nested ui.resourceUri) — no ChatGPT openai/* keys, no legacy flat ui/resourceUri key
- Drop the legacy flat key ui/resourceUri entirely — clean break
- Host-specific keys added via ServerCoreBuilder::with_host_layer(HostType::ChatGpt)
- General pattern: .with_host_layer(HostType) — extensible for future hosts without API changes
- When ChatGPT layer is added, openai/outputTemplate and other ChatGPT keys are emitted alongside standard keys
- This is a BREAKING CHANGE for existing ChatGPT users who relied on auto-emitted openai/* keys — they now need .with_host_layer(HostType::ChatGpt)

### Preview mode for Claude
- --mode standard becomes the new default (no flag needed)
- --mode chatgpt remains as explicit opt-in for ChatGPT-strict validation
- --mode claude-desktop added later if research reveals Claude-specific requirements
- Standard mode validation mirrors ext-apps reference patterns (research phase determines exact checks)
- Standard mode uses McpApps postMessage bridge (JSON-RPC over postMessage)
- ChatGPT mode continues with ChatGPT postMessage emulation + window.openai stub

### Widget bridge normalization
- window.mcpBridge is the canonical developer-facing API — developers write against this
- AppBridge refactored to be host-agnostic: normalizes all host communication differences
- Developer writes one callback (e.g., mcpBridge.onToolResult(callback)) regardless of host
- Host-specific capabilities available via mcpBridge.extensions namespace:
  - mcpBridge.extensions.chatgpt.requestDisplayMode() — available when running in ChatGPT, undefined otherwise
  - mcpBridge.extensions.claude — reserved for future Claude-specific APIs
- Bridge hides postMessage vs window.openai vs other host delivery mechanisms
- widget-runtime package updated in this phase (not deferred)

### Phase scope
- SDK: Refactor metadata emission to standard-first + opt-in layers
- Preview: --mode standard as default, update validation rules
- Bridge: Normalize AppBridge, add extensions namespace
- Examples: Verify chess, map, dataviz render in standard mode
- Deferred: Book chapters, course content, quizzes/exercises → follow-up phase

### Claude's Discretion
- Internal implementation of host layer registration and metadata enrichment pipeline
- How to detect which host layers are active during _meta construction
- Widget-runtime bridge refactoring approach (incremental vs rewrite)
- Test strategy for verifying cross-host compatibility

</decisions>

<specifics>
## Specific Ideas

- "OpenAI's own documentation explicitly recommends not doing client detection — build with standard keys, layer on extensions"
- "The recommended pattern is additive layering, not branching: base layer (standard MCP Apps) + host extension layers"
- "Both _meta.ui.resourceUri and _meta['openai/outputTemplate'] can coexist — Claude uses the former, ChatGPT uses the latter, they don't interfere"
- ext-apps repo (modelcontextprotocol/ext-apps) is the gold standard reference for Claude Desktop behavior
- cohort-heatmap-server and customer-segmentation-server examples map closest to PMCP's existing patterns
- "Expect more variation in the future as each provider shows their advantage"
- This is a paradigm shift: ChatGPT-first → standard-first with ChatGPT as an opt-in layer

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `HostType` enum (mcp_apps.rs): Already has Claude variant, extensible with new hosts
- `UIAdapter` trait (adapter.rs): Per-platform transformation with host_type(), transform(), inject_bridge()
- `MultiPlatformResource` builder (builder.rs): .with_adapter() pattern for multi-host resources
- `ChatGptAdapter` (adapter.rs): Full ChatGPT bridge injection (~340 lines) — model for future host adapters
- `McpAppsAdapter` (adapter.rs): Standard MCP Apps postMessage bridge — becomes the base layer
- `deep_merge()` (ui.rs): Recursive JSON object merging for combining metadata from multiple sources
- `build_meta_map()` (ui.rs): Currently emits triple keys — needs refactoring to standard-only default
- `WidgetMeta`, `ChatGptToolMeta` structs (mcp_apps.rs): Host-specific metadata types
- `PreviewMode` enum (mcp-preview): Already has Standard/ChatGpt variants
- `AppBridge` class (widget-runtime.mjs): Existing bridge abstraction — needs host-agnostic refactor

### Established Patterns
- Dual-emit metadata: nested ui.* + flat openai/* keys (Phases 34, 40) — changing to standard-only default
- _meta single source of truth on tools, auto-propagated to resources (Phase 43)
- Builder pattern on ServerCoreBuilder for configuration
- Feature-gated optional functionality

### Integration Points
- ServerCoreBuilder: Add .with_host_layer(HostType) method
- build_meta_map() in ui.rs: Refactor to emit standard keys only by default, add host layer keys conditionally
- ToolInfo metadata emission: Host-aware _meta construction
- mcp-preview ConfigResponse: Thread mode through to validation and bridge selection
- widget-runtime AppBridge: Refactor constructor to detect host and normalize API

</code_context>

<deferred>
## Deferred Ideas

- Book chapter updates for cross-client MCP Apps — follow-up phase
- Course content updates for standard-first pattern — follow-up phase
- Quiz/exercise updates — follow-up phase
- --mode claude-desktop in mcp-preview — add if research reveals Claude-specific requirements

</deferred>

---

*Phase: 45-extend-mcp-apps-support-to-claude-desktop*
*Context gathered: 2026-03-09*
