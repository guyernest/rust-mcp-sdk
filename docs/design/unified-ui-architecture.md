# Unified UI Architecture: MCP Apps, MCP-UI, and ChatGPT Apps

**Version:** 1.0
**Date:** 2025-01-11
**Status:** Draft

## Executive Summary

After analyzing three UI approaches for MCP servers—**MCP Apps (SEP-1865)**, **MCP-UI**, and **ChatGPT Apps**—this document proposes a **unified multi-layer architecture** that supports all platforms through a common abstraction with host-specific adapters.

The key insight is that **MCP-UI already solved this problem** with their adapter pattern. We should adopt a similar approach in PMCP SDK rather than implementing each platform separately.

---

## Landscape Analysis

### Three Approaches, One Goal

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MCP UI Ecosystem                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   MCP Apps (SEP-1865)         MCP-UI                    ChatGPT Apps        │
│   ─────────────────────       ──────                    ──────────────      │
│   Official MCP extension      Community standard        OpenAI-specific     │
│   Anthropic + OpenAI          mcpui.dev                 Apps SDK            │
│   + MCP-UI collab                                                           │
│                                                                              │
│   MIME: text/html+mcp         MIME: text/html           MIME: text/html+    │
│                                     text/uri-list             skybridge     │
│                                     remote-dom                              │
│                                                                              │
│   Comm: postMessage           Comm: postMessage         Comm: window.openai │
│         JSON-RPC                    UI Actions                  API         │
│                                                                              │
│   State: None specified       State: None built-in      State: widgetState  │
│                                                                (managed)     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       │ MCP-UI already bridges these!
                                       ▼
                          ┌─────────────────────────┐
                          │   Apps SDK Adapter      │
                          │   Translates between    │
                          │   postMessage ↔ openai  │
                          └─────────────────────────┘
```

### Comparison Matrix

| Feature | MCP Apps (SEP-1865) | MCP-UI | ChatGPT Apps |
|---------|---------------------|--------|--------------|
| **MIME Type** | `text/html+mcp` | `text/html`, `text/uri-list`, `application/vnd.mcp-ui.remote-dom` | `text/html+skybridge` |
| **URI Scheme** | `ui://` | `ui://` | `ui://widget/` |
| **Communication** | postMessage JSON-RPC | postMessage (UI Actions) | window.openai API |
| **Tool Response** | Standard MCP | Standard MCP + UIResource | structuredContent + _meta |
| **Action Types** | MCP tools/call | tool, prompt, intent, notify, link | callTool, sendFollowUpMessage |
| **State Management** | Not specified | Not built-in | widgetState (ChatGPT-managed) |
| **Metadata Namespace** | `ui/*` | Implicit | `openai/*` |
| **Hosts Supported** | Claude, generic MCP | Nanobot, MCPJam, ChatGPT* | ChatGPT only |
| **Remote DOM** | No | Yes (Shopify remote-dom) | No |
| **CSP Configuration** | Not specified | Not specified | `openai/widgetCSP` |

*ChatGPT support via Apps SDK adapter

### Key Insight: MCP-UI's Adapter Pattern

MCP-UI solves the multi-host problem elegantly:

```typescript
// Server creates ONE resource
const resource = createUIResource({
    uri: 'ui://my-tool/widget',
    content: { type: 'rawHtml', htmlString: '<html>...' },
    adapters: {
        appsSdk: { enabled: true }  // Enable ChatGPT translation
    }
});

// Client renders appropriately for host
<UIResourceRenderer
    resource={resource}
    onUIAction={handleAction}  // Unified action handling
/>
```

The Apps SDK adapter:
1. Injects a bridge script into HTML
2. Translates `postMessage` ↔ `window.openai` calls
3. Switches MIME type to `text/html+skybridge` for ChatGPT

---

## Proposed Architecture for PMCP SDK

