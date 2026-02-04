# ChatGPT Apps Integration - Implementation Plan

**Version:** 1.0
**Date:** 2025-01-11
**Status:** Draft
**Related:** `chatgpt-apps-integration.md`

## Overview

This document provides a detailed implementation plan for adding ChatGPT Apps support to the PMCP SDK. The plan is organized into phases that can be executed incrementally.

---

## Phase 0: Preparation

### 0.1 Create Feature Flag

**Files:** `Cargo.toml`, `src/lib.rs`

```toml
[features]
default = ["server", "client"]
chatgpt-apps = []  # NEW: Enable ChatGPT Apps support
```

**Rationale:** Allow users to opt-in to ChatGPT-specific features without affecting existing MCP implementations.

### 0.2 Create Module Structure

```
src/
├── types/
│   ├── mod.rs
│   ├── ui.rs           # Existing
│   └── chatgpt.rs      # NEW: ChatGPT-specific types
├── server/
│   ├── mod.rs
│   ├── ui.rs           # Existing - extend
│   └── chatgpt.rs      # NEW: ChatGPT builders
└── lib.rs              # Re-exports
```

### 0.3 Test Infrastructure

Create test fixtures and mocks for ChatGPT Apps:

```
tests/
├── chatgpt_apps_test.rs       # NEW
├── fixtures/
│   └── chatgpt/
│       ├── widget.html
│       ├── tool_response.json
│       └── resource_meta.json
```

---

## Phase 1: Core Protocol Types

**Goal:** Add fundamental types for ChatGPT Apps without breaking changes.

### 1.1 Add Skybridge MIME Type

**File:** `src/types/ui.rs`

```rust
/// Supported MIME types for UI resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIMimeType {
    /// HTML with MCP postMessage support (`text/html+mcp`)
    HtmlMcp,
    /// HTML with ChatGPT Skybridge support (`text/html+skybridge`)
    #[cfg(feature = "chatgpt-apps")]
    HtmlSkybridge,
}

impl UIMimeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HtmlMcp => "text/html+mcp",
            #[cfg(feature = "chatgpt-apps")]
            Self::HtmlSkybridge => "text/html+skybridge",
        }
    }
}
```

**Tests:**
- [ ] MIME type string conversion
- [ ] Serialization/deserialization
- [ ] Feature flag gating

### 1.2 Add Widget CSP Type

**File:** `src/types/chatgpt.rs` (NEW)

```rust
//! ChatGPT Apps specific types
//!
//! This module provides types for building ChatGPT Apps with the PMCP SDK.
//! Enable with the `chatgpt-apps` feature.

use serde::{Deserialize, Serialize};

/// Widget Content Security Policy
///
/// Defines which domains the widget can interact with.
///
/// # Example
///
/// ```rust
/// use pmcp::types::chatgpt::WidgetCSP;
///
/// let csp = WidgetCSP::new()
///     .connect("https://api.example.com")
///     .resources("https://cdn.example.com")
///     .resources("https://*.oaistatic.com");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WidgetCSP {
    /// Domains widget can fetch from (connect-src)
    #[serde(default)]
    pub connect_domains: Vec<String>,

    /// Domains for static assets like images, fonts, scripts
    #[serde(default)]
    pub resource_domains: Vec<String>,

    /// Domains for external redirects via openExternal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_domains: Option<Vec<String>>,

    /// Domains allowed for iframes (use with extreme caution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_domains: Option<Vec<String>>,
}

impl WidgetCSP {
    /// Create empty CSP
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connect domain (for fetch/XHR)
    pub fn connect(mut self, domain: impl Into<String>) -> Self {
        self.connect_domains.push(domain.into());
        self
    }

    /// Add a resource domain (for images, scripts, etc.)
    pub fn resources(mut self, domain: impl Into<String>) -> Self {
        self.resource_domains.push(domain.into());
        self
    }

    /// Add a redirect domain (for openExternal)
    pub fn redirect(mut self, domain: impl Into<String>) -> Self {
        self.redirect_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }

