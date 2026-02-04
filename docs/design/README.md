# PMCP SDK UI Design Documents

This directory contains design documents for adding rich UI/widget support to the PMCP SDK, enabling interactive experiences across multiple MCP host platforms.

## Document Index

| Document | Description |
|----------|-------------|
| [chatgpt-apps-integration.md](./chatgpt-apps-integration.md) | Detailed analysis of ChatGPT Apps requirements and API changes |
| [chatgpt-apps-implementation-plan.md](./chatgpt-apps-implementation-plan.md) | 6-phase implementation plan with code examples |
| [widget-hosting-architecture.md](./widget-hosting-architecture.md) | CDN hosting + Lambda patterns |
| [unified-ui-architecture.md](./unified-ui-architecture.md) | **RECOMMENDED**: Multi-platform unified architecture |
| [stateless-first-architecture.md](./stateless-first-architecture.md) | **KEY INSIGHT**: Eliminating server-side state |

## Quick Decision Guide

### Which Approach Should We Take?

After analyzing three MCP UI approaches—**MCP Apps (SEP-1865)**, **MCP-UI**, and **ChatGPT Apps**—we recommend the **unified multi-layer architecture** described in `unified-ui-architecture.md`.

```
┌─────────────────────────────────────────────────────────────────┐
│                    RECOMMENDED APPROACH                          │
│                                                                  │
│   Unified Core Types → Adapter Layer → Platform-Specific Output │
│                                                                  │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐ │
│   │ UIResource   │───►│ Adapters:    │───►│ ChatGPT output   │ │
│   │ UIAction     │    │ - ChatGPT    │    │ MCP Apps output  │ │
│   │ UIMetadata   │    │ - MCP Apps   │    │ MCP-UI output    │ │
│   │              │    │ - MCP-UI     │    │                  │ │
│   └──────────────┘    └──────────────┘    └──────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Why Not Separate Implementations?

| Approach | Pros | Cons |
|----------|------|------|
| **Separate (ChatGPT + MCP)** | Simpler initial code | Duplicate logic, inconsistent APIs, hard to add new platforms |
| **Unified (Recommended)** | Single codebase, consistent API, easy to extend | Slightly more complex abstraction layer |

### Decision Matrix

| If your use case is... | Use... |
|------------------------|--------|
| ChatGPT-only app | `UIResourceBuilder::new(...).chatgpt().build()` |
| Claude/generic MCP host | `UIResourceBuilder::new(...).mcp_apps().build()` |
| Maximum compatibility | `UIResourceBuilder::new(...).all_adapters().build()` |
| MCP-UI ecosystem (Nanobot, etc.) | `UIResourceBuilder::new(...).mcp_ui().build()` |

## Key Insights

### 1. Stateless-First: No Database Needed (Usually)

**The widget already has full state.** Send complete context with each tool call:

```
┌─────────────────────────────────────────────────────────────────┐
│  STATELESS-FIRST ARCHITECTURE                                   │
│                                                                  │
│  Widget (has state)  ──────►  Lambda (pure function)  ──────►  Widget (new state)
│                                                                  │
│  - widgetState holds everything                                  │
│  - Tool calls include full context                               │
│  - Lambda computes and returns new state                         │
│  - NO DATABASE for most use cases                                │
└─────────────────────────────────────────────────────────────────┘
```

**Chess example:** FEN notation is ~70 bytes. Send the entire board position with each move!

```typescript
await callTool('chess_move', {
    position: 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
    from: 'e2', to: 'e4'
});
```

**Only add server state when truly required:**
- Multi-player (shared state between users)
- Cross-conversation persistence ("resume my game tomorrow")
- Audit/compliance (legal logging requirements)
- Anti-cheat (adversarial client)

### 2. MCP-UI Already Solved Multi-Platform

MCP-UI's adapter pattern proves the unified approach works:
- Server creates one `UIResource`
- Adapters transform for each host
- Apps SDK adapter bridges to ChatGPT's `window.openai`

### 3. MIME Types Matter

| Platform | MIME Type | Communication |
|----------|-----------|---------------|
| MCP Apps (SEP-1865) | `text/html+mcp` | postMessage + JSON-RPC |
| ChatGPT Apps | `text/html+skybridge` | window.openai API |
| MCP-UI | `text/html`, `text/uri-list`, `remote-dom` | postMessage + UI Actions |

### 4. Widget Hosting Strategy

For complex widgets (chess, maps, dashboards):

```
Lambda returns:    HTML template referencing CDN
CDN hosts:         React/Vue bundles (versioned)
ChatGPT manages:   UI state
DynamoDB stores:   Business data
```

## Implementation Priority

### Phase 1: Core Types (Required)
- Add `text/html+skybridge` MIME type
- Extend `CallToolResult` with `structured_content` and `_meta`
- Add `WidgetCSP`, `WidgetMeta` types

### Phase 2: Adapter Layer
- Implement `UIAdapter` trait
- Create `ChatGptAdapter`, `McpAppsAdapter`
- Optional: `McpUiAdapter` for full MCP-UI compatibility

### Phase 3: Builder API
- Unified `UIResourceBuilder` with adapter support
- `MultiPlatformUIResource` for multiple outputs

### Phase 4: Widget Runtime
- `@pmcp/widget-runtime` JavaScript library
- Works with both `postMessage` and `window.openai`

### Phase 5: CLI Tools
- `cargo pmcp ui init` - Initialize widget project
- `cargo pmcp ui build` - Build for all platforms
- `cargo pmcp ui validate` - Validate against platform specs

## Open Questions

1. **Should we support MCP-UI's Remote DOM?**
   - Adds complexity, but enables non-iframe rendering
   - Recommendation: Feature flag (`remote-dom`)

2. **How much MCP-UI compatibility?**
   - Full: Implement their adapter protocol
   - Partial: Just support similar patterns
   - Recommendation: Partial (aligned patterns, not wire-compatible)

3. **Widget state persistence outside ChatGPT?**
   - ChatGPT manages `widgetState`
   - Other hosts: localStorage fallback? Server-side?
   - Recommendation: localStorage fallback with optional server sync

## Getting Started

1. Read `unified-ui-architecture.md` for the recommended approach
2. Review `chatgpt-apps-integration.md` for ChatGPT-specific details
3. See `widget-hosting-architecture.md` for Lambda + UI patterns
4. Check `chatgpt-apps-implementation-plan.md` for phase-by-phase tasks
