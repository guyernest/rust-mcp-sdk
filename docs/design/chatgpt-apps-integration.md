# ChatGPT Apps Integration Design Document

**Version:** 1.0
**Date:** 2025-01-11
**Status:** Draft
**Author:** PMCP SDK Team

## Executive Summary

This document outlines the design for integrating PMCP SDK with OpenAI's ChatGPT Apps platform, enabling developers to build rich, interactive widget experiences that work with stateless serverless backends.

## Table of Contents

1. [Background](#background)
2. [Current State Analysis](#current-state-analysis)
3. [ChatGPT Apps Architecture](#chatgpt-apps-architecture)
4. [Gap Analysis](#gap-analysis)
5. [Proposed Design](#proposed-design)
6. [Widget Hosting Architecture](#widget-hosting-architecture)
7. [State Management Strategy](#state-management-strategy)
8. [Implementation Plan](#implementation-plan)
9. [API Changes](#api-changes)
10. [Migration Guide](#migration-guide)

---

## Background

### What are ChatGPT Apps?

ChatGPT Apps are MCP servers that provide tools with rich UI experiences rendered inside ChatGPT's interface. They use:

- **MCP Protocol**: Standard tool definitions and invocations
- **Skybridge Runtime**: OpenAI's widget sandboxing with `window.openai` API
- **Structured Content**: Three-tier response model (`structuredContent`, `content`, `_meta`)

### Why This Matters

PMCP SDK already supports MCP Apps Extension (SEP-1865) with `text/html+mcp`. ChatGPT Apps use a similar but distinct model (`text/html+skybridge`) with additional metadata and a different widget runtime API.

---

## Current State Analysis

### What PMCP SDK Currently Provides

| Component | Implementation | Location |
|-----------|---------------|----------|
| UI Resources | `UIResource`, `UIResourceContents` | `src/types/ui.rs` |
| UI Builder | `UIResourceBuilder` | `src/server/ui.rs` |
| Tool-UI Association | `ToolInfo::with_ui()`, `TypedTool::with_ui()` | `src/types/protocol.rs` |
| MIME Type | `text/html+mcp` only | `src/types/ui.rs` |
| Tool Response | `content`, `is_error` only | `src/types/protocol.rs` |
| Widget Runtime | `postMessage` with `mcp-tool-result` | Examples |

### Current Widget Communication Model

```javascript
// Current MCP Apps Extension model
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        const data = event.data.result;
        // Render UI with data
    }
});

// Request tool data
window.parent.postMessage({
    jsonrpc: '2.0',
    method: 'tools/call',
    params: { name: 'my_tool', arguments: {...} },
    id: 1
}, '*');
```

---

## ChatGPT Apps Architecture

### Three-Tier Response Model

ChatGPT Apps use a fundamentally different response structure:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Tool Response                                │
├─────────────────────────────────────────────────────────────────┤
│  structuredContent    │  Concise JSON for MODEL + WIDGET        │
│                       │  - Model reads for narration            │
│                       │  - Widget reads via toolOutput          │
├───────────────────────┼─────────────────────────────────────────┤
│  content              │  Markdown/text narration                │
│                       │  - Model uses for response              │
│                       │  - Optional                             │
├───────────────────────┼─────────────────────────────────────────┤
│  _meta                │  Large/sensitive data for WIDGET ONLY   │
│                       │  - Never sent to model                  │
│                       │  - Widget reads via toolResponseMetadata│
└─────────────────────────────────────────────────────────────────┘
```

### Widget Runtime API (`window.openai`)

```typescript
interface OpenAIWidgetRuntime {
    // Data Access
    toolInput: object;              // Arguments passed to tool
    toolOutput: object;             // structuredContent from response
    toolResponseMetadata: object;   // _meta from response
    widgetState: object;            // Persisted UI state

    // State Management
    setWidgetState(state: object): void;

    // Tool Invocation
    callTool(name: string, args: object): Promise<object>;
    sendFollowUpMessage(message: string): void;

    // File Handling
    uploadFile(file: File): Promise<FileRef>;
    getFileDownloadUrl(fileId: string): string;

    // Layout Control
    requestDisplayMode(mode: 'compact' | 'expanded'): void;
    requestModal(config: ModalConfig): void;
    notifyIntrinsicHeight(height: number): void;
    openExternal(url: string): void;

    // Context
    theme: 'light' | 'dark';
    displayMode: 'compact' | 'expanded';
    maxHeight: number;
    safeArea: SafeAreaInsets;
    view: 'chat' | 'canvas';
    userAgent: string;
    locale: string;
}
```

### Tool Metadata Requirements

```json
{
    "name": "kanban_board",
    "title": "Show Kanban Board",
    "inputSchema": { ... },
    "annotations": {
        "readOnlyHint": false,
        "openWorldHint": false,
        "destructiveHint": false
    },
    "_meta": {
        "openai/outputTemplate": "ui://widget/kanban-board.html",
        "openai/toolInvocation/invoking": "Preparing the board...",
        "openai/toolInvocation/invoked": "Board ready.",
        "openai/widgetAccessible": true,
        "openai/visibility": "public"
    }
}
```

### Resource Metadata Requirements

```json
{
    "uri": "ui://widget/kanban-board.html",
    "mimeType": "text/html+skybridge",
    "text": "<html>...</html>",
    "_meta": {
        "openai/widgetPrefersBorder": true,
        "openai/widgetDomain": "https://chatgpt.com",
        "openai/widgetDescription": "Interactive Kanban board",
        "openai/widgetCSP": {
            "connect_domains": ["https://api.example.com"],
            "resource_domains": ["https://*.oaistatic.com"],
            "redirect_domains": ["https://checkout.example.com"],
            "frame_domains": []
        }
    }
}
```

---

## Gap Analysis

### Critical Gaps

| Gap | Current | Required | Priority |
|-----|---------|----------|----------|
| MIME Type | `text/html+mcp` | `text/html+skybridge` | P0 |
| Tool Response | `content` only | `structuredContent`, `content`, `_meta` | P0 |
| Resource `_meta` | None | Widget configuration | P0 |
| Tool `_meta` | `ui/resourceUri` only | OpenAI metadata fields | P1 |
| Widget Runtime | `postMessage` | `window.openai` compatibility | P1 |
| CSP Support | None | `openai/widgetCSP` | P1 |

### Compatibility Matrix

| Feature | MCP Apps (SEP-1865) | ChatGPT Apps | Compatible? |
|---------|---------------------|--------------|-------------|
| URI Scheme | `ui://` | `ui://widget/` | Yes (superset) |
| Tool Hints | Standard MCP | Same (for elicitation) | Yes |
| Widget Sandboxing | iframe | iframe | Yes |
| Data Delivery | `postMessage` | `window.openai` | No (different) |
| State Persistence | None | `widgetState` | ChatGPT advantage |

---

## Proposed Design

### New Type Definitions

```rust
// src/types/chatgpt.rs (NEW FILE)

/// ChatGPT-specific MIME types
pub enum ChatGptMimeType {
    /// Standard MCP HTML (`text/html+mcp`)
    HtmlMcp,
    /// ChatGPT Skybridge (`text/html+skybridge`)
    HtmlSkybridge,
}

/// Widget Content Security Policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetCSP {
    /// Domains widget can fetch from
    pub connect_domains: Vec<String>,
    /// Domains for static assets (images, fonts, scripts)
    pub resource_domains: Vec<String>,
    /// Domains for external redirects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_domains: Option<Vec<String>>,
    /// Domains allowed for iframes (use with caution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,
}

/// Widget configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetMeta {
    /// Whether widget prefers a border
    #[serde(rename = "openai/widgetPrefersBorder")]
    pub prefers_border: Option<bool>,

    /// Dedicated origin for the widget
    #[serde(rename = "openai/widgetDomain")]
    pub domain: Option<String>,

    /// Content Security Policy
    #[serde(rename = "openai/widgetCSP")]
    pub csp: Option<WidgetCSP>,

    /// Widget self-description
    #[serde(rename = "openai/widgetDescription")]
    pub description: Option<String>,
}

/// Tool invocation messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocationMeta {
    /// Message shown while tool is running
    #[serde(rename = "openai/toolInvocation/invoking")]
    pub invoking: Option<String>,

    /// Message shown when tool completes
    #[serde(rename = "openai/toolInvocation/invoked")]
    pub invoked: Option<String>,
}

/// Tool visibility setting
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolVisibility {
    /// Tool visible to model (default)
    Public,
    /// Tool hidden from model, only callable from widget
    Private,
}

/// Complete ChatGPT tool metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGptToolMeta {
    /// UI template URI
    #[serde(rename = "openai/outputTemplate")]
    pub output_template: Option<String>,

    /// Invocation messages
    #[serde(flatten)]
    pub invocation: Option<ToolInvocationMeta>,

    /// Widget can call this tool
    #[serde(rename = "openai/widgetAccessible")]
    pub widget_accessible: Option<bool>,

    /// Tool visibility
    #[serde(rename = "openai/visibility")]
    pub visibility: Option<ToolVisibility>,

    /// File parameter names
    #[serde(rename = "openai/fileParams")]
    pub file_params: Option<Vec<String>>,
}
```

### Extended Tool Response

```rust
// src/types/protocol.rs - EXTENDED

/// Tool call result with ChatGPT Apps support
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result (narration for model)
    #[serde(default)]
    pub content: Vec<Content>,

    /// Whether the tool call represents an error
    #[serde(default)]
    pub is_error: bool,

    /// Structured content for model AND widget (NEW)
    /// Keep concise - this affects model context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,

    /// Metadata for widget only - never sent to model (NEW)
    /// Use for large data, sensitive info, or UI-specific data
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, Value>>,
}
```

### Extended UI Resource Builder

```rust
// src/server/ui.rs - EXTENDED

impl UIResourceBuilder {
    /// Use ChatGPT Skybridge format
    pub fn skybridge(mut self) -> Self {
        self.mime_type = UIMimeType::HtmlSkybridge;
        self
    }

    /// Set widget border preference
    pub fn prefers_border(mut self, prefers: bool) -> Self {
        self.widget_meta.prefers_border = Some(prefers);
        self
    }

    /// Set widget domain for dedicated origin
    pub fn widget_domain(mut self, domain: impl Into<String>) -> Self {
        self.widget_meta.domain = Some(domain.into());
        self
    }

    /// Set widget description
    pub fn widget_description(mut self, desc: impl Into<String>) -> Self {
        self.widget_meta.description = Some(desc.into());
        self
    }

    /// Configure Content Security Policy
    pub fn csp(mut self, csp: WidgetCSP) -> Self {
        self.widget_meta.csp = Some(csp);
        self
    }
}
```

### Extended TypedTool

```rust
// src/server/typed_tool.rs - EXTENDED

impl<T, F> TypedTool<T, F> {
    /// Set ChatGPT output template
    pub fn with_output_template(mut self, uri: impl Into<String>) -> Self {
        self.chatgpt_meta.output_template = Some(uri.into());
        self
    }

    /// Set loading message
    pub fn with_invoking_message(mut self, msg: impl Into<String>) -> Self {
        self.chatgpt_meta.invocation
            .get_or_insert_default()
            .invoking = Some(msg.into());
        self
    }

    /// Set completion message
    pub fn with_invoked_message(mut self, msg: impl Into<String>) -> Self {
        self.chatgpt_meta.invocation
            .get_or_insert_default()
            .invoked = Some(msg.into());
        self
    }

    /// Allow widget to call this tool
    pub fn widget_accessible(mut self, accessible: bool) -> Self {
        self.chatgpt_meta.widget_accessible = Some(accessible);
        self
    }

    /// Set tool visibility
    pub fn visibility(mut self, visibility: ToolVisibility) -> Self {
        self.chatgpt_meta.visibility = Some(visibility);
        self
    }

    /// Declare file parameters
    pub fn file_params(mut self, params: Vec<String>) -> Self {
        self.chatgpt_meta.file_params = Some(params);
        self
    }
}
```

---

## Widget Hosting Architecture

### The Stateless Lambda Challenge

**Problem**: Interactive widgets (chess boards, maps, forms) need:
1. Rich UI frameworks (React, Vue)
2. State persistence across interactions
3. Real-time updates
4. Large asset bundles

**Constraint**: Lambda is stateless and has size limits

### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ChatGPT                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                     Widget Iframe                              │  │
│  │                                                                │  │
│  │  ┌────────────────────┐    ┌─────────────────────────────┐   │  │
│  │  │  window.openai     │    │  Widget Bundle (from CDN)   │   │  │
│  │  │                    │    │                             │   │  │
│  │  │  toolOutput ───────┼────► Data Layer                  │   │  │
│  │  │  widgetState ◄─────┼────► State Layer                 │   │  │
│  │  │  callTool() ───────┼────► Action Layer                │   │  │
│  │  │                    │    │                             │   │  │
│  │  └────────────────────┘    └─────────────────────────────┘   │  │
│  │                                       │                       │  │
│  └───────────────────────────────────────┼───────────────────────┘  │
└──────────────────────────────────────────┼──────────────────────────┘
                                           │
           ┌───────────────────────────────┼───────────────────────────┐
           │                               │                           │
           ▼                               ▼                           │
┌──────────────────────┐        ┌──────────────────────┐              │
│   CloudFront CDN     │        │   API Gateway        │              │
│                      │        │   (MCP Endpoint)     │              │
│  Widget Bundles:     │        │                      │              │
│  - chess-widget.js   │        │  POST /mcp           │              │
│  - map-widget.js     │        │                      │              │
│  - form-widget.js    │        └──────────┬───────────┘              │
│  - common.css        │                   │                          │
│                      │                   ▼                          │
└──────────┬───────────┘        ┌──────────────────────┐              │
           │                    │   Lambda Function    │              │
           │                    │   (MCP Server)       │              │
           │                    │                      │              │
           │                    │  - Tool handlers     │              │
           │                    │  - Business logic    │              │
           │                    │  - Data operations   │              │
           │                    └──────────┬───────────┘              │
           │                               │                          │
           ▼                               ▼                          │
┌──────────────────────┐        ┌──────────────────────┐              │
│   S3 Bucket          │        │   DynamoDB           │              │
│   (Widget Source)    │        │   (App Data)         │              │
│                      │        │                      │              │
│  Source files for    │        │  - Game states       │              │
│  widget bundles      │        │  - User preferences  │              │
│                      │        │  - Session data      │              │
└──────────────────────┘        └──────────────────────┘              │
                                                                       │
                        AWS Infrastructure ────────────────────────────┘
```

### Hosting Options Comparison

| Option | Pros | Cons | Best For |
|--------|------|------|----------|
| **Inline HTML** | Simple, no hosting | Size limits, no frameworks | Simple widgets |
| **S3 + CloudFront** | Cheap, fast, simple | Manual deployment | Static widgets |
| **Amplify Hosting** | CI/CD, preview URLs | More complex | Complex apps |
| **Cloudflare Pages** | Fast, global, cheap | Different ecosystem | Edge-first apps |

### Recommended Approach: Hybrid Model

1. **Template Reference**: MCP server returns HTML shell that references CDN bundles
2. **CDN-Hosted Bundles**: React/Vue apps built and deployed to CloudFront
3. **Lambda Data Layer**: All data operations go through MCP tools
4. **ChatGPT State**: Use `widgetState` for UI state (managed by ChatGPT)
5. **DynamoDB**: For persistent app data that survives sessions

---

## State Management Strategy

### Three Tiers of State

```
┌─────────────────────────────────────────────────────────────────┐
│                      State Architecture                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Tier 1: Widget State (ChatGPT-Managed)                 │   │
│  │                                                          │   │
│  │  • UI-specific state (selected items, expanded panels)  │   │
│  │  • Persisted by ChatGPT across re-renders               │   │
│  │  • Read: window.openai.widgetState                      │   │
│  │  • Write: window.openai.setWidgetState()                │   │
│  │  • Scope: Current conversation                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                             │                                    │
│                             ▼                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Tier 2: Tool Response Data (Ephemeral)                 │   │
│  │                                                          │   │
│  │  • Data returned by tool calls                          │   │
│  │  • structuredContent → window.openai.toolOutput         │   │
│  │  • _meta → window.openai.toolResponseMetadata           │   │
│  │  • Refreshed on each tool call                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                             │                                    │
│                             ▼                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Tier 3: Persistent App State (Server-Side)             │   │
│  │                                                          │   │
│  │  • Business data (game moves, documents, settings)      │   │
│  │  • Stored in DynamoDB or other backend                  │   │
│  │  • Accessed via tool calls                              │   │
│  │  • Survives across conversations                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Chess Game Example

```javascript
// Widget code for chess game

// Tier 1: UI State (ChatGPT-managed)
const [widgetState, setWidgetState] = useWidgetState({
    selectedSquare: null,
    highlightedMoves: [],
    boardFlipped: false,
    animating: false
});

// Tier 2: Tool Response Data
const gameData = window.openai.toolOutput;
// { gameId: "abc123", board: [...], turn: "white", moveHistory: [...] }

// Tier 3: Persistent State (via tool calls)
async function makeMove(from, to) {
    setWidgetState({ ...widgetState, animating: true });

    const result = await window.openai.callTool('chess_move', {
        gameId: gameData.gameId,
        from,
        to
    });

    // Result contains new board state from server
    // Widget will re-render with new toolOutput
}
```

### Lambda Handler for Chess

```rust
// Lambda remains stateless - all state in DynamoDB

async fn chess_move(args: ChessMoveArgs, _extra: RequestHandlerExtra) -> pmcp::Result<CallToolResult> {
    // Load game from DynamoDB
    let game = db.get_game(&args.game_id).await?;

    // Validate and apply move
    let new_game = game.apply_move(&args.from, &args.to)?;

    // Save to DynamoDB
    db.save_game(&new_game).await?;

    // Return structured content for model + widget
    Ok(CallToolResult {
        content: vec![Content::Text {
            text: format!("Moved {} from {} to {}",
                new_game.last_piece(), args.from, args.to)
        }],
        is_error: false,
        structured_content: Some(json!({
            "gameId": new_game.id,
            "board": new_game.board_state(),
            "turn": new_game.current_turn(),
            "lastMove": { "from": args.from, "to": args.to }
        })),
        _meta: Some(json!({
            "fullMoveHistory": new_game.move_history(),
            "capturedPieces": new_game.captured_pieces(),
            "analysis": new_game.engine_analysis()
        }).as_object().cloned()),
    })
}
```

---

## Implementation Plan

See `chatgpt-apps-implementation-plan.md` for detailed implementation phases.

---

## API Changes

### Breaking Changes

None - all changes are additive.

### New Public API

```rust
// New types
pub use pmcp::types::chatgpt::{
    ChatGptMimeType,
    WidgetCSP,
    WidgetMeta,
    ToolInvocationMeta,
    ToolVisibility,
    ChatGptToolMeta,
};

// Extended builders
UIResourceBuilder::skybridge()
UIResourceBuilder::prefers_border()
UIResourceBuilder::widget_domain()
UIResourceBuilder::widget_description()
UIResourceBuilder::csp()

TypedTool::with_output_template()
TypedTool::with_invoking_message()
TypedTool::with_invoked_message()
TypedTool::widget_accessible()
TypedTool::visibility()
TypedTool::file_params()

// Extended response
CallToolResult::structured_content
CallToolResult::_meta
```

---

## Migration Guide

### From MCP Apps Extension to ChatGPT Apps

```rust
// Before (MCP Apps Extension)
let tool = TypedTool::new("my_tool", handler)
    .with_ui("ui://my-widget");

let resource = UIResourceBuilder::new("ui://my-widget", "My Widget")
    .html_template(HTML)
    .build()?;

// After (ChatGPT Apps)
let tool = TypedTool::new("my_tool", handler)
    .with_output_template("ui://widget/my-widget.html")
    .with_invoking_message("Loading...")
    .with_invoked_message("Ready!")
    .widget_accessible(true);

let resource = UIResourceBuilder::new("ui://widget/my-widget.html", "My Widget")
    .skybridge()
    .prefers_border(true)
    .widget_domain("https://chatgpt.com")
    .csp(WidgetCSP::new()
        .connect("https://api.example.com"))
    .html_template(HTML)
    .build()?;
```

### Widget Code Migration

```javascript
// Before (postMessage)
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        renderUI(event.data.result);
    }
});

// After (window.openai)
function App() {
    const data = window.openai.toolOutput;
    const meta = window.openai.toolResponseMetadata;
    const [state, setState] = useWidgetState({ selected: null });

    return <MyComponent data={data} state={state} onSelect={setState} />;
}
```

---

## Appendix

### A. OpenAI Metadata Reference

| Key | Type | Description |
|-----|------|-------------|
| `openai/outputTemplate` | string | UI template URI |
| `openai/toolInvocation/invoking` | string | Loading message |
| `openai/toolInvocation/invoked` | string | Completion message |
| `openai/widgetAccessible` | boolean | Widget can call tool |
| `openai/visibility` | "public"\|"private" | Model visibility |
| `openai/fileParams` | string[] | File parameter names |
| `openai/widgetPrefersBorder` | boolean | Border preference |
| `openai/widgetDomain` | string | Dedicated origin |
| `openai/widgetCSP` | object | Content Security Policy |
| `openai/widgetDescription` | string | Widget description |

### B. Elicitation Rules

ChatGPT uses tool annotations for user confirmation:

| Condition | Requires Confirmation? |
|-----------|----------------------|
| `readOnlyHint: true` | No |
| `readOnlyHint: false`, `openWorldHint: false` | No |
| `readOnlyHint: false`, `openWorldHint: true` | **Yes** |
| `destructiveHint: true` | Warning shown, no extra confirmation |

### C. Related Documents

- `chatgpt-apps-implementation-plan.md` - Implementation phases
- `widget-hosting-guide.md` - Detailed hosting options
- `examples/chatgpt-chess/` - Chess game example
- `examples/chatgpt-maps/` - Interactive maps example