### Multi-Layer Design

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PMCP SDK Architecture                               │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Layer 3: Builder API                            │ │
│  │                                                                         │ │
│  │   UIResourceBuilder::new("ui://my-tool/widget", "My Widget")           │ │
│  │       .html_template("<html>...")                                       │ │
│  │       .adapter(Adapter::ChatGpt { csp: ..., border: true })            │ │
│  │       .adapter(Adapter::McpApps)                                        │ │
│  │       .adapter(Adapter::McpUi { remote_dom: false })                   │ │
│  │       .build()                                                          │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                         │
│                                    ▼                                         │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Layer 2: Adapter Layer                          │ │
│  │                                                                         │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                 │ │
│  │  │ McpApps      │  │ ChatGptApps  │  │ McpUi        │                 │ │
│  │  │ Adapter      │  │ Adapter      │  │ Adapter      │                 │ │
│  │  │              │  │              │  │              │                 │ │
│  │  │ text/html+mcp│  │ text/html+   │  │ text/html    │                 │ │
│  │  │ postMessage  │  │ skybridge    │  │ postMessage  │                 │ │
│  │  │ JSON-RPC     │  │ window.openai│  │ UI Actions   │                 │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                 │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                         │
│                                    ▼                                         │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Layer 1: Core Types                             │ │
│  │                                                                         │ │
│  │   UIResource, UIResourceContents, UIAction, ToolResponse               │ │
│  │   (Host-agnostic abstractions)                                         │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Layer 1: Core Types (Host-Agnostic)

```rust
// src/types/ui_core.rs

/// Universal UI resource (works with any host)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub content: UIContent,
    pub metadata: UIMetadata,
}

/// Content types supported across platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIContent {
    /// Inline HTML (universal)
    Html { html: String },
    /// External URL (MCP-UI, some hosts)
    Url { url: String },
    /// Remote DOM (MCP-UI only)
    #[cfg(feature = "remote-dom")]
    RemoteDom {
        script: String,
        framework: RemoteDomFramework
    },
}

/// Universal UI actions (superset of all platforms)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UIAction {
    /// Call an MCP tool
    Tool {
        name: String,
        arguments: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    /// Send a message to the AI
    Prompt { text: String },
    /// High-level intent (MCP-UI)
    Intent {
        action: String,
        data: serde_json::Value
    },
    /// Notification (MCP-UI)
    Notify {
        level: NotifyLevel,
        message: String
    },
    /// Navigation (limited support)
    Link { url: String },
    /// State update (ChatGPT)
    SetState { state: serde_json::Value },
}

/// Universal metadata (merged from all platforms)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UIMetadata {
    /// Widget description
    pub description: Option<String>,
    /// Preferred dimensions
    pub dimensions: Option<UIDimensions>,
    /// Initial data to pass to widget
    pub initial_data: Option<serde_json::Value>,
    /// Content Security Policy (ChatGPT)
    pub csp: Option<ContentSecurityPolicy>,
    /// Additional host-specific metadata
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Tool response with UI support (universal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponseWithUI {
    /// Standard MCP content (always included)
    pub content: Vec<Content>,
    /// Whether this is an error
    pub is_error: bool,
    /// Structured content for model + widget (ChatGPT)
    pub structured_content: Option<serde_json::Value>,
    /// Widget-only metadata (ChatGPT)
    pub widget_meta: Option<serde_json::Map<String, serde_json::Value>>,
    /// Embedded UI resource (MCP-UI)
    pub ui_resource: Option<UIResource>,
}
```

### Layer 2: Adapter Layer

```rust
// src/adapters/mod.rs

/// Adapter trait for host-specific rendering
pub trait UIAdapter {
    /// Get the MIME type for this adapter
    fn mime_type(&self) -> &'static str;

    /// Transform UIResource for this host
    fn transform_resource(&self, resource: &UIResource) -> AdaptedResource;

    /// Transform UIAction for this host
    fn transform_action(&self, action: &UIAction) -> serde_json::Value;

    /// Generate bridge script (if needed)
    fn bridge_script(&self) -> Option<String>;

    /// Generate metadata for tool descriptor
    fn tool_metadata(&self, resource_uri: &str) -> serde_json::Map<String, serde_json::Value>;

    /// Generate metadata for resource
    fn resource_metadata(&self, metadata: &UIMetadata) -> serde_json::Map<String, serde_json::Value>;
}

/// Result of adapting a resource
pub struct AdaptedResource {
    pub uri: String,
    pub mime_type: String,
    pub content: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

#### MCP Apps Adapter (SEP-1865)

```rust
// src/adapters/mcp_apps.rs

/// Adapter for standard MCP Apps (SEP-1865)
pub struct McpAppsAdapter;

