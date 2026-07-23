//! Transport-independent MCP server core implementation.
//!
//! This module provides the core server functionality that is decoupled from
//! transport mechanisms, enabling deployment to various environments including
//! WASM/WASI targets.

use crate::error::{Error, Result};
use crate::server::limits::PayloadLimits;
use crate::shared::middleware::{EnhancedMiddlewareChain, MiddlewareContext};
use crate::shared::protocol_helpers::{create_notification, create_request};
// `ResponsePayload` is needed by the wasm-only envelope-builder branch (the
// non-wasm path delegates to `task_dispatch`) and by the test module.
// `JSONRPCError` is needed only by the wasm-only branch.
#[cfg(any(target_arch = "wasm32", test))]
use crate::types::jsonrpc::ResponsePayload;
#[cfg(target_arch = "wasm32")]
use crate::types::JSONRPCError;
use crate::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, Content, GetPromptRequest,
    GetPromptResult, Implementation, InitializeRequest, InitializeResult, JSONRPCResponse,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest,
    ListToolsResult, Notification, PromptInfo, ProtocolVersion, ReadResourceRequest,
    ReadResourceResult, Request, RequestId, ServerCapabilities, ToolInfo,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashSet;
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
use super::tasks::TaskRouter;
#[cfg(not(target_arch = "wasm32"))]
use super::tool_middleware::{ToolContext, ToolMiddlewareChain};
use super::{PromptHandler, ResourceHandler, SamplingHandler, ToolHandler};
#[cfg(not(target_arch = "wasm32"))]
use crate::types::tools::TaskSupport;

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

