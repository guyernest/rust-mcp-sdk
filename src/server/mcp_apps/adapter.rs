// Allow doc_markdown since this module has many technical terms (ChatGPT, MCP-UI, etc.)
#![allow(clippy::doc_markdown)]

//! UI Adapter implementations for different MCP host platforms.

use crate::types::mcp_apps::{ExtendedUIMimeType, HostType, WidgetCSP, WidgetMeta};
use serde_json::Value;
use std::collections::HashMap;

/// Trait for adapting UI resources to specific MCP host platforms.
///
/// Adapters transform HTML content into platform-specific formats,
/// enabling a single tool implementation to work across multiple hosts.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::mcp_apps::{UIAdapter, ChatGptAdapter};
///
/// let adapter = ChatGptAdapter::new();
/// let html = "<html><body>Chess Board</body></html>";
/// let transformed = adapter.transform("ui://chess/board", "Chess Board", html);
/// ```
pub trait UIAdapter: Send + Sync {
    /// Get the host type this adapter targets.
    fn host_type(&self) -> HostType;

    /// Get the MIME type this adapter produces.
    fn mime_type(&self) -> ExtendedUIMimeType;

    /// Transform HTML content for this host platform.
    ///
    /// Returns the transformed resource with platform-specific metadata.
    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource;

    /// Inject platform-specific communication bridge into HTML content.
    ///
    /// For ChatGPT Apps, this wraps the HTML with `window.openai` bridge code.
    /// For MCP-UI, this adds postMessage bridge code.
    /// For MCP Apps, returns HTML unchanged — widgets use the ext-apps SDK.
    ///
    /// Prefer calling `transform()` instead of this method directly.
    fn inject_bridge(&self, html: &str) -> String {
        html.to_string()
    }

    /// Get CSP headers required by this platform.
    fn required_csp(&self) -> Option<WidgetCSP>;
}

/// A UI resource transformed for a specific host platform.
#[derive(Debug, Clone)]
pub struct TransformedResource {
    /// Original resource URI.
    pub uri: String,
    /// Display name.
    pub name: String,
    /// MIME type for this platform.
    pub mime_type: ExtendedUIMimeType,
    /// Transformed HTML content (may include platform bridge, or raw HTML
    /// for SDK-based platforms like MCP Apps).
    pub content: String,
    /// Platform-specific metadata.
    pub metadata: HashMap<String, Value>,
}

impl TransformedResource {
    /// Take platform metadata as the `_meta` map format used by `Content::Resource`.
    ///
    /// Returns `None` if metadata is empty, `Some(map)` otherwise.
    /// Drains the metadata from this resource.
    pub fn take_meta(&mut self) -> Option<serde_json::Map<String, Value>> {
        if self.metadata.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.metadata).into_iter().collect())
        }
    }
}

/// Inline a lightweight `App` shim to replace CDN ext-apps SDK imports.
///
/// Hosts like Claude Desktop run widgets in Electron iframes that block
/// external script loading from CDNs (esm.sh, jsdelivr, etc.). This replaces
/// `import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps..."` with
/// a self-contained inline implementation of the `App` class (~1.5KB).
///
/// The shim implements the core ext-apps API surface used by widgets:
/// - `new App(info, caps)` / `app.connect()` / `app.getHostContext()`
/// - `app.ontoolresult` / `app.onhostcontextchanged` callbacks
/// - `app.callServerTool({ name, arguments })`
///
/// For mcp-preview, a separate import-map redirect in `index.html` covers
/// all CDN providers by redirecting to a local bundle with full SDK features.
pub fn inline_ext_apps_shim(html: &str) -> std::borrow::Cow<'_, str> {
    // Match CDN import patterns for @modelcontextprotocol/ext-apps.
    // NOTE: keep in sync with cdnPattern regex in crates/mcp-preview/assets/index.html
    const CDN_MARKERS: &[&str] = &[
        "esm.sh/@modelcontextprotocol/ext-apps",
        "cdn.jsdelivr.net/npm/@modelcontextprotocol/ext-apps",
        "unpkg.com/@modelcontextprotocol/ext-apps",
        "cdn.skypack.dev/@modelcontextprotocol/ext-apps",
    ];

    // Find the import statement: `import ... from "https://<CDN>/@modelcontextprotocol/ext-apps...";`
    // We need to find the full line from `import` to the closing `;`
    let Some((import_start, import_end)) = find_cdn_import(html, CDN_MARKERS) else {
        return std::borrow::Cow::Borrowed(html);
    };

    let mut result = String::with_capacity(html.len() + EXT_APPS_SHIM.len());
    result.push_str(&html[..import_start]);
    result.push_str(EXT_APPS_SHIM);
    result.push_str(&html[import_end..]);
    std::borrow::Cow::Owned(result)
}

