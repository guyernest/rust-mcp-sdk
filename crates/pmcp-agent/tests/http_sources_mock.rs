//! Local mock-server tests for the feature-gated HTTP completion sources.
//!
//! A dependency-free in-process HTTP/1.1 mock (raw `tokio::net::TcpListener`)
//! captures the request the source sends — method, path, headers (auth), and
//! JSON body — and replies with a canned response, so the tests assert wire
//! behaviour (URL/path/`Authorization`/`x-api-key`/`system` hoist/timeout), not
//! only pure transforms. No new crate.

#![cfg(any(feature = "openai-compat", feature = "anthropic"))]
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A captured HTTP request.
#[derive(Debug, Clone, Default)]
struct Captured {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }
}

/// Handle to a running mock: its base URL and the captured request slot.
struct Mock {
    base_url: String,
    captured: Arc<Mutex<Option<Captured>>>,
}

/// Spawn a one-shot mock HTTP server that captures the first request, optionally
/// delays, then replies with `status` + `resp_body` (JSON).
async fn spawn_mock(status: u16, resp_body: String, delay: Option<Duration>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let captured: Arc<Mutex<Option<Captured>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let req = read_request(&mut stream).await;
        *cap.lock().unwrap() = Some(req);

        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }

        let reason = if (200..300).contains(&status) {
            "OK"
        } else {
            "ERR"
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
            resp_body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
    });

    Mock {
        base_url: format!("http://127.0.0.1:{}", addr.port()),
        captured,
    }
}

/// Read one HTTP/1.1 request (headers + Content-Length body) from the stream.
async fn read_request(stream: &mut tokio::net::TcpStream) -> Captured {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];

    // Read until we have the full header block.
    let header_end = loop {
        let n = stream.read(&mut tmp).await.expect("read headers");
        if n == 0 {
            break buf.len();
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let header_text = String::from_utf8_lossy(&buf[..header_end.min(buf.len())]).to_string();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_string();
            let v = v.trim().to_string();
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.parse().unwrap_or(0);
            }
            headers.push((k, v));
        }
    }

    // Body bytes already read past the header block.
    let mut body = buf[header_end.min(buf.len())..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).await.expect("read body");
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }

    Captured {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).to_string(),
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ===========================================================================
// OpenAiCompatSource mock cases
// ===========================================================================

#[cfg(feature = "openai-compat")]
mod openai {
    use super::*;
    use pmcp::types::sampling::{
        CreateMessageParams, SamplingMessage, SamplingMessageContent, ToolChoice,
    };
    use pmcp::types::Role;
    use pmcp_agent::seams::CompletionSource;
    use pmcp_agent::sources::{HttpSourceOptions, OpenAiCompatSource, SecretString};