/// Enrich a tool's `_meta` with host-specific keys.
///
/// Reads the standard `ui.resourceUri` and adds host-specific aliases.
/// For `ChatGpt`, this adds `openai/outputTemplate`, `openai/widgetAccessible`,
/// and default `openai/toolInvocation/*` messages. Uses `entry().or_insert` so
/// server-provided values are never overwritten.
#[cfg(feature = "mcp-apps")]
pub(crate) fn enrich_meta_for_host(
    meta: &mut serde_json::Map<String, serde_json::Value>,
    host: crate::types::mcp_apps::HostType,
) {
    use crate::types::mcp_apps::HostType;

    if host == HostType::ChatGpt {
        // Extract URI from standard nested key
        if let Some(uri) = meta
            .get("ui")
            .and_then(|v| v.get("resourceUri"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            meta.entry("openai/outputTemplate".to_string())
                .or_insert_with(|| serde_json::Value::String(uri));
            meta.entry("openai/widgetAccessible".to_string())
                .or_insert(serde_json::Value::Bool(true));
            meta.entry("openai/toolInvocation/invoking".to_string())
                .or_insert_with(|| serde_json::Value::String("Running...".into()));
            meta.entry("openai/toolInvocation/invoked".to_string())
                .or_insert_with(|| serde_json::Value::String("Done".into()));
        }
    }
    // Claude, McpUi, Generic: no enrichment needed (standard keys only)
}

/// Keys to propagate from tool `_meta` to resource `_meta` via the URI index.
///
/// Includes the standard `ui` nested object and all `openai/*` descriptor keys
/// (which are only present if a host layer was applied). Display-only keys
/// (`openai/widgetPrefersBorder`, `openai/widgetDescription`, `openai/widgetCSP`,
/// `openai/widgetDomain`) are excluded to avoid breaking `ChatGPT`'s Templates.
const RESOURCE_PROPAGATION_PREFIXES: &[&str] = &[
    "openai/outputTemplate",
    "openai/toolInvocation/",
    "openai/widgetAccessible",
];

/// Build a URI-to-tool-meta index from registered tool metadata.
///
/// Maps resource URIs (from `ui.resourceUri` nested key) to the linked tool's
/// propagation-eligible `_meta` keys. Used to auto-propagate widget descriptor
/// keys onto `ResourceInfo` during `resources/list` and `resources/read`.
/// When multiple tools share the same URI, first tool registered wins.
pub(crate) fn build_uri_to_tool_meta(
    tool_infos: &HashMap<String, ToolInfo>,
) -> HashMap<String, serde_json::Map<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for info in tool_infos.values() {
        if let Some(meta) = info.widget_meta() {
            // Index by standard nested ui.resourceUri key
            let uri = meta
                .get("ui")
                .and_then(|v| v.get("resourceUri"))
                .and_then(|v| v.as_str());
            if let Some(uri) = uri {
                // Collect propagation-eligible keys
                let propagated: serde_json::Map<String, serde_json::Value> = meta
                    .iter()
                    .filter(|(k, _)| {
                        RESOURCE_PROPAGATION_PREFIXES
                            .iter()
                            .any(|prefix| k.starts_with(prefix))
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                // First tool registered wins (per user decision).
                // Skip empty propagation maps to avoid `_meta: {}` on resources/list.
                if !propagated.is_empty() {
                    map.entry(uri.to_string()).or_insert(propagated);
                }
            }
        }
    }
    map
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

    /// Cached tool metadata (populated at registration, immutable)
    tool_infos: HashMap<String, ToolInfo>,

    /// Cached URI-to-tool-meta index for widget resource `_meta` propagation.
    /// Maps resource URIs (from `ui.resourceUri`) to propagation-eligible `_meta` keys.
    uri_to_tool_meta: HashMap<String, serde_json::Map<String, serde_json::Value>>,

    /// Cached prompt metadata (populated at registration, immutable)
    prompt_infos: HashMap<String, PromptInfo>,

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

    /// Task router for experimental MCP Tasks support (optional)
    #[cfg(not(target_arch = "wasm32"))]
    task_router: Option<Arc<dyn TaskRouter>>,

    /// Task store for MCP Tasks with polling (standard capability path)
    #[cfg(not(target_arch = "wasm32"))]
    task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,

    /// Per-tool TOUT-02 double-wrap tripwire opt-out set (D-08). A tool named
    /// here has the tripwire suppressed at the Payload wrap tail. Populated via
    /// [`ServerCore::with_suppress_double_wrap`] from
    /// `ServerCoreBuilder::suppress_double_wrap_check`; the high-level `Server`
    /// carries an IDENTICAL set so both dispatchers consult the same rule.
    #[cfg(not(target_arch = "wasm32"))]
    suppress_double_wrap: HashSet<String>,

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

    /// Payload and resource limits for denial-of-service protection
    payload_limits: PayloadLimits,

    /// The configured protocol-version accept-list (Phase 112, VERS-01/02).
    ///
    /// Defaults to the v1-only legacy set ([`default_accept_list`](crate::types::protocol::context::default_accept_list),
    /// which EXCLUDES `2026-07-28`) unless the author opts into v2 via
    /// [`ServerCoreBuilder::with_supported_protocol_versions`](crate::server::builder::ServerCoreBuilder::with_supported_protocol_versions).
    /// Read at ingress to decide whether to run era-detection at all
    /// ([`is_v2_opted_in`](Self::is_v2_opted_in)) and to enforce the accept-list
    /// in the shared resolver. A non-opted-in server behaves exactly as today.
    supported_protocol_versions: Vec<ProtocolVersion>,

    /// Outbound server-to-client request dispatcher.
    ///
    /// Populated by the enclosing `Server` via
    /// [`ServerCore::with_server_request_dispatcher`]. Consumed at dispatch
    /// sites to construct per-request peer handles via `attach_peer`.
    /// `None` preserves the graceful-fallback contract for every existing
    /// `ServerCore::new()` call site.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::struct_field_names)]
    server_request_dispatcher:
        Option<Arc<crate::server::server_request_dispatcher::ServerRequestDispatcher>>,

    /// Cached peer handle built alongside the dispatcher.
    /// One Arc allocation at setup time; dispatch sites clone this Arc
    /// (refcount bump) rather than constructing a fresh `DispatchPeerHandle`
    /// per request.
    #[cfg(not(target_arch = "wasm32"))]
    peer_handle: Option<Arc<dyn crate::shared::peer::PeerHandle>>,
}

/// Outcome of a tool handler call — either a normal result or a task creation.
enum ToolCallOutcome {
    /// Standard tool result wrapped as `CallToolResult`
    Result(CallToolResult),
    /// Tool returned a Task-shaped value — returned as `CreateTaskResult` with `_meta`.
    ///
    /// Carries the raw task-shaped tool `Value`. The shared
    /// `task_dispatch::TaskDispatch::build_task_created_response` re-extracts the
    /// task id and the terminal [`CallToolResult`] from this value (store mints the
    /// canonical id; terminal result drives synchronous-completion persistence).
    #[cfg(not(target_arch = "wasm32"))]
    TaskCreated { task_value: Value },
}

impl ServerCore {
    /// Create a new `ServerCore` with the given configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        info: Implementation,
        capabilities: ServerCapabilities,
        tools: HashMap<String, Arc<dyn ToolHandler>>,
        prompts: HashMap<String, Arc<dyn PromptHandler>>,
        tool_infos: HashMap<String, ToolInfo>,
        prompt_infos: HashMap<String, PromptInfo>,
        resources: Option<Arc<dyn ResourceHandler>>,
        sampling: Option<Arc<dyn SamplingHandler>>,
        auth_provider: Option<Arc<dyn AuthProvider>>,
        tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
        protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,
        #[cfg(not(target_arch = "wasm32"))] tool_middleware: Arc<RwLock<ToolMiddlewareChain>>,
        #[cfg(not(target_arch = "wasm32"))] task_router: Option<Arc<dyn TaskRouter>>,
        #[cfg(not(target_arch = "wasm32"))] task_store: Option<
            Arc<dyn crate::server::task_store::TaskStore>,
        >,
        stateless_mode: bool,
        payload_limits: PayloadLimits,
    ) -> Self {
        let uri_to_tool_meta = build_uri_to_tool_meta(&tool_infos);
        Self {
            info,
            capabilities,
            tools,
            prompts,
            tool_infos,
            uri_to_tool_meta,
            prompt_infos,
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
            #[cfg(not(target_arch = "wasm32"))]
            task_router,
            #[cfg(not(target_arch = "wasm32"))]
            task_store,
            #[cfg(not(target_arch = "wasm32"))]
            suppress_double_wrap: HashSet::new(),
            stateless_mode,
            payload_limits,
            supported_protocol_versions: crate::types::protocol::context::default_accept_list(),
            #[cfg(not(target_arch = "wasm32"))]
            server_request_dispatcher: None,
            #[cfg(not(target_arch = "wasm32"))]
            peer_handle: None,
        }
    }

    /// Attach a server-to-client request dispatcher.
    ///
    /// The dispatcher is the outbound-plus-correlation layer consumed at
    /// handler dispatch sites so tool handlers can invoke
    /// `extra.peer()?.sample(...)` mid-execution. Calling this is optional —
    /// when absent, existing behaviour (no peer handle) is preserved.
    ///
    /// Also constructs and caches a reusable `Arc<dyn PeerHandle>` so
    /// per-request dispatch only clones the Arc (refcount bump), not
    /// allocating a new `DispatchPeerHandle` each time.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_server_request_dispatcher(
        mut self,
        dispatcher: Arc<crate::server::server_request_dispatcher::ServerRequestDispatcher>,
    ) -> Self {
        let peer: Arc<dyn crate::shared::peer::PeerHandle> = Arc::new(
            crate::server::peer_impl::DispatchPeerHandle::new(dispatcher.clone()),
        );
        self.peer_handle = Some(peer);
        self.server_request_dispatcher = Some(dispatcher);
        self
    }

    /// Carry the per-tool TOUT-02 double-wrap tripwire opt-out set (D-08) from
    /// the builder into the running `ServerCore`.
    ///
    /// Threaded from `ServerCoreBuilder::build` so the tripwire at the Payload
    /// wrap tail consults the SAME suppression set the high-level `Server` uses —
    /// the two dispatchers can never drift on which tools are suppressed. An
    /// empty set (the default) preserves the tripwire for every tool.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_suppress_double_wrap(mut self, suppress: HashSet<String>) -> Self {
        self.suppress_double_wrap = suppress;
        self
    }

    /// Carry the configured protocol-version accept-list (Phase 112, VERS-01/02)
    /// from the builder into the running `ServerCore`.
    ///
    /// Threaded from [`ServerCoreBuilder::build`](crate::server::builder::ServerCoreBuilder::build)
    /// so ingress era-resolution reads the exact set the author opted into. The
    /// builder guarantees a non-empty list (an explicitly-empty accept-list falls
    /// back to the v1-only default), so this never installs an all-reject server.
    #[must_use]
    pub(crate) fn with_supported_protocol_versions(
        mut self,
        versions: Vec<ProtocolVersion>,
    ) -> Self {
        self.supported_protocol_versions = versions;
        self
    }

    /// The configured protocol-version accept-list read at ingress (test-only
    /// accessor; production reads the field directly via the shared resolver).
    #[cfg(test)]
    pub(crate) fn supported_protocol_versions(&self) -> &[ProtocolVersion] {
        &self.supported_protocol_versions
    }

    /// Whether this server opted into the v2 (`2026-07-28`) era (test-only
    /// convenience over [`context::is_v2_opted_in`](crate::types::protocol::context::is_v2_opted_in);
    /// production resolves opt-in inside the shared ingress resolver).
    #[cfg(test)]
    pub(crate) fn is_v2_opted_in(&self) -> bool {
        crate::types::protocol::context::is_v2_opted_in(&self.supported_protocol_versions)
    }

    /// Resolve the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
    /// ONCE at native ingress (Phase 112, VERS-01) via the shared free
    /// [`resolve_ingress_protocol_context`] both dispatch surfaces call. The
    /// `Err` is mapped to a structured rejection by the caller.
    fn resolve_ingress_protocol_context(
        &self,
        request: &Request,
    ) -> std::result::Result<
        Option<crate::types::protocol::ProtocolContext>,
        crate::types::protocol::context::ProtocolNegotiationError,
    > {
        resolve_ingress_protocol_context(&self.supported_protocol_versions, request)
    }

    /// Attach the cached peer handle to `extra` when a dispatcher is configured.
    /// No-op on wasm32 (peer is non-wasm) and when no dispatcher is attached.
    #[inline]
    fn attach_peer(&self, extra: RequestHandlerExtra) -> RequestHandlerExtra {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(peer) = self.peer_handle.as_ref() {
            return extra.with_peer(peer.clone());
        }
        extra
    }

    /// Get the configured payload limits.
    pub fn payload_limits(&self) -> &PayloadLimits {
        &self.payload_limits
    }

    /// Check if the server is initialized.
    pub async fn is_initialized(&self) -> bool {
        contract_pre_session_lifecycle!();
        *self.initialized.read().await
    }

    /// Get client capabilities if available.
    pub async fn get_client_capabilities(&self) -> Option<ClientCapabilities> {
        self.client_capabilities.read().await.clone()
    }

    /// Handle initialization request.
    async fn handle_initialize(&self, init_req: &InitializeRequest) -> Result<InitializeResult> {
        contract_pre_session_lifecycle!();
        // Store client capabilities
        *self.client_capabilities.write().await = Some(init_req.capabilities.clone());
        *self.initialized.write().await = true;

        let negotiated_version = crate::negotiate_protocol_version(&init_req.protocol_version);

        Ok(InitializeResult {
            protocol_version: ProtocolVersion(negotiated_version.to_string()),
            capabilities: self.capabilities.clone(),
            server_info: self.info.clone(),
            instructions: None,
        })
    }

    /// Handle list tools request.
    async fn handle_list_tools(&self, _req: &ListToolsRequest) -> Result<ListToolsResult> {
        contract_pre_tool_dispatch_integrity!();
        let tools: Vec<ToolInfo> = self.tool_infos.values().cloned().collect();

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    /// Handle call tool request.
    async fn handle_call_tool(
        &self,
        req: &CallToolRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<ToolCallOutcome> {
        contract_pre_tool_dispatch_integrity!();
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

        // Create request handler extra data with auth_context and task request.
        // Middleware below takes `&mut extra`, so bind as mut.
        let request_id = format!("tool_{}", req.name);
        let mut extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_task_request(req.task.clone())
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            // Thread the once-at-ingress resolved protocol context so handlers
            // read era/identity via extra.era()/client_info() (Phase 112).
            .with_protocol_context(protocol_context),
        );

        // D-03.3 (TOUT-01): clone the result-`_meta` slot before `extra` moves
        // into `handle_output` (see the high-level `Server` dispatcher for the
        // twin); drained onto the Payload envelope after the handler returns.
        #[cfg(not(target_arch = "wasm32"))]
        let result_meta_handle = extra.result_meta_handle();

        // Execute tool with or without middleware depending on platform
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            // Create tool context for middleware
            let context = ToolContext::new(&req.name, &request_id);

            // Clone arguments for middleware processing
            let mut args = req.arguments.clone();

            // Process request through tool middleware chain.
            // Middleware rejection short-circuits tool execution (on_error already
            // called by chain). REQUEST middleware runs BEFORE the handler for
            // EVERY tool, regardless of the ToolOutput variant it returns.
            self.tool_middleware
                .read()
                .await
                .process_request(&req.name, &mut args, &mut extra, &context)
                .await?;

            // Enforce tool argument size limit (post-middleware, so inflated args are caught)
            if self.payload_limits.max_tool_args_bytes < usize::MAX {
                let args_size = json_serialized_len(&args)?;
                if args_size > self.payload_limits.max_tool_args_bytes {
                    return Err(Error::validation(format!(
                        "Tool arguments for '{}' exceed size limit ({} bytes > {} max)",
                        req.name, args_size, self.payload_limits.max_tool_args_bytes
                    )));
                }
            }

            // Execute the tool. `handle_output` returns `Result<ToolOutput>`; the
            // SHARED `resolve_tool_output` (D-05) is the SINGLE place that decides
            // Payload-vs-Result and encodes the response-middleware-bypass rule, so
            // this dispatcher and the high-level `Server` can never drift on it.
            let output = handler.handle_output(args, extra).await;
            match crate::server::task_dispatch::resolve_tool_output(output) {
                // VERBATIM (D-04 + D-04a — USER-APPROVED and LOCKED: "keep the
                // bypass, harden it"): the handler owns the full `CallToolResult`
                // envelope, including its own redaction/sanitization. Emit it as-is
                // — bypassing RESPONSE middleware (redaction/sanitization/audit),
                // the create-path gate, and the text-wrap / widget-enrichment tail.
                // REQUEST middleware already fired above for every tool, and a
                // handler `Err(_)` still routes through the Middleware arm below.
                crate::server::task_dispatch::DispatchOutput::Verbatim(call_result) => {
                    return Ok(ToolCallOutcome::Result(call_result));
                },
                crate::server::task_dispatch::DispatchOutput::Middleware(mut result) => {
                    // Process response through tool middleware chain (Payload/error only)
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
                },
            }
        };

        #[cfg(target_arch = "wasm32")]
        let result = {
            // On WASM, execute tool directly without middleware
            let args = req.arguments.clone();
            handler.handle(args, extra).await
        };

        // Convert result to CallToolResult.
        //
        // `Error::ToolRejected` is an APPLICATION-level rejection (e.g. Code
        // Mode policy: a SELECT missing its LIMIT), not a protocol fault. Map
        // it to a successful `CallToolResult { isError: true }` so the model
        // reads the reason + suggestions and retries with corrected input —
        // rather than `?`-propagating it into a JSON-RPC error that reads as a
        // server crash. All other errors keep propagating as protocol errors.
        let value = match result {
            Ok(value) => value,
            Err(crate::error::Error::ToolRejected { message, details }) => {
                return Ok(ToolCallOutcome::Result(CallToolResult::rejected(
                    message, details,
                )));
            },
            Err(e) => return Err(e),
        };
        let tool_info = self.tool_infos.get(&req.name);

        // Task detection: return CreateTaskResult only when ALL of:
        // 1. task_store is configured
        // 2. Tool declares taskSupport (Required or Optional)
        // 3. Client sent `task` field in the request (explicit task-augmented call)
        // 4. Tool returned a Task-shaped Value (has taskId + status)
        // When the client doesn't send `task`, fall through to CallToolResult
        // so non-task-aware clients (like ChatGPT) get normal tool output.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let tool_task_support = tool_info
                .as_ref()
                .and_then(|info| info.execution.as_ref())
                .and_then(|exec| exec.task_support.as_ref())
                .copied();

            // Warn when a Required tool is called without task-augmented request
            if req.task.is_none() && matches!(tool_task_support, Some(TaskSupport::Required)) {
                tracing::warn!(
                    tool = req.name.as_str(),
                    "Tool declares taskSupport=Required but client did not send task field; returning CallToolResult for compatibility"
                );
            }

            let has_task_support = req.task.is_some()
                && self.task_store.is_some()
                && tool_task_support
                    .is_some_and(|ts| matches!(ts, TaskSupport::Required | TaskSupport::Optional));

            if has_task_support {
                // Task-shaped iff it carries both a `taskId` and a `status` (same
                // shape gate as `task_dispatch::maybe_build_task_created`). The
                // shared create-path re-extracts the task id + terminal
                // CallToolResult from the value, so only the raw value crosses here.
                let is_task_shaped = value.get("taskId").and_then(|v| v.as_str()).is_some()
                    && value.get("status").is_some();
                if is_task_shaped {
                    return Ok(ToolCallOutcome::TaskCreated { task_value: value });
                }
                // Tool declares task support but didn't return a Task — fall through to normal path
                // (handles the "optional" case where the tool might not create a task).
                tracing::debug!(
                    tool = req.name.as_str(),
                    "Tool declares taskSupport but returned non-Task value; using normal CallToolResult path"
                );
            }
        }

        // TOUT-02 double-wrap tripwire: BEFORE this tail text-wraps `value` into
        // content, WARN (+ debug_assert in debug/CI) if it structurally resembles
        // an already-built `CallToolResult` — the silent double-wrap bug. Honors
        // the per-tool `suppress_double_wrap_check` opt-out (D-08) via the SAME
        // suppression set the high-level `Server` uses, so the two dispatchers
        // never drift. Non-wasm only (mirrors the create-path gate above).
        #[cfg(not(target_arch = "wasm32"))]
        crate::server::task_dispatch::double_wrap_tripwire(
            &req.name,
            &value,
            self.suppress_double_wrap.contains(req.name.as_str()),
        );

        // A declared outputSchema means structuredContent is emitted below
        // (via widget enrichment or the schema bridge) — validate the value
        // against it regardless of which branch does the emitting.
        if let Some(schema) = tool_info.and_then(|i| i.output_schema.as_ref()) {
            crate::server::output_validation::warn_on_schema_mismatch(&req.name, schema, &value);
        }

        let call_result = if let Some(info) = tool_info.filter(|i| i.widget_meta().is_some()) {
            // Widget tool: structured data goes in structuredContent,
            // text is a brief summary to avoid duplication in `ChatGPT`
            let summary = summarize_structured_output(&value);
            CallToolResult::new(vec![Content::text(summary)]).with_widget_enrichment(info, value)
        } else if tool_info.is_some_and(|i| i.output_schema.is_some()) {
            // Declared outputSchema: bridge it to the wire (MCP spec — a tool
            // that declares an outputSchema SHOULD return structuredContent
            // conforming to it). Dual-emit (compact text voice, matching the
            // high-level `Server` dispatcher) keeps text-only clients working.
            CallToolResult::structured(value)
        } else {
            let text = serde_json::to_string_pretty(&value)?;
            CallToolResult::new(vec![Content::text(text)])
        };

        // D-03.3: drain any handler-set result `_meta` onto the Payload envelope
        // with handler-key-wins precedence (Payload path only; the Verbatim,
        // create-path, and error arms all returned earlier). Shadow-rebind so the
        // wasm branch, which never sets the slot, needs no `mut`.
        #[cfg(not(target_arch = "wasm32"))]
        let call_result = {
            let mut call_result = call_result;
            if let Some(handler_meta) = result_meta_handle.take_result_meta() {
                crate::server::cancellation::merge_result_meta(&mut call_result, handler_meta);
            }
            call_result
        };

        Ok(ToolCallOutcome::Result(call_result))
    }

    /// Handle list prompts request.
    async fn handle_list_prompts(&self, _req: &ListPromptsRequest) -> Result<ListPromptsResult> {
        let prompts: Vec<PromptInfo> = self.prompt_infos.values().cloned().collect();

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
        req: &GetPromptRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<GetPromptResult> {
        let handler = self
            .prompts
            .get(&req.name)
            .ok_or_else(|| Error::internal(format!("Prompt '{}' not found", req.name)))?;

        // Create request handler extra data with auth_context, the request `_meta`
        // (so handlers read trace-context/namespaced keys via extra), and the
        // once-at-ingress resolved protocol context (so handlers read
        // era/client_info via extra.era()/client_info() — Phase 112, mirrors
        // handle_call_tool).
        let request_id = format!("prompt_{}", req.name);
        let extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            .with_protocol_context(protocol_context),
        );

        handler.handle(req.arguments.clone(), extra).await
    }

    /// Handle list resources request.
    async fn handle_list_resources(
        &self,
        req: &ListResourcesRequest,
        auth_context: Option<AuthContext>,
    ) -> Result<ListResourcesResult> {
        let mut result = match &self.resources {
            Some(handler) => {
                let request_id = "list_resources".to_string();
                let extra = self.attach_peer(
                    RequestHandlerExtra::new(
                        request_id.clone(),
                        self.cancellation_manager
                            .create_token(request_id.clone())
                            .await,
                    )
                    .with_auth_context(auth_context),
                );
                handler.list(req.cursor.clone(), extra).await?
            },
            None => ListResourcesResult {
                resources: vec![],
                next_cursor: None,
            },
        };

        // Enrich ResourceInfo items with tool _meta for widget resources.
        // Only resources with URIs in the uri_to_tool_meta index (built from
        // tool _meta at construction) receive _meta -- non-widget resources
        // are unaffected.
        if !self.uri_to_tool_meta.is_empty() {
            for resource in &mut result.resources {
                if let Some(tool_meta) = self.uri_to_tool_meta.get(&resource.uri) {
                    let meta = resource.meta.get_or_insert_with(serde_json::Map::new);
                    crate::types::ui::deep_merge(meta, tool_meta.clone());
                }
            }
        }

        Ok(result)
    }

    /// Handle read resource request.
    async fn handle_read_resource(
        &self,
        req: &ReadResourceRequest,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> Result<ReadResourceResult> {
        let handler = self.resources.as_ref().ok_or_else(|| {
            Error::internal(format!("Resource handler not available for '{}'", req.uri))
        })?;

        // Thread the request `_meta` + once-at-ingress resolved protocol context
        // into `extra` so resource handlers read era/client_info/trace_context on
        // a v2 connection (Phase 112, mirrors handle_call_tool / handle_get_prompt).
        let request_id = format!("read_{}", req.uri);
        let extra = self.attach_peer(
            RequestHandlerExtra::new(
                request_id.clone(),
                self.cancellation_manager
                    .create_token(request_id.clone())
                    .await,
            )
            .with_auth_context(auth_context)
            .with_request_meta(request_meta_to_value(req._meta.as_ref()))
            .with_protocol_context(protocol_context),
        );

        let mut result = handler.read(&req.uri, extra).await?;

        // Merge tool descriptor keys into content _meta for widget resources.
        // Display keys (from ChatGptAdapter/WidgetMeta) are already in content
        // meta. Descriptor keys (openai/outputTemplate, openai/widgetAccessible,
        // etc.) come from the linked tool's _meta via the uri_to_tool_meta index.
        if !self.uri_to_tool_meta.is_empty() {
            for content in &mut result.contents {
                if let Content::Resource { uri, meta, .. } = content {
                    if let Some(tool_meta) = self.uri_to_tool_meta.get(uri.as_str()) {
                        let content_meta = meta.get_or_insert_with(serde_json::Map::new);
                        crate::types::ui::deep_merge(content_meta, tool_meta.clone());
                    }
                }
            }
        }

        Ok(result)
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
    ///
    /// Delegates to the SINGLE-SOURCE envelope builder in `task_dispatch` so the
    /// shared task unit and `ServerCore` cannot drift (Concern #3 — envelope drift).
    fn error_response(id: RequestId, code: i32, message: String) -> JSONRPCResponse {
        contract_pre_error_code_mapping!();
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::server::task_dispatch::error_response(id, code, message)
        }
        #[cfg(target_arch = "wasm32")]
        {
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
    }

    /// Create a success response.
    ///
    /// Delegates to the SINGLE-SOURCE envelope builder in `task_dispatch` so the
    /// shared task unit and `ServerCore` cannot drift (Concern #3 — envelope drift).
    fn success_response(id: RequestId, result: Value) -> JSONRPCResponse {
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::server::task_dispatch::success_response(id, result)
        }
        #[cfg(target_arch = "wasm32")]
        {
            JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id,
                payload: ResponsePayload::Result(result),
            }
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

/// The wire result of a v2 `server/discover` request (Phase 112, VERS-04).
///
/// A read-only projection of the server's ALREADY-COMPUTED capabilities plus
/// its implementation info. It reuses the existing [`ServerCapabilities`] /
/// [`Implementation`] types — it does NOT invent a parallel capability model —
/// and is produced ONLY through the isolated [`discover_result_from_capabilities`]
/// conversion fn so a final-spec wire adjustment stays localized.
///
/// It is `#[non_exhaustive]` (spec-defined fields may be added without a break)
/// and crate-private (server-produced wire output, not a client-constructed
/// public API type this phase — the pmcp `Client` gains v2 discover parsing in
/// Phase 113).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerDiscoverResult {
    /// The negotiated protocol version this projection was produced under.
    pub protocol_version: String,
    /// The server's already-computed capabilities (incl. the `extensions` map).
    pub capabilities: ServerCapabilities,
    /// The server's self-reported implementation info.
    pub server_info: Implementation,
}

/// Isolated conversion fn producing the [`ServerDiscoverResult`] wire shape
/// (Phase 112, VERS-04).
///
/// This is the SINGLE place the discover wire shape is assembled: it projects
/// the already-computed `capabilities` (including `extensions`) and `info`
/// read-only — never recomputing capabilities and never triggering any
/// initialize-style side effect. Keeping the shape behind one fn means a
/// final-spec change is localized (Codex MEDIUM — "server/discover wire shape is
/// provisional").
pub(crate) fn discover_result_from_capabilities(
    capabilities: &ServerCapabilities,
    info: &Implementation,
    negotiated_version: String,
) -> ServerDiscoverResult {
    ServerDiscoverResult {
        protocol_version: negotiated_version,
        capabilities: capabilities.clone(),
        server_info: info.clone(),
    }
}

/// Internal disposition discriminator for the v2 `resultType` envelope
/// (Phase 112, VERS-07 / D-08).
///
/// This is NOT a public field on any Result struct — handlers keep returning
/// today's types (semver-safe, zero public-API churn). This phase only ever
/// emits [`ResponseDisposition::Complete`]; the [`InputRequired`](Self::InputRequired)
/// and [`Task`](Self::Task) variants are the concrete path Phases 113 and 114
/// select at dispatch: they thread a non-default disposition with the response
/// and the SAME serialization helper ([`inject_v2_result_envelope`]) emits it,
/// without touching this envelope code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseDisposition {
    /// The result is a final, complete result (the default; absent-means-complete).
    Complete,
    // Why: the two non-Complete dispositions are the established selection path
    // for Phases 113 (`input_required`) and 114 (`task`) — they are wired by
    // those phases at dispatch and emitted by the shared helper below. Retained
    // here (rather than added later) so the mechanism 113/114 depend on exists
    // and is exercised by the `as_wire_str` unit test this phase.
    /// The result requests further input before it can complete (Phase 113).
    #[allow(dead_code)]
    InputRequired,
    /// The result is a task handle rather than a terminal result (Phase 114).
    #[allow(dead_code)]
    Task,
}