impl UIAdapter for McpAppsAdapter {
    fn mime_type(&self) -> &'static str {
        "text/html+mcp"
    }

    fn transform_resource(&self, resource: &UIResource) -> AdaptedResource {
        let html = match &resource.content {
            UIContent::Html { html } => {
                // Inject postMessage bridge
                inject_mcp_bridge(html)
            }
            UIContent::Url { url } => {
                // Create iframe loader
                format!(r#"<iframe src="{}" sandbox="allow-scripts"></iframe>"#, url)
            }
            #[cfg(feature = "remote-dom")]
            UIContent::RemoteDom { .. } => {
                // MCP Apps doesn't support remote-dom, provide fallback
                "<p>Remote DOM not supported in this host</p>".to_string()
            }
        };

        AdaptedResource {
            uri: resource.uri.clone(),
            mime_type: self.mime_type().to_string(),
            content: html,
            metadata: self.resource_metadata(&resource.metadata),
        }
    }

    fn bridge_script(&self) -> Option<String> {
        Some(MCP_APPS_BRIDGE_SCRIPT.to_string())
    }

    fn tool_metadata(&self, resource_uri: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert("ui/resourceUri".to_string(), json!(resource_uri));
        meta
    }

    fn resource_metadata(&self, _metadata: &UIMetadata) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()  // MCP Apps has minimal resource metadata
    }

    fn transform_action(&self, action: &UIAction) -> serde_json::Value {
        // MCP Apps uses JSON-RPC over postMessage
        match action {
            UIAction::Tool { name, arguments, message_id } => json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments },
                "id": message_id
            }),
            UIAction::SetState { state } => json!({
                "jsonrpc": "2.0",
                "method": "ui/setState",
                "params": { "state": state }
            }),
            _ => json!(null)  // Other actions not supported
        }
    }
}

const MCP_APPS_BRIDGE_SCRIPT: &str = r#"
<script>
(function() {
    const pending = new Map();
    let nextId = 1;

    window.mcpUI = {
        callTool: (name, args) => {
            return new Promise((resolve, reject) => {
                const id = nextId++;
                pending.set(id, { resolve, reject });
                window.parent.postMessage({
                    jsonrpc: '2.0',
                    method: 'tools/call',
                    params: { name, arguments: args },
                    id
                }, '*');
            });
        }
    };

    window.addEventListener('message', (event) => {
        if (event.data.type === 'mcp-tool-result') {
            // Handle tool result
            window.dispatchEvent(new CustomEvent('mcp-data', { detail: event.data.result }));
        }
        if (event.data.id && pending.has(event.data.id)) {
            const { resolve, reject } = pending.get(event.data.id);
            pending.delete(event.data.id);
            event.data.error ? reject(event.data.error) : resolve(event.data.result);
        }
    });
})();
</script>
"#;
```

#### ChatGPT Apps Adapter

```rust
// src/adapters/chatgpt.rs

/// Adapter for ChatGPT Apps (OpenAI Apps SDK)
pub struct ChatGptAdapter {
    pub csp: Option<ContentSecurityPolicy>,
    pub prefers_border: bool,
    pub domain: Option<String>,
    pub invoking_message: Option<String>,
    pub invoked_message: Option<String>,
}

impl Default for ChatGptAdapter {
    fn default() -> Self {
        Self {
            csp: None,
            prefers_border: true,
            domain: Some("https://chatgpt.com".to_string()),
            invoking_message: None,
            invoked_message: None,
        }
    }
}