/// Find the byte range of a CDN import statement in HTML.
///
/// Looks for `import ... from "https://<CDN_MARKER>..."` and returns
/// the (start, end) byte offsets spanning from `import` to the trailing `;`
/// or end of line.
fn find_cdn_import(html: &str, markers: &[&str]) -> Option<(usize, usize)> {
    markers
        .iter()
        .find_map(|marker| find_cdn_import_for_marker(html, marker))
}

/// Try to locate a CDN import for one specific marker. Returns the byte
/// range `(import_kw_start, end_after_terminator)` if all four anchor
/// points (marker → opening quote → `import` keyword on same line →
/// closing quote) are found.
fn find_cdn_import_for_marker(html: &str, marker: &str) -> Option<(usize, usize)> {
    let marker_pos = html.find(marker)?;
    let import_kw = locate_import_keyword_for_marker(html, marker_pos)?;
    let end = compute_import_end(html, marker_pos, marker.len());
    Some((import_kw, end))
}

/// Walk backward from `marker_pos` to find the start of the `import`
/// keyword on the same line that opens the URL with `"` or `'`.
fn locate_import_keyword_for_marker(html: &str, marker_pos: usize) -> Option<usize> {
    let before = &html[..marker_pos];
    let qp = before.rfind(['"', '\''])?;
    // Bound the backward search to the current line to avoid matching an
    // unrelated `import` keyword from an earlier statement.
    let line_start = html[..qp].rfind('\n').map_or(0, |i| i + 1);
    let on_line = &html[line_start..qp];
    on_line.rfind("import").map(|rel| line_start + rel)
}

/// Compute the end byte offset of an import statement: starting from the
/// closing quote, advance past any trailing `;`, space, or tab.
fn compute_import_end(html: &str, marker_pos: usize, marker_len: usize) -> usize {
    let after_marker = &html[marker_pos + marker_len..];
    let close_quote = after_marker
        .find(['"', '\''])
        .map_or(html.len(), |i| marker_pos + marker_len + i + 1);
    let mut end = close_quote;
    while end < html.len() && matches!(html.as_bytes()[end], b';' | b' ' | b'\t') {
        end += 1;
    }
    end
}

/// Minimal inline implementation of the `@modelcontextprotocol/ext-apps` `App` class.
///
/// Implements the JSON-RPC 2.0 postMessage protocol used by MCP hosts:
/// - `ui/initialize` handshake with `hostContext` delivery
/// - `ui/toolResult` and `ui/hostContextChanged` notifications
/// - `tools/call` proxy for widget-initiated tool calls
const EXT_APPS_SHIM: &str = r"
// Inline ext-apps App shim (replaces CDN import for hosts that block external scripts)
const _extPending = new Map();
let _extNextId = 1;
let _extApp = null;

function _extSend(method, params) {
  return new Promise((resolve, reject) => {
    const id = _extNextId++;
    const timer = setTimeout(() => { _extPending.delete(id); reject(new Error('Timeout')); }, 30000);
    _extPending.set(id, { resolve, reject, timer });
    window.parent.postMessage({ jsonrpc: '2.0', id, method, params }, '*');
  });
}

window.addEventListener('message', (e) => {
  const d = e.data;
  if (!d || d.jsonrpc !== '2.0') return;
  if (typeof d.id === 'number' && !d.method) {
    const p = _extPending.get(d.id);
    if (p) { _extPending.delete(d.id); clearTimeout(p.timer); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); }
    return;
  }
  if (!_extApp) return;
  if (d.method === 'ui/toolResult' || d.method === 'ui/notifications/tool-result') {
    if (_extApp.ontoolresult) _extApp.ontoolresult(d.params);
  } else if (d.method === 'ui/hostContextChanged' || d.method === 'ui/notifications/host-context-changed') {
    if (d.params) _extApp._hc = Object.assign(_extApp._hc || {}, d.params);
    if (_extApp.onhostcontextchanged) _extApp.onhostcontextchanged(_extApp._hc);
  }
});

