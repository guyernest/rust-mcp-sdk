//! Transport-independent MCP server core implementation.
//!
//! This module provides the core server functionality that is decoupled from
//! transport mechanisms, enabling deployment to various environments including
//! WASM/WASI targets.

use crate::error::{Error, Result};
use crate::types::jsonrpc::ResponsePayload;
use crate::types::{
    CallToolParams, CallToolResult, ClientCapabilities, ClientRequest, Content, GetPromptParams,
    GetPromptResult, Implementation, InitializeParams, InitializeResult, JSONRPCError,
    JSONRPCResponse, ListPromptsParams, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesParams, ListResourcesResult, ListToolsParams,
    ListToolsResult, Notification, PromptInfo, ProtocolVersion, ReadResourceParams,
    ReadResourceResult, Request, RequestId, ServerCapabilities, ToolInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::RwLock;

#[cfg(not(target_arch = "wasm32"))]
use super::auth::{AuthProvider, ToolAuthorizer};
#[cfg(not(target_arch = "wasm32"))]
use super::cancellation::{CancellationManager, RequestHandlerExtra};
#[cfg(not(target_arch = "wasm32"))]
use super::roots::RootsManager;
#[cfg(not(target_arch = "wasm32"))]
use super::subscriptions::SubscriptionManager;
use super::{PromptHandler, ResourceHandler, SamplingHandler, ToolHandler};

/// Protocol-agnostic request handler trait.
///
/// This trait defines the core interface for handling MCP protocol requests
/// without any dependency on transport mechanisms. Implementations can be
/// deployed to various environments including WASM/WASI.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Handle a single request and return a response.
    ///
    /// This method processes MCP requests in a stateless manner without
    /// knowledge of the underlying transport mechanism.
    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse;

    /// Handle a notification (no response expected).
    ///
    /// Notifications are one-way messages that don't require a response.
    async fn handle_notification(&self, notification: Notification) -> Result<()>;

    /// Get server capabilities.
    ///
    /// Returns the capabilities that this server supports.
    fn capabilities(&self) -> &ServerCapabilities;

    /// Get server information.
    ///
    /// Returns metadata about the server implementation.
    fn info(&self) -> &Implementation;
}

/// Protocol handler trait for WASM environments (single-threaded).
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait ProtocolHandler {
    /// Handle a single request and return a response.
    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse;

    /// Handle a notification (no response expected).
    async fn handle_notification(&self, notification: Notification) -> Result<()>;

    /// Get server capabilities.
    fn capabilities(&self) -> &ServerCapabilities;

    /// Get server information.
    fn info(&self) -> &Implementation;
}

/// Core server implementation without transport dependencies.
///
/// This struct contains all the business logic for an MCP server without
/// any coupling to specific transport mechanisms. It can be used with
/// various transport adapters to deploy to different environments.
#[allow(dead_code)]
#[allow(missing_debug_implementations)]
pub struct ServerCore {
    /// Server metadata
    info: Implementation,

    /// Server capabilities
    capabilities: ServerCapabilities,

    /// Registered tool handlers
    tools: HashMap<String, Arc<dyn ToolHandler>>,

    /// Registered prompt handlers
    prompts: HashMap<String, Arc<dyn PromptHandler>>,

    /// Resource handler (optional)
    resources: Option<Arc<dyn ResourceHandler>>,

    /// Sampling handler (optional)
    sampling: Option<Arc<dyn SamplingHandler>>,

    /// Client capabilities (set during initialization)
    client_capabilities: Arc<RwLock<Option<ClientCapabilities>>>,

    /// Server initialization state
    initialized: Arc<RwLock<bool>>,

    /// Cancellation manager for request cancellation
    cancellation_manager: CancellationManager,

    /// Roots manager for directory/URI registration
    roots_manager: Arc<RwLock<RootsManager>>,

    /// Subscription manager for resource subscriptions
    subscription_manager: Arc<RwLock<SubscriptionManager>>,

    /// Authentication provider (optional)
    auth_provider: Option<Arc<dyn AuthProvider>>,

    /// Tool authorizer for fine-grained access control (optional)
    tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
}