impl UIAdapter for ChatGptAdapter {
    fn mime_type(&self) -> &'static str {
        "text/html+skybridge"
    }

    fn transform_resource(&self, resource: &UIResource) -> AdaptedResource {
        let html = match &resource.content {
            UIContent::Html { html } => {
                // ChatGPT injects window.openai automatically for skybridge
                // No bridge script needed from our side
                html.clone()
            }
            UIContent::Url { url } => {
                format!(r#"<iframe src="{}" style="width:100%;height:100%;border:none;"></iframe>"#, url)
            }
            #[cfg(feature = "remote-dom")]
            UIContent::RemoteDom { .. } => {
                "<p>Remote DOM not supported in ChatGPT</p>".to_string()
            }
        };

        AdaptedResource {
            uri: resource.uri.clone(),
            mime_type: self.mime_type().to_string(),
            content: html,
            metadata: self.resource_metadata(&resource.metadata),
        }
    }

    fn bridge_script(&self) -> Option<String> {
        // ChatGPT injects window.openai automatically
        // But we can provide a compatibility layer for code that uses mcpUI
        Some(CHATGPT_COMPAT_SCRIPT.to_string())
    }

    fn tool_metadata(&self, resource_uri: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();

        meta.insert("openai/outputTemplate".to_string(), json!(resource_uri));

        if let Some(msg) = &self.invoking_message {
            meta.insert("openai/toolInvocation/invoking".to_string(), json!(msg));
        }
        if let Some(msg) = &self.invoked_message {
            meta.insert("openai/toolInvocation/invoked".to_string(), json!(msg));
        }

        meta
    }

    fn resource_metadata(&self, metadata: &UIMetadata) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();

        meta.insert("openai/widgetPrefersBorder".to_string(), json!(self.prefers_border));

        if let Some(domain) = &self.domain {
            meta.insert("openai/widgetDomain".to_string(), json!(domain));
        }

        if let Some(desc) = &metadata.description {
            meta.insert("openai/widgetDescription".to_string(), json!(desc));
        }

        if let Some(csp) = &self.csp {
            meta.insert("openai/widgetCSP".to_string(), json!({
                "connect_domains": csp.connect_domains,
                "resource_domains": csp.resource_domains,
                "redirect_domains": csp.redirect_domains,
                "frame_domains": csp.frame_domains,
            }));
        }

        meta
    }

    fn transform_action(&self, action: &UIAction) -> serde_json::Value {
        // ChatGPT uses window.openai API directly
        match action {
            UIAction::Tool { name, arguments, .. } => json!({
                "type": "callTool",
                "name": name,
                "arguments": arguments
            }),
            UIAction::SetState { state } => json!({
                "type": "setWidgetState",
                "state": state
            }),
            UIAction::Prompt { text } => json!({
                "type": "sendFollowUpMessage",
                "message": text
            }),
            _ => json!(null)
        }
    }
}

const CHATGPT_COMPAT_SCRIPT: &str = r#"
<script>
// Compatibility layer: mcpUI API backed by window.openai
(function() {
    if (window.openai) {
        window.mcpUI = {
            callTool: (name, args) => window.openai.callTool(name, args),
            getData: () => window.openai.toolOutput,
            getMeta: () => window.openai.toolResponseMetadata,
            getState: () => window.openai.widgetState,
            setState: (state) => window.openai.setWidgetState(state),
            sendMessage: (msg) => window.openai.sendFollowUpMessage(msg),
        };
    }
})();
</script>
"#;
```

#### MCP-UI Adapter

```rust
// src/adapters/mcp_ui.rs

/// Adapter for MCP-UI hosts (Nanobot, MCPJam, etc.)
pub struct McpUiAdapter {
    pub enable_apps_sdk_bridge: bool,  // For ChatGPT compatibility
}