class App {
  constructor(info, caps) {
    this._info = info || {};
    this._caps = caps || {};
    this._hc = undefined;
    this.ontoolresult = null;
    this.onhostcontextchanged = null;
    this.onerror = null;
    _extApp = this;
  }
  async connect() {
    try {
      const r = await Promise.race([
        _extSend('ui/initialize', { appInfo: { name: this._info.name || 'widget', version: this._info.version || '1.0.0' }, appCapabilities: this._caps || {}, protocolVersion: '2026-01-26' }),
        new Promise(resolve => setTimeout(() => resolve(null), 2000))
      ]);
      if (r && r.hostContext) this._hc = r.hostContext;
    } catch (_) { /* host may not support ui/initialize */ }
  }
  getHostContext() { return this._hc; }
  async callServerTool(params) { return _extSend('tools/call', params); }
}
";

/// Inject a script block into HTML, preferring before `</head>`.
///
/// Tries three strategies in order:
/// 1. Insert before `</head>` if present
/// 2. Wrap in `<head>` and insert before `<body` if present
/// 3. Prepend to the HTML
fn inject_script_into_head(html: &str, script: &str) -> String {
    if html.contains("</head>") {
        html.replace("</head>", &format!("{script}</head>"))
    } else if html.contains("<body") {
        let pos = html.find("<body").unwrap_or(0);
        format!("{}<head>{}</head>{}", &html[..pos], script, &html[pos..])
    } else {
        format!("{script}{html}")
    }
}

/// Adapter for ChatGPT Apps (OpenAI Apps SDK).
///
/// Transforms resources to use `text/html;profile=mcp-app` MIME type and
/// injects the `window.openai` bridge for widget communication.
#[derive(Debug, Clone, Default)]
pub struct ChatGptAdapter {
    /// Optional widget metadata for ChatGPT.
    pub widget_meta: Option<WidgetMeta>,
}

impl ChatGptAdapter {
    /// Create a new ChatGPT adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set widget metadata for this adapter.
    #[must_use]
    pub fn with_widget_meta(mut self, meta: WidgetMeta) -> Self {
        self.widget_meta = Some(meta);
        self
    }
}