impl ResponseDisposition {
    /// The wire `resultType` discriminator string.
    pub(crate) fn as_wire_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::InputRequired => "input_required",
            Self::Task => "task",
        }
    }
}

/// Inject the v2-only response envelope (`resultType` + `serverInfo`) at the
/// era-gated serialization boundary (Phase 112, VERS-07 / D-07 / D-08).
///
/// This is the ONE shared implementation BOTH native dispatch sites
/// (`core.rs` and `server/mod.rs`) call — not a per-site copy. The envelope
/// model is pinned (Codex HIGH #5):
///
/// - era != V2 (or no resolved context) → response left BYTE-IDENTICAL to
///   today (the v1 promise — no key added, golden-fixtured).
/// - error responses / notifications (no `result`) → NO injection.
/// - `result` is a JSON object → insert `resultType` (from `disposition`,
///   `complete` this phase) INTO the inner result object UNLESS the handler
///   already set one (respected, never overwritten — the 113/114 path), and
///   attach `serverInfo` (never overwritten).
/// - `result` is scalar/array/null → left unchanged (cannot key a non-object;
///   no in-scope v2 method returns a non-object).
pub(crate) fn inject_v2_result_envelope(
    response: &mut JSONRPCResponse,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
    server_info: &Implementation,
    disposition: ResponseDisposition,
) {
    // v2-only: a v1 (or non-opted-in) response is left byte-identical.
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return;
    }

    // Only success results carry the envelope; errors / notifications do not.
    let crate::types::jsonrpc::ResponsePayload::Result(ref mut value) = response.payload else {
        return;
    };

    // A non-object result (scalar/array/null) cannot carry a key — leave it.
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    // Collision-safe: respect a handler-set `resultType` (the 113/114 path).
    obj.entry("resultType".to_string())
        .or_insert_with(|| Value::String(disposition.as_wire_str().to_string()));
    // Attach serverInfo on the v2 object result; never overwrite a handler value.
    obj.entry("serverInfo".to_string())
        .or_insert_with(|| serde_json::to_value(server_info).unwrap_or(Value::Null));
}