impl UIAdapter for McpUiAdapter {
    fn mime_type(&self) -> &'static str {
        "text/html"  // MCP-UI uses plain text/html
    }

    fn transform_resource(&self, resource: &UIResource) -> AdaptedResource {
        let html = match &resource.content {
            UIContent::Html { html } => {
                let mut result = inject_mcp_ui_bridge(html);
                if self.enable_apps_sdk_bridge {
                    result = inject_apps_sdk_adapter(&result);
                }
                result
            }
            UIContent::Url { url } => url.clone(),  // MCP-UI supports text/uri-list
            #[cfg(feature = "remote-dom")]
            UIContent::RemoteDom { script, framework } => {
                // MCP-UI native remote-dom support
                script.clone()
            }
        };

        let mime = match &resource.content {
            UIContent::Html { .. } => "text/html",
            UIContent::Url { .. } => "text/uri-list",
            #[cfg(feature = "remote-dom")]
            UIContent::RemoteDom { framework, .. } => {
                match framework {
                    RemoteDomFramework::React => "application/vnd.mcp-ui.remote-dom+javascript; framework=react",
                    RemoteDomFramework::WebComponents => "application/vnd.mcp-ui.remote-dom+javascript",
                }
            }
        };

        AdaptedResource {
            uri: resource.uri.clone(),
            mime_type: mime.to_string(),
            content: html,
            metadata: self.resource_metadata(&resource.metadata),
        }
    }

    fn transform_action(&self, action: &UIAction) -> serde_json::Value {
        // MCP-UI UI Actions format
        match action {
            UIAction::Tool { name, arguments, message_id } => json!({
                "type": "tool",
                "name": name,
                "params": arguments,
                "messageId": message_id
            }),
            UIAction::Prompt { text } => json!({
                "type": "prompt",
                "text": text
            }),
            UIAction::Intent { action, data } => json!({
                "type": "intent",
                "action": action,
                "data": data
            }),
            UIAction::Notify { level, message } => json!({
                "type": "notify",
                "level": format!("{:?}", level).to_lowercase(),
                "message": message
            }),
            UIAction::Link { url } => json!({
                "type": "link",
                "url": url
            }),
            UIAction::SetState { .. } => json!(null),  // Not supported in MCP-UI
        }
    }

    fn bridge_script(&self) -> Option<String> {
        Some(MCP_UI_BRIDGE_SCRIPT.to_string())
    }

    fn tool_metadata(&self, resource_uri: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        meta.insert("ui/resourceUri".to_string(), json!(resource_uri));
        meta
    }

    fn resource_metadata(&self, _metadata: &UIMetadata) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }
}
```

### Layer 3: Builder API

```rust
// src/server/ui_builder.rs

/// Unified UI Resource Builder
///
/// Creates UI resources that work across all supported platforms.
///
/// # Example
///
/// ```rust
/// use pmcp::server::UIResourceBuilder;
/// use pmcp::adapters::{ChatGptAdapter, McpAppsAdapter};
///
/// let resource = UIResourceBuilder::new("ui://my-tool/widget", "My Widget")
///     .html_template("<html>...</html>")
///     .description("Interactive widget for my tool")
///     .csp(ContentSecurityPolicy::new()
///         .connect("https://api.example.com"))
///     .adapter(ChatGptAdapter::default()
///         .invoking_message("Loading...")
///         .invoked_message("Ready!"))
///     .adapter(McpAppsAdapter)
///     .build()?;
///
/// // Get adapted output for specific host
/// let chatgpt_output = resource.for_chatgpt();
/// let mcp_apps_output = resource.for_mcp_apps();
/// ```
pub struct UIResourceBuilder {
    uri: String,
    name: String,
    description: Option<String>,
    content: Option<UIContent>,
    metadata: UIMetadata,
    adapters: Vec<Box<dyn UIAdapter>>,
}

impl UIResourceBuilder {
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            content: None,
            metadata: UIMetadata::default(),
            adapters: Vec::new(),
        }
    }

    pub fn html_template(mut self, html: impl Into<String>) -> Self {
        self.content = Some(UIContent::Html { html: html.into() });
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.content = Some(UIContent::Url { url: url.into() });
        self
    }

    #[cfg(feature = "remote-dom")]
    pub fn remote_dom(mut self, script: impl Into<String>, framework: RemoteDomFramework) -> Self {
        self.content = Some(UIContent::RemoteDom {
            script: script.into(),
            framework,
        });
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self.metadata.description = self.description.clone();
        self
    }

    pub fn csp(mut self, csp: ContentSecurityPolicy) -> Self {
        self.metadata.csp = Some(csp);
        self
    }

    pub fn initial_data(mut self, data: serde_json::Value) -> Self {
        self.metadata.initial_data = Some(data);
        self
    }

    /// Add an adapter for a specific host
    pub fn adapter<A: UIAdapter + 'static>(mut self, adapter: A) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    /// Convenience: add ChatGPT adapter with defaults
    pub fn chatgpt(self) -> Self {
        self.adapter(ChatGptAdapter::default())
    }

    /// Convenience: add MCP Apps adapter
    pub fn mcp_apps(self) -> Self {
        self.adapter(McpAppsAdapter)
    }

    /// Convenience: add MCP-UI adapter
    pub fn mcp_ui(self) -> Self {
        self.adapter(McpUiAdapter { enable_apps_sdk_bridge: false })
    }

    /// Convenience: add all adapters
    pub fn all_adapters(self) -> Self {
        self.chatgpt().mcp_apps().mcp_ui()
    }

    pub fn build(self) -> Result<MultiPlatformUIResource> {
        let content = self.content.ok_or_else(|| {
            Error::validation("UI content must be set")
        })?;

        let core_resource = UIResource {
            uri: self.uri,
            name: self.name,
            description: self.description,
            content,
            metadata: self.metadata,
        };

        // If no adapters specified, default to MCP Apps
        let adapters = if self.adapters.is_empty() {
            vec![Box::new(McpAppsAdapter) as Box<dyn UIAdapter>]
        } else {
            self.adapters
        };

        Ok(MultiPlatformUIResource {
            core: core_resource,
            adapters,
        })
    }
}