impl UIAdapter for ChatGptAdapter {
    fn host_type(&self) -> HostType {
        HostType::ChatGpt
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        ExtendedUIMimeType::HtmlSkybridge
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        let injected_html = self.inject_bridge(html);

        // Build ChatGPT descriptor metadata.
        // Start with any descriptor keys from widget_meta (e.g., openai/widgetAccessible),
        // then ensure openai/outputTemplate is always set from the resource URI.
        let mut metadata: HashMap<String, Value> = self
            .widget_meta
            .as_ref()
            .map(|wm| {
                crate::types::ui::filter_to_descriptor_keys(&wm.to_meta_map())
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();

        // ChatGptAdapter always emits openai/outputTemplate from the resource URI
        metadata
            .entry("openai/outputTemplate".to_string())
            .or_insert_with(|| Value::String(uri.to_string()));

        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content: injected_html,
            metadata,
        }
    }

    fn inject_bridge(&self, html: &str) -> String {
        // ChatGPT Apps bridge script - Full OpenAI Apps SDK alignment
        let bridge_script = r#"
<script>
// ChatGPT Apps Bridge - Full window.openai API wrapper
// Aligned with OpenAI Apps SDK documentation
(function() {
    'use strict';

    // State management
    var widgetState = {};
    var _toolOutput = null;

    // Initialize bridge when ready
    if (window.openai) {
        initBridge();
    } else {
        window.addEventListener('load', function() {
            if (window.openai) initBridge();
        });
    }

    function initBridge() {
        // Notify ChatGPT that widget is ready
        window.openai.onWidgetReady?.();

        // Listen for state updates from ChatGPT
        window.openai.onStateUpdate?.(function(newState) {
            widgetState = Object.assign({}, widgetState, newState);
            window.dispatchEvent(new CustomEvent('widgetStateUpdate', { detail: widgetState }));
        });
    }

    // Listen for tool results via postMessage from ChatGPT host
    window.addEventListener('message', function(event) {
        if (event.source !== window.parent) return;
        var msg = event.data;
        if (!msg || msg.jsonrpc !== '2.0') return;
        if (msg.method === 'ui/toolResult' || msg.method === 'ui/notifications/tool-result') {
            var data = msg.params && msg.params.structuredContent;
            if (data) {
                _toolOutput = data;
                window.dispatchEvent(new CustomEvent('widgetStateUpdate', {
                    detail: { toolOutput: data }
                }));
            }
        }
    }, { passive: true });

    // Listen for openai:set_globals (alternate data delivery)
    window.addEventListener('openai:set_globals', function(event) {
        var data = event.detail && event.detail.globals && event.detail.globals.toolOutput;
        if (data) {
            _toolOutput = data;
            window.dispatchEvent(new CustomEvent('widgetStateUpdate', {
                detail: { toolOutput: data }
            }));
        }
    }, { passive: true });

    // Expose bridge API - aligned with window.openai
    window.mcpBridge = {
        // ========================================
        // Core Operations
        // ========================================

        // Call an MCP tool
        callTool: async (name, args) => {
            if (window.openai?.callTool) {
                return window.openai.callTool(name, args);
            }
            throw new Error('ChatGPT bridge not available');
        },

        // ========================================
        // State Management
        // ========================================

        // Get current widget state
        getState: () => window.openai?.widgetState ?? widgetState,

        // Update widget state
        setState: (newState) => {
            widgetState = { ...widgetState, ...newState };
            window.openai?.setWidgetState?.(widgetState);
        },

        // ========================================
        // Tool Context (Read-only)
        // ========================================

        // Arguments supplied when the tool was invoked
        get toolInput() {
            return window.openai?.toolInput ?? {};
        },

        // The structuredContent returned by the tool
        get toolOutput() {
            return _toolOutput ?? window.openai?.toolOutput;
        },

        // The _meta payload (widget-only, never sent to model)
        get toolResponseMetadata() {
            return window.openai?.toolResponseMetadata ?? {};
        },

        // ========================================
        // Communication
        // ========================================

        // Send follow-up message
        sendMessage: (message) => {
            window.openai?.sendFollowUpMessage?.({ prompt: message });
        },

        // Open external URL
        openExternal: (url) => {
            window.openai?.openExternal?.({ href: url });
        },

        // ========================================
        // File Operations
        // ========================================

        // Upload a file and get a file ID
        uploadFile: async (file) => {
            if (window.openai?.uploadFile) {
                return window.openai.uploadFile(file);
            }
            return null;
        },

        // Get a temporary download URL for a file
        getFileDownloadUrl: async (fileId) => {
            if (window.openai?.getFileDownloadUrl) {
                return window.openai.getFileDownloadUrl({ fileId });
            }
            return null;
        },

        // ========================================
        // Display Modes
        // ========================================

        // Request a display mode change (inline, pip, fullscreen)
        requestDisplayMode: async (mode) => {
            if (window.openai?.requestDisplayMode) {
                return window.openai.requestDisplayMode({ mode });
            }
        },

        // Close the widget
        requestClose: () => {
            window.openai?.requestClose?.();
        },

        // Report the widget's intrinsic height
        notifyIntrinsicHeight: (height) => {
            window.openai?.notifyIntrinsicHeight?.(height);
        },

        // Set the URL for the "Open in App" button
        setOpenInAppUrl: (href) => {
            window.openai?.setOpenInAppUrl?.({ href });
        },

        // ========================================
        // Environment Context (Read-only)
        // ========================================

        // Current theme ('light' or 'dark')
        get theme() {
            return window.openai?.theme ?? 'light';
        },

        // Current locale (e.g., 'en-US')
        get locale() {
            return window.openai?.locale ?? 'en-US';
        },

        // Current display mode ('inline', 'pip', 'fullscreen')
        get displayMode() {
            return window.openai?.displayMode ?? 'inline';
        },

        // Maximum widget height in pixels
        get maxHeight() {
            return window.openai?.maxHeight;
        },

        // Safe area insets
        get safeArea() {
            return window.openai?.safeArea;
        },

        // User agent string
        get userAgent() {
            return window.openai?.userAgent;
        },

        // Widget view type ('default' or 'compact')
        get view() {
            return window.openai?.view ?? 'default';
        }
    };

    // Dispatch ready event for widgets waiting on bridge
    window.dispatchEvent(new Event('mcpBridgeReady'));
})();
</script>
"#;

        inject_script_into_head(html, bridge_script)
    }

    fn required_csp(&self) -> Option<WidgetCSP> {
        // ChatGPT has its own CSP management
        None
    }
}

/// Adapter for MCP Apps (SEP-1865 standard).
///
/// Transforms resources to use `text/html;profile=mcp-app` MIME type.
/// Widgets use the `@modelcontextprotocol/ext-apps` SDK for host communication.
#[derive(Debug, Clone, Default)]
pub struct McpAppsAdapter {
    /// Optional Content Security Policy.
    pub csp: Option<WidgetCSP>,
}

impl McpAppsAdapter {
    /// Create a new MCP Apps adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set CSP for this adapter.
    #[must_use]
    pub fn with_csp(mut self, csp: WidgetCSP) -> Self {
        self.csp = Some(csp);
        self
    }
}

impl UIAdapter for McpAppsAdapter {
    fn host_type(&self) -> HostType {
        HostType::Generic
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        // Use the official MCP Apps MIME type: text/html;profile=mcp-app
        // (matches @modelcontextprotocol/ext-apps RESOURCE_MIME_TYPE)
        ExtendedUIMimeType::HtmlMcpApp
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        // Inline the ext-apps SDK if the widget imports from a CDN, making it
        // self-contained for hosts that block external script loading (Claude Desktop).
        // See GUIDE.md for recommended patterns.
        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content: inline_ext_apps_shim(html).into_owned(),
            metadata: HashMap::new(),
        }
    }

