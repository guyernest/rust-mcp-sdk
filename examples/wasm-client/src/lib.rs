use std::sync::atomic::{AtomicU64, Ordering};

use pmcp::client::Client;
use pmcp::types::ClientCapabilities;
use pmcp::{WasmHttpClient, WasmHttpConfig, WasmWebSocketTransport};
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use web_sys::console;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Error")]
    pub type JsError;
}

#[wasm_bindgen(module = "/src/utils.js")]
extern "C" {
    #[wasm_bindgen(js_name = "newError")]
    fn new_js_error(message: String, code: Option<i32>, data: JsValue) -> JsError;
}

#[derive(Serialize)]
struct StructuredError {
    message: String,
    code: Option<i32>,
    data: Option<Value>,
}

impl From<pmcp::Error> for StructuredError {
    fn from(err: pmcp::Error) -> Self {
        match err {
            pmcp::Error::Protocol {
                code,
                message,
                data,
            } => Self {
                message,
                code: Some(code.as_i32()),
                data,
            },
            other => Self {
                message: other.to_string(),
                code: None,
                data: None,
            },
        }
    }
}

fn to_js_error(err: pmcp::Error) -> JsValue {
    let structured: StructuredError = err.into();
    let data = serde_wasm_bindgen::to_value(&structured.data).unwrap_or(JsValue::NULL);
    new_js_error(structured.message, structured.code, data).into()
}

/// Connection type for the WASM client
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub enum ConnectionType {
    WebSocket,
    Http,
}

/// WASM MCP Client that supports both HTTP and WebSocket transports
#[wasm_bindgen]
pub struct WasmClient {
    connection_type: Option<ConnectionType>,
    ws_client: Option<Client<WasmWebSocketTransport>>,
    http_client: Option<WasmHttpClient>,
    next_request_id: AtomicU64,
}