    fn user_params() -> CreateMessageParams {
        CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hi".to_string(),
                meta: None,
            },
        )])
        .with_system_prompt("sys")
        .with_tool_choice(ToolChoice::auto())
    }

    #[tokio::test]
    async fn posts_to_chat_completions_with_bearer_and_body() {
        let resp = serde_json::json!({
            "model": "gpt-x",
            "choices": [{
                "message": {
                    "content": "",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": { "name": "search", "arguments": "{\"q\":\"rust\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })
        .to_string();
        let mock = spawn_mock(200, resp, None).await;

        // Localhost http is allowed by policy.
        let source = OpenAiCompatSource::new(
            format!("{}/v1", mock.base_url),
            "gpt-x",
            SecretString::new("sk-secret"),
        )
        .expect("source builds");

        let result = source
            .create_message(user_params())
            .await
            .expect("completion ok");

        // Tool call preserved.
        let has = result.content.iter().any(|c| {
            matches!(c, SamplingMessageContent::ToolUse { id, name, .. } if id == "call-1" && name == "search")
        });
        assert!(has, "tool_use must survive");

        // Wire assertions.
        let cap = mock.captured.lock().unwrap().clone().expect("captured");
        assert_eq!(cap.method, "POST");
        assert_eq!(cap.path, "/v1/chat/completions");
        assert_eq!(cap.header("authorization"), Some("Bearer sk-secret"));
        let body: serde_json::Value = serde_json::from_str(&cap.body).expect("json body");
        assert_eq!(body["model"], "gpt-x");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "sys");
        assert_eq!(body["tool_choice"], "auto");
    }

    #[tokio::test]
    async fn request_timeout_is_transient_error() {
        use pmcp_agent::seams::RetryClass;
        // Mock delays 2s; source timeout is 200ms.
        let mock = spawn_mock(200, "{}".to_string(), Some(Duration::from_secs(2))).await;
        let opts = HttpSourceOptions::default().with_timeout(Duration::from_millis(200));
        let source = OpenAiCompatSource::with_options(
            format!("{}/v1", mock.base_url),
            "m",
            SecretString::new("k"),
            opts,
        )
        .expect("builds");

        let err = source
            .create_message(user_params())
            .await
            .expect_err("must time out");
        assert_ne!(err.retry_class(), RetryClass::Fatal, "timeout is retryable");
    }

    #[tokio::test]
    async fn server_error_status_classifies_transient() {
        use pmcp_agent::seams::RetryClass;
        let mock = spawn_mock(503, "{}".to_string(), None).await;
        let source =
            OpenAiCompatSource::new(format!("{}/v1", mock.base_url), "m", SecretString::new("k"))
                .expect("builds");
        let err = source
            .create_message(user_params())
            .await
            .expect_err("5xx errors");
        assert!(matches!(err.retry_class(), RetryClass::Transient { .. }));
    }
}

// ===========================================================================
// AnthropicSource mock cases
// ===========================================================================

#[cfg(feature = "anthropic")]
mod anthropic {
    use super::*;
    use pmcp::types::sampling::{CreateMessageParams, SamplingMessage, SamplingMessageContent};
    use pmcp::types::Role;
    use pmcp_agent::seams::CompletionSource;
    use pmcp_agent::sources::{AnthropicSource, SecretString};

    fn user_params() -> CreateMessageParams {
        CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hi".to_string(),
                meta: None,
            },
        )])
        .with_system_prompt("be nice")
        .with_max_tokens(512)
    }

    #[tokio::test]
    async fn posts_messages_with_api_key_and_hoisted_system() {
        let resp = serde_json::json!({
            "model": "claude-x",
            "stop_reason": "tool_use",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "sure" },
                { "type": "tool_use", "id": "tu-1", "name": "search", "input": {"q": "rust"} }
            ]
        })
        .to_string();
        let mock = spawn_mock(200, resp, None).await;

        let source = AnthropicSource::new(
            format!("{}/v1/messages", mock.base_url),
            "claude-x",
            SecretString::new("sk-ant-secret"),
        )
        .expect("source builds");

        let result = source
            .create_message(user_params())
            .await
            .expect("completion ok");
        let has = result.content.iter().any(|c| {
            matches!(c, SamplingMessageContent::ToolUse { id, name, .. } if id == "tu-1" && name == "search")
        });
        assert!(has, "tool_use must survive");

        let cap = mock.captured.lock().unwrap().clone().expect("captured");
        assert_eq!(cap.method, "POST");
        assert_eq!(cap.path, "/v1/messages");
        assert_eq!(cap.header("x-api-key"), Some("sk-ant-secret"));
        assert!(cap.header("anthropic-version").is_some());
        // No bearer leak of the key.
        assert!(cap.header("authorization").is_none());
        let body: serde_json::Value = serde_json::from_str(&cap.body).expect("json body");
        assert_eq!(body["model"], "claude-x");
        assert_eq!(body["system"], "be nice");
        assert_eq!(body["max_tokens"], 512);
        // System must NOT appear as a message.
        let msgs = body["messages"].as_array().expect("messages array");
        assert!(msgs.iter().all(|m| m["role"] != "system"));
    }
}