impl ServerCore {
    /// Create a new `ServerCore` with the given configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        info: Implementation,
        capabilities: ServerCapabilities,
        tools: HashMap<String, Arc<dyn ToolHandler>>,
        prompts: HashMap<String, Arc<dyn PromptHandler>>,
        resources: Option<Arc<dyn ResourceHandler>>,
        sampling: Option<Arc<dyn SamplingHandler>>,
        auth_provider: Option<Arc<dyn AuthProvider>>,
        tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
    ) -> Self {
        Self {
            info,
            capabilities,
            tools,
            prompts,
            resources,
            sampling,
            client_capabilities: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            cancellation_manager: CancellationManager::new(),
            roots_manager: Arc::new(RwLock::new(RootsManager::new())),
            subscription_manager: Arc::new(RwLock::new(SubscriptionManager::new())),
            auth_provider,
            tool_authorizer,
        }
    }

    /// Check if the server is initialized.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get client capabilities if available.
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Handle initialization request.
    async fn handle_initialize(&self, init_req: &InitializeParams) -> Result<InitializeResult> {
        // Store client capabilities
        *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
        *self.initialized.write().await = true;

        // Negotiate protocol version
        let negotiated_version =
            if crate::SUPPORTED_PROTOCOL_VERSIONS.contains(&init_req.protocol_version.as_str()) {
                init_req.protocol_version.clone()
            } else {
                crate::DEFAULT_PROTOCOL_VERSION.to_string()
            };

        Ok(InitializeResult {
            protocol_version: ProtocolVersion(negotiated_version),
            capabilities: self.capabilities.clone(),
            server_info: self.info.clone(),
            instructions: None,
        })
    }

    /// Handle list tools request.
    async fn handle_list_tools(&self, _req: &ListToolsParams) -> Result<ListToolsResult> {
        let tools = self
            .tools
            .iter()
            .map(|(name, handler)| {
                // Use tool metadata if provided, otherwise use defaults
                if let Some(mut info) = handler.metadata() {
                    // Ensure the name matches the registered name
                    info.name.clone_from(name);
                    info
                } else {
                    ToolInfo {
                        name: name.clone(),
                        description: None,
                        input_schema: serde_json::json!({}),
                    }
                }
            })
            .collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    /// Handle call tool request.
    async fn handle_call_tool(&self, req: &CallToolParams) -> Result<CallToolResult> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Tool '{}' not found", req.name)))?;

        // Authorization check would occur here when auth context is available from transport layer
        // Currently, tool_authorizer is stored but awaits transport layer integration

        // Create request handler extra data
        let request_id = format!("tool_{}", req.name);
        let extra = RequestHandlerExtra {
            cancellation_token: self
                .cancellation_manager
                .create_token(request_id.clone())
                .await,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
        };

        // Execute the tool
        let result = handler.handle(req.arguments.clone(), extra).await?;

        Ok(CallToolResult {
            content: vec![Content::Text {
                text: serde_json::to_string_pretty(&result)?,
            }],
            is_error: false,
        })
    }

    /// Handle list prompts request.
    async fn handle_list_prompts(&self, _req: &ListPromptsParams) -> Result<ListPromptsResult> {
        let prompts: Vec<PromptInfo> = self
            .prompts
            .iter()
            .map(|(name, handler)| {
                // Use prompt metadata if provided, otherwise use defaults
                if let Some(mut info) = handler.metadata() {
                    // DEBUG: Log what metadata() returns
                    eprintln!("[DEBUG] handle_list_prompts: Prompt '{}' metadata:", name);
                    eprintln!("[DEBUG]   description: {:?}", info.description);
                    eprintln!(
                        "[DEBUG]   arguments count: {:?}",
                        info.arguments.as_ref().map(|a| a.len())
                    );

                    // Ensure the name matches the registered name
                    info.name.clone_from(name);

                    // DEBUG: Log the final PromptInfo before returning
                    eprintln!(
                        "[DEBUG]   Final PromptInfo - name: {}, desc present: {}, args present: {}",
                        info.name,
                        info.description.is_some(),
                        info.arguments.is_some()
                    );

                    info
                } else {
                    eprintln!(
                        "[DEBUG] handle_list_prompts: Prompt '{}' has NO metadata!",
                        name
                    );
                    PromptInfo {
                        name: name.clone(),
                        description: None,
                        arguments: None,
                    }
                }
            })
            .collect();

        eprintln!(
            "[DEBUG] handle_list_prompts: Returning {} prompts",
            prompts.len()
        );

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    /// Handle get prompt request.
    async fn handle_get_prompt(&self, req: &GetPromptParams) -> Result<GetPromptResult> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Prompt '{}' not found", req.name)))?;

        // Create request handler extra data
        let request_id = format!("prompt_{}", req.name);
        let extra = RequestHandlerExtra {
            cancellation_token: self
                .cancellation_manager
                .create_token(request_id.clone())
                .await,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
        };

        handler.handle(req.arguments.clone(), extra).await
    }

    /// Handle list resources request.
    async fn handle_list_resources(
        &self,
        req: &ListResourcesParams,
    ) -> Result<ListResourcesResult> {
        match &self.resources {
            Some(handler) => {
                let request_id = "list_resources".to_string();
                let extra = RequestHandlerExtra {
                    cancellation_token: self
                        .cancellation_manager
                        .create_token(request_id.clone())
                        .await,
                    request_id,
                    session_id: None,
                    auth_info: None,
                    auth_context: None,
                };
                handler.list(req.cursor.clone(), extra).await
            },
            None => Ok(ListResourcesResult {
                resources: vec![],
                next_cursor: None,
            }),
        }
    }

    /// Handle read resource request.
    async fn handle_read_resource(&self, req: &ReadResourceParams) -> Result<ReadResourceResult> {
        let handler = self.resources.as_ref().ok_or_else(|| {
            Error::internal(format!("Resource handler not available for '{}'", req.uri))
        })?;

        let request_id = format!("read_{}", req.uri);
        let extra = RequestHandlerExtra {
            cancellation_token: self
                .cancellation_manager
                .create_token(request_id.clone())
                .await,
            request_id,
            session_id: None,
            auth_info: None,
            auth_context: None,
        };

        handler.read(&req.uri, extra).await
    }

    /// Handle list resource templates request.
    async fn handle_list_resource_templates(
        &self,
        _req: &ListResourceTemplatesRequest,
    ) -> Result<ListResourceTemplatesResult> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    /// Create an error response.
    fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
        JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            payload: ResponsePayload::Error(JSONRPCError {
                code,
                message,
                data: None,
            }),
        }
    }

    /// Create a success response.
    fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
        JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id,
            payload: ResponsePayload::Result(result),
        }
    }
}