    // inject_bridge: uses default (no-op) — widgets use ext-apps SDK.

    fn required_csp(&self) -> Option<WidgetCSP> {
        self.csp.clone()
    }
}

/// Adapter for MCP-UI (community standard).
///
/// Supports multiple output formats including HTML, URLs, and Remote DOM.
#[derive(Debug, Clone, Default)]
pub struct McpUiAdapter {
    /// Preferred output format.
    pub preferred_format: McpUiFormat,
}

/// MCP-UI output formats.
#[derive(Debug, Clone, Default)]
pub enum McpUiFormat {
    /// HTML with postMessage bridge.
    #[default]
    Html,
    /// URL reference (for CDN-hosted widgets).
    Url,
    /// Remote DOM (for non-iframe rendering).
    RemoteDom,
}

impl McpUiAdapter {
    /// Create a new MCP-UI adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set preferred output format.
    #[must_use]
    pub fn with_format(mut self, format: McpUiFormat) -> Self {
        self.preferred_format = format;
        self
    }
}

impl UIAdapter for McpUiAdapter {
    fn host_type(&self) -> HostType {
        HostType::Nanobot // MCP-UI hosts like Nanobot
    }

    fn mime_type(&self) -> ExtendedUIMimeType {
        match self.preferred_format {
            McpUiFormat::Html => ExtendedUIMimeType::HtmlPlain,
            McpUiFormat::Url => ExtendedUIMimeType::UriList,
            McpUiFormat::RemoteDom => ExtendedUIMimeType::RemoteDom,
        }
    }

    fn transform(&self, uri: &str, name: &str, html: &str) -> TransformedResource {
        let content = match self.preferred_format {
            McpUiFormat::Html => self.inject_bridge(html),
            McpUiFormat::Url => uri.to_string(),
            McpUiFormat::RemoteDom => {
                format!(
                    r#"{{"type":"remote-dom","uri":"{}","name":"{}"}}"#,
                    uri, name
                )
            },
        };

        TransformedResource {
            uri: uri.to_string(),
            name: name.to_string(),
            mime_type: self.mime_type(),
            content,
            metadata: HashMap::new(),
        }
    }

