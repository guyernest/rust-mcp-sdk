//! HTTP transport implementation for WASM environments.
//!
//! This module provides an HTTP transport that works in browser environments
//! using the Fetch API for communication with stateless MCP servers (e.g., AWS Lambda).

#![cfg(target_arch = "wasm32")]

use crate::error::{Error, Result};
use crate::shared::pending_slot::PendingSlot;
use crate::shared::transport::{Transport, TransportMessage};
use async_trait::async_trait;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

/// HTTP transport configuration for WASM.
#[derive(Debug, Clone)]
pub struct WasmHttpConfig {
    /// The HTTP endpoint URL
    pub url: String,
    /// Additional headers to include in requests
    pub extra_headers: Vec<(String, String)>,
}

/// HTTP transport for WASM environments.
///
/// This transport uses the browser's Fetch API to communicate with
/// stateless MCP servers over HTTP. It's ideal for serverless deployments
/// like AWS Lambda with API Gateway.
///
/// # Examples
///
/// ```rust,ignore
/// use pmcp::shared::WasmHttpTransport;
///
/// # async fn example() -> pmcp::Result<()> {
/// let config = WasmHttpConfig {
///     url: "https://api.example.com/mcp".to_string(),
///     extra_headers: vec![],
/// };
/// let transport = WasmHttpTransport::new(config);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct WasmHttpTransport {
    config: WasmHttpConfig,
    session_id: Option<String>,
    protocol_version: Option<String>,
    /// One-slot buffer holding the response parsed by `send()` until
    /// `receive()` pops it — bridges the high-level client's
    /// send-then-loop-receive correlation onto one-shot Fetch.
    pending: PendingSlot,
}

impl WasmHttpTransport {
    /// Create a new HTTP transport.
    pub fn new(config: WasmHttpConfig) -> Self {
        Self {
            config,
            session_id: None,
            protocol_version: None,
            pending: PendingSlot::new(),
        }
    }

    /// Get the current session ID, if any.
    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    /// Set the protocol version for subsequent requests.
    pub fn set_protocol_version(&mut self, version: Option<String>) {
        self.protocol_version = version;
    }

    /// Perform an HTTP request and handle the response.
    async fn do_request(&mut self, message: &TransportMessage) -> Result<TransportMessage> {
        // Encode as a JSON-RPC 2.0 frame via the shared codec. Serializing the
        // untagged `TransportMessage` directly would emit `{"id":…,"request":…}`,
        // which the server rejects with -32700 "Unknown message type".
        let json_bytes = crate::shared::transport::serialize_message(message)?;
        let body = String::from_utf8(json_bytes)
            .map_err(|e| Error::internal(format!("Serialized message is not UTF-8: {}", e)))?;

        let response_text = self.do_http_request(&body).await?;

        // The streamable-HTTP server answers `initialize` as a raw JSON body but
        // streams request/response results (e.g. `tools/call`, `tasks/*`) as a
        // single Server-Sent Events frame — `text/event-stream` — regardless of
        // the `Accept` header. A browser Fetch cannot negotiate SSE streaming, so
        // accept BOTH shapes here and unwrap the JSON-RPC payload before parsing.
        let payload = Self::extract_jsonrpc_payload(&response_text)?;
        crate::shared::transport::parse_message(payload.as_bytes())
    }

    /// Unwrap the JSON-RPC payload from an HTTP response body that is either a
    /// raw JSON object or a single Server-Sent Events (`text/event-stream`) frame.
    ///
    /// A JSON body is returned verbatim. For an SSE frame we return the `data:`
    /// field of the first event (per the SSE spec, multiple `data:` lines within
    /// one event are joined with `\n`), which carries the JSON-RPC response.
    fn extract_jsonrpc_payload(body: &str) -> Result<String> {
        let trimmed = body.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(body.to_string());
        }