    /// Add a frame domain (for iframes - use with caution)
    ///
    /// **Warning:** Widgets with frame_domains are subject to
    /// higher scrutiny during ChatGPT App review.
    pub fn frame(mut self, domain: impl Into<String>) -> Self {
        self.frame_domains
            .get_or_insert_with(Vec::new)
            .push(domain.into());
        self
    }
}
```

**Tests:**
- [ ] Builder pattern
- [ ] JSON serialization matches OpenAI spec
- [ ] Empty fields are omitted

### 1.3 Add Widget Metadata Type

**File:** `src/types/chatgpt.rs`

```rust
/// Widget configuration metadata for ChatGPT Apps
///
/// These fields are added to the resource's `_meta` field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WidgetMeta {
    /// Whether widget prefers a border
    #[serde(rename = "openai/widgetPrefersBorder", skip_serializing_if = "Option::is_none")]
    pub prefers_border: Option<bool>,

    /// Dedicated origin for the widget
    #[serde(rename = "openai/widgetDomain", skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Content Security Policy
    #[serde(rename = "openai/widgetCSP", skip_serializing_if = "Option::is_none")]
    pub csp: Option<WidgetCSP>,

    /// Widget self-description
    #[serde(rename = "openai/widgetDescription", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl WidgetMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prefers_border(mut self, prefers: bool) -> Self {
        self.prefers_border = Some(prefers);
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn csp(mut self, csp: WidgetCSP) -> Self {
        self.csp = Some(csp);
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}
```

### 1.4 Add Tool Metadata Types

**File:** `src/types/chatgpt.rs`

```rust
/// Tool visibility setting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolVisibility {
    /// Tool visible to model (default)
    #[default]
    Public,
    /// Tool hidden from model, only callable from widget
    Private,
}

/// Tool invocation messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolInvocationMeta {
    /// Message shown while tool is running
    #[serde(rename = "openai/toolInvocation/invoking", skip_serializing_if = "Option::is_none")]
    pub invoking: Option<String>,

    /// Message shown when tool completes
    #[serde(rename = "openai/toolInvocation/invoked", skip_serializing_if = "Option::is_none")]
    pub invoked: Option<String>,
}

/// Complete ChatGPT tool metadata
///
/// # Example
///
/// ```rust
/// use pmcp::types::chatgpt::{ChatGptToolMeta, ToolVisibility};
///
/// let meta = ChatGptToolMeta::new()
///     .output_template("ui://widget/board.html")
///     .invoking("Preparing...")
///     .invoked("Ready!")
///     .widget_accessible(true)
///     .visibility(ToolVisibility::Public);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatGptToolMeta {
    /// UI template URI
    #[serde(rename = "openai/outputTemplate", skip_serializing_if = "Option::is_none")]
    pub output_template: Option<String>,

    /// Message shown while tool is running
    #[serde(rename = "openai/toolInvocation/invoking", skip_serializing_if = "Option::is_none")]
    pub invoking: Option<String>,

    /// Message shown when tool completes
    #[serde(rename = "openai/toolInvocation/invoked", skip_serializing_if = "Option::is_none")]
    pub invoked: Option<String>,

    /// Widget can call this tool
    #[serde(rename = "openai/widgetAccessible", skip_serializing_if = "Option::is_none")]
    pub widget_accessible: Option<bool>,

    /// Tool visibility
    #[serde(rename = "openai/visibility", skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ToolVisibility>,

    /// File parameter names
    #[serde(rename = "openai/fileParams", skip_serializing_if = "Option::is_none")]
    pub file_params: Option<Vec<String>>,
}

