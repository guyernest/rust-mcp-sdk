//! API handlers for tool and resource operations

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::proxy::{McpRequestError, ToolCallResult, ToolInfo};
use crate::server::AppState;

/// Configuration response
#[derive(Serialize)]
pub struct ConfigResponse {
    pub mcp_url: String,
    pub theme: String,
    pub locale: String,
    pub initial_tool: Option<String>,
    pub mode: String,
    pub descriptor_keys: Vec<String>,
    pub invocation_keys: Vec<String>,
    /// OAuth configuration for browser-based PKCE flow (null when OAuth not configured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_config: Option<OAuthConfigResponse>,
}

/// OAuth configuration exposed to the browser.
#[derive(Serialize)]
pub struct OAuthConfigResponse {
    pub client_id: String,
    pub authorization_endpoint: String,
    pub scopes: Vec<String>,
}

/// Get preview configuration
///
/// In standard mode, descriptor and invocation keys are empty (no ChatGPT
/// extensions active). In chatgpt mode, the full set of ChatGPT-specific
/// keys is returned so the browser-side AppBridge activates emulation.
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    use crate::server::PreviewMode;

    let is_chatgpt = state.config.mode == PreviewMode::ChatGpt;

    let descriptor_keys = if is_chatgpt {
        // Mirror of pmcp::types::ui::CHATGPT_DESCRIPTOR_KEYS
        vec![
            "openai/outputTemplate".into(),
            "openai/toolInvocation/invoking".into(),
            "openai/toolInvocation/invoked".into(),
            "openai/widgetAccessible".into(),
        ]
    } else {
        // Standard MCP Apps: nested _meta.ui.resourceUri — top-level key is "ui"
        vec!["ui".into()]
    };

    let invocation_keys = if is_chatgpt {
        vec![
            "openai/toolInvocation/invoking".into(),
            "openai/toolInvocation/invoked".into(),
        ]
    } else {
        vec![]
    };

    let oauth_config = state
        .config
        .oauth_config
        .as_ref()
        .map(|oc| OAuthConfigResponse {
            client_id: oc.client_id.clone(),
            authorization_endpoint: oc.authorization_endpoint.clone(),
            scopes: oc.scopes.clone(),
        });

    Json(ConfigResponse {
        mcp_url: state.config.mcp_url.clone(),
        theme: state.config.theme.clone(),
        locale: state.config.locale.clone(),
        initial_tool: state.config.initial_tool.clone(),
        mode: state.config.mode.to_string(),
        descriptor_keys,
        invocation_keys,
        oauth_config,
    })
}

/// Tools list response
#[derive(Serialize)]
pub struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List available tools from the MCP server.
///
/// In ChatGPT mode, enriches each tool's `_meta` with ChatGPT-specific keys
/// derived from the standard `ui.resourceUri` nested key. This ensures
/// mcp-preview validates the full ChatGPT protocol even when the server
/// emits standard-only metadata.
///
/// Returns upstream 401/403 status codes instead of wrapping them.
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolsResponse>, (StatusCode, String)> {
    match state.proxy.list_tools().await {
        Ok(mut tools) => {
            if state.config.mode == crate::server::PreviewMode::ChatGpt {
                for tool in &mut tools {
                    enrich_meta_for_chatgpt(&mut tool.meta);
                }
            }
            Ok(Json(ToolsResponse { tools, error: None }))
        },
        Err(McpRequestError::AuthRequired(status_code, body)) => {
            let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::UNAUTHORIZED);
            Err((status, body))
        },
        Err(McpRequestError::Other(e)) => Ok(Json(ToolsResponse {
            tools: vec![],
            error: Some(e.to_string()),
        })),
    }
}

