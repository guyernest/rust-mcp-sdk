//! WebSocket handler for live updates

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::server::AppState;

/// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// Ping to keep connection alive
    #[serde(rename = "ping")]
    Ping,
    /// Pong response
    #[serde(rename = "pong")]
    Pong,
    /// Tool call request
    #[serde(rename = "call_tool")]
    CallTool { name: String, arguments: Value },
    /// Tool call result
    #[serde(rename = "tool_result")]
    ToolResult { success: bool, result: Value },
    /// Error message
    #[serde(rename = "error")]
    Error { message: String },
    /// Log message from preview
    #[serde(rename = "log")]
    Log { level: String, message: String },
}

/// WebSocket upgrade handler
pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(_) => break,
        };

        // Parse message
        let ws_msg: WsMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                let error = WsMessage::Error {
                    message: format!("Invalid message: {e}"),
                };
                let _ = sender
                    .send(Message::Text(serde_json::to_string(&error).unwrap().into()))
                    .await;
                continue;
            },
        };

        // Handle message
        let response = match ws_msg {
            WsMessage::Ping => WsMessage::Pong,
            WsMessage::CallTool { name, arguments } => {
                match state.proxy.call_tool(&name, arguments).await {
                    Ok(result) => WsMessage::ToolResult {
                        success: result.success,
                        result: serde_json::to_value(result).unwrap_or_default(),
                    },
                    Err(e) => WsMessage::Error {
                        message: e.to_string(),
                    },
                }
            },
            WsMessage::Log { level, message } => {
                tracing::info!(level = %level, "Widget log: {}", message);
                continue; // No response needed
            },
            _ => continue,
        };

        // Send response
        if let Ok(text) = serde_json::to_string(&response) {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    }
}