    fn inject_bridge(&self, html: &str) -> String {
        // MCP-UI bridge script
        let bridge_script = r"
<script>
// MCP-UI Bridge
(function() {
    'use strict';

    let requestId = 0;
    const pendingRequests = new Map();

    window.addEventListener('message', (event) => {
        const msg = event.data;
        if (msg.jsonrpc !== '2.0') return;

        if (msg.id !== undefined && pendingRequests.has(msg.id)) {
            const pending = pendingRequests.get(msg.id);
            pendingRequests.delete(msg.id);
            clearTimeout(pending.timer);
            if (msg.error) pending.reject(new Error(msg.error.message));
            else pending.resolve(msg.result);
        }

        if (msg.method && !msg.id) {
            window.dispatchEvent(new CustomEvent('mcpNotification', { detail: msg }));
        }
    });

    function sendRequest(method, params) {
        return new Promise((resolve, reject) => {
            const id = ++requestId;
            const timer = setTimeout(() => {
                if (pendingRequests.has(id)) {
                    pendingRequests.delete(id);
                    reject(new Error('Request timeout'));
                }
            }, 30000);
            pendingRequests.set(id, { resolve, reject, timer });
            window.parent.postMessage({ jsonrpc: '2.0', id, method, params }, '*');
        });
    }

    // MCP-UI specific actions
    window.mcpBridge = {
        callTool: (name, args) => sendRequest('tools/call', { name, arguments: args }),
        readResource: (uri) => sendRequest('resources/read', { uri }),
        getPrompt: (name, args) => sendRequest('prompts/get', { name, arguments: args }),

        // MCP-UI specific
        sendIntent: (action, data) => sendRequest('ui/intent', { action, data }),
        notify: (level, message) => {
            window.parent.postMessage({
                jsonrpc: '2.0',
                method: 'ui/notify',
                params: { level, message }
            }, '*');
        },
        openLink: (url) => {
            window.parent.postMessage({
                jsonrpc: '2.0',
                method: 'ui/link',
                params: { url }
            }, '*');
        }
    };

    window.mcpBridge.notify('info', 'Widget ready');
})();
</script>
";

        inject_script_into_head(html, bridge_script)
    }

