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

/// Handle WebSocket connection.
///
/// Loops over inbound frames; each frame goes through:
/// 1. `extract_text_frame` — text? close? other? error?
/// 2. `parse_ws_message` — JSON → `WsMessage` (parse errors sent back inline)
/// 3. `dispatch_ws_message` — produce `Some(response)` to send, or `None` to
///    skip (e.g. log messages, server-only variants).
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        let text = match extract_text_frame(msg) {
            Some(t) => t,
            None => break,
        };

        let ws_msg = match parse_ws_message(&text, &mut sender).await {
            Some(m) => m,
            None => continue,
        };

        let Some(response) = dispatch_ws_message(ws_msg, &state).await else {
            continue;
        };

        if !send_response(&mut sender, &response).await {
            break;
        }
    }
}

/// Extract a text payload from an inbound frame.
///
/// Returns `Some(text)` for `Message::Text`. Returns `None` for `Close` or
/// stream errors (caller should break the loop). For other variants (Binary,
/// Ping, Pong, etc.) returns `Some("")` which the caller treats as "skip".
fn extract_text_frame(
    msg: Result<Message, axum::Error>,
) -> Option<axum::extract::ws::Utf8Bytes> {
    match msg {
        Ok(Message::Text(text)) => Some(text),
        Ok(Message::Close(_)) | Err(_) => None,
        Ok(_) => Some(axum::extract::ws::Utf8Bytes::from("")),
    }
}

/// Parse a JSON frame into a `WsMessage`. On parse error, send an error
/// message back inline and return `None` (caller should `continue`).
async fn parse_ws_message<S>(text: &str, sender: &mut S) -> Option<WsMessage>
where
    S: SinkExt<Message> + Unpin,
{
    if text.is_empty() {
        // Empty payload from non-text frame; just skip.
        return None;
    }
    match serde_json::from_str(text) {
        Ok(m) => Some(m),
        Err(e) => {
            let error = WsMessage::Error {
                message: format!("Invalid message: {e}"),
            };
            if let Ok(json) = serde_json::to_string(&error) {
                let _ = sender.send(Message::Text(json.into())).await;
            }
            None
        },
    }
}

/// Dispatch a parsed message; return `Some(response)` or `None` to skip.
async fn dispatch_ws_message(msg: WsMessage, state: &Arc<AppState>) -> Option<WsMessage> {
    match msg {
        WsMessage::Ping => Some(WsMessage::Pong),
        WsMessage::CallTool { name, arguments } => {
            Some(handle_call_tool_message(state, &name, arguments).await)
        },
        WsMessage::Log { level, message } => {
            tracing::info!(level = %level, "Widget log: {}", message);
            None
        },
        // Pong / ToolResult / Error are server-emitted only; clients shouldn't
        // send them. Drop without a response.
        _ => None,
    }
}

/// Invoke the proxy for a CallTool message, return a ToolResult or Error reply.
async fn handle_call_tool_message(
    state: &Arc<AppState>,
    name: &str,
    arguments: Value,
) -> WsMessage {
    match state.proxy.call_tool(name, arguments).await {
        Ok(result) => WsMessage::ToolResult {
            success: result.success,
            result: serde_json::to_value(result).unwrap_or_default(),
        },
        Err(e) => WsMessage::Error {
            message: e.to_string(),
        },
    }
}

/// Serialize and send a response. Returns `false` on send failure (caller
/// should break the loop).
async fn send_response<S>(sender: &mut S, response: &WsMessage) -> bool
where
    S: SinkExt<Message> + Unpin,
{
    let Ok(text) = serde_json::to_string(response) else {
        return true; // serialization failure is rare; skip rather than close
    };
    sender.send(Message::Text(text.into())).await.is_ok()
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