impl ChatGptToolMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_template(mut self, uri: impl Into<String>) -> Self {
        self.output_template = Some(uri.into());
        self
    }

    pub fn invoking(mut self, msg: impl Into<String>) -> Self {
        self.invoking = Some(msg.into());
        self
    }

    pub fn invoked(mut self, msg: impl Into<String>) -> Self {
        self.invoked = Some(msg.into());
        self
    }

    pub fn widget_accessible(mut self, accessible: bool) -> Self {
        self.widget_accessible = Some(accessible);
        self
    }

    pub fn visibility(mut self, visibility: ToolVisibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    pub fn file_params(mut self, params: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.file_params = Some(params.into_iter().map(Into::into).collect());
        self
    }

    /// Convert to serde_json::Map for merging into _meta
    pub fn to_meta_map(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }
}
```

### 1.5 Extend CallToolResult

**File:** `src/types/protocol.rs`

```rust
/// Tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Tool execution result (narration for model)
    #[serde(default)]
    pub content: Vec<Content>,

    /// Whether the tool call represents an error
    #[serde(default)]
    pub is_error: bool,

    /// Structured content for model AND widget
    ///
    /// Keep this concise - it affects model context window.
    /// Use `_meta` for large data that only the widget needs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,

    /// Metadata for widget only - never sent to model
    ///
    /// Use for:
    /// - Large datasets
    /// - Sensitive information
    /// - UI-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl CallToolResult {
    /// Create a simple text result
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Content::Text { text: text.into() }],
            is_error: false,
            structured_content: None,
            _meta: None,
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![Content::Text { text: message.into() }],
            is_error: true,
            structured_content: None,
            _meta: None,
        }
    }

    /// Create a ChatGPT Apps result with structured content
    ///
    /// # Arguments
    ///
    /// * `structured` - Concise JSON for model and widget
    /// * `narration` - Text narration for model response
    /// * `meta` - Large data for widget only (optional)
    #[cfg(feature = "chatgpt-apps")]
    pub fn chatgpt(
        structured: serde_json::Value,
        narration: impl Into<String>,
        meta: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Self {
        Self {
            content: vec![Content::Text { text: narration.into() }],
            is_error: false,
            structured_content: Some(structured),
            _meta: meta,
        }
    }

    /// Add structured content
    pub fn with_structured_content(mut self, content: serde_json::Value) -> Self {
        self.structured_content = Some(content);
        self
    }

    /// Add metadata (for widget only)
    pub fn with_meta(mut self, meta: serde_json::Map<String, serde_json::Value>) -> Self {
        self._meta = Some(meta);
        self
    }
}
```

**Tests:**
- [ ] Backward compatibility (old responses still work)
- [ ] JSON serialization with all fields
- [ ] Empty optional fields are omitted

---

## Phase 2: Builder Extensions

**Goal:** Extend existing builders with ChatGPT-specific methods.

### 2.1 Extend UIResourceBuilder

**File:** `src/server/ui.rs`

```rust
#[cfg(feature = "chatgpt-apps")]
use crate::types::chatgpt::{WidgetCSP, WidgetMeta};

pub struct UIResourceBuilder {
    uri: String,
    name: String,
    description: Option<String>,
    mime_type: UIMimeType,
    content: Option<String>,
    #[cfg(feature = "chatgpt-apps")]
    widget_meta: WidgetMeta,
}

impl UIResourceBuilder {
    // ... existing methods ...

    /// Use ChatGPT Skybridge format instead of standard MCP
    ///
    /// This sets the MIME type to `text/html+skybridge` and enables
    /// ChatGPT-specific widget configuration.
    #[cfg(feature = "chatgpt-apps")]
    pub fn skybridge(mut self) -> Self {
        self.mime_type = UIMimeType::HtmlSkybridge;
        self
    }

    /// Set widget border preference
    #[cfg(feature = "chatgpt-apps")]
    pub fn prefers_border(mut self, prefers: bool) -> Self {
        self.widget_meta.prefers_border = Some(prefers);
        self
    }

    /// Set widget domain for dedicated origin
    ///
    /// Example: `"https://chatgpt.com"`
    #[cfg(feature = "chatgpt-apps")]
    pub fn widget_domain(mut self, domain: impl Into<String>) -> Self {
        self.widget_meta.domain = Some(domain.into());
        self
    }

    /// Set widget description
    ///
    /// This helps reduce redundant text beneath the widget.
    #[cfg(feature = "chatgpt-apps")]
    pub fn widget_description(mut self, desc: impl Into<String>) -> Self {
        self.widget_meta.description = Some(desc.into());
        self
    }

    /// Configure Content Security Policy
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::ui::UIResourceBuilder;
    /// use pmcp::types::chatgpt::WidgetCSP;
    ///
    /// let resource = UIResourceBuilder::new("ui://widget/app", "My App")
    ///     .skybridge()
    ///     .csp(WidgetCSP::new()
    ///         .connect("https://api.example.com")
    ///         .resources("https://cdn.example.com"))
    ///     .html_template("<html>...</html>")
    ///     .build();
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn csp(mut self, csp: WidgetCSP) -> Self {
        self.widget_meta.csp = Some(csp);
        self
    }