#[wasm_bindgen]
impl WasmClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
        Self {
            connection_type: None,
            ws_client: None,
            http_client: None,
            next_request_id: AtomicU64::new(1),
        }
    }

    /// Generate a unique request ID for each MCP call.
    ///
    /// Uses an atomic counter to avoid concurrent call corruption
    /// that previously occurred with hardcoded IDs.
    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed) as i64
    }

    /// Connect to an MCP server.
    /// - URLs starting with ws:// or wss:// will use WebSocket
    /// - URLs starting with http:// or https:// will use HTTP
    #[wasm_bindgen]
    pub async fn connect(&mut self, url: String) -> Result<(), JsValue> {
        console::log_1(&format!("Connecting to {}...", url).into());

        // Determine connection type based on URL scheme
        if url.starts_with("ws://") || url.starts_with("wss://") {
            // WebSocket connection
            self.connection_type = Some(ConnectionType::WebSocket);
            let transport = WasmWebSocketTransport::connect(&url)
                .await
                .map_err(to_js_error)?;
            let mut client = Client::new(transport);
            let capabilities = ClientCapabilities::default();
            client.initialize(capabilities).await.map_err(to_js_error)?;
            self.ws_client = Some(client);
            console::log_1(&"WebSocket connection successful.".into());
        } else if url.starts_with("http://") || url.starts_with("https://") {
            // HTTP connection
            self.connection_type = Some(ConnectionType::Http);
            let config = WasmHttpConfig {
                url: url.clone(),
                extra_headers: vec![],
            };
            let mut client = WasmHttpClient::new(config);

            // Initialize the connection - wrap in TransportMessage
            let init_request = pmcp::shared::TransportMessage::Request {
                id: self.next_id().into(),
                request: pmcp::types::Request::Client(Box::new(
                    pmcp::types::ClientRequest::Initialize(pmcp::types::InitializeParams {
                        protocol_version: pmcp::LATEST_PROTOCOL_VERSION.to_string(),
                        capabilities: ClientCapabilities::default(),
                        client_info: pmcp::types::Implementation {
                            name: "wasm-mcp-client".to_string(),
                            version: "1.0.0".to_string(),
                        },
                    }),
                )),
            };

            let response: pmcp::shared::TransportMessage =
                client.request(init_request).await.map_err(to_js_error)?;

            // Verify we got a successful initialization response
            if let pmcp::shared::TransportMessage::Response(resp) = response {
                if let pmcp::types::jsonrpc::ResponsePayload::Result(_) = resp.payload {
                    console::log_1(
                        &format!(
                            "HTTP connection successful. Session: {:?}",
                            client.session_id()
                        )
                        .into(),
                    );
                } else {
                    return Err(new_js_error(
                        "Invalid initialization response".to_string(),
                        None,
                        JsValue::NULL,
                    )
                    .into());
                }
            } else {
                return Err(new_js_error(
                    "Expected response message".to_string(),
                    None,
                    JsValue::NULL,
                )
                .into());
            }

            self.http_client = Some(client);
        } else {
            return Err(new_js_error(
                "Invalid URL scheme. Use ws://, wss://, http://, or https://".to_string(),
                None,
                JsValue::NULL,
            )
            .into());
        }

        Ok(())
    }

    /// List available tools from the server
    #[wasm_bindgen]
    pub async fn list_tools(&mut self) -> Result<JsValue, JsValue> {
        match self.connection_type {
            Some(ConnectionType::WebSocket) => {
                let client = self.ws_client.as_mut().ok_or_else(|| {
                    new_js_error("Not connected".to_string(), None, JsValue::NULL)
                })?;
                let result = client.list_tools(None).await.map_err(to_js_error)?;
                serde_wasm_bindgen::to_value(&result.tools).map_err(|e| e.into())
            },
            Some(ConnectionType::Http) => {
                let client = self.http_client.as_mut().ok_or_else(|| {
                    new_js_error("Not connected".to_string(), None, JsValue::NULL)
                })?;

                // Create list tools request as TransportMessage
                let request = pmcp::shared::TransportMessage::Request {
                    id: self.next_id().into(),
                    request: pmcp::types::Request::Client(Box::new(
                        pmcp::types::ClientRequest::ListTools(pmcp::types::ListToolsRequest {
                            cursor: None,
                        }),
                    )),
                };

                let response: pmcp::shared::TransportMessage =
                    client.request(request).await.map_err(to_js_error)?;

                // Extract tools from response
                if let pmcp::shared::TransportMessage::Response(resp) = response {
                    if let pmcp::types::jsonrpc::ResponsePayload::Result(value) = resp.payload {
                        if let Ok(result) =
                            serde_json::from_value::<pmcp::types::ListToolsResult>(value)
                        {
                            return serde_wasm_bindgen::to_value(&result.tools)
                                .map_err(|e| e.into());
                        }
                    }
                }

                Err(new_js_error(
                    "Invalid response from server".to_string(),
                    None,
                    JsValue::NULL,
                )
                .into())
            },
            None => Err(new_js_error("Not connected".to_string(), None, JsValue::NULL).into()),
        }
    }

    /// Call a tool on the server
    #[wasm_bindgen]
    pub async fn call_tool(&mut self, name: String, args: JsValue) -> Result<JsValue, JsValue> {
        let arguments: Value = serde_wasm_bindgen::from_value(args)?;

        match self.connection_type {
            Some(ConnectionType::WebSocket) => {
                let client = self.ws_client.as_mut().ok_or_else(|| {
                    new_js_error("Not connected".to_string(), None, JsValue::NULL)
                })?;
                let result = client
                    .call_tool(name, arguments)
                    .await
                    .map_err(to_js_error)?;
                serde_wasm_bindgen::to_value(&result).map_err(|e| e.into())
            },
            Some(ConnectionType::Http) => {
                let client = self.http_client.as_mut().ok_or_else(|| {
                    new_js_error("Not connected".to_string(), None, JsValue::NULL)
                })?;

                // Create call tool request as TransportMessage
                let request = pmcp::shared::TransportMessage::Request {
                    id: self.next_id().into(),
                    request: pmcp::types::Request::Client(Box::new(
                        pmcp::types::ClientRequest::CallTool(pmcp::types::CallToolRequest {
                            name,
                            arguments,
                            _meta: None,
                            task: None,
                        }),
                    )),
                };

                let response: pmcp::shared::TransportMessage =
                    client.request(request).await.map_err(to_js_error)?;

                // Extract result from response
                if let pmcp::shared::TransportMessage::Response(resp) = response {
                    if let pmcp::types::jsonrpc::ResponsePayload::Result(value) = resp.payload {
                        if let Ok(result) =
                            serde_json::from_value::<pmcp::types::CallToolResult>(value)
                        {
                            return serde_wasm_bindgen::to_value(&result).map_err(|e| e.into());
                        }
                    }
                }

                Err(new_js_error(
                    "Invalid response from server".to_string(),
                    None,
                    JsValue::NULL,
                )
                .into())
            },
            None => Err(new_js_error("Not connected".to_string(), None, JsValue::NULL).into()),
        }
    }

    /// Get the connection type
    #[wasm_bindgen]
    pub fn connection_type(&self) -> Option<ConnectionType> {
        self.connection_type.clone()
    }

    /// Get the current session ID (if any)
    #[wasm_bindgen]
    pub fn session_id(&self) -> Option<String> {
        match self.connection_type {
            Some(ConnectionType::WebSocket) => {
                self.ws_client.as_ref().and_then(|_| None) // WebSocket client doesn't expose session directly
            },
            Some(ConnectionType::Http) => self.http_client.as_ref().and_then(|c| c.session_id()),
            None => None,
        }
    }
}