        let mut data = String::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                // A single optional leading space after the colon is part of the
                // SSE framing, not the payload.
                data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            } else if line.is_empty() && !data.is_empty() {
                // Blank line terminates the first event that carried data.
                break;
            }
        }

        if data.is_empty() {
            return Err(Error::internal(format!(
                "Response body is neither JSON nor an SSE data frame (first 120 chars): {}",
                body.chars().take(120).collect::<String>()
            )));
        }
        Ok(data)
    }

    /// Perform an HTTP request and return the raw response text.
    async fn do_http_request(&mut self, body: &str) -> Result<String> {
        // Get window object
        let window =
            web_sys::window().ok_or_else(|| Error::internal("No window object available"))?;

        // Create headers
        let headers = Headers::new()
            .map_err(|e| Error::internal(format!("Failed to create headers: {:?}", e)))?;

        headers
            .set("Content-Type", "application/json")
            .map_err(|e| Error::internal(format!("Failed to set Content-Type: {:?}", e)))?;

        headers
            .set("Accept", "application/json")
            .map_err(|e| Error::internal(format!("Failed to set Accept: {:?}", e)))?;

        // Add session ID if present
        if let Some(ref session_id) = self.session_id {
            headers
                .set("mcp-session-id", session_id)
                .map_err(|e| Error::internal(format!("Failed to set session ID: {:?}", e)))?;
        }

        // Add protocol version if present
        if let Some(ref version) = self.protocol_version {
            headers
                .set("mcp-protocol-version", version)
                .map_err(|e| Error::internal(format!("Failed to set protocol version: {:?}", e)))?;
        }

        // Add extra headers
        for (key, value) in &self.config.extra_headers {
            headers
                .set(key, value)
                .map_err(|e| Error::internal(format!("Failed to set header {}: {:?}", key, e)))?;
        }

        // Body is already serialized

        // Create request init
        let request_init = RequestInit::new();
        request_init.set_method("POST");
        request_init.set_headers(&headers);
        request_init.set_body(&JsValue::from_str(body));

        // Create request
        let request = Request::new_with_str_and_init(&self.config.url, &request_init)
            .map_err(|e| Error::internal(format!("Failed to create request: {:?}", e)))?;

        // Send request
        let response_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| Error::internal(format!("Fetch failed: {:?}", e)))?;

        let response: Response = response_value
            .dyn_into()
            .map_err(|e| Error::internal(format!("Invalid response type: {:?}", e)))?;

        // Check status
        if !response.ok() {
            let status = response.status();
            let text = JsFuture::from(
                response
                    .text()
                    .map_err(|e| Error::internal(format!("Failed to get error text: {:?}", e)))?,
            )
            .await
            .map_err(|e| Error::internal(format!("Failed to read error text: {:?}", e)))?;

            return Err(Error::internal(format!(
                "HTTP error {}: {}",
                status,
                text.as_string()
                    .unwrap_or_else(|| "Unknown error".to_string())
            )));
        }

        // Extract headers
        let response_headers = response.headers();

        // Update session ID if present in response
        if let Ok(Some(session_id)) = response_headers.get("mcp-session-id") {
            self.session_id = Some(session_id);
        }

        // Update protocol version if present in response
        if let Ok(Some(version)) = response_headers.get("mcp-protocol-version") {
            self.protocol_version = Some(version);
        }

        // Parse response body
        let text = JsFuture::from(
            response
                .text()
                .map_err(|e| Error::internal(format!("Failed to get response text: {:?}", e)))?,
        )
        .await
        .map_err(|e| Error::internal(format!("Failed to read response text: {:?}", e)))?;

        text.as_string()
            .ok_or_else(|| Error::internal("Response text is not a string"))
    }
}

#[async_trait(?Send)]
impl Transport for WasmHttpTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        match message {
            // A Request expects exactly one JSON-RPC response. POST it, parse the
            // response, and BUFFER it so the client's subsequent `receive()` can
            // correlate it. `do_request` injects `extra_headers` (the bearer) and
            // manages `mcp-session-id`.
            //
            // `put` propagates an error when a prior response is still buffered (a
            // double `send()` before `receive()`) — the first response is never
            // silently dropped (MEDIUM-4).
            TransportMessage::Request { .. } => {
                let response = self.do_request(&message).await?;
                self.pending.put(response)
            },
            // A Notification (or an outbound Response to a server-initiated request)
            // gets an HTTP 202 with an EMPTY body — there is no JSON-RPC response to
            // parse or correlate. POST it and ignore the empty body; do NOT occupy the
            // pending slot, or the next Request's `receive()` would drain this instead
            // of its own response. Without this arm, `initialize`'s trailing
            // `notifications/initialized` POST would try to parse an empty 202 body and
            // fail the whole handshake, so `Client::initialize` (and thus every browser
            // login) would error.
            TransportMessage::Notification(_) | TransportMessage::Response(_) => {
                let json_bytes = crate::shared::transport::serialize_message(&message)?;
                let body = String::from_utf8(json_bytes).map_err(|e| {
                    Error::internal(format!("Serialized message is not UTF-8: {}", e))
                })?;
                self.do_http_request(&body).await?;
                Ok(())
            },
        }
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        // Return the response buffered by the matching `send()`. Errors only when
        // called before any `send()` populated the slot.
        //
        // The single slot is the correct cardinality for a one-shot request/response
        // Fetch: one POST yields exactly one JSON-RPC response object. This transport
        // is intentionally non-SSE, so it does NOT deliver server-initiated
        // notifications — a streaming/SSE transport is the vehicle for those.
        self.pending.take()
    }

    async fn close(&mut self) -> Result<()> {
        // HTTP connections are stateless, so no explicit close needed
        Ok(())
    }
}

/// Alternative HTTP transport that better fits request-response pattern
#[derive(Debug, Clone)]
pub struct WasmHttpClient {
    transport: WasmHttpTransport,
}

impl WasmHttpClient {
    /// Create a new HTTP client.
    pub fn new(config: WasmHttpConfig) -> Self {
        Self {
            transport: WasmHttpTransport::new(config),
        }
    }

    /// Send a request and wait for response.
    pub async fn request<R>(&mut self, request: impl serde::Serialize) -> Result<R>
    where
        R: serde::de::DeserializeOwned,
    {
        // Serialize the request
        let body = serde_json::to_string(&request)
            .map_err(|e| Error::internal(format!("Failed to serialize request: {}", e)))?;

        // Perform the HTTP request
        let response_text = self.transport.do_http_request(&body).await?;

        // Parse the response
        serde_json::from_str(&response_text)
            .map_err(|e| Error::internal(format!("Failed to parse response: {}", e)))
    }

    /// Get the current session ID.
    pub fn session_id(&self) -> Option<String> {
        self.transport.session_id()
    }

    /// Get the protocol version.
    pub fn protocol_version(&self) -> Option<String> {
        self.transport.protocol_version.clone()
    }
}