    /// Build with ChatGPT widget metadata included
    #[cfg(feature = "chatgpt-apps")]
    pub fn build_with_chatgpt_meta(self) -> Result<(UIResource, UIResourceContents, WidgetMeta)> {
        let (resource, contents) = self.build_with_contents()?;
        Ok((resource, contents, self.widget_meta))
    }
}
```

### 2.2 Extend TypedTool

**File:** `src/server/typed_tool.rs`

```rust
#[cfg(feature = "chatgpt-apps")]
use crate::types::chatgpt::{ChatGptToolMeta, ToolVisibility};

pub struct TypedTool<T, F> {
    // ... existing fields ...
    #[cfg(feature = "chatgpt-apps")]
    chatgpt_meta: ChatGptToolMeta,
}

impl<T, F> TypedTool<T, F> {
    // ... existing methods ...

    /// Set ChatGPT output template URI
    ///
    /// This is the ChatGPT equivalent of `with_ui()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// let tool = TypedTool::new("kanban_board", handler)
    ///     .with_output_template("ui://widget/kanban-board.html");
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn with_output_template(mut self, uri: impl Into<String>) -> Self {
        self.chatgpt_meta.output_template = Some(uri.into());
        self
    }

    /// Set message shown while tool is running
    ///
    /// # Example
    ///
    /// ```rust
    /// let tool = TypedTool::new("load_data", handler)
    ///     .with_invoking_message("Loading data from server...");
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn with_invoking_message(mut self, msg: impl Into<String>) -> Self {
        self.chatgpt_meta.invoking = Some(msg.into());
        self
    }

    /// Set message shown when tool completes
    ///
    /// # Example
    ///
    /// ```rust
    /// let tool = TypedTool::new("load_data", handler)
    ///     .with_invoked_message("Data loaded successfully!");
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn with_invoked_message(mut self, msg: impl Into<String>) -> Self {
        self.chatgpt_meta.invoked = Some(msg.into());
        self
    }

    /// Allow widget to call this tool via `window.openai.callTool()`
    ///
    /// # Example
    ///
    /// ```rust
    /// let tool = TypedTool::new("refresh_data", handler)
    ///     .widget_accessible(true);  // Widget can refresh data
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn widget_accessible(mut self, accessible: bool) -> Self {
        self.chatgpt_meta.widget_accessible = Some(accessible);
        self
    }

    /// Set tool visibility
    ///
    /// Use `Private` to hide tool from model (widget-only).
    ///
    /// # Example
    ///
    /// ```rust
    /// // This tool is only callable from the widget
    /// let tool = TypedTool::new("internal_action", handler)
    ///     .visibility(ToolVisibility::Private)
    ///     .widget_accessible(true);
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn visibility(mut self, visibility: ToolVisibility) -> Self {
        self.chatgpt_meta.visibility = Some(visibility);
        self
    }

    /// Declare which parameters accept file uploads
    ///
    /// # Example
    ///
    /// ```rust
    /// let tool = TypedTool::new("process_image", handler)
    ///     .file_params(vec!["imageToProcess"]);
    /// ```
    #[cfg(feature = "chatgpt-apps")]
    pub fn file_params(mut self, params: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.chatgpt_meta.file_params = Some(params.into_iter().map(Into::into).collect());
        self
    }
}
```

---

## Phase 3: Widget Hosting Support

**Goal:** Provide utilities for hosting widget assets with serverless architectures.

### 3.1 CDN Reference Support

**File:** `src/server/chatgpt.rs` (NEW)

```rust
//! ChatGPT Apps helpers for widget hosting

/// Widget bundle reference for CDN-hosted assets
///
/// Use this when your widget is too complex to inline in the response.
///
/// # Example
///
/// ```rust
/// use pmcp::server::chatgpt::WidgetBundle;
///
/// let bundle = WidgetBundle::new()
///     .script("https://cdn.example.com/widgets/chess/1.0.0/app.js")
///     .style("https://cdn.example.com/widgets/chess/1.0.0/app.css")
///     .module(true);  // ES module
///
/// let html = bundle.to_html_template("chess-root");
/// ```
#[derive(Debug, Clone, Default)]
pub struct WidgetBundle {
    scripts: Vec<String>,
    styles: Vec<String>,
    is_module: bool,
    root_element: String,
}

