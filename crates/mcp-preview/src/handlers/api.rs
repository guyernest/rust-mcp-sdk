//! API handlers for tool and resource operations

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::proxy::{ToolCallResult, ToolInfo};
use crate::server::AppState;

/// Configuration response
#[derive(Serialize)]
pub struct ConfigResponse {
    pub mcp_url: String,
    pub theme: String,
    pub locale: String,
    pub initial_tool: Option<String>,
}

/// Get preview configuration
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        mcp_url: state.config.mcp_url.clone(),
        theme: state.config.theme.clone(),
        locale: state.config.locale.clone(),
        initial_tool: state.config.initial_tool.clone(),
    })
}

/// Tools list response
#[derive(Serialize)]
pub struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List available tools from the MCP server
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ToolsResponse>, (StatusCode, String)> {
    match state.proxy.list_tools().await {
        Ok(tools) => Ok(Json(ToolsResponse { tools, error: None })),
        Err(e) => Ok(Json(ToolsResponse {
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

/// Call a tool on the MCP server
pub async fn call_tool(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CallToolRequest>,
) -> Result<Json<ToolCallResult>, (StatusCode, String)> {
    let result = state
        .proxy
        .call_tool(&request.name, request.arguments)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

/// Query parameters for resource read requests
#[derive(Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
}

/// List UI resources from the MCP server.
///
/// Fetches all resources via the proxy and filters to those whose
/// MIME type contains "html" (case-insensitive), returning only
/// resources suitable for widget rendering.
pub async fn list_resources(State(state): State<Arc<AppState>>) -> Json<Value> {
    match state.proxy.list_resources().await {
        Ok(resources) => {
            let ui_resources: Vec<_> = resources
                .into_iter()
                .filter(|r| {
                    r.mime_type
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains("html"))
                })
                .collect();
            json_response(json!({ "resources": ui_resources }))
        },
        Err(e) => json_response(json!({ "resources": [], "error": e.to_string() })),
    }
}

/// Read a resource by URI from the MCP server.
pub async fn read_resource(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ReadResourceParams>,
) -> Json<Value> {
    match state.proxy.read_resource(&params.uri).await {
        Ok(result) => json_response(json!({ "contents": result.contents })),
        Err(e) => json_response(json!({ "contents": null, "error": e.to_string() })),
    }
}

/// Reconnect to the MCP server by resetting the session and
/// re-initializing via a tool list request.
pub async fn reconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.proxy.reset_session().await;
    match state.proxy.list_tools().await {
        Ok(tools) => json_response(json!({
            "success": true,
            "toolCount": tools.len()
        })),
        Err(e) => json_response(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Check whether the MCP session is currently connected.
pub async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let connected = state.proxy.is_connected().await;
    json_response(json!({ "connected": connected }))
}

/// Wrap a `serde_json::Value` in an Axum `Json` response.
fn json_response(value: Value) -> Json<Value> {
    Json(value)
}