/// UI Resource with multiple platform outputs
pub struct MultiPlatformUIResource {
    core: UIResource,
    adapters: Vec<Box<dyn UIAdapter>>,
}

impl MultiPlatformUIResource {
    /// Get output for ChatGPT Apps
    pub fn for_chatgpt(&self) -> Option<AdaptedResource> {
        self.adapters.iter()
            .find(|a| a.mime_type() == "text/html+skybridge")
            .map(|a| a.transform_resource(&self.core))
    }

    /// Get output for MCP Apps (SEP-1865)
    pub fn for_mcp_apps(&self) -> Option<AdaptedResource> {
        self.adapters.iter()
            .find(|a| a.mime_type() == "text/html+mcp")
            .map(|a| a.transform_resource(&self.core))
    }

    /// Get output for MCP-UI hosts
    pub fn for_mcp_ui(&self) -> Option<AdaptedResource> {
        self.adapters.iter()
            .find(|a| a.mime_type() == "text/html" || a.mime_type().starts_with("application/vnd.mcp-ui"))
            .map(|a| a.transform_resource(&self.core))
    }

    /// Get output for detected/preferred host
    pub fn for_host(&self, host: HostType) -> Option<AdaptedResource> {
        match host {
            HostType::ChatGpt => self.for_chatgpt(),
            HostType::Claude => self.for_mcp_apps(),
            HostType::Nanobot | HostType::McpJam => self.for_mcp_ui(),
            HostType::Generic => self.for_mcp_apps(),
        }
    }

    /// Get tool metadata for all adapters (merged)
    pub fn tool_metadata(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut meta = serde_json::Map::new();
        for adapter in &self.adapters {
            for (k, v) in adapter.tool_metadata(&self.core.uri) {
                meta.insert(k, v);
            }
        }
        meta
    }
}

/// Known MCP host types
#[derive(Debug, Clone, Copy)]
pub enum HostType {
    ChatGpt,
    Claude,
    Nanobot,
    McpJam,
    Generic,
}
```

---

## Usage Examples

### Example 1: Simple Widget (All Platforms)

```rust
use pmcp::server::UIResourceBuilder;

// One resource definition works everywhere
let resource = UIResourceBuilder::new("ui://calculator/widget", "Calculator")
    .html_template(include_str!("calculator.html"))
    .description("Simple calculator widget")
    .all_adapters()  // Support ChatGPT, MCP Apps, MCP-UI
    .build()?;

// Tool handler returns appropriate format
async fn calculator_tool(args: CalcArgs, extra: RequestHandlerExtra) -> Result<CallToolResult> {
    let result = calculate(args)?;

    // Return with structured content for ChatGPT, standard for others
    Ok(CallToolResult {
        content: vec![Content::Text { text: format!("Result: {}", result) }],
        structured_content: Some(json!({ "result": result })),
        ..Default::default()
    })
}
```

### Example 2: ChatGPT-Specific Configuration

```rust
use pmcp::server::UIResourceBuilder;
use pmcp::adapters::ChatGptAdapter;

let resource = UIResourceBuilder::new("ui://chess/board", "Chess Board")
    .html_template(include_str!("chess.html"))
    .csp(ContentSecurityPolicy::new()
        .connect("https://api.chess.com")
        .resources("https://cdn.chess.com"))
    .adapter(ChatGptAdapter::default()
        .invoking_message("Setting up the board...")
        .invoked_message("Your move!"))
    .adapter(McpAppsAdapter)  // Also support standard MCP
    .build()?;