impl WidgetBundle {
    pub fn new() -> Self {
        Self {
            root_element: "root".to_string(),
            ..Default::default()
        }
    }

    /// Add a script URL
    pub fn script(mut self, url: impl Into<String>) -> Self {
        self.scripts.push(url.into());
        self
    }

    /// Add a stylesheet URL
    pub fn style(mut self, url: impl Into<String>) -> Self {
        self.styles.push(url.into());
        self
    }

    /// Set whether scripts are ES modules
    pub fn module(mut self, is_module: bool) -> Self {
        self.is_module = is_module;
        self
    }

    /// Set the root element ID
    pub fn root_id(mut self, id: impl Into<String>) -> Self {
        self.root_element = id.into();
        self
    }

    /// Generate HTML template that loads from CDN
    pub fn to_html_template(&self) -> String {
        let styles = self.styles.iter()
            .map(|url| format!(r#"<link rel="stylesheet" href="{}">"#, url))
            .collect::<Vec<_>>()
            .join("\n    ");

        let script_type = if self.is_module { "module" } else { "text/javascript" };
        let scripts = self.scripts.iter()
            .map(|url| format!(r#"<script type="{}" src="{}"></script>"#, script_type, url))
            .collect::<Vec<_>>()
            .join("\n    ");

        format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    {}
</head>
<body>
    <div id="{}"></div>
    {}
</body>
</html>"#, styles, self.root_element, scripts)
    }
}
```

### 3.2 Amplify Integration Helpers

**File:** `src/server/chatgpt.rs`

```rust
/// Configuration for Amplify-hosted widgets
#[derive(Debug, Clone)]
pub struct AmplifyWidgetConfig {
    /// Amplify app ID
    pub app_id: String,
    /// Environment (e.g., "main", "dev")
    pub branch: String,
    /// Region
    pub region: String,
    /// Widget path within the app
    pub widget_path: String,
}

impl AmplifyWidgetConfig {
    /// Get the full CDN URL for the widget
    pub fn cdn_url(&self) -> String {
        format!(
            "https://{}.{}.amplifyapp.com{}",
            self.branch, self.app_id, self.widget_path
        )
    }

    /// Generate CSP for Amplify-hosted widgets
    pub fn default_csp(&self) -> WidgetCSP {
        WidgetCSP::new()
            .connect(&format!("https://{}.{}.amplifyapp.com", self.branch, self.app_id))
            .resources("https://*.amplifyapp.com")
            .resources("https://*.cloudfront.net")
    }
}
```

---

## Phase 4: Dual-Mode Widget Runtime

**Goal:** Create a JavaScript library that works with both MCP hosts and ChatGPT.

### 4.1 Widget Runtime Library

**File:** `widget-runtime/src/index.ts` (NEW package)

```typescript
/**
 * PMCP Widget Runtime
 *
 * Unified API for widgets that work in both MCP hosts and ChatGPT.
 */

export interface WidgetRuntime {
    /** Get tool output data */
    getData<T = unknown>(): T | null;

    /** Get response metadata (widget-only data) */
    getMeta<T = unknown>(): T | null;

    /** Get persisted widget state */
    getState<T = unknown>(): T | null;

    /** Set widget state (persisted by host) */
    setState<T>(state: T): void;

    /** Call an MCP tool */
    callTool<T = unknown>(name: string, args: Record<string, unknown>): Promise<T>;

    /** Check if running in ChatGPT */
    isOpenAI(): boolean;

    /** Check if running in standard MCP host */
    isMCP(): boolean;

    /** Get current theme */
    getTheme(): 'light' | 'dark';

    /** Get locale */
    getLocale(): string;

    /** Request display mode change */
    requestDisplayMode?(mode: 'compact' | 'expanded'): void;

    /** Notify host of content height */
    notifyHeight?(height: number): void;
}

class ChatGPTRuntime implements WidgetRuntime {
    private get openai() {
        return (window as any).openai;
    }

    getData<T>(): T | null {
        return this.openai?.toolOutput ?? null;
    }

    getMeta<T>(): T | null {
        return this.openai?.toolResponseMetadata ?? null;
    }

    getState<T>(): T | null {
        return this.openai?.widgetState ?? null;
    }

    setState<T>(state: T): void {
        this.openai?.setWidgetState(state);
    }

    async callTool<T>(name: string, args: Record<string, unknown>): Promise<T> {
        return this.openai?.callTool(name, args);
    }

    isOpenAI(): boolean {
        return true;
    }

    isMCP(): boolean {
        return false;
    }

    getTheme(): 'light' | 'dark' {
        return this.openai?.theme ?? 'light';
    }

    getLocale(): string {
        return this.openai?.locale ?? 'en-US';
    }

    requestDisplayMode(mode: 'compact' | 'expanded'): void {
        this.openai?.requestDisplayMode(mode);
    }

    notifyHeight(height: number): void {
        this.openai?.notifyIntrinsicHeight(height);
    }
}

class MCPRuntime implements WidgetRuntime {
    private data: unknown = null;
    private state: unknown = null;
    private pendingRequests = new Map<number, {
        resolve: (value: unknown) => void;
        reject: (error: Error) => void;
    }>();
    private nextId = 1;

    constructor() {
        window.addEventListener('message', this.handleMessage.bind(this));
    }

    private handleMessage(event: MessageEvent) {
        if (event.data.type === 'mcp-tool-result') {
            this.data = event.data.result;
            // Trigger re-render if using React/etc
            window.dispatchEvent(new CustomEvent('pmcp-data-update', {
                detail: this.data
            }));
        }

        if (event.data.type === 'mcp-response' && event.data.id) {
            const pending = this.pendingRequests.get(event.data.id);
            if (pending) {
                this.pendingRequests.delete(event.data.id);
                if (event.data.error) {
                    pending.reject(new Error(event.data.error.message));
                } else {
                    pending.resolve(event.data.result);
                }
            }
        }
    }

    getData<T>(): T | null {
        return this.data as T;
    }

    getMeta<T>(): T | null {
        // Standard MCP doesn't have separate meta
        return null;
    }

    getState<T>(): T | null {
        return this.state as T;
    }

    setState<T>(state: T): void {
        this.state = state;
        // Could persist to localStorage for MCP hosts
        try {
            localStorage.setItem('pmcp-widget-state', JSON.stringify(state));
        } catch {
            // localStorage not available
        }
    }

    async callTool<T>(name: string, args: Record<string, unknown>): Promise<T> {
        const id = this.nextId++;

        return new Promise((resolve, reject) => {
            this.pendingRequests.set(id, { resolve: resolve as any, reject });

            window.parent.postMessage({
                jsonrpc: '2.0',
                method: 'tools/call',
                params: { name, arguments: args },
                id
            }, '*');

            // Timeout after 30 seconds
            setTimeout(() => {
                if (this.pendingRequests.has(id)) {
                    this.pendingRequests.delete(id);
                    reject(new Error('Tool call timeout'));
                }
            }, 30000);
        });
    }

    isOpenAI(): boolean {
        return false;
    }

    isMCP(): boolean {
        return true;
    }

    getTheme(): 'light' | 'dark' {
        // Could detect from CSS or media query
        return window.matchMedia('(prefers-color-scheme: dark)').matches
            ? 'dark' : 'light';
    }

    getLocale(): string {
        return navigator.language;
    }
}

/**
 * Get the appropriate runtime for the current environment
 */
export function getRuntime(): WidgetRuntime {
    if (typeof window !== 'undefined' && (window as any).openai) {
        return new ChatGPTRuntime();
    }
    return new MCPRuntime();
}

// React hook (optional)
export function useWidgetRuntime(): WidgetRuntime {
    // Could add React-specific optimizations
    return getRuntime();
}

// Auto-initialize
export const runtime = getRuntime();
```

---

## Phase 5: cargo-pmcp CLI Extensions

**Goal:** Add ChatGPT App-specific commands.

### 5.1 New CLI Commands

**File:** `cargo-pmcp/src/commands/chatgpt.rs` (NEW)

```rust
//! ChatGPT Apps CLI commands

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum ChatGptCommand {
    /// Initialize ChatGPT App configuration
    Init {
        /// App name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Validate ChatGPT App manifest
    Validate {
        /// Path to manifest (default: .pmcp/chatgpt-app.toml)
        #[arg(short, long)]
        manifest: Option<String>,
    },

    /// Build widget bundle for deployment
    Build {
        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: String,

        /// Widget source directory
        #[arg(short, long, default_value = "widget")]
        source: String,
    },

    /// Generate CSP configuration from widget analysis
    Csp {
        /// Widget HTML file to analyze
        file: String,
    },
}
```

### 5.2 App Manifest

**File:** `.pmcp/chatgpt-app.toml` (template)

```toml
[app]
name = "My ChatGPT App"
description = "Description of my app"
version = "1.0.0"

[widget]
# MIME type: "skybridge" or "mcp"
type = "skybridge"

# Widget domain for CSP
domain = "https://chatgpt.com"

# Prefer border around widget
border = true

[widget.csp]
# Domains for fetch/XHR
connect = [
    "https://api.example.com"
]

# Domains for static assets
resources = [
    "https://cdn.example.com",
    "https://*.oaistatic.com"
]

[hosting]
# Where widgets are hosted: "inline", "s3", "amplify", "cloudflare"
type = "amplify"

# Amplify configuration (if type = "amplify")
[hosting.amplify]
app_id = "d1234567890"
branch = "main"
region = "us-east-1"

[tools.default]
# Default tool settings
widget_accessible = true
```

---

## Phase 6: Examples and Documentation

### 6.1 Chess Game Example

**File:** `examples/chatgpt_chess.rs`

A complete chess game with:
- Lambda backend (stateless)
- DynamoDB for game state
- CDN-hosted React widget
- Full ChatGPT Apps integration

### 6.2 Interactive Map Example

**File:** `examples/chatgpt_map.rs`

Interactive map with:
- Location selection
- Real-time updates
- State persistence

### 6.3 Documentation

- `docs/chatgpt-apps-guide.md` - Getting started guide
- `docs/widget-hosting-guide.md` - Hosting options
- `docs/migration-guide.md` - MCP to ChatGPT migration

---

## Testing Strategy

### Unit Tests

```
tests/
├── chatgpt_types_test.rs      # Type serialization
├── chatgpt_builders_test.rs   # Builder patterns
├── chatgpt_response_test.rs   # Response structure
└── widget_csp_test.rs         # CSP validation
```

### Integration Tests

```
tests/integration/
├── chatgpt_server_test.rs     # Full server flow
└── widget_hosting_test.rs     # CDN integration
```

### Property Tests

```rust
#[test]
fn proptest_widget_csp_serialization() {
    // CSP roundtrips through JSON correctly
}

#[test]
fn proptest_tool_meta_merging() {
    // Tool metadata merges correctly with existing _meta
}
```

---

## Rollout Plan

| Week | Phase | Deliverables |
|------|-------|--------------|
| 1 | Phase 0-1 | Feature flag, core types, extended CallToolResult |
| 2 | Phase 2 | Builder extensions, API refinement |
| 3 | Phase 3 | Widget hosting utilities |
| 4 | Phase 4 | Widget runtime library |
| 5 | Phase 5 | CLI commands |
| 6 | Phase 6 | Examples, documentation, release |

---

## Success Criteria

- [ ] All existing MCP tests pass (backward compatibility)
- [ ] ChatGPT Apps types serialize to OpenAI spec
- [ ] Example chess game works in ChatGPT
- [ ] Widget runtime works in both environments
- [ ] Documentation complete and reviewed
- [ ] cargo-pmcp chatgpt commands functional

---

## Open Questions

1. **Widget State Persistence**: Should we provide a DynamoDB helper for server-side state, or rely entirely on ChatGPT's widgetState?

2. **Streaming Support**: ChatGPT may support streaming tool responses in the future. How do we prepare?

3. **OAuth Integration**: Should we provide helpers for ChatGPT OAuth flows?

4. **Rate Limiting**: How do we handle ChatGPT's rate limits for tool calls from widgets?