/// Tool call request
#[derive(Deserialize)]
pub struct CallToolRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// Call a tool on the MCP server.
///
/// In ChatGPT mode, enriches the response `_meta` with invocation keys
/// so the protocol tab validates correctly.
///
/// Returns upstream 401/403 status codes instead of wrapping them.
pub async fn call_tool(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CallToolRequest>,
) -> Result<Json<ToolCallResult>, (StatusCode, String)> {
    match state
        .proxy
        .call_tool(&request.name, request.arguments)
        .await
    {
        Ok(mut result) => {
            if state.config.mode == crate::server::PreviewMode::ChatGpt {
                enrich_meta_for_chatgpt(&mut result.meta);
            }
            Ok(Json(result))
        },
        Err(McpRequestError::AuthRequired(status_code, body)) => {
            let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::UNAUTHORIZED);
            Err((status, body))
        },
        Err(McpRequestError::Other(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Query parameters for resource read requests
#[derive(Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
}

/// List UI resources from the MCP server and any file-based widgets.
///
/// When `widgets_dir` is configured, discovered `.html` files are merged with
/// proxy-fetched resources. Proxy resources are filtered to those whose MIME type
/// contains "html" (case-insensitive).
pub async fn list_resources(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut all_resources: Vec<serde_json::Value> = Vec::new();

    // Add file-based widgets from widgets_dir (if configured)
    if let Some(ref widgets_dir) = state.config.widgets_dir {
        match std::fs::read_dir(widgets_dir) {
            Ok(entries) => {
                let mut widget_entries: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("html"))
                    .collect();
                widget_entries.sort_by_key(|e| e.file_name());

                for entry in widget_entries {
                    if let Some(stem) = entry
                        .path()
                        .file_stem()
                        .and_then(|s| s.to_str().map(String::from))
                    {
                        all_resources.push(json!({
                            "uri": format!("ui://app/{}", stem),
                            "name": stem,
                            "description": format!("Widget from {}", entry.path().display()),
                            "mimeType": "text/html"
                        }));
                    }
                }

                tracing::debug!(
                    "Discovered {} widget(s) from {}",
                    all_resources.len(),
                    widgets_dir.display()
                );
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to read widgets directory {}: {}",
                    widgets_dir.display(),
                    e
                );
            },
        }
    }

    // Also fetch proxy resources (from the MCP server)
    match state.proxy.list_resources().await {
        Ok(resources) => {
            let ui_resources = resources.into_iter().filter(|r| {
                // Accept resources with HTML MIME types or ui:// URIs (MCP Apps convention)
                let mime_match = r
                    .mime_type
                    .as_deref()
                    .is_some_and(|m| m.to_lowercase().contains("html"));
                let uri_match = r.uri.starts_with("ui://");
                mime_match || uri_match
            });
            for r in ui_resources {
                // Avoid duplicates: skip proxy resources whose URI matches a disk widget
                let dominated = all_resources
                    .iter()
                    .any(|existing| existing.get("uri").and_then(|v| v.as_str()) == Some(&r.uri));
                if !dominated {
                    all_resources.push(json!({
                        "uri": r.uri,
                        "name": r.name,
                        "description": r.description,
                        "mimeType": r.mime_type,
                        "_meta": r.meta
                    }));
                }
            }
        },
        Err(McpRequestError::AuthRequired(_, _)) => {
            // Auth errors from list_resources are non-fatal when widgets_dir provides resources
            tracing::warn!("Proxy list_resources returned auth error (401/403)");
            if all_resources.is_empty() {
                return Json(json!({ "resources": [], "error": "Authentication required" }));
            }
        },
        Err(McpRequestError::Other(e)) => {
            tracing::warn!("Proxy list_resources failed: {}", e);
            if all_resources.is_empty() {
                return Json(json!({ "resources": [], "error": e.to_string() }));
            }
        },
    }

    // In ChatGPT mode, enrich each resource's _meta with openai/* keys
    if state.config.mode == crate::server::PreviewMode::ChatGpt {
        for resource in &mut all_resources {
            enrich_value_meta_for_chatgpt(resource);
        }
    }

    Json(json!({ "resources": all_resources }))
}

/// Read a resource by URI.
///
/// When `widgets_dir` is configured and the URI matches `ui://app/{name}`,
/// reads the widget HTML directly from disk and auto-injects the bridge
/// script tag. Every browser refresh reads fresh HTML from disk (hot-reload).
///
/// For all other URIs, falls through to the MCP proxy.
pub async fn read_resource(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ReadResourceParams>,
) -> Json<Value> {
    // Check if this is a file-based widget (ui://app/{name})
    if let Some(ref widgets_dir) = state.config.widgets_dir {
        if let Some(widget_name) = params.uri.strip_prefix("ui://app/") {
            let file_path = widgets_dir.join(format!("{}.html", widget_name));
            let html = match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    tracing::debug!(
                        "Reading widget file: {} ({} bytes)",
                        file_path.display(),
                        content.len()
                    );
                    // Auto-inject bridge script
                    inject_bridge_script(&content, "/assets/widget-runtime.mjs")
                },
                Err(err) => {
                    tracing::warn!("Failed to read widget {}: {}", file_path.display(), err);
                    widget_error_html(widget_name, &file_path, &err.to_string())
                },
            };

            return Json(json!({
                "contents": [{
                    "uri": params.uri,
                    "text": html,
                    "mimeType": "text/html"
                }]
            }));
        }
    }

    // Fall through to proxy for non-widget or non-configured resources
    match state.proxy.read_resource(&params.uri).await {
        Ok(result) => Json(json!({
            "contents": result.contents,
            "_meta": result.meta
        })),
        Err(e) => Json(json!({ "contents": null, "error": e.to_string() })),
    }
}

/// Insert a bridge script tag into widget HTML.
///
/// Delegates to the shared `pmcp-widget-utils` crate (single source of truth).
fn inject_bridge_script(html: &str, bridge_url: &str) -> String {
    pmcp_widget_utils::inject_bridge_script(html, bridge_url)
}