#[async_trait]
impl ProtocolHandler for ServerCore {
    async fn handle_request(&self, id: RequestId, request: Request) -> JSONRPCResponse {
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };

                match self.handle_initialize(init_req).await {
                    Ok(result) => Self::success_response(id, serde_json::to_value(result).unwrap()),
                    Err(e) => Self::error_response(id, -32603, e.to_string()),
                }
            },
            Request::Client(ref boxed_req) => {
                // Check if server is initialized for server requests
                if !self.is_initialized().await {
                    return Self::error_response(
                        id,
                        -32002,
                        "Server not initialized. Call initialize first.".to_string(),
                    );
                }

                match boxed_req.as_ref() {
                    ClientRequest::ListTools(req) => match self.handle_list_tools(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::CallTool(req) => match self.handle_call_tool(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::ListPrompts(req) => match self.handle_list_prompts(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::GetPrompt(req) => match self.handle_get_prompt(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::ListResources(req) => {
                        match self.handle_list_resources(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ReadResource(req) => {
                        match self.handle_read_resource(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListResourceTemplates(req) => {
                        match self.handle_list_resource_templates(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    _ => Self::error_response(id, -32601, "Method not supported".to_string()),
                }
            },
            Request::Server(_) => {
                Self::error_response(id, -32601, "Method not supported".to_string())
            },
        }
    }

    async fn handle_notification(&self, _notification: Notification) -> Result<()> {
        // Handle notifications if needed
        // Most notifications from client to server don't require action
        Ok(())
    }

    fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    fn info(&self) -> &Implementation {
        &self.info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClientCapabilities;

    struct TestTool;

    #[async_trait]
    impl ToolHandler for TestTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }

    #[tokio::test]
    async fn test_server_core_initialization() {
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );

        let server = ServerCore::new(
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            None,
            None,
            None,
            None,
        );

        assert!(!server.is_initialized().await);

        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));

        let response = server.handle_request(RequestId::from(1i64), init_req).await;

        match response.payload {
            ResponsePayload::Result(_) => {
                assert!(server.is_initialized().await);
            },
            ResponsePayload::Error(e) => panic!("Initialization failed: {}", e.message),
        }
    }

    #[tokio::test]
    async fn test_server_core_list_tools() {
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );

        let server = ServerCore::new(
            Implementation {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            None,
            None,
            None,
            None,
        );

        // Initialize first
        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeParams {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        })));
        server.handle_request(RequestId::from(1i64), init_req).await;

        // List tools
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
            cursor: None,
        })));
        let response = server.handle_request(RequestId::from(2i64), list_req).await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(e) => panic!("List tools failed: {}", e.message),
        }
    }
}
