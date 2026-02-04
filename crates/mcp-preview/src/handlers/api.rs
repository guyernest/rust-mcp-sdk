//! API handlers for tool operations

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