/// Build the v2 `server/discover` response (Phase 112, VERS-04, D-09/D-10).
///
/// The SINGLE shared projection consumed by BOTH the production HTTP caller
/// (`Server::handle_discover` → the streamable-HTTP `HttpIngress::Discover`
/// classifier) and the discover unit tests — there is exactly one projection and
/// one envelope path, no duplicate capability type and no `#[allow(dead_code)]`
/// wrapper.
///
/// A READ-ONLY projection of the server's already-computed `capabilities`
/// (including the `extensions` map) via the isolated
/// [`discover_result_from_capabilities`] conversion fn — it never recomputes
/// capabilities and never triggers an initialize-style side effect (no
/// `is_initialized` mutation). It is era-gated: only an `Era::V2` request is
/// served; a v1 / non-opted-in request receives standard `-32601`
/// method-not-found (D-10), the same reject the public `parse_request` produces
/// for `server/discover`.
pub(crate) fn build_discover_response(
    id: RequestId,
    capabilities: &ServerCapabilities,
    info: &Implementation,
    protocol_context: Option<&crate::types::protocol::ProtocolContext>,
) -> JSONRPCResponse {
    // Era gate (D-10): v2 only. A v1 / non-opted-in request is method-not-found.
    if !matches!(
        protocol_context.map(|c| c.era),
        Some(crate::types::protocol::Era::V2)
    ) {
        return ServerCore::error_response(
            id,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND,
            "Method not found: server/discover".to_string(),
        );
    }

    let negotiated_version = protocol_context.map_or_else(
        || crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
        |ctx| ctx.negotiated_version.as_str().to_string(),
    );

    // Read-only projection of the ALREADY-COMPUTED capabilities — no recompute.
    let result = discover_result_from_capabilities(capabilities, info, negotiated_version);
    let mut response = ServerCore::success_response(id, serde_json::to_value(result).unwrap());
    // Parity: the v2 object result carries resultType + serverInfo via the SAME
    // shared envelope helper every other v2 result uses.
    inject_v2_result_envelope(
        &mut response,
        protocol_context,
        info,
        ResponseDisposition::Complete,
    );
    response
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
            return Self::error_response(
                id,
                crate::types::protocol::error_codes::INTERNAL_ERROR,
                e.to_string(),
            );
        }

        // Resolve the per-request ProtocolContext ONCE at native ingress
        // (opted-in servers only — D-04). This is the single authoritative
        // resolution threaded through dispatch; the HTTP layer (Plan 06) resolves
        // once for its header gate and passes the same value in, never re-derived.
        let protocol_context = match self.resolve_ingress_protocol_context(&request) {
            Ok(ctx) => ctx,
            Err(negotiation_error) => {
                let (code, message) = negotiation_error_to_rejection(&negotiation_error);
                return Self::error_response(id, code, message);
            },
        };

        // Execute the actual request handling with auth_context
        let mut response = self
            .handle_request_internal(id.clone(), request, auth_context, protocol_context.clone())
            .await;

        // Inject the v2-only response envelope (resultType + serverInfo) at the
        // era-gated serialization boundary (VERS-07 / D-07 / D-08). This is a
        // no-op for v1 / non-opted-in responses (byte-identical) and for
        // error/notification/non-object results. This phase emits `complete`.
        inject_v2_result_envelope(
            &mut response,
            protocol_context.as_ref(),
            &self.info,
            ResponseDisposition::Complete,
        );

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
    /// Resolve the owner ID from the authentication context using the task router.
    ///
    /// Returns `None` if no task router is configured. When a task router is
    /// available, it delegates to [`TaskRouter::resolve_owner`] which uses
    /// the priority chain: OAuth subject > client ID > session ID > "local".
    /// When only a `TaskStore` is configured (no `TaskRouter`), derives
    /// the owner from the auth context directly.
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_task_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
        // Delegate to the shared TaskDispatch unit (owner-resolution lives there,
        // once, for both dispatchers).
        self.task_dispatch().resolve_owner(auth_context)
    }

    /// Borrow this `ServerCore`'s task backends into the shared dispatch unit.
    #[cfg(not(target_arch = "wasm32"))]
    fn task_dispatch(&self) -> crate::server::task_dispatch::TaskDispatch<'_> {
        crate::server::task_dispatch::TaskDispatch {
            task_store: &self.task_store,
            task_router: &self.task_router,
        }
    }

    /// Build the `tools/call` create-task response for a `TaskCreated` outcome.
    ///
    /// Per `D-STORE-MINTS-ID` (review finding #3): when a [`TaskStore`] is
    /// configured the store mints the canonical task id via `store.create()`;
    /// that store-minted id is reflected on the WIRE in BOTH
    /// `CreateTaskResult.task.taskId` AND the `_meta.relatedTask.taskId`
    /// envelope (never the tool's fabricated id). When the terminal `result`
    /// is present (synchronous completion) it is persisted via
    /// `store.set_result()` and the task is transitioned `Working -> Completed`
    /// BEFORE the response returns, so a subsequent `tasks/get` shows
    /// `Completed`.
    ///
    /// Falls back to the legacy tool-fabricated envelope only when no store is
    /// configured (preserves prior behavior for router-only servers).
    #[cfg(not(target_arch = "wasm32"))]
    async fn build_task_created_response(
        &self,
        id: RequestId,
        task_value: Value,
        auth_context: Option<&AuthContext>,
    ) -> JSONRPCResponse {
        // Delegate to the shared TaskDispatch unit. It RE-EXTRACTS the task id and
        // the terminal result from `task_value` internally (store mints the id;
        // `extract_terminal_result` recovers the terminal CallToolResult), so the
        // store-minted-id and synchronous-completion-persistence invariants live in
        // exactly one place.
        self.task_dispatch()
            .build_task_created_response(id, task_value, auth_context)
            .await
    }

    /// Handle a `tasks/result` request.
    ///
    /// Per review finding #2 (store-vs-router precedence): serves from the
    /// configured [`TaskStore`] FIRST when it `supports_results()`, but FALLS
    /// THROUGH to the [`TaskRouter`](crate::server::tasks::TaskRouter) on store
    /// `NotFound`/unsupported — never a hard error when a router can serve it.
    /// When the store has no result and NO router is configured, returns a
    /// SPECIFIED "task not completed" error (`-32002`), distinct from the
    /// truly-no-backend `-32601`.
    /// Internal request handler without middleware processing.
    async fn handle_request_internal(
        &self,
        id: RequestId,
        request: Request,
        auth_context: Option<AuthContext>,
        protocol_context: Option<crate::types::protocol::ProtocolContext>,
    ) -> JSONRPCResponse {
        contract_pre_session_lifecycle!();
        match request {
            Request::Client(ref boxed_req)
                if matches!(**boxed_req, ClientRequest::Initialize(_)) =>
            {
                let ClientRequest::Initialize(init_req) = boxed_req.as_ref() else {
                    unreachable!("Pattern matched for Initialize");
                };

                match self.handle_initialize(init_req).await {
                    Ok(result) => Self::success_response(id, serde_json::to_value(result).unwrap()),
                    Err(e) => Self::error_response(
                        id,
                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                        e.to_string(),
                    ),
                }
            },
            Request::Client(ref boxed_req) => {
                // Check if server is initialized for server requests (skip in stateless mode)
                // Stateless mode is for serverless deployments where each request may create
                // a fresh server instance (AWS Lambda, Cloudflare Workers, etc.)
                if !self.stateless_mode && !self.is_initialized().await {
                    return Self::error_response(
                        id,
                        // FROZEN wire value -32002 (byte-identical); read from the
                        // centralized table by name (Pitfall 6).
                        crate::types::protocol::error_codes::V1_TASK_PENDING,
                        "Server not initialized. Call initialize first.".to_string(),
                    );
                }

                match boxed_req.as_ref() {
                    ClientRequest::ListTools(req) => match self.handle_list_tools(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        ),
                    },
                    ClientRequest::CallTool(req) => {
                        // Check for task-augmented call: explicit task field or tool requires task
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(ref task_router) = self.task_router {
                            // Determine if this tool requires task augmentation
                            let tool_execution = self
                                .tool_infos
                                .get(&req.name)
                                .and_then(|m| m.execution.as_ref());
                            let needs_task = req.task.is_some() || {
                                let exec_value =
                                    tool_execution.and_then(|e| serde_json::to_value(e).ok());
                                task_router.tool_requires_task(&req.name, exec_value.as_ref())
                            };
                            if needs_task {
                                let owner_id = self
                                    .resolve_task_owner(auth_context.as_ref())
                                    .unwrap_or_else(|| "local".to_string());
                                let task_params =
                                    req.task.clone().unwrap_or_else(|| serde_json::json!({}));
                                #[allow(clippy::used_underscore_binding)]
                                let progress_token = req
                                    ._meta
                                    .as_ref()
                                    .and_then(|m| m.progress_token.as_ref())
                                    .map(|t| serde_json::to_value(t).unwrap());
                                return match task_router
                                    .handle_task_call(
                                        &req.name,
                                        req.arguments.clone(),
                                        task_params,
                                        &owner_id,
                                        progress_token,
                                    )
                                    .await
                                {
                                    Ok(result) => Self::success_response(id, result),
                                    Err(e) => Self::error_response(
                                        id,
                                        crate::types::protocol::error_codes::INTERNAL_ERROR,
                                        e.to_string(),
                                    ),
                                };
                            }
                        }
                        // Normal tool call path (no task augmentation)
                        // Extract continuation context before the handler call
                        #[cfg(not(target_arch = "wasm32"))]
                        #[allow(clippy::used_underscore_binding)]
                        let continuation_ctx = req
                            ._meta
                            .as_ref()
                            .and_then(|m| m._task_id.clone())
                            .map(|task_id| (task_id, req.name.clone()));

                        match self
                            .handle_call_tool(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(outcome) => match outcome {
                                #[cfg(not(target_arch = "wasm32"))]
                                ToolCallOutcome::TaskCreated { task_value } => {
                                    // The shared unit re-extracts task_id + terminal
                                    // result from task_value (single-source create path).
                                    self.build_task_created_response(
                                        id,
                                        task_value,
                                        auth_context.as_ref(),
                                    )
                                    .await
                                },
                                ToolCallOutcome::Result(result) => {
                                    // Fire-and-forget workflow continuation recording
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let (Some((task_id, tool_name)), Some(ref task_router)) =
                                        (continuation_ctx, &self.task_router)
                                    {
                                        let owner_id = self
                                            .resolve_task_owner(auth_context.as_ref())
                                            .unwrap_or_else(|| "local".to_string());
                                        let tool_result_value =
                                            serde_json::to_value(&result).unwrap_or_default();
                                        if let Err(e) = task_router
                                            .handle_workflow_continuation(
                                                &task_id,
                                                &tool_name,
                                                tool_result_value,
                                                &owner_id,
                                            )
                                            .await
                                        {
                                            tracing::warn!(
                                                "Workflow continuation recording failed for task {}: {}",
                                                task_id,
                                                e
                                            );
                                        }
                                    }
                                    Self::success_response(
                                        id,
                                        serde_json::to_value(result).unwrap(),
                                    )
                                },
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListPrompts(req) => match self.handle_list_prompts(req).await {
                        Ok(result) => {
                            Self::success_response(id, serde_json::to_value(result).unwrap())
                        },
                        Err(e) => Self::error_response(
                            id,
                            crate::types::protocol::error_codes::INTERNAL_ERROR,
                            e.to_string(),
                        ),
                    },
                    ClientRequest::GetPrompt(req) => {
                        match self
                            .handle_get_prompt(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListResources(req) => {
                        match self.handle_list_resources(req, auth_context.clone()).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ReadResource(req) => {
                        match self
                            .handle_read_resource(req, auth_context.clone(), protocol_context)
                            .await
                        {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    ClientRequest::ListResourceTemplates(req) => {
                        match self.handle_list_resource_templates(req).await {
                            Ok(result) => {
                                Self::success_response(id, serde_json::to_value(result).unwrap())
                            },
                            Err(e) => Self::error_response(
                                id,
                                crate::types::protocol::error_codes::INTERNAL_ERROR,
                                e.to_string(),
                            ),
                        }
                    },
                    // Task endpoint routing (TaskStore preferred, TaskRouter
                    // fallback) — delegated to the shared TaskDispatch unit so the
                    // routing logic lives in exactly one place (HTASK-02).
                    #[cfg(not(target_arch = "wasm32"))]
                    request @ (ClientRequest::TasksGet(_)
                    | ClientRequest::TasksResult(_)
                    | ClientRequest::TasksList(_)
                    | ClientRequest::TasksCancel(_)) => {
                        self.task_dispatch()
                            .route_tasks_endpoint(id, request, auth_context.as_ref())
                            .await
                    },
                    _ => Self::error_response(
                        id,
                        crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                        "Method not supported".to_string(),
                    ),
                }
            },
            Request::Server(_) => Self::error_response(
                id,
                crate::types::protocol::error_codes::METHOD_NOT_FOUND,
                "Method not supported".to_string(),
            ),
        }
    }

    /// Internal notification handler without middleware processing.
    async fn handle_notification_internal(&self, _notification: Notification) -> Result<()> {
        // Handle notifications if needed
        // Most notifications from client to server don't require action
        Ok(())
    }
}

/// Generate a brief text summary of structured output for widget tools.
///
/// When a tool has widget metadata, `structuredContent` carries the full data
/// for the widget. The `content` text should be a concise summary rather than
/// a JSON dump, since `ChatGPT` displays both and duplication is undesirable.
fn summarize_structured_output(value: &Value) -> String {
    match value {
        Value::Array(arr) => format_record_count(arr.len()),
        Value::Object(map) => {
            // Look for common collection patterns inside the object
            // e.g. { "results": [...], "total": 42 } or { "items": [...] }
            for key in ["results", "items", "data", "records", "rows", "entries"] {
                if let Some(Value::Array(arr)) = map.get(key) {
                    return format_record_count(arr.len());
                }
            }
            let field_count = map.len();
            match field_count {
                0 => "Empty result.".to_string(),
                1 => "Result with 1 field.".to_string(),
                n => format!("Result with {n} fields."),
            }
        },
        Value::String(s) => {
            if s.len() <= 200 {
                s.clone()
            } else {
                let truncated: String = s.chars().take(200).collect();
                format!("{truncated}...")
            }
        },
        Value::Null => "No result.".to_string(),
        other => other.to_string(),
    }
}

fn format_record_count(len: usize) -> String {
    match len {
        0 => "No records returned.".to_string(),
        1 => "1 record returned.".to_string(),
        n => format!("{n} records returned."),
    }
}

/// Compute the serialized JSON byte length without allocating.
fn json_serialized_len(value: &impl serde::Serialize) -> Result<usize> {
    struct CountingWriter(usize);
    impl std::io::Write for CountingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = CountingWriter(0);
    serde_json::to_writer(&mut counter, value)
        .map_err(|e| Error::validation(format!("Cannot measure argument size: {e}")))?;
    Ok(counter.0)
}

/// Convert an optional per-request `_meta` into raw JSON for handler surfacing /
/// ingress era-resolution (Phase 112). Centralizes the `RequestMeta -> Value`
/// conversion that every prompt/resource/tool dispatch site would otherwise
/// hand-roll.
pub(crate) fn request_meta_to_value<T: serde::Serialize>(
    meta: Option<&T>,
) -> Option<serde_json::Value> {
    meta.and_then(|m| serde_json::to_value(m).ok())
}

/// Extract the request's `_meta` object as raw JSON for ingress era-resolution
/// (Phase 112, D-11 — the per-request signal is transport-agnostic).
///
/// # Go-forward policy (Phase 112 Plan 09)
///
/// EVERY [`ClientRequest`] variant that carries a per-request
/// `_meta: Option<RequestMeta>` field MUST be read here so its era/identity/trace
/// signal reaches ingress resolution. As of this plan that is `CallTool`,
/// `GetPrompt`, and `ReadResource` — the three name/uri-bearing methods. Variants
/// with NO `_meta` field yield `None` and resolve to the v1 fallback by design
/// (an opted-in server serves them as v1 unless a header/`_meta` signal says
/// otherwise).
///
/// The inner match is EXHAUSTIVE with no wildcard arm: a future `ClientRequest`
/// variant is a `non-exhaustive patterns` COMPILE ERROR here, forcing the author
/// to classify it as `_meta`-bearing or not. That compile-time tripwire — not a
/// doc comment or a hand-maintained test — is what keeps this in sync with the
/// enum.
#[allow(clippy::used_underscore_binding)] // _meta is part of the MCP protocol spec
pub(crate) fn extract_request_meta_value(request: &Request) -> Option<serde_json::Value> {
    match request {
        Request::Client(boxed) => match boxed.as_ref() {
            // `_meta`-bearing variants — read the per-request signal.
            ClientRequest::CallTool(req) => request_meta_to_value(req._meta.as_ref()),
            ClientRequest::GetPrompt(req) => request_meta_to_value(req._meta.as_ref()),
            ClientRequest::ReadResource(req) => request_meta_to_value(req._meta.as_ref()),
            // Non-`_meta`-bearing variants — enumerated explicitly (no wildcard)
            // so adding a variant forces a decision above rather than silently
            // dropping its signal.
            ClientRequest::Initialize(_)
            | ClientRequest::ListTools(_)
            | ClientRequest::ListPrompts(_)
            | ClientRequest::ListResources(_)
            | ClientRequest::ListResourceTemplates(_)
            | ClientRequest::Subscribe(_)
            | ClientRequest::Unsubscribe(_)
            | ClientRequest::Complete(_)
            | ClientRequest::CreateMessage(_)
            | ClientRequest::TasksGet(_)
            | ClientRequest::TasksResult(_)
            | ClientRequest::TasksList(_)
            | ClientRequest::TasksCancel(_)
            | ClientRequest::SetLoggingLevel { .. }
            | ClientRequest::Ping => None,
        },
        Request::Server(_) => None,
    }
}

/// Resolve the per-request [`ProtocolContext`](crate::types::protocol::ProtocolContext)
/// ONCE at native ingress, shared by BOTH dispatch surfaces (`ServerCore` and the
/// high-level `Server`) so the opt-in gate + resolver sequence lives in exactly
/// one place (Pitfall 3 — twin-wiring drift).
///
/// Returns `Ok(None)` immediately for a non-opted-in server so it runs ZERO
/// era-detection and its v1 path is byte-for-byte unchanged (D-04). For an
/// opted-in server it delegates to the single shared
/// [`resolve_protocol_context`](crate::types::protocol::context::resolve_protocol_context),
/// enforcing the configured accept-list against the request's `_meta`.
pub(crate) fn resolve_ingress_protocol_context(
    accept_list: &[crate::types::ProtocolVersion],
    request: &Request,
) -> std::result::Result<
    Option<crate::types::protocol::ProtocolContext>,
    crate::types::protocol::context::ProtocolNegotiationError,
> {
    if !crate::types::protocol::context::is_v2_opted_in(accept_list) {
        return Ok(None);
    }
    let meta = extract_request_meta_value(request);
    crate::types::protocol::context::resolve_protocol_context(accept_list, meta.as_ref())
}

/// Map a [`ProtocolNegotiationError`](crate::types::protocol::context::ProtocolNegotiationError)
/// to a structured JSON-RPC rejection `(code, message)`.
///
/// Both variants surface as `INVALID_PARAMS` (-32602): a bad/unsupported
/// per-request `protocolVersion` or a malformed reserved `_meta` key is an
/// invalid method parameter. (v2 semantic error-code values are finalized from
/// the 2026-07-28 schema; VERS-06.)
pub(crate) fn negotiation_error_to_rejection(
    error: &crate::types::protocol::context::ProtocolNegotiationError,
) -> (i32, String) {
    use crate::types::protocol::context::ProtocolNegotiationError;
    use crate::types::protocol::error_codes::INVALID_PARAMS;
    match error {
        ProtocolNegotiationError::UnsupportedVersion(v) => {
            (INVALID_PARAMS, format!("Unsupported protocol version: {v}"))
        },
        ProtocolNegotiationError::MalformedMeta(reason) => {
            (INVALID_PARAMS, format!("Malformed _meta: {reason}"))
        },
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

    /// Build `tool_infos` cache from a tools `HashMap` (mirrors builder logic).
    fn build_tool_infos(
        tools: &HashMap<String, Arc<dyn ToolHandler>>,
    ) -> HashMap<String, ToolInfo> {
        tools
            .iter()
            .map(|(name, handler)| {
                let mut info = handler
                    .metadata()
                    .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
                info.name.clone_from(name);
                (name.clone(), info)
            })
            .collect()
    }

    #[tokio::test]
    async fn test_server_core_initialization() {
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode
            PayloadLimits::default(),
        );

        assert!(!server.is_initialized().await);

        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
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
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode
            PayloadLimits::default(),
        );

        // Initialize first
        let init_req = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new("test-client", "1.0.0"),
        })));
        server
            .handle_request(RequestId::from(1i64), init_req, None)
            .await;

        // List tools
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
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

    struct EraProbeTool;

    #[async_trait]
    impl ToolHandler for EraProbeTool {
        async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> Result<Value> {
            // Prove the ingress-resolved context is visible IN the handler.
            let era = extra.era().map(|e| format!("{e:?}"));
            let traceparent = extra.trace_context().map(|tc| tc.traceparent);
            Ok(serde_json::json!({ "era": era, "traceparent": traceparent }))
        }
    }

    /// Extract the probe tool's JSON payload from a wrapped `CallToolResult`
    /// (the dispatcher emits the handler value as text content).
    fn probe_payload(result: &Value) -> Value {
        let text = result["content"][0]["text"]
            .as_str()
            .expect("probe result carries text content");
        serde_json::from_str(text).expect("probe text content is JSON")
    }

    fn probe_call_with_v2_meta() -> Request {
        use crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;
        let meta = crate::types::protocol::RequestMeta::new()
            .with_meta(
                RESERVED_PROTOCOL_VERSION_KEY,
                serde_json::json!("2026-07-28"),
            )
            .with_meta(
                "traceparent",
                serde_json::json!("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
            );
        Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "probe".to_string(),
            arguments: serde_json::json!({}),
            _meta: Some(meta),
            task: None,
        })))
    }

    /// End-to-end: a v2 `_meta` + `traceparent` presented at ingress is resolved
    /// once and visible in the invoked handler via `extra.era()` /
    /// `extra.trace_context()` (Codex MEDIUM — ingress→handler threading proven).
    #[tokio::test]
    async fn test_v2_meta_visible_in_handler_end_to_end() {
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;

        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("probe-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(7i64), probe_call_with_v2_meta(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                let probe = probe_payload(&result);
                assert_eq!(probe["era"], "V2");
                assert_eq!(
                    probe["traceparent"],
                    "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
                );
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    /// A non-opted-in (default v1-only) server runs ZERO era-detection: even a v2
    /// `_meta` signal resolves to no context, so the handler reads `era()==None`
    /// (D-04, byte-for-byte-unchanged v1 path).
    #[tokio::test]
    async fn test_non_opted_in_server_resolves_no_context() {
        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("v1-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .build()
            .unwrap();

        let response = server
            .handle_request(RequestId::from(8i64), probe_call_with_v2_meta(), None)
            .await;
        match response.payload {
            ResponsePayload::Result(result) => {
                let probe = probe_payload(&result);
                assert_eq!(probe["era"], serde_json::Value::Null);
            },
            ResponsePayload::Error(e) => panic!("probe call failed: {}", e.message),
        }
    }

    /// An explicitly-unsupported per-request version is rejected with a structured
    /// error rather than silently served (accept-list enforcement, Codex HIGH #2).
    #[tokio::test]
    async fn test_unsupported_version_rejected_at_ingress() {
        use crate::types::protocol::context::RESERVED_PROTOCOL_VERSION_KEY;
        use crate::types::protocol::PROTOCOL_VERSION_2026_07_28;

        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("probe-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_supported_protocol_versions([ProtocolVersion(
                PROTOCOL_VERSION_2026_07_28.to_string(),
            )])
            .build()
            .unwrap();

        let meta = crate::types::protocol::RequestMeta::new().with_meta(
            RESERVED_PROTOCOL_VERSION_KEY,
            serde_json::json!("1999-01-01"),
        );
        let call = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name: "probe".to_string(),
            arguments: serde_json::json!({}),
            _meta: Some(meta),
            task: None,
        })));
        let response = server
            .handle_request(RequestId::from(9i64), call, None)
            .await;
        match response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, crate::types::protocol::error_codes::INVALID_PARAMS);
            },
            ResponsePayload::Result(_) => {
                panic!("unsupported version must be rejected, not served")
            },
        }
    }

    // ---- Phase 112 Plan 05: server/discover (VERS-04) ----

    fn v2_ctx() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(
            crate::types::protocol::Era::V2,
            ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
        )
    }

    fn v1_ctx() -> crate::types::protocol::ProtocolContext {
        crate::types::protocol::ProtocolContext::new(
            crate::types::protocol::Era::V1,
            ProtocolVersion("2025-11-25".to_string()),
        )
    }

    /// Build a v2-opted-in server carrying a `.with_extension`-populated key.
    fn discover_server() -> ServerCore {
        crate::server::builder::ServerCoreBuilder::new()
            .name("discover-server")
            .version("9.9.9")
            .tool("probe", EraProbeTool)
            .stateless_mode(true)
            .with_extension(
                "io.example/experimental",
                serde_json::json!({ "enabled": true }),
            )
            .with_supported_protocol_versions([
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
            .build()
            .unwrap()
    }

    /// A v2 `server/discover` projects the already-computed capabilities INCLUDING
    /// the `.with_extension`-populated extensions map, and carries serverInfo.
    #[test]
    fn server_discover_v2_projects_capabilities_with_extensions() {
        let server = discover_server();
        // The wire method still classifies as the internal (non-public-enum) request.
        let internal = crate::types::protocol::classify_internal_method(
            "server/discover",
            &serde_json::json!({}),
        )
        .expect("server/discover classifies as internal");
        assert!(matches!(
            internal,
            crate::types::protocol::InternalClientRequest::ServerDiscover(_)
        ));
        let ctx = v2_ctx();
        // Projection is produced by the ONE shared free fn the production caller uses.
        let response = build_discover_response(
            RequestId::from(1i64),
            &server.capabilities,
            &server.info,
            Some(&ctx),
        );

        let ResponsePayload::Result(value) = response.payload else {
            panic!("v2 server/discover must return a result");
        };
        // extensions map projected
        assert_eq!(
            value["capabilities"]["extensions"]["io.example/experimental"]["enabled"],
            serde_json::json!(true)
        );
        // serverInfo present
        assert_eq!(value["serverInfo"]["name"], "discover-server");
        assert_eq!(value["serverInfo"]["version"], "9.9.9");
        // negotiated version reflected
        assert_eq!(value["protocolVersion"], "2026-07-28");
    }

    /// A v1 / non-opted-in `server/discover` receives standard -32601 (D-10).
    #[test]
    fn server_discover_v1_returns_method_not_found() {
        let server = discover_server();

        // v1 era context
        let ctx = v1_ctx();
        let resp = build_discover_response(
            RequestId::from(2i64),
            &server.capabilities,
            &server.info,
            Some(&ctx),
        );
        let ResponsePayload::Error(e) = resp.payload else {
            panic!("v1 server/discover must be an error");
        };
        assert_eq!(
            e.code,
            crate::types::protocol::error_codes::METHOD_NOT_FOUND
        );
        assert_eq!(e.code, -32601);

        // no resolved context at all → also -32601
        let resp_none = build_discover_response(
            RequestId::from(3i64),
            &server.capabilities,
            &server.info,
            None,
        );
        let ResponsePayload::Error(e2) = resp_none.payload else {
            panic!("context-less server/discover must be an error");
        };
        assert_eq!(e2.code, -32601);
    }

    /// The public `parse_request` maps `server/discover` to -32601 (v1 for free)
    /// — proving the interception seam preserves the v1 wire behavior.
    #[test]
    fn server_discover_public_parse_is_method_not_found() {
        let req = crate::types::JSONRPCRequest::new(
            RequestId::from(1i64),
            "server/discover".to_string(),
            Some(serde_json::json!({})),
        );
        let err = crate::shared::parse_request(req).unwrap_err();
        assert!(err.to_string().contains("Method not found"));
    }

    /// `server/discover` does NOT mutate initialization state (read-only, no
    /// initialize-style side effect).
    #[tokio::test]
    async fn server_discover_does_not_mutate_init_state() {
        // Non-stateless server so `is_initialized()` is meaningful.
        let server = crate::server::builder::ServerCoreBuilder::new()
            .name("discover-server")
            .version("1.0.0")
            .tool("probe", EraProbeTool)
            .with_supported_protocol_versions([ProtocolVersion(
                crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string(),
            )])
            .build()
            .unwrap();

        assert!(!server.is_initialized().await);
        let ctx = v2_ctx();
        let _ = build_discover_response(
            RequestId::from(1i64),
            &server.capabilities,
            &server.info,
            Some(&ctx),
        );
        assert!(
            !server.is_initialized().await,
            "server/discover must not flip initialization state"
        );
    }

    /// Golden fixture: pin the discover wire shape so a change is caught.
    #[test]
    fn server_discover_wire_shape_golden() {
        let caps = ServerCapabilities::tools_only();
        let info = Implementation::new("golden-server", "1.2.3");
        let result = discover_result_from_capabilities(&caps, &info, "2026-07-28".to_string());
        let value = serde_json::to_value(&result).unwrap();
        let expected = serde_json::json!({
            "protocolVersion": "2026-07-28",
            "capabilities": { "tools": { "listChanged": true } },
            "serverInfo": { "name": "golden-server", "version": "1.2.3" }
        });
        assert_eq!(value, expected, "discover wire shape drifted from golden");
    }

    // ---- Phase 112 Plan 05: resultType + serverInfo envelope (VERS-07) ----

    fn result_response(id: i64, result: Value) -> JSONRPCResponse {
        JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(result),
        }
    }

    /// A v2 OBJECT success result gains inner-result `resultType:"complete"` and
    /// a `serverInfo` object.
    #[test]
    fn result_type_envelope_v2_object_gets_complete_and_server_info() {
        let info = Implementation::new("srv", "2.0.0");
        let ctx = v2_ctx();
        let mut resp = result_response(1, serde_json::json!({ "tools": [] }));
        inject_v2_result_envelope(&mut resp, Some(&ctx), &info, ResponseDisposition::Complete);
        let ResponsePayload::Result(v) = resp.payload else {
            panic!("expected result");
        };
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["serverInfo"]["name"], "srv");
        assert_eq!(v["serverInfo"]["version"], "2.0.0");
    }

    /// A handler-set `resultType` is PRESERVED, never overwritten (the 113/114
    /// path). This proves the disposition the serialization layer reads.
    #[test]
    fn result_type_envelope_preserves_handler_disposition() {
        let info = Implementation::new("srv", "2.0.0");
        let ctx = v2_ctx();
        let mut resp = result_response(1, serde_json::json!({ "resultType": "task", "x": 1 }));
        // Even though we ask for Complete, the handler's "task" must survive.
        inject_v2_result_envelope(&mut resp, Some(&ctx), &info, ResponseDisposition::Complete);
        let ResponsePayload::Result(v) = resp.payload else {
            panic!("expected result");
        };
        assert_eq!(
            v["resultType"], "task",
            "handler disposition must be preserved"
        );
        // Non-default dispositions round-trip through the wire discriminator.
        assert_eq!(
            ResponseDisposition::InputRequired.as_wire_str(),
            "input_required"
        );
        assert_eq!(ResponseDisposition::Task.as_wire_str(), "task");
    }

    /// A v2 scalar/null result is left unchanged (cannot key a non-object), and
    /// error responses get no injection.
    #[test]
    fn result_type_envelope_non_object_and_error_untouched() {
        let info = Implementation::new("srv", "2.0.0");
        let ctx = v2_ctx();

        // scalar
        let mut scalar = result_response(1, serde_json::json!(42));
        inject_v2_result_envelope(
            &mut scalar,
            Some(&ctx),
            &info,
            ResponseDisposition::Complete,
        );
        let ResponsePayload::Result(v) = scalar.payload else {
            panic!("expected result");
        };
        assert_eq!(v, serde_json::json!(42));

        // null
        let mut null = result_response(2, Value::Null);
        inject_v2_result_envelope(&mut null, Some(&ctx), &info, ResponseDisposition::Complete);
        let ResponsePayload::Result(v) = null.payload else {
            panic!("expected result");
        };
        assert_eq!(v, Value::Null);

        // error → no injection
        let mut err = ServerCore::error_response(RequestId::from(3i64), -32601, "nope".to_string());
        inject_v2_result_envelope(&mut err, Some(&ctx), &info, ResponseDisposition::Complete);
        assert!(matches!(err.payload, ResponsePayload::Error(_)));
    }

    /// Golden byte-identity: a v1 (or non-opted-in) response is UNCHANGED — no
    /// resultType, no serverInfo — for both a success and an error.
    #[test]
    fn result_type_envelope_v1_byte_identical_golden() {
        let info = Implementation::new("srv", "2.0.0");
        let ctx = v1_ctx();

        // v1 success — byte-identical
        let original = serde_json::json!({ "tools": [], "nextCursor": null });
        let mut resp = result_response(1, original.clone());
        inject_v2_result_envelope(&mut resp, Some(&ctx), &info, ResponseDisposition::Complete);
        let ResponsePayload::Result(v) = resp.payload else {
            panic!("expected result");
        };
        assert_eq!(v, original, "v1 success must stay byte-identical");

        // No context at all — also byte-identical.
        let mut resp_none = result_response(2, original.clone());
        inject_v2_result_envelope(&mut resp_none, None, &info, ResponseDisposition::Complete);
        let ResponsePayload::Result(v2) = resp_none.payload else {
            panic!("expected result");
        };
        assert_eq!(v2, original);

        // v1 error/task-pending — byte-identical (frozen -32002 survives).
        let mut err = ServerCore::error_response(
            RequestId::from(3i64),
            -32002,
            "Task not completed".to_string(),
        );
        let before = serde_json::to_value(&err).unwrap();
        inject_v2_result_envelope(&mut err, Some(&ctx), &info, ResponseDisposition::Complete);
        let after = serde_json::to_value(&err).unwrap();
        assert_eq!(before, after, "v1 error must stay byte-identical");
    }

    /// End-to-end through `handle_request`: a v2 tools/list carries the envelope.
    #[tokio::test]
    async fn result_type_envelope_end_to_end_v2_handle_request() {
        let server = discover_server();
        // `probe_call_with_v2_meta` carries the v2 `_meta` so ingress resolves
        // Era::V2; the tool result is a JSON object → gains the envelope.
        let response = server
            .handle_request(RequestId::from(1i64), probe_call_with_v2_meta(), None)
            .await;
        let ResponsePayload::Result(v) = response.payload else {
            panic!("expected result");
        };
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["serverInfo"]["name"], "discover-server");
    }

    #[tokio::test]
    async fn test_stateless_mode_allows_requests_without_init() {
        // Create server in stateless mode
        let mut tools = HashMap::new();
        tools.insert(
            "test-tool".to_string(),
            Arc::new(TestTool) as Arc<dyn ToolHandler>,
        );
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None, // task_router
            None, // task_store
            true, // stateless_mode enabled
            PayloadLimits::default(),
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
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
        let tool_infos = build_tool_infos(&tools);

        let server = ServerCore::new(
            Implementation::new("test-server", "1.0.0"),
            ServerCapabilities::tools_only(),
            tools,
            HashMap::new(),
            tool_infos,
            HashMap::new(),
            None,
            None,
            None,
            None,
            Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            Arc::new(RwLock::new(ToolMiddlewareChain::new())),
            None,  // task_router
            None,  // task_store
            false, // stateless_mode disabled (normal mode)
            PayloadLimits::default(),
        );

        // Try to list tools WITHOUT initializing first
        let list_req = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
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

    #[test]
    fn test_build_uri_to_tool_meta_indexes_by_standard_key() {
        // Create a tool with openai/* keys (propagation-eligible)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        meta.insert(
            "openai/outputTemplate".to_string(),
            serde_json::json!("ui://chess/board"),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        // Should index by the standard ui.resourceUri key
        assert!(
            index.contains_key("ui://chess/board"),
            "must index by ui.resourceUri value"
        );
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_build_uri_to_tool_meta_includes_openai_when_present() {
        // Create a tool with both standard and openai keys (ChatGpt layer was applied)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        meta.insert(
            "openai/outputTemplate".to_string(),
            serde_json::json!("ui://chess/board"),
        );
        meta.insert(
            "openai/widgetAccessible".to_string(),
            serde_json::json!(true),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        assert!(index.contains_key("ui://chess/board"));
        let entry = &index["ui://chess/board"];
        // Should include the openai keys in the indexed meta
        assert!(
            entry.contains_key("openai/outputTemplate"),
            "must include openai/outputTemplate in index entry"
        );
        assert!(
            entry.contains_key("openai/widgetAccessible"),
            "must include openai/widgetAccessible in index entry"
        );
    }

    #[test]
    fn test_build_uri_to_tool_meta_skips_empty_propagation() {
        // Create a tool with standard-only _meta (no openai/* keys to propagate)
        let mut tool_infos = HashMap::new();
        let mut info = ToolInfo::new(
            "chess",
            Some("Chess tool".to_string()),
            serde_json::json!({"type": "object"}),
        );
        let mut meta = serde_json::Map::new();
        meta.insert(
            "ui".to_string(),
            serde_json::json!({"resourceUri": "ui://chess/board"}),
        );
        info._meta = Some(meta);
        tool_infos.insert("chess".to_string(), info);

        let index = build_uri_to_tool_meta(&tool_infos);
        // Should NOT index when there are no propagation-eligible keys,
        // to avoid producing _meta: {} on resources/list
        assert!(
            !index.contains_key("ui://chess/board"),
            "must not index tools with no propagation-eligible keys"
        );
    }

    #[test]
    fn test_summarize_array() {
        let empty = serde_json::json!([]);
        assert_eq!(summarize_structured_output(&empty), "No records returned.");

        let single = serde_json::json!([{"id": 1}]);
        assert_eq!(summarize_structured_output(&single), "1 record returned.");

        let multi = serde_json::json!([1, 2, 3, 4, 5]);
        assert_eq!(summarize_structured_output(&multi), "5 records returned.");
    }

    #[test]
    fn test_summarize_object_with_collection() {
        let val = serde_json::json!({"results": [1, 2, 3], "total": 3});
        assert_eq!(summarize_structured_output(&val), "3 records returned.");

        let val = serde_json::json!({"items": [], "page": 1});
        assert_eq!(summarize_structured_output(&val), "No records returned.");

        let val = serde_json::json!({"data": [{"name": "a"}]});
        assert_eq!(summarize_structured_output(&val), "1 record returned.");
    }

    #[test]
    fn test_summarize_plain_object() {
        let val = serde_json::json!({"name": "test", "value": 42});
        assert_eq!(summarize_structured_output(&val), "Result with 2 fields.");

        let val = serde_json::json!({});
        assert_eq!(summarize_structured_output(&val), "Empty result.");
    }

    #[test]
    fn test_summarize_primitives() {
        assert_eq!(summarize_structured_output(&Value::Null), "No result.");
        assert_eq!(
            summarize_structured_output(&serde_json::json!("hello")),
            "hello"
        );
        assert_eq!(summarize_structured_output(&serde_json::json!(42)), "42");
    }

    #[test]
    fn test_summarize_string_truncation_multibyte() {
        // Multi-byte chars: each emoji is 4 bytes, 201 of them = 804 bytes
        let long_emoji = "\u{1F600}".repeat(201);
        let result = summarize_structured_output(&Value::String(long_emoji));
        assert!(result.ends_with("..."));
        // Should not panic and should truncate at char boundary
        assert!(result.len() > 3);
    }

    // -----------------------------------------------------------------------
    // Phase 112-09 (Gap B): per-request `_meta`/`ProtocolContext` spine wired for
    // GetPrompt + ReadResource, not only CallTool.
    // -----------------------------------------------------------------------
    mod phase_112_09_context_spine {
        use super::*;
        use crate::types::protocol::{Era, ProtocolContext, RequestMeta};
        use std::sync::Mutex;

        fn get_prompt_request(name: &str, meta: Option<RequestMeta>) -> Request {
            Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
                name: name.to_string(),
                arguments: HashMap::new(),
                _meta: meta,
            })))
        }

        fn read_resource_request(uri: &str, meta: Option<RequestMeta>) -> Request {
            Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
                uri: uri.to_string(),
                _meta: meta,
            })))
        }

        #[test]
        fn extract_request_meta_value_reads_prompt_and_resource_meta() {
            let meta = RequestMeta::new().with_meta("ns/key", serde_json::json!("v"));
            let expected = serde_json::to_value(&meta).unwrap();

            // GetPrompt with _meta → Some(json) equal to to_value(meta).
            let got = extract_request_meta_value(&get_prompt_request("p", Some(meta.clone())));
            assert_eq!(got, Some(expected.clone()));

            // ReadResource with _meta → Some(json) equal to to_value(meta).
            let got = extract_request_meta_value(&read_resource_request("mem://x", Some(meta)));
            assert_eq!(got, Some(expected));

            // _meta == None → None (v1 fallback preserved) for both methods.
            assert_eq!(
                extract_request_meta_value(&get_prompt_request("p", None)),
                None
            );
            assert_eq!(
                extract_request_meta_value(&read_resource_request("mem://x", None)),
                None
            );
        }

        #[test]
        fn all_meta_bearing_client_requests_are_extracted() {
            // Positive coverage for the three `_meta`-bearing variants. The real
            // drift guard is the WILDCARD-FREE exhaustive match in
            // extract_request_meta_value: a new variant is a compile error there,
            // not a silent `None`, so this test need not enumerate the enum.
            let meta = RequestMeta::new().with_meta("io.example/x", serde_json::json!(1));
            let expected = serde_json::to_value(&meta).unwrap();

            let mut call_tool_req = CallToolRequest::new("t", serde_json::json!({}));
            call_tool_req._meta = Some(meta.clone());
            let call_tool = Request::Client(Box::new(ClientRequest::CallTool(call_tool_req)));
            let get_prompt = get_prompt_request("p", Some(meta.clone()));
            let read_resource = read_resource_request("mem://x", Some(meta));

            for req in [&call_tool, &get_prompt, &read_resource] {
                assert_eq!(
                    extract_request_meta_value(req),
                    Some(expected.clone()),
                    "every _meta-bearing ClientRequest variant must extract Some"
                );
            }
        }

        proptest::proptest! {
            #[test]
            fn extract_request_meta_value_fuzz_never_panics(
                key in "[a-zA-Z0-9._/-]{0,64}",
                strval in ".{0,4096}",
                use_prompt in proptest::prelude::any::<bool>(),
            ) {
                // Arbitrary namespaced key + oversized string value on a RequestMeta
                // set on GetPrompt or ReadResource. extract must round-trip to the
                // SAME serde_json::Value and never panic.
                let meta = RequestMeta::new().with_meta(key, serde_json::json!(strval));
                let expected = serde_json::to_value(&meta).unwrap();
                let req = if use_prompt {
                    get_prompt_request("p", Some(meta))
                } else {
                    read_resource_request("mem://x", Some(meta))
                };
                proptest::prop_assert_eq!(extract_request_meta_value(&req), Some(expected));
            }
        }

        // Capturing handlers record the RequestHandlerExtra signals the REAL
        // dispatch entrypoint threaded into them.
        #[derive(Clone, Debug, Default, PartialEq)]
        struct Captured {
            era: Option<Era>,
            has_client_info: bool,
            traceparent: Option<String>,
        }

        struct CapturingPrompt(Arc<Mutex<Option<Captured>>>);

        #[async_trait]
        impl PromptHandler for CapturingPrompt {
            async fn handle(
                &self,
                _args: HashMap<String, String>,
                extra: RequestHandlerExtra,
            ) -> Result<GetPromptResult> {
                *self.0.lock().unwrap() = Some(Captured {
                    era: extra.era(),
                    has_client_info: extra.client_info().is_some(),
                    traceparent: extra.trace_context().map(|t| t.traceparent),
                });
                Ok(GetPromptResult::new(vec![], None))
            }
        }

        struct CapturingResource(Arc<Mutex<Option<Captured>>>);

        #[async_trait]
        impl ResourceHandler for CapturingResource {
            async fn read(
                &self,
                _uri: &str,
                extra: RequestHandlerExtra,
            ) -> Result<ReadResourceResult> {
                *self.0.lock().unwrap() = Some(Captured {
                    era: extra.era(),
                    has_client_info: extra.client_info().is_some(),
                    traceparent: extra.trace_context().map(|t| t.traceparent),
                });
                Ok(ReadResourceResult::new(vec![Content::text("ok")]))
            }

            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<ListResourcesResult> {
                Ok(ListResourcesResult {
                    resources: vec![],
                    next_cursor: None,
                })
            }
        }

        fn build_core(
            prompt_cap: Arc<Mutex<Option<Captured>>>,
            resource_cap: Arc<Mutex<Option<Captured>>>,
        ) -> ServerCore {
            let mut prompts: HashMap<String, Arc<dyn PromptHandler>> = HashMap::new();
            prompts.insert(
                "greeting".to_string(),
                Arc::new(CapturingPrompt(prompt_cap)) as Arc<dyn PromptHandler>,
            );
            let resources: Option<Arc<dyn ResourceHandler>> =
                Some(Arc::new(CapturingResource(resource_cap)));

            ServerCore::new(
                Implementation::new("test-server", "1.0.0"),
                ServerCapabilities::default(),
                HashMap::new(),
                prompts,
                HashMap::new(),
                HashMap::new(),
                resources,
                None,
                None,
                None,
                Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
                Arc::new(RwLock::new(ToolMiddlewareChain::new())),
                None, // task_router
                None, // task_store
                true, // stateless_mode — skip the initialize gate
                PayloadLimits::default(),
            )
            .with_supported_protocol_versions(vec![
                ProtocolVersion("2025-11-25".to_string()),
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            ])
        }

        fn v2_meta_with_trace() -> RequestMeta {
            RequestMeta::new().with_meta(
                "traceparent",
                serde_json::json!("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
            )
        }

        fn v2_context() -> ProtocolContext {
            ProtocolContext::new(
                Era::V2,
                ProtocolVersion(crate::types::protocol::PROTOCOL_VERSION_2026_07_28.to_string()),
            )
            .with_client_info(Implementation::new("test-client", "9.9.9"))
        }

        // Enter through the REAL core dispatch entrypoint (handle_request_internal),
        // NOT the leaf handlers — a dropped dispatch-arm thread would regress this.
        #[tokio::test]
        async fn prompt_resource_protocol_context_via_dispatch_core() {
            // --- v2 dispatch: era==V2, client_info==Some, trace_context populated.
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());

            core.handle_request_internal(
                RequestId::from(1i64),
                get_prompt_request("greeting", Some(v2_meta_with_trace())),
                None,
                Some(v2_context()),
            )
            .await;
            core.handle_request_internal(
                RequestId::from(2i64),
                read_resource_request("mem://greeting", Some(v2_meta_with_trace())),
                None,
                Some(v2_context()),
            )
            .await;

            for cap in [&pcap, &rcap] {
                let c = cap.lock().unwrap().clone().expect("handler ran");
                assert_eq!(c.era, Some(Era::V2), "era must be V2 on a v2 dispatch");
                assert!(c.has_client_info, "client_info must be visible on v2");
                assert_eq!(
                    c.traceparent.as_deref(),
                    Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
                    "trace_context must reflect the W3C traceparent (proves with_request_meta)"
                );
            }

            // --- opted-in v1 fallback: era==Some(V1) (distinct from None).
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());
            let v1 = ProtocolContext::new(Era::V1, ProtocolVersion("2025-11-25".to_string()));
            core.handle_request_internal(
                RequestId::from(3i64),
                get_prompt_request("greeting", None),
                None,
                Some(v1.clone()),
            )
            .await;
            core.handle_request_internal(
                RequestId::from(4i64),
                read_resource_request("mem://greeting", None),
                None,
                Some(v1),
            )
            .await;
            assert_eq!(pcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));
            assert_eq!(rcap.lock().unwrap().clone().unwrap().era, Some(Era::V1));

            // --- non-opted-in server (resolver returns None): era==None.
            let pcap = Arc::new(Mutex::new(None));
            let rcap = Arc::new(Mutex::new(None));
            let core = build_core(pcap.clone(), rcap.clone());
            core.handle_request_internal(
                RequestId::from(5i64),
                get_prompt_request("greeting", None),
                None,
                None,
            )
            .await;
            core.handle_request_internal(
                RequestId::from(6i64),
                read_resource_request("mem://greeting", None),
                None,
                None,
            )
            .await;
            assert_eq!(pcap.lock().unwrap().clone().unwrap().era, None);
            assert_eq!(rcap.lock().unwrap().clone().unwrap().era, None);
        }
    }
}