    fn required_csp(&self) -> Option<WidgetCSP> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chatgpt_adapter_transform() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><head></head><body>Hello</body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert_eq!(transformed.mime_type, ExtendedUIMimeType::HtmlSkybridge);
        assert!(transformed.content.contains("window.mcpBridge"));
        assert!(transformed.content.contains("window.openai"));
    }

    #[test]
    fn test_mcp_apps_adapter_transform() {
        let adapter = McpAppsAdapter::new();
        let html = "<html><body></body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert_eq!(transformed.mime_type, ExtendedUIMimeType::HtmlMcpApp);
    }

    #[test]
    fn test_mcp_ui_adapter_transform() {
        let adapter = McpUiAdapter::new();
        let html = "<html><body></body></html>";

        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert!(transformed.content.contains("mcpBridge"));
    }

    #[test]
    fn test_chatgpt_adapter_with_widget_meta_strips_display_keys() {
        // Display-only keys (prefers_border, description) should be stripped
        let display_only = WidgetMeta::new()
            .prefers_border(true)
            .description("A test widget");

        let adapter = ChatGptAdapter::new().with_widget_meta(display_only);
        let html = "<html><body></body></html>";
        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        // Display keys should NOT be in metadata
        assert!(
            !transformed
                .metadata
                .contains_key("openai/widgetPrefersBorder"),
            "display keys should be stripped from ChatGPT metadata"
        );
        assert!(
            !transformed
                .metadata
                .contains_key("openai/widgetDescription"),
            "display keys should be stripped from ChatGPT metadata"
        );
        // openai/outputTemplate is always present (from URI)
        assert!(
            transformed.metadata.contains_key("openai/outputTemplate"),
            "openai/outputTemplate must always be present in ChatGPT adapter"
        );
    }

    #[test]
    fn test_chatgpt_adapter_with_widget_meta_keeps_descriptor_keys() {
        // resource_uri produces openai/outputTemplate — a descriptor key
        let meta = WidgetMeta::new()
            .resource_uri("ui://test/widget.html")
            .prefers_border(true);

        let adapter = ChatGptAdapter::new().with_widget_meta(meta);
        let html = "<html><body></body></html>";
        let transformed = adapter.transform("ui://test/widget.html", "Test Widget", html);

        assert!(
            transformed.metadata.contains_key("openai/outputTemplate"),
            "descriptor keys should be kept"
        );
        assert!(
            !transformed
                .metadata
                .contains_key("openai/widgetPrefersBorder"),
            "display keys should be stripped"
        );
    }

    #[test]
    fn test_bridge_injection_with_head() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><head><title>Test</title></head><body></body></html>";
        let result = adapter.inject_bridge(html);

        assert!(result.contains("window.mcpBridge"));
        assert!(result.contains("</head>"));
    }

    #[test]
    fn test_bridge_injection_without_head() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><body>Content</body></html>";
        let result = adapter.inject_bridge(html);

        assert!(result.contains("window.mcpBridge"));
    }

    #[test]
    fn test_chatgpt_bridge_listens_for_both_method_forms() {
        let adapter = ChatGptAdapter::new();
        let html = "<html><head></head><body></body></html>";
        let result = adapter.inject_bridge(html);

        // Must handle short-form (from AppBridge/ext-apps SDK)
        assert!(
            result.contains("ui/toolResult"),
            "ChatGPT bridge must listen for short-form ui/toolResult"
        );
        // Must also handle long-form (from spec-compliant hosts)
        assert!(
            result.contains("ui/notifications/tool-result"),
            "ChatGPT bridge must listen for long-form ui/notifications/tool-result"
        );
        // window.openai path must be preserved
        assert!(
            result.contains("window.openai"),
            "ChatGPT bridge must preserve window.openai path"
        );
    }

    #[test]
    fn test_mcp_apps_no_bridge_injection() {
        let adapter = McpAppsAdapter::new();
        let html = "<html><head></head><body>My Widget</body></html>";
        let result = adapter.inject_bridge(html);

        // McpAppsAdapter no longer injects a bridge — widget developers use
        // the @modelcontextprotocol/ext-apps SDK directly.
        assert_eq!(result, html, "McpAppsAdapter must return HTML unchanged");
    }

    #[test]
    fn test_mcp_apps_transform_preserves_html_without_cdn() {
        let adapter = McpAppsAdapter::new();
        let html = "<html><head></head><body>My Widget</body></html>";
        let transformed = adapter.transform("ui://test/widget.html", "Test", html);

        assert_eq!(
            transformed.content, html,
            "McpAppsAdapter.transform must serve HTML as-is when no CDN imports"
        );
        assert_eq!(transformed.mime_type, ExtendedUIMimeType::HtmlMcpApp);
    }

    #[test]
    fn test_inline_shim_replaces_esm_import() {
        let html = r#"<script type="module">
import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps@1.2.2";
const app = new App({ name: "test" });
</script>"#;
        let fixed = inline_ext_apps_shim(html);
        assert!(
            !fixed.contains("esm.sh"),
            "CDN import should be removed: {fixed}"
        );
        assert!(
            fixed.contains("class App"),
            "inline shim should define App class"
        );
        assert!(
            fixed.contains("ui/initialize"),
            "inline shim should implement ui/initialize protocol"
        );
        assert!(
            fixed.contains("const app = new App"),
            "widget code after import should be preserved"
        );
    }

    #[test]
    fn test_inline_shim_replaces_jsdelivr_import() {
        let html = r#"import { App } from "https://cdn.jsdelivr.net/npm/@modelcontextprotocol/ext-apps@1.2.2/+esm";
const app = new App({ name: "test" });"#;
        let fixed = inline_ext_apps_shim(html);
        assert!(
            !fixed.contains("jsdelivr"),
            "jsdelivr import should be removed"
        );
        assert!(fixed.contains("class App"), "should inline the shim");
    }

    #[test]
    fn test_inline_shim_no_match() {
        let html = "<html><body>No imports here</body></html>";
        let fixed = inline_ext_apps_shim(html);
        assert_eq!(&*fixed, html, "should not modify HTML without CDN imports");
    }

    #[test]
    fn test_inline_shim_preserves_non_ext_apps_imports() {
        let html = r#"<script type="module">
import { Chart } from "https://esm.sh/chart.js@4.4.0";
import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps@1.2.2";
const app = new App({ name: "test" });
</script>"#;
        let fixed = inline_ext_apps_shim(html);
        assert!(
            fixed.contains("chart.js"),
            "non-ext-apps imports should be preserved"
        );
        assert!(
            !fixed.contains("esm.sh/@modelcontextprotocol"),
            "ext-apps import should be replaced"
        );
    }

    #[test]
    fn test_mcp_apps_transform_inlines_shim() {
        let adapter = McpAppsAdapter::new();
        let html = r#"<html><head></head><body>
<script type="module">
import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps@1.2.2";
const app = new App({ name: "test" });
await app.connect();
</script>
</body></html>"#;
        let transformed = adapter.transform("ui://test/widget.html", "Test", html);
        assert!(
            transformed.content.contains("class App"),
            "McpAppsAdapter.transform should inline the App shim"
        );
        assert!(
            transformed.content.contains("await app.connect()"),
            "widget code should be preserved"
        );
    }
}