/// Generate a styled HTML error page for a widget that failed to load from disk.
fn widget_error_html(name: &str, path: &std::path::Path, error: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Widget Error: {name}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            padding: 20px;
        }}
        .error-card {{
            background: #4a1515;
            border: 1px solid #ff6b6b;
            border-radius: 12px;
            padding: 24px 32px;
            max-width: 560px;
            width: 100%;
        }}
        h2 {{ color: #ff6b6b; margin: 0 0 12px 0; font-size: 1.2rem; }}
        .file-path {{
            font-family: monospace;
            font-size: 0.85rem;
            color: #ffcc00;
            background: rgba(0,0,0,0.3);
            padding: 6px 10px;
            border-radius: 6px;
            word-break: break-all;
            margin-bottom: 12px;
        }}
        .error-message {{ font-family: monospace; font-size: 0.85rem; color: #ff9999; }}
        .hint {{ margin-top: 16px; font-size: 0.85rem; color: #888; }}
    </style>
</head>
<body>
    <div class="error-card">
        <h2>Widget Load Error</h2>
        <div class="file-path">{path}</div>
        <div class="error-message">{error}</div>
        <div class="hint">Create or fix the widget file and refresh the browser to retry.</div>
    </div>
</body>
</html>"#,
        name = name,
        path = path.display(),
        error = error,
    )
}

/// Reconnect to the MCP server by resetting the session and
/// re-initializing via a tool list request.
pub async fn reconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.proxy.reset_session().await;
    match state.proxy.list_tools().await {
        Ok(tools) => Json(json!({
            "success": true,
            "toolCount": tools.len()
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Check whether the MCP session is currently connected.
pub async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let connected = state.proxy.is_connected().await;
    Json(json!({ "connected": connected }))
}

/// Forward a raw JSON-RPC request to the MCP server.
///
/// Used by the WASM bridge client to avoid CORS issues: the browser
/// fetches same-origin `/api/mcp` and this handler proxies to the
/// actual MCP server. Forwards MCP session headers so the WASM client
/// can maintain its own session.
pub async fn forward_mcp(
    State(state): State<Arc<AppState>>,
    req_headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    use crate::proxy::{MCP_PROTOCOL_VERSION, MCP_SESSION_ID};

    // Extract MCP headers from the WASM client's request to forward upstream
    let session_id = req_headers
        .get(MCP_SESSION_ID)
        .and_then(|v| v.to_str().ok());
    let protocol_version = req_headers
        .get(MCP_PROTOCOL_VERSION)
        .and_then(|v| v.to_str().ok());

    match state
        .proxy
        .forward_raw(body, session_id, protocol_version)
        .await
    {
        Ok(result) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            if let Some(ref sid) = result.session_id {
                if let Ok(val) = HeaderValue::from_str(sid) {
                    headers.insert(MCP_SESSION_ID, val);
                }
            }
            if let Some(ref ver) = result.protocol_version {
                if let Ok(val) = HeaderValue::from_str(ver) {
                    headers.insert(MCP_PROTOCOL_VERSION, val);
                }
            }
            (StatusCode::OK, headers, result.body).into_response()
        },
        Err(McpRequestError::AuthRequired(status_code, body)) => {
            let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::UNAUTHORIZED);
            (status, body).into_response()
        },
        Err(McpRequestError::Other(e)) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

/// Inject ChatGPT-specific keys into a `_meta` map.
///
/// Derives `openai/outputTemplate` from `ui.resourceUri` (nested) or
/// `ui/resourceUri` (flat legacy key). Also injects default invocation
/// messages and `widgetAccessible`. Uses `entry().or_insert` so server-provided
/// keys are never overwritten.
fn enrich_chatgpt_meta_map(map: &mut serde_json::Map<String, Value>) {
    let resource_uri = map
        .get("ui")
        .and_then(|ui| ui.get("resourceUri"))
        .and_then(Value::as_str)
        .or_else(|| map.get("ui/resourceUri").and_then(Value::as_str))
        .map(String::from);

    if let Some(uri) = resource_uri {
        map.entry("openai/outputTemplate")
            .or_insert_with(|| Value::String(uri));
        map.entry("openai/widgetAccessible")
            .or_insert_with(|| Value::Bool(true));
        map.entry("openai/toolInvocation/invoking")
            .or_insert_with(|| Value::String("Running...".into()));
        map.entry("openai/toolInvocation/invoked")
            .or_insert_with(|| Value::String("Done".into()));
    }
}

/// Enrich a typed `_meta` field (`Option<Value>`) for ChatGPT mode.
fn enrich_meta_for_chatgpt(meta: &mut Option<Value>) {
    if let Some(Value::Object(ref mut map)) = meta {
        enrich_chatgpt_meta_map(map);
    }
}

/// Enrich a loose JSON object's `_meta` field for ChatGPT mode.
fn enrich_value_meta_for_chatgpt(resource: &mut Value) {
    if let Some(Value::Object(map)) = resource.get_mut("_meta") {
        enrich_chatgpt_meta_map(map);
    }
}
