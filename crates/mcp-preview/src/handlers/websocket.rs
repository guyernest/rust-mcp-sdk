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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ping_message() {
        let raw = r#"{"type":"ping"}"#;
        let msg: WsMessage = serde_json::from_str(raw).expect("parse ping");
        assert!(matches!(msg, WsMessage::Ping));
    }

    #[test]
    fn parses_call_tool_message() {
        let raw = r#"{"type":"call_tool","data":{"name":"echo","arguments":{"value":1}}}"#;
        let msg: WsMessage = serde_json::from_str(raw).expect("parse call_tool");
        match msg {
            WsMessage::CallTool { name, arguments } => {
                assert_eq!(name, "echo");
                assert_eq!(arguments.get("value").and_then(Value::as_i64), Some(1));
            },
            other => panic!("expected CallTool, got {other:?}"),
        }
    }

    #[test]
    fn parses_log_message() {
        let raw = r#"{"type":"log","data":{"level":"info","message":"hi"}}"#;
        let msg: WsMessage = serde_json::from_str(raw).expect("parse log");
        match msg {
            WsMessage::Log { level, message } => {
                assert_eq!(level, "info");
                assert_eq!(message, "hi");
            },
            other => panic!("expected Log, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_message() {
        let raw = r#"{"type":"nonexistent"}"#;
        let result: Result<WsMessage, _> = serde_json::from_str(raw);
        assert!(result.is_err(), "unknown variant must fail to parse");
    }

    #[test]
    fn pong_serializes_with_type_tag() {
        let msg = WsMessage::Pong;
        let json = serde_json::to_string(&msg).expect("serialize pong");
        assert!(json.contains(r#""type":"pong""#));
    }

    #[test]
    fn error_serializes_with_message() {
        let msg = WsMessage::Error {
            message: "boom".to_string(),
        };
        let json = serde_json::to_string(&msg).expect("serialize error");
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains(r#""message":"boom""#));
    }
}