```

### Example 3: CDN-Hosted Widget

```rust
use pmcp::server::UIResourceBuilder;

let resource = UIResourceBuilder::new("ui://dashboard/main", "Dashboard")
    .url("https://cdn.example.com/widgets/dashboard/v1.0.0/index.html")
    .all_adapters()
    .build()?;
```

---

## Widget Runtime Library

### Unified JavaScript API

```typescript
// @pmcp/widget-runtime

interface PMCPRuntime {
    // Data access (works everywhere)
    getData<T>(): T | null;
    getMeta<T>(): T | null;

    // State (ChatGPT-managed, localStorage fallback elsewhere)
    getState<T>(): T | null;
    setState<T>(state: T): void;

    // Actions
    callTool<T>(name: string, args: object): Promise<T>;
    sendMessage(text: string): void;

    // Context
    getTheme(): 'light' | 'dark';
    getLocale(): string;
    getHostType(): 'chatgpt' | 'claude' | 'nanobot' | 'generic';
}

// Auto-detects environment
export const runtime: PMCPRuntime = detectRuntime();

// React hook
export function usePMCPRuntime(): PMCPRuntime;
```

### Usage in Widget

```typescript
import { runtime, usePMCPRuntime } from '@pmcp/widget-runtime';

function ChessBoard() {
    const pmcp = usePMCPRuntime();
    const gameData = pmcp.getData<GameState>();
    const [uiState, setUiState] = useState(pmcp.getState() ?? { selected: null });

    const makeMove = async (from: string, to: string) => {
        const result = await pmcp.callTool('chess_move', { from, to });
        // UI updates automatically via getData()
    };

    // Persist UI state
    useEffect(() => {
        pmcp.setState(uiState);
    }, [uiState]);

    return <Board data={gameData} onMove={makeMove} />;
}
```

---

## Comparison: Unified vs Separate Implementation

### Option A: Separate Implementations (NOT Recommended)

```
❌ Drawbacks:
- Duplicate code for each platform
- Inconsistent APIs
- Breaking changes when new platforms emerge
- Users must choose platform at design time
- Widget code tied to specific platform
```

### Option B: Unified Multi-Layer (RECOMMENDED)

```
✅ Benefits:
- Single codebase, multiple outputs
- Consistent developer experience
- New platforms = new adapter (no breaking changes)
- Widget code works everywhere
- Feature detection at runtime
- Follows MCP-UI's proven pattern
```

---

## Migration Path

### From Current PMCP SDK

```rust
// Before (current implementation)
let resource = UIResourceBuilder::new("ui://test", "Test")
    .html_template("<html>...")
    .build()?;  // Only produces text/html+mcp

// After (unified)
let resource = UIResourceBuilder::new("ui://test", "Test")
    .html_template("<html>...")
    .all_adapters()  // NEW: Support all platforms
    .build()?;

// Backward compatible: without all_adapters(), defaults to MCP Apps
```

### From MCP-UI

Users of MCP-UI can continue using their existing code. PMCP SDK's adapter layer is compatible:

```rust
// PMCP SDK can consume MCP-UI resources
let mcp_ui_resource: serde_json::Value = // from MCP-UI server
let pmcp_resource = UIResource::from_mcp_ui(mcp_ui_resource)?;
```

---

## Conclusion

**Recommendation: Implement the unified multi-layer architecture.**

This approach:
1. Learns from MCP-UI's successful adapter pattern
2. Supports all three platforms (MCP Apps, ChatGPT Apps, MCP-UI)
3. Provides a clean, consistent API for PMCP SDK users
4. Enables future platform support without breaking changes
5. Simplifies widget development with unified runtime

The additional complexity of the adapter layer is justified by:
- Reduced long-term maintenance
- Better developer experience
- Platform-agnostic widget code
- Alignment with community standards

---

## References

- [MCP Apps (SEP-1865)](https://blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps/)
- [MCP-UI Documentation](https://mcpui.dev)
- [ChatGPT Apps SDK](https://developers.openai.com/apps-sdk/)
- [MCP-UI GitHub](https://github.com/MCP-UI-Org/mcp-ui)
- [WorkOS MCP-UI Technical Overview](https://workos.com/blog/mcp-ui-a-technical-deep-dive-into-interactive-agent-interfaces)
