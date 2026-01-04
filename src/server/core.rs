//! Transport-independent MCP server core implementation.
//!
//! This module provides the core server functionality that is decoupled from
//! transport mechanisms, enabling deployment to various environments including
//! WASM/WASI targets.

use crate::error::{Error, Result};
use crate::shared::middleware::{EnhancedMiddlewareChain, MiddlewareContext};
use crate::shared::protocol_helpers::{create_notification, create_request};
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
use super::auth::{AuthContext, AuthProvider, ToolAuthorizer};
#[cfg(not(target_arch = "wasm32"))]
use super::cancellation::{CancellationManager, RequestHandlerExtra};
#[cfg(not(target_arch = "wasm32"))]
use super::roots::RootsManager;
#[cfg(not(target_arch = "wasm32"))]
use super::subscriptions::SubscriptionManager;
#[cfg(not(target_arch = "wasm32"))]
use super::tool_middleware::{ToolContext, ToolMiddlewareChain};
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
    ///
    /// # Parameters
    ///
    /// * `id` - The request ID from the JSON-RPC request
    /// * `request` - The MCP protocol request to handle
    /// * `auth_context` - Optional authentication context from the transport layer
    ///
    /// The `auth_context` parameter enables OAuth token pass-through from the
    /// transport layer to tool middleware, allowing tools to authenticate with
    /// backend services using the user's credentials.
    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse;

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

    /// Protocol middleware chain for request/response/notification processing
    protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,

    /// Tool middleware chain for cross-cutting concerns in tool execution
    #[cfg(not(target_arch = "wasm32"))]
    tool_middleware: Arc<RwLock<ToolMiddlewareChain>>,

    /// Stateless mode flag for serverless deployments
    ///
    /// When true, the server skips initialization state checking, allowing
    /// requests to be processed without requiring an initialize call first.
    /// This is essential for stateless environments like AWS Lambda, Cloudflare
    /// Workers, and other serverless platforms where each request may create
    /// a fresh server instance.
    ///
    /// Default: false (maintains backward compatibility)
    stateless_mode: bool,
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
        protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,
        #[cfg(not(target_arch = "wasm32"))] tool_middleware: Arc<RwLock<ToolMiddlewareChain>>,
        stateless_mode: bool,
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
            protocol_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            tool_middleware,
            stateless_mode,
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
                    ToolInfo::new(name.clone(), None, serde_json::json!({}))
                }
            })
            .collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    /// Handle call tool request.
    async fn handle_call_tool(
        &self,
        req: &CallToolParams,
        auth_context: Option<AuthContext>,
    ) -> Result<CallToolResult> {
        let handler = self
            .tools
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Tool '{}' not found", req.name)))?;

        // Authorization check with tool_authorizer if available
        if let Some(authorizer) = &self.tool_authorizer {
            if let Some(ref auth_ctx) = auth_context {
                if !authorizer.can_access_tool(auth_ctx, &req.name).await? {
                    return Err(Error::authentication(format!(
                        "User not authorized to call tool '{}'",
                        req.name
                    )));
                }
            }
        }

        // Create request handler extra data with auth_context
        let request_id = format!("tool_{}", req.name);
        let mut extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

        // Execute tool with or without middleware depending on platform
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            // Create tool context for middleware
            let context = ToolContext::new(&req.name, &request_id);

            // Clone arguments for middleware processing
            let mut args = req.arguments.clone();

            // Process request through tool middleware chain
            // Middleware rejection short-circuits tool execution (on_error already called by chain)
            self.tool_middleware
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Execute the tool with potentially modified args and extra
            let mut result = handler.handle(args, extra).await;

            // Process response through tool middleware chain
            if let Err(e) = self
                .tool_middleware
                .read()
                .await
                .process_response(&req.name, &mut result, &context)
                .await
            {
                // Log error but continue with original result
                tracing::warn!("Tool response middleware processing failed: {}", e);
            }

            // If tool execution failed, call handle_tool_error
            if let Err(ref e) = result {
                self.tool_middleware
                    .read()
                    .await
                    .handle_tool_error(&req.name, e, &context)
                    .await;
            }

            result
        };

        #[cfg(target_arch = "wasm32")]
        let result = {
            // On WASM, execute tool directly without middleware
            let args = req.arguments.clone();
            handler.handle(args, extra).await
        };

        // Convert result to CallToolResult
        let value = result?;
        Ok(CallToolResult {
            content: vec![Content::Text {
                text: serde_json::to_string_pretty(&value)?,
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
                    tracing::debug!(
                        target: "mcp.prompts",
                        prompt = %name,
                        description = ?info.description,
                        arguments_count = ?info.arguments.as_ref().map(|a| a.len()),
                        "Prompt metadata retrieved"
                    );

                    // Ensure the name matches the registered name
                    info.name.clone_from(name);

                    tracing::debug!(
                        target: "mcp.prompts",
                        prompt = %info.name,
                        has_description = info.description.is_some(),
                        has_arguments = info.arguments.is_some(),
                        "Final PromptInfo"
                    );

                    info
                } else {
                    tracing::debug!(
                        target: "mcp.prompts",
                        prompt = %name,
                        "Prompt has no metadata"
                    );
                    PromptInfo {
                        name: name.clone(),
                        description: None,
                        arguments: None,
                    }
                }
            })
            .collect();

        tracing::debug!(
            target: "mcp.prompts",
            count = prompts.len(),
            "Returning prompts"
        );

        Ok(ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    /// Handle get prompt request.
    async fn handle_get_prompt(
        &self,
        req: &GetPromptParams,
        auth_context: Option<AuthContext>,
    ) -> Result<GetPromptResult> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Prompt '{}' not found", req.name)))?;

        // Create request handler extra data with auth_context
        let request_id = format!("prompt_{}", req.name);
        let extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

        handler.handle(req.arguments.clone(), extra).await
    }

    /// Handle list resources request.
    async fn handle_list_resources(
        &self,
        req: &ListResourcesParams,
        auth_context: Option<AuthContext>,
    ) -> Result<ListResourcesResult> {
        match &self.resources {
            Some(handler) => {
                let request_id = "list_resources".to_string();
                let extra = RequestHandlerExtra::new(
                    request_id.clone(),
                    self.cancellation_manager
                        .create_token(request_id.clone())
                        .await,
                )
                .with_auth_context(auth_context);
                handler.list(req.cursor.clone(), extra).await
            },
            None => Ok(ListResourcesResult {
                resources: vec![],
                next_cursor: None,
            }),
        }
    }

    /// Handle read resource request.
    async fn handle_read_resource(
        &self,
        req: &ReadResourceParams,
        auth_context: Option<AuthContext>,
    ) -> Result<ReadResourceResult> {
        let handler = self.resources.as_ref().ok_or_else(|| {
            Error::internal(format!("Resource handler not available for '{}'", req.uri))
        })?;

        let request_id = format!("read_{}", req.uri);
        let extra = RequestHandlerExtra::new(
            request_id.clone(),
            self.cancellation_manager
                .create_token(request_id.clone())
                .await,
        )
        .with_auth_context(auth_context);

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

// Implement MiddlewareExecutor for ServerCore to enable workflow tool execution
// with consistent middleware application
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl crate::server::middleware_executor::MiddlewareExecutor for ServerCore {
    async fn execute_tool_with_middleware(
        &self,
        tool_name: &str,
        mut args: Value,
        mut extra: RequestHandlerExtra,
    ) -> Result<Value> {
        // Get the tool handler
        let handler = self
            .tools
            .get(tool_name)
            .ok_or_else(|| Error::internal(format!("Tool '{}' not found", tool_name)))?;

        // Authorization check with tool_authorizer if available
        if let Some(authorizer) = &self.tool_authorizer {
            if let Some(ref auth_ctx) = extra.auth_context {
                if !authorizer.can_access_tool(auth_ctx, tool_name).await? {
                    return Err(Error::authentication(format!(
                        "User not authorized to call tool '{}'",
                        tool_name
                    )));
                }
            }
        }

        // Create tool context for middleware
        let context = ToolContext::new(tool_name, &extra.request_id);

        // Process request through tool middleware chain
        // Middleware rejection short-circuits tool execution (on_error already called by chain)
        self.tool_middleware
            .read()
            .await
            .process_request(tool_name, &mut args, &mut extra, &context)
            .await?;

        // Execute the tool with potentially modified args and extra
        let mut result = handler.handle(args, extra).await;

        // Process response through tool middleware chain
        if let Err(e) = self
            .tool_middleware
            .read()
            .await
            .process_response(tool_name, &mut result, &context)
            .await
        {
            // Log error but continue with original result
            tracing::warn!("Tool response middleware processing failed: {}", e);
        }

        // If tool execution failed, call handle_tool_error
        if let Err(ref e) = result {
            self.tool_middleware
                .read()
                .await
                .handle_tool_error(tool_name, e, &context)
                .await;
        }

        result
    }
}

#[async_trait]
impl ProtocolHandler for ServerCore {
    async fn handle_request(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse {
        // Convert Request to JSONRPCRequest for middleware processing
        let mut jsonrpc_request = create_request(id.clone(), request.clone());

        // Create middleware context with request_id, method, and start_time
        let context = MiddlewareContext::with_request_id(id.to_string());
        context.set_metadata("method".to_string(), jsonrpc_request.method.clone());

        // Process request through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_request_with_context(&mut jsonrpc_request, &context)
            .await
        {
            // Middleware rejected the request (on_error already called by chain)
            return Self::error_response(id, -32603, e.to_string());
        }

        // Execute the actual request handling with auth_context
        let mut response = self
            .handle_request_internal(id.clone(), request, auth_context)
            .await;

        // Process response through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_response_with_context(&mut response, &context)
            .await
        {
            // Log error but return the response anyway
            tracing::warn!("Response middleware processing failed: {}", e);
        }

        response
    }

    async fn handle_notification(&self, notification: Notification) -> Result<()> {
        // Convert Notification to JSONRPCNotification for middleware processing
        let mut jsonrpc_notification = create_notification(notification.clone());

        // Create middleware context with method and start_time (no request_id for notifications)
        let context = MiddlewareContext::default();
        context.set_metadata("method".to_string(), jsonrpc_notification.method.clone());

        // Process notification through protocol middleware chain (read-only access)
        if let Err(e) = self
            .protocol_middleware
            .read()
            .await
            .process_notification_with_context(&mut jsonrpc_notification, &context)
            .await
        {
            // Log error but continue
            tracing::warn!("Notification middleware processing failed: {}", e);
        }

        // Handle the actual notification (current implementation does nothing)
        self.handle_notification_internal(notification).await
    }

    fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    fn info(&self) -> &Implementation {
        &self.info
    }
}

impl ServerCore {
    /// Internal request handler without middleware processing.
    async fn handle_request_internal(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
    ) -> JSONRPCResponse {
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
                // Check if server is initialized for server requests (skip in stateless mode)
                // Stateless mode is for serverless deployments where each request may create
                // a fresh server instance (AWS Lambda, Cloudflare Workers, etc.)
                if !self.stateless_mode && !self.is_initialized().await {
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
                    ClientRequest::CallTool(req) => {
                        match self.handle_call_tool(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListPrompts(req) => match self.handle_list_prompts(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(id, -32603, e.to_string()),
                    },
                    ClientRequest::GetPrompt(req) => {
                        match self.handle_get_prompt(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ListResources(req) => {
                        match self.handle_list_resources(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(id, -32603, e.to_string()),
                        }
                    },
                    ClientRequest::ReadResource(req) => {
                        match self.handle_read_resource(req, auth_context.clone()).await {
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

    /// Internal notification handler without middleware processing.
    async fn handle_notification_internal(&self, _notification: Notification) -> Result<()> {
        // Handle notifications if needed
        // Most notifications from client to server don't require action
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::tool_middleware::ToolMiddlewareChain;
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
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            false, // stateless_mode
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

        let response = server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

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
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            false, // stateless_mode
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
        server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

        // List tools
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(2i64), list_req, None)
            .await;

        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(e) => panic!("List tools failed: {}", e.message),
        }
    }

    #[tokio::test]
    async fn test_stateless_mode_allows_requests_without_init() {
        // Create server in stateless mode
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
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            true, // stateless_mode enabled
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), list_req, None)
            .await;

        // Should succeed in stateless mode
        match response.payload {
            ResponsePayload::Result(result) => {
                let tools_result: ListToolsResult = serde_json::from_value(result).unwrap();
                assert_eq!(tools_result.tools.len(), 1);
                assert_eq!(tools_result.tools[0].name, "test-tool");
            },
            ResponsePayload::Error(e) => panic!(
                "List tools should succeed in stateless mode without init: {}",
                e.message
            ),
        }
    }

    #[tokio::test]
    async fn test_normal_mode_requires_initialization() {
        // Create server in normal mode (stateless_mode = false)
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
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            false, // stateless_mode disabled (normal mode)
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsParams {
            cursor: None,
        })));
        let response = server
            .handle_request(RequestId::from(1i64), list_req, None)
            .await;

        // Should fail in normal mode
        match response.payload {
            ResponsePayload::Result(_) => {
                panic!("List tools should fail in normal mode without initialization")
            },
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32002);
                assert!(e.message.contains("not initialized"));
            },
        }
    }
}
