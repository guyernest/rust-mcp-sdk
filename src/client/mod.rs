//! MCP client implementation.

use crate::error::{Error, Result};
use crate::shared::{
    EnhancedMiddlewareChain, MiddlewareContext, Protocol, ProtocolOptions, Transport,
};
use crate::types::tasks::{
    resolve_poll_interval, CancelTaskRequest, CancelTaskResult, CreateTaskResult,
    GetTaskPayloadRequest, GetTaskRequest, GetTaskResult, ListTasksRequest, ListTasksResult, Task,
    TaskMetadata, TaskPollDecision, TaskStatus, MIN_POLL_MS,
};
use crate::types::{
    CallToolRequest, CallToolResult, CancelledNotification, ClientCapabilities, ClientNotification,
    ClientRequest, CompleteRequest, CompleteResult, CreateMessageParams, CreateMessageResult,
    GetPromptRequest, GetPromptResult, Implementation, InitializeRequest, InitializeResult,
    ListPromptsRequest, ListPromptsResult, ListResourceTemplatesRequest,
    ListResourceTemplatesResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest,
    ListToolsResult, LoggingLevel, Notification, ProgressNotification, PromptInfo,
    ReadResourceRequest, ReadResourceResult, Request, RequestId, ResourceInfo, ResourceTemplate,
    ServerCapabilities, SubscribeRequest, ToolInfo, UnsubscribeRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, oneshot, RwLock};

#[cfg(target_arch = "wasm32")]
use futures::SinkExt;
#[cfg(target_arch = "wasm32")]
use futures_channel::{mpsc, oneshot};
#[cfg(target_arch = "wasm32")]
use futures_locks::RwLock;

#[cfg(all(not(target_arch = "wasm32"), feature = "http-client"))]
pub mod auth;
pub mod host;
pub mod http_logging_middleware;
pub mod http_middleware;
#[cfg(all(not(target_arch = "wasm32"), feature = "oauth"))]
pub mod oauth;
pub mod oauth_middleware;
mod options;
pub mod transport;

pub use options::ClientOptions;

pub use host::{
    ApprovalDecision, ClientHostRegistry, HostElicitationHandler, HostSamplingHandler,
    PreflightApproval, RootsProvider, SamplingResultReview,
};

/// Response from a task-augmented `tools/call`.
///
/// When calling [`Client::call_tool_with_task`], the server may return either
/// an async task (poll with `tasks/get`) or a synchronous result.
#[derive(Debug, Clone)]
pub enum ToolCallResponse {
    /// The server returned a synchronous result (no task created).
    Result(CallToolResult),
    /// The server created an async task. Poll with [`Client::tasks_get`]
    /// until the task reaches a terminal status, then call
    /// [`Client::tasks_result`] to get the final `CallToolResult`.
    Task(Task),
}

/// Options controlling [`Client::wait_for_task`] polling.
///
/// A caller who holds a [`TaskMetadata`] (e.g. from
/// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task))
/// composes options directly via [`WaitForTaskOptions::from_metadata`] /
/// [`From<TaskMetadata>`] — no hand-copying of poll fields.
#[derive(Debug, Clone, Default)]
pub struct WaitForTaskOptions {
    /// Override polling interval, in **milliseconds**. When `None`, the
    /// task-reported `pollInterval` (then a built-in default) is used. The
    /// effective interval is clamped to a small floor so a zero value cannot
    /// hot-spin the poll loop.
    pub poll_interval: Option<u64>,
    /// Maximum total time to poll before returning a timeout error, in
    /// **seconds**. When `None`, polling continues until the task is terminal
    /// (or enters `input_required`, which surfaces an error immediately — see
    /// [`Client::wait_for_task`]).
    pub max_poll_duration_secs: Option<u64>,
}

impl WaitForTaskOptions {
    /// Build options from a [`TaskMetadata`], copying its poll fields verbatim.
    pub fn from_metadata(meta: &TaskMetadata) -> Self {
        Self {
            poll_interval: meta.poll_interval,
            max_poll_duration_secs: meta.max_poll_duration_secs,
        }
    }

    /// Fill any unset fields from `meta`; existing `self` values take precedence.
    #[must_use]
    pub fn or_from_metadata(mut self, meta: &TaskMetadata) -> Self {
        self.poll_interval = self.poll_interval.or(meta.poll_interval);
        self.max_poll_duration_secs = self.max_poll_duration_secs.or(meta.max_poll_duration_secs);
        self
    }
}

impl From<TaskMetadata> for WaitForTaskOptions {
    fn from(meta: TaskMetadata) -> Self {
        Self::from_metadata(&meta)
    }
}

/// MCP client for connecting to servers.
pub struct Client<T: Transport> {
    transport: Arc<RwLock<T>>,
    protocol: Arc<RwLock<Protocol>>,
    middleware_chain: Arc<RwLock<EnhancedMiddlewareChain>>,
    capabilities: Option<ClientCapabilities>,
    server_capabilities: Option<ServerCapabilities>,
    server_version: Option<Implementation>,
    instructions: Option<String>,
    initialized: bool,
    info: Implementation,
    notification_tx: Option<mpsc::Sender<Notification>>,
    active_requests: Arc<RwLock<HashMap<RequestId, oneshot::Sender<()>>>>,
    options: ClientOptions,
    /// Registered host handlers answering inbound server -> client requests
    /// (sampling / elicitation / roots). Immutable after construction.
    host_registry: crate::client::host::ClientHostRegistry,
}

impl<T: Transport> std::fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("transport", &"<Arc<RwLock<Transport>>>")
            .field("protocol", &"<Arc<RwLock<Protocol>>>")
            .field("capabilities", &self.capabilities)
            .field("server_capabilities", &self.server_capabilities)
            .field("initialized", &self.initialized)
            .field("info", &self.info)
            .field("host_registry", &self.host_registry)
            .finish()
    }
}

impl<T: Transport> Client<T> {
    /// Create a new client with the given transport.
    ///
    /// Uses default client information with the name "pmcp-client" and the
    /// current crate version.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport};
    ///
    /// let transport = StdioTransport::new();
    /// let client = Client::new(transport);
    /// ```
    pub fn new(transport: T) -> Self {
        Self::with_info(
            transport,
            Implementation::new("pmcp-client", env!("CARGO_PKG_VERSION")),
        )
    }

    /// Create a new client with custom info.
    ///
    /// Allows specifying custom client name and version information that will
    /// be sent to the server during initialization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation::new("my-custom-client", "2.1.0");
    /// let client = Client::with_info(transport, client_info);
    /// ```
    pub fn with_info(transport: T, client_info: Implementation) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            options: ClientOptions::default(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
        }
    }

    /// Create a new client with custom protocol options.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{Client, StdioTransport, Implementation};
    /// use pmcp::shared::ProtocolOptions;
    ///
    /// // Custom options for high-throughput scenarios
    /// let options = ProtocolOptions {
    ///     enforce_strict_capabilities: false,
    ///     debounced_notification_methods: vec![
    ///         "notifications/progress".to_string(),
    ///         "notifications/message".to_string(),
    ///     ],
    /// };
    ///
    /// let transport = StdioTransport::new();
    /// let client_info = Implementation::new("high-throughput-client", "1.0.0");
    ///
    /// let client = Client::with_options(transport, client_info, options);
    /// ```
    pub fn with_options(
        transport: T,
        client_info: Implementation,
        options: ProtocolOptions,
    ) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(options))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: client_info,
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            options: ClientOptions::default(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
        }
    }

    /// Construct a client with caller-supplied [`ClientOptions`].
    ///
    /// Mirrors [`Self::new`] but wires in a [`ClientOptions`] value so that
    /// [`Self::list_all_tools`] / [`Self::list_all_prompts`] / etc. honour a
    /// custom `max_iterations` cap.
    ///
    /// ## `ClientBuilder` parity
    ///
    /// [`ClientBuilder`] does not currently expose a `.client_options()` setter.
    /// If you need a custom [`ClientOptions`], construct the client via
    /// [`Self::with_client_options`] directly.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(transport: T) -> pmcp::Result<()> {
    /// use pmcp::{Client, ClientOptions};
    ///
    /// let opts = ClientOptions::default().with_max_iterations(50);
    /// let _client = Client::with_client_options(transport, opts);
    /// # Ok(()) }
    /// ```
    pub fn with_client_options(transport: T, options: ClientOptions) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            protocol: Arc::new(RwLock::new(Protocol::new(ProtocolOptions::default()))),
            middleware_chain: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            capabilities: None,
            server_capabilities: None,
            server_version: None,
            instructions: None,
            initialized: false,
            info: Implementation::default(),
            notification_tx: None,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            options,
            host_registry: crate::client::host::ClientHostRegistry::default(),
        }
    }

    /// Initialize the connection with the server.
    ///
    /// Performs the MCP initialization handshake, negotiating capabilities and
    /// receiving server information. This must be called before using other
    /// client methods.
    ///
    /// # Host capabilities are registry-derived (`sampling` / `elicitation` / `roots`)
    ///
    /// The three host-side capability fields — `sampling`, `elicitation`, and
    /// `roots` — are **derived from the handlers registered on
    /// [`ClientBuilder`]**, not from the value passed here. If no matching host
    /// handler is registered, the corresponding field is forced to `None` on the
    /// wire even when the caller set it (the anti-capability-lie rule: a client
    /// must not advertise a host capability it cannot service). Register handlers
    /// via [`ClientBuilder::on_sampling`], [`ClientBuilder::on_elicitation`], and
    /// [`ClientBuilder::on_roots`] to advertise these capabilities. When a
    /// handler *is* registered, any caller-configured detail for that field
    /// (e.g. `roots.list_changed`) is preserved. All other capability fields
    /// (`tasks`, `experimental`, ...) pass through unchanged.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// let capabilities = ClientCapabilities::default();
    /// let server_info = client.initialize(capabilities).await?;
    ///
    /// println!("Server: {} v{}",
    ///          server_info.server_info.name,
    ///          server_info.server_info.version);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is already initialized
    /// - The server rejects the initialization
    /// - Communication with the server fails
    pub async fn initialize(
        &mut self,
        mut capabilities: ClientCapabilities,
    ) -> Result<InitializeResult> {
        if self.initialized {
            return Err(Error::InvalidState("Client already initialized".into()));
        }

        // HOST-05: make the three host capability fields reflect the registry
        // (registry-authoritative anti-capability-lie) before advertising them.
        self.derive_host_capabilities(&mut capabilities);

        self.capabilities = Some(capabilities.clone());

        // Send initialize request
        let request = Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest {
            protocol_version: crate::types::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities,
            client_info: self.info.clone(),
        })));

        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        // Parse initialize result
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                let init_result = serde_json::from_value::<InitializeResult>(result)
                    .map_err(|e| Error::parse(format!("Invalid initialize result: {e}")))?;

                // Validate protocol version
                if !crate::types::SUPPORTED_PROTOCOL_VERSIONS
                    .contains(&init_result.protocol_version.as_str())
                {
                    return Err(Error::protocol_msg(format!(
                        "Server protocol version {} not supported",
                        init_result.protocol_version
                    )));
                }

                self.server_capabilities = Some(init_result.capabilities.clone());
                self.server_version = Some(init_result.server_info.clone());
                self.instructions.clone_from(&init_result.instructions);
                self.initialized = true;

                // Send initialized notification
                self.send_notification(Notification::Client(ClientNotification::Initialized))
                    .await?;

                Ok(init_result)
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Apply the HOST-05 registry-authoritative rule to the three host
    /// capability fields (`sampling`/`elicitation`/`roots`), leaving every other
    /// field (`tasks`, `experimental`, ...) untouched.
    ///
    /// Per field:
    /// - **Handler absent** => force `None` (locked anti-capability-lie: a
    ///   caller-set value with no registered handler is discarded, closing the
    ///   spoofing hole where a client advertises a host capability it cannot
    ///   actually service).
    /// - **Handler present, caller left `None`** => insert `Some(default())`.
    /// - **Handler present, caller configured detail** => preserve the caller's
    ///   value unchanged (keeps configured sampling tool support / roots
    ///   `list_changed` / elicitation modes).
    ///
    /// There is deliberately no independent public setter for these three
    /// fields — advertisement is derived, never independently assertable.
    fn derive_host_capabilities(&self, capabilities: &mut ClientCapabilities) {
        // Apply the HOST-05 rule to one capability field:
        // - handler absent (`registered == false`) => force `None`,
        // - handler present, caller left `None` => insert `Some(default())`,
        // - handler present, caller configured detail => leave untouched.
        fn sync_cap<C: Default>(slot: &mut Option<C>, registered: bool) {
            if !registered {
                *slot = None;
            } else if slot.is_none() {
                *slot = Some(C::default());
            }
        }

        sync_cap(
            &mut capabilities.sampling,
            self.host_registry.sampling.is_some(),
        );
        sync_cap(
            &mut capabilities.elicitation,
            self.host_registry.elicitation.is_some(),
        );
        sync_cap(&mut capabilities.roots, self.host_registry.roots.is_some());
    }

    /// Get server capabilities after initialization.
    pub fn get_server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    /// Get server version information after initialization.
    pub fn get_server_version(&self) -> Option<&Implementation> {
        self.server_version.as_ref()
    }

    /// Get server instructions after initialization.
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Send a ping to the server.
    pub async fn ping(&self) -> Result<()> {
        self.ensure_initialized()?;
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Set the logging level on the server.
    pub async fn set_logging_level(&self, level: LoggingLevel) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("logging", "logging/setLevel")?;

        let request = Request::Client(Box::new(ClientRequest::SetLoggingLevel { level }));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List available tools.
    ///
    /// Retrieves information about all tools available on the server, including
    /// their names, descriptions, and input schemas.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all tools
    /// let tools = client.list_tools(None).await?;
    /// for tool in tools.tools {
    ///     println!("Tool: {} - {}",
    ///              tool.name,
    ///              tool.description.unwrap_or_else(|| "No description".to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional pagination cursor for retrieving additional results
    pub async fn list_tools(&self, cursor: Option<String>) -> Result<ListToolsResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListTools(ListToolsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool.
    ///
    /// Invokes a server-provided tool with the specified name and arguments.
    /// The server must have declared the tool via the tools capability during initialization.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `arguments` - JSON value containing the tool's arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Call a simple tool with no arguments
    /// let result = client.call_tool(
    ///     "list_files".to_string(),
    ///     json!({})
    /// ).await?;
    ///
    /// // Call a tool with specific arguments
    /// let search_result = client.call_tool(
    ///     "search".to_string(),
    ///     json!({
    ///         "query": "rust programming",
    ///         "limit": 10
    ///     })
    /// ).await?;
    ///
    /// // Tools can return structured data
    /// if let Some(content) = result.content.first() {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Tool result: {}", text);
    ///         }
    ///         _ => println!("Non-text tool result"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support tools
    /// - The tool name doesn't exist
    /// - The arguments are invalid for the tool
    /// - Network or protocol errors occur
    pub async fn call_tool(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // =========================================================================
    // MCP Tasks (2025-11-25)
    // =========================================================================

    /// Call a tool with task augmentation.
    ///
    /// Sends a `tools/call` request with the `task` field set, signaling to the
    /// server that this client supports async task polling. The server may return
    /// either a `CreateTaskResult` (async task created) or a `CallToolResult`
    /// (sync result) depending on the tool's `taskSupport` declaration.
    ///
    /// Use [`call_tool`](Self::call_tool) instead if you don't need task support.
    ///
    /// # Returns
    ///
    /// - `Ok(ToolCallResponse::Task(task))` if the server created an async task.
    ///   Poll with [`tasks_get`](Self::tasks_get) until the task reaches a
    ///   terminal status.
    /// - `Ok(ToolCallResponse::Result(result))` if the server returned the
    ///   result synchronously.
    pub async fn call_tool_with_task(
        &self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResponse> {
        self.ensure_initialized()?;
        self.assert_capability("tools", "tools/call")?;

        let request = Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest {
            name,
            arguments,
            _meta: None,
            task: Some(serde_json::json!({})),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                // Try CreateTaskResult first (more specific), fall back to CallToolResult.
                // This avoids brittle key-name duck-typing.
                if let Ok(task_result) = serde_json::from_value::<CreateTaskResult>(result.clone())
                {
                    Ok(ToolCallResponse::Task(task_result.task))
                } else {
                    let tool_result: CallToolResult =
                        serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))?;
                    Ok(ToolCallResponse::Result(tool_result))
                }
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get the current status of a task.
    ///
    /// Polls the server for the task's current state. Call this repeatedly
    /// (respecting `task.poll_interval`) until the task reaches a terminal
    /// status (`Completed`, `Failed`, or `Cancelled`).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The server doesn't support tasks
    /// - The task ID doesn't exist or belongs to another owner
    pub async fn tasks_get(&self, task_id: &str) -> Result<Task> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", "tasks/get")?;

        let request = Request::Client(Box::new(ClientRequest::TasksGet(GetTaskRequest {
            task_id: task_id.to_string(),
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        let task_result: GetTaskResult = self.parse_task_payload(response, "tasks/get").await?;
        Ok(task_result.task)
    }

    /// Get the final result of a completed task.
    ///
    /// For a task-augmented `tools/call`, this returns the `CallToolResult`
    /// that the tool would have returned synchronously. Only valid when
    /// the task status is `Completed`.
    pub async fn tasks_result(&self, task_id: &str) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", "tasks/result")?;

        let request = Request::Client(Box::new(ClientRequest::TasksResult(
            GetTaskPayloadRequest {
                task_id: task_id.to_string(),
            },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        self.parse_task_payload::<CallToolResult>(response, "tasks/result")
            .await
    }

    /// Poll a task to terminal status, then return its `tasks/result`.
    ///
    /// Drives `tasks/get` in a loop until [`TaskStatus::is_terminal`], honoring
    /// the polling interval (caller override, else the task-reported
    /// `pollInterval`, else a built-in default) and an optional overall timeout.
    /// On terminal status it fetches and returns the persisted
    /// [`CallToolResult`] via [`Client::tasks_result`].
    ///
    /// # Wasm safety
    ///
    /// The delay between polls uses [`crate::runtime::sleep`] (not
    /// `tokio::time::sleep` directly) and the timeout is measured with
    /// [`web_time::Instant`] (not `std::time::Instant`, which panics on
    /// `wasm32`), so this compiles and runs in the browser.
    ///
    /// # Hot-loop protection
    ///
    /// The effective interval is clamped to a small floor (50 ms), so a zero or
    /// absent `pollInterval` cannot turn the loop into a busy spin.
    ///
    /// # Errors
    ///
    /// - Propagates `tasks/get` / `tasks/result` transport and protocol errors.
    /// - Returns [`Error::Timeout`] when `opts.max_poll_duration_secs` elapses
    ///   before the task reaches a terminal status. Each sleep is clamped to
    ///   the remaining budget, so a large (possibly server-reported) poll
    ///   interval cannot overshoot the caller's budget by more than roughly
    ///   the 50 ms clamp floor.
    /// - Returns [`Error::Validation`] when the task enters
    ///   [`TaskStatus::InputRequired`]: that state is NOT terminal and needs
    ///   client-side action (elicitation) this poller cannot provide, so
    ///   polling on would hang forever under the default (unbounded) options.
    ///   Handle the required input, then resume polling.
    ///
    /// # Durable and replay consumers
    ///
    /// Do **not** wrap `wait_for_task` inside a durable / replay workflow step.
    /// It sleeps, loops, and owns the whole polling lifecycle, which is
    /// non-deterministic under replay (each re-execution would re-sleep and
    /// re-poll). A durable consumer should instead call
    /// [`Task::poll_decision`](crate::types::tasks::Task::poll_decision) plus
    /// [`resolve_poll_interval`] once per tick inside its own memoized step and
    /// persist the decision
    /// between ticks — those are pure, replay-deterministic functions of the
    /// polled task, unlike this blocking poller (D-11 / D-16).
    ///
    /// See the pmcp-book "Durable and replay consumers" section
    /// (heading `## Durable and replay consumers` in
    /// `pmcp-book/src/ch12-7-tasks.md`) for the full per-poll pattern:
    /// <https://paiml.github.io/rust-mcp-sdk/ch12-7-tasks.html#durable-and-replay-consumers>.
    /// (This is a deliberate plain-text/URL reference, not a rustdoc intra-doc
    /// link, so it never fails `cargo doc` even before that page ships.)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pmcp::client::WaitForTaskOptions;
    ///
    /// // `result` came from a task-augmented tools/call.
    /// if let Some(meta) = result.related_task() {
    ///     let final_result = client
    ///         .wait_for_related_task(&meta, WaitForTaskOptions::default())
    ///         .await?;
    /// }
    /// ```
    pub async fn wait_for_task(
        &self,
        task_id: &str,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", "tasks/get")?;

        // Wasm-safe monotonic clock: IS std::time::Instant on native, browser-safe on wasm.
        let start = web_time::Instant::now();
        loop {
            let task = self.tasks_get(task_id).await?;

            // Single source of truth for the stop / ask / sleep decision: the
            // `poll_decision()` classifier in src/types/tasks.rs (D-13). No
            // parallel terminal-status or input-required comparison lives here,
            // so the poller and the classifier cannot drift. This matches the
            // `#[non_exhaustive]` `TaskPollDecision` exhaustively (no `_` arm)
            // because it is in-crate — a future variant becomes a compile error
            // here, forcing an explicit decision.
            match task.poll_decision() {
                // Terminal — stop polling and fetch the persisted result below.
                TaskPollDecision::Terminal { .. } => break,
                // `input_required` is NOT terminal, and the task cannot progress
                // without client-side action this poller does not perform —
                // surface it (returning BEFORE any tasks/result fetch) instead
                // of spinning until a (possibly absent) timeout (CR-01).
                TaskPollDecision::InputRequired => {
                    return Err(Error::validation(format!(
                        "task {task_id} is input_required; wait_for_task cannot provide \
                         input — handle the elicitation, then resume polling"
                    )));
                },
                // Still running — resolve the next sleep through the shared
                // resolver (D-02: caller override, else the server-reported
                // pollInterval hint, else the default, floored to MIN_POLL_MS).
                TaskPollDecision::InProgress { poll_hint } => {
                    let mut interval = resolve_poll_interval(opts.poll_interval, poll_hint);

                    // Enforce the overall polling budget (millisecond precision)
                    // and clamp the next sleep to the REMAINING budget — the
                    // interval may be server-chosen (task-reported pollInterval),
                    // and an unclamped sleep would overshoot a caller-specified
                    // budget by up to one arbitrary server interval. This clamp
                    // is loop state (not task state), so it stays INLINE here
                    // rather than moving into the classifier or resolver (WR-01 /
                    // D-09).
                    if let Some(max_secs) = opts.max_poll_duration_secs {
                        let budget_ms = max_secs.saturating_mul(1000);
                        let elapsed_ms =
                            u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let remaining_ms = budget_ms.saturating_sub(elapsed_ms);
                        if remaining_ms == 0 {
                            return Err(Error::timeout(budget_ms));
                        }
                        interval = interval.min(remaining_ms.max(MIN_POLL_MS));
                    }
                    crate::runtime::sleep(std::time::Duration::from_millis(interval)).await;
                },
            }
        }

        self.tasks_result(task_id).await
    }

    /// Poll a task referenced by [`TaskMetadata`] to terminal, then return its
    /// `tasks/result` — the zero-glue counterpart of [`Client::wait_for_task`].
    ///
    /// Any fields left unset in `opts` are filled from `meta`
    /// ([`WaitForTaskOptions::or_from_metadata`]) so a caller who holds a
    /// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task)
    /// result composes without hand-copying poll fields.
    ///
    /// # Errors
    ///
    /// Same as [`Client::wait_for_task`].
    pub async fn wait_for_related_task(
        &self,
        meta: &TaskMetadata,
        opts: WaitForTaskOptions,
    ) -> Result<CallToolResult> {
        self.wait_for_task(&meta.task_id, opts.or_from_metadata(meta))
            .await
    }

    /// List tasks owned by the current client.
    pub async fn tasks_list(&self, cursor: Option<String>) -> Result<ListTasksResult> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", "tasks/list")?;

        let request = Request::Client(Box::new(ClientRequest::TasksList(ListTasksRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        self.parse_task_payload::<ListTasksResult>(response, "tasks/list")
            .await
    }

    /// Cancel a running task.
    pub async fn tasks_cancel(&self, task_id: &str) -> Result<Task> {
        self.ensure_initialized()?;
        self.assert_capability("tasks", "tasks/cancel")?;

        let request = Request::Client(Box::new(ClientRequest::TasksCancel(CancelTaskRequest {
            task_id: task_id.to_string(),
            result: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        let cancel_result: CancelTaskResult =
            self.parse_task_payload(response, "tasks/cancel").await?;
        Ok(cancel_result.task)
    }

    /// Deserialize a `tasks/*` response payload into `T`, emitting a structured
    /// WARN (method + transport identity + serde error) on a deserialize failure
    /// before surfacing it. Centralizes the four task endpoints' identical
    /// result-vs-error handling.
    ///
    /// Lock-on-error: the transport identity is read only on the cold failure
    /// path (D-LOCK-ON-ERROR — no cached field on `Client`).
    async fn parse_task_payload<D: serde::de::DeserializeOwned>(
        &self,
        response: crate::types::JSONRPCResponse,
        method: &'static str,
    ) -> Result<D> {
        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                match serde_json::from_value::<D>(result) {
                    Ok(value) => Ok(value),
                    Err(e) => {
                        let transport = self.transport.read().await.transport_type();
                        Self::log_task_deserialize_error(
                            method,
                            std::any::type_name::<D>(),
                            transport,
                            &e,
                        );
                        Err(Error::parse(e.to_string()))
                    },
                }
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Call a tool and automatically poll until the task completes.
    ///
    /// This is a high-level convenience method that encapsulates the full
    /// task lifecycle:
    #[cfg(not(target_arch = "wasm32"))]
    /// 1. Calls the tool with task augmentation
    /// 2. If the server returns a task, polls `tasks/get` until terminal status
    /// 3. Returns the final `CallToolResult`
    ///
    /// If the server returns a sync result (no task), returns it immediately.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    /// * `arguments` - Tool arguments
    /// * `max_polls` - Maximum number of poll attempts before giving up (0 = unlimited)
    pub async fn call_tool_and_poll(
        &self,
        name: String,
        arguments: serde_json::Value,
        max_polls: usize,
    ) -> Result<CallToolResult> {
        /// Default polling interval when the server doesn't specify one.
        const DEFAULT_POLL_INTERVAL_MS: u64 = 5000;

        let response = self.call_tool_with_task(name, arguments).await?;

        match response {
            ToolCallResponse::Result(result) => Ok(result),
            ToolCallResponse::Task(initial_task) => {
                let task_id = initial_task.task_id.clone();
                let mut poll_ms = initial_task
                    .poll_interval
                    .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
                let mut polls = 0;

                loop {
                    polls += 1;

                    let task = self.tasks_get(&task_id).await?;

                    if task.status == TaskStatus::InputRequired {
                        return Err(Error::internal(format!(
                            "Task {} requires input — handle interactively via tasks_get/tasks_cancel",
                            task_id
                        )));
                    }

                    if task.status.is_terminal() {
                        if task.status == TaskStatus::Completed {
                            // Try to get the full result via tasks/result
                            match self.tasks_result(&task_id).await {
                                Ok(result) => return Ok(result),
                                // Only fall back for method-not-found (-32601); propagate real errors
                                Err(Error::Protocol { code, .. })
                                    if code == crate::error::ErrorCode::METHOD_NOT_FOUND =>
                                {
                                    let text = task
                                        .status_message
                                        .unwrap_or_else(|| "Task completed".to_string());
                                    return Ok(CallToolResult::new(vec![
                                        crate::types::Content::text(text),
                                    ]));
                                },
                                Err(e) => return Err(e),
                            }
                        } else {
                            // Failed or Cancelled
                            let text = task
                                .status_message
                                .unwrap_or_else(|| format!("Task {}", task.status));
                            return Ok(CallToolResult::error(vec![crate::types::Content::text(
                                text,
                            )]));
                        }
                    }

                    if max_polls > 0 && polls >= max_polls {
                        return Err(Error::internal(format!(
                            "Task {} did not complete after {} polls",
                            task_id, max_polls
                        )));
                    }

                    // Honor updated poll_interval from server (e.g., exponential backoff)
                    if let Some(interval) = task.poll_interval {
                        poll_ms = interval;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
                }
            },
        }
    }

    // =========================================================================
    // Prompts
    // =========================================================================

    /// List available prompts.
    ///
    /// Retrieves information about all prompts available on the server, including
    /// their names, descriptions, and required arguments.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large prompt lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all prompts
    /// let prompts = client.list_prompts(None).await?;
    /// for prompt in prompts.prompts {
    ///     println!("Prompt: {} - {}",
    ///              prompt.name,
    ///              prompt.description.unwrap_or_else(|| "No description".to_string()));
    ///     
    ///     // Show required arguments
    ///     if let Some(args) = prompt.arguments {
    ///         for arg in args {
    ///             println!("  - {}: {} (required: {})",
    ///                      arg.name,
    ///                      arg.description.unwrap_or_else(|| "No description".to_string()),
    ///                      arg.required);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - Network or protocol errors occur
    pub async fn list_prompts(&self, cursor: Option<String>) -> Result<ListPromptsResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListPrompts(ListPromptsRequest {
            cursor,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Get a prompt.
    ///
    /// Retrieves a specific prompt from the server with the provided arguments.
    /// The prompt is processed by the server and returned with filled-in content.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the prompt to retrieve
    /// * `arguments` - Key-value pairs for prompt arguments
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    /// use std::collections::HashMap;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Get a prompt with arguments
    /// let mut args = HashMap::new();
    /// args.insert("language".to_string(), "Rust".to_string());
    /// args.insert("topic".to_string(), "async programming".to_string());
    ///
    /// let prompt_result = client.get_prompt(
    ///     "code_review".to_string(),
    ///     args
    /// ).await?;
    ///
    /// println!("Prompt description: {}",
    ///          prompt_result.description.unwrap_or_else(|| "No description".to_string()));
    ///
    /// // Process the prompt messages
    /// for message in prompt_result.messages {
    ///     println!("Role: {}", message.role);
    ///     match &message.content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Content: {}", text);
    ///         }
    ///         _ => println!("Non-text content"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support prompts
    /// - The prompt name doesn't exist
    /// - Required arguments are missing
    /// - Network or protocol errors occur
    pub async fn get_prompt(
        &self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<GetPromptResult> {
        self.ensure_initialized()?;
        self.assert_capability("prompts", "prompts/get")?;

        let request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptRequest {
            name,
            arguments,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // === Typed call helpers ===

    /// Call a tool with typed, serializable arguments.
    ///
    /// Serializes `args` via `serde_json::to_value` and delegates to
    /// [`Self::call_tool`]. Serialization failures are mapped to
    /// [`Error::validation`] with the underlying serde error message.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Search { query: String, limit: u32 }
    ///
    /// let _ = client.call_tool_typed(
    ///     "search",
    ///     &Search { query: "rust mcp".into(), limit: 10 },
    /// ).await?;
    /// # Ok(()) }
    /// ```
    pub async fn call_tool_typed<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<CallToolResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed arguments: {e}")))?;
        self.call_tool(name.into(), value).await
    }

    /// Typed sibling of [`Self::call_tool_with_task`].
    ///
    /// Delegates to the two-argument [`Self::call_tool_with_task`]; there is no
    /// `TaskMetadata` parameter on the live client API, so none is exposed here.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    /// #[derive(Serialize)]
    /// struct Args { file: String }
    /// let _ = client.call_tool_typed_with_task("scan", &Args { file: "a.rs".into() }).await?;
    /// # Ok(()) }
    /// ```
    pub async fn call_tool_typed_with_task<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<ToolCallResponse> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed_with_task arguments: {e}")))?;
        self.call_tool_with_task(name.into(), value).await
    }

    /// Typed sibling of [`Self::call_tool_and_poll`].
    ///
    /// Delegates to the three-argument [`Self::call_tool_and_poll`]
    /// (`name, arguments, max_polls: usize`). There is no `poll_interval` or
    /// `TaskMetadata` parameter on the live client API — the server-supplied
    /// `poll_interval` is honoured internally by `call_tool_and_poll`.
    ///
    /// `max_polls = 0` means unlimited polls, matching the sibling's semantics.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    /// #[derive(Serialize)]
    /// struct Args { job: String }
    /// let _ = client.call_tool_typed_and_poll(
    ///     "build",
    ///     &Args { job: "nightly".into() },
    ///     30, // max_polls
    /// ).await?;
    /// # Ok(()) }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn call_tool_typed_and_poll<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
        max_polls: usize,
    ) -> Result<CallToolResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("call_tool_typed_and_poll arguments: {e}")))?;
        self.call_tool_and_poll(name.into(), value, max_polls).await
    }

    /// Get a prompt with typed, serializable arguments.
    ///
    /// Serializes `args` to a JSON object, then coerces each leaf to a `String`
    /// for the wire-level `HashMap<String, String>` arguments:
    /// - `null` entries are omitted
    /// - `string` entries pass through unchanged (no JSON-quoting)
    /// - `number` and `bool` entries use `Display` (e.g. `42`, `true`)
    /// - `array` and `object` entries are re-serialized via
    ///   [`serde_json::to_string`]
    ///
    /// Non-object top-level serializations are rejected with
    /// [`Error::validation`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct SummaryArgs { topic: String, length: u32 }
    ///
    /// let _ = client.get_prompt_typed(
    ///     "summarize",
    ///     &SummaryArgs { topic: "rust async".into(), length: 200 },
    /// ).await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_prompt_typed<A: serde::Serialize + ?Sized + Sync>(
        &self,
        name: impl Into<String> + Send,
        args: &A,
    ) -> Result<GetPromptResult> {
        let value = serde_json::to_value(args)
            .map_err(|e| Error::validation(format!("get_prompt_typed arguments: {e}")))?;
        let serde_json::Value::Object(obj) = value else {
            return Err(Error::validation(
                "prompts/get arguments must serialize to a JSON object",
            ));
        };
        let mut out: HashMap<String, String> = HashMap::with_capacity(obj.len());
        for (k, v) in obj {
            match v {
                serde_json::Value::Null => {},
                serde_json::Value::String(s) => {
                    out.insert(k, s);
                },
                serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
                    out.insert(k, v.to_string());
                },
                serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                    let nested = serde_json::to_string(&v).map_err(|e| {
                        Error::validation(format!("get_prompt_typed nested arg {k}: {e}"))
                    })?;
                    out.insert(k, nested);
                },
            }
        }
        self.get_prompt(name.into(), out).await
    }

    /// List available resources.
    ///
    /// Retrieves information about all resources available on the server, including
    /// their names, descriptions, URIs, and MIME types.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large resource lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resources
    /// let resources = client.list_resources(None).await?;
    /// for resource in resources.resources {
    ///     println!("Resource: {} ({})", resource.name, resource.uri);
    ///     if let Some(description) = resource.description {
    ///         println!("  Description: {}", description);
    ///     }
    ///     if let Some(mime_type) = resource.mime_type {
    ///         println!("  MIME Type: {}", mime_type);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - Network or protocol errors occur
    pub async fn list_resources(&self, cursor: Option<String>) -> Result<ListResourcesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResources(
            ListResourcesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// List resource templates.
    ///
    /// Retrieves information about all resource templates available on the server.
    /// Resource templates define patterns for dynamically generated resources.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Optional cursor for pagination of large template lists
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // List all resource templates
    /// let templates = client.list_resource_templates(None).await?;
    /// for template in templates.resource_templates {
    ///     println!("Template: {} ({})", template.name, template.uri_template);
    ///     if let Some(description) = template.description {
    ///         println!("  Description: {}", description);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource templates
    /// - Network or protocol errors occur
    pub async fn list_resource_templates(
        &self,
        cursor: Option<String>,
    ) -> Result<ListResourceTemplatesResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/templates/list")?;

        let request = Request::Client(Box::new(ClientRequest::ListResourceTemplates(
            ListResourceTemplatesRequest { cursor },
        )));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    // === Auto-paginating list helpers ===

    /// List all tools across all pages, auto-paginating on `next_cursor`.
    ///
    /// Loops calling [`Self::list_tools`], terminating when the server returns
    /// `next_cursor: None`. Safety cap: if the loop runs more than
    /// `self.options.max_iterations` iterations (default `100`), returns
    /// [`Error::Validation`] instead of continuing or silently truncating.
    ///
    /// Empty-string cursors (`Some("")`) do NOT terminate the loop — only
    /// `None` does. This matches the MCP spec, which treats the cursor as an
    /// opaque server token and does not ascribe meaning to the empty string.
    ///
    /// # Memory
    ///
    /// This helper accumulates **all pages** in memory before returning. For
    /// very large servers, prefer the paginated single-page
    /// [`Self::list_tools`] and stream the output yourself — this helper is a
    /// convenience API and will amplify memory usage proportional to the
    /// total tool count.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_tools`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_tools exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let tools = client.list_all_tools().await?;
    /// println!("discovered {} tools", tools.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_tools(&self) -> Result<Vec<ToolInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ToolInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_tools(cursor).await?;
            out.extend(page.tools);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_tools exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all prompts across all pages, auto-paginating on `next_cursor`.
    ///
    /// Semantics identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer [`Self::list_prompts`] for
    /// very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_prompts`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_prompts exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let prompts = client.list_all_prompts().await?;
    /// println!("discovered {} prompts", prompts.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_prompts(&self) -> Result<Vec<PromptInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<PromptInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_prompts(cursor).await?;
            out.extend(page.prompts);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_prompts exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all resources across all pages, auto-paginating on `next_cursor`.
    ///
    /// Semantics identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer [`Self::list_resources`] for
    /// very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_resources`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_resources exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let resources = client.list_all_resources().await?;
    /// println!("discovered {} resources", resources.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_resources(&self) -> Result<Vec<ResourceInfo>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ResourceInfo> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_resources(cursor).await?;
            out.extend(page.resources);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_resources exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// List all resource templates across all pages, auto-paginating on
    /// `next_cursor`.
    ///
    /// Uses the distinct `resources/templates/list` capability path (all
    /// other `list_all_*` helpers hit their own methods). Semantics otherwise
    /// identical to [`Self::list_all_tools`]: bounded by
    /// `self.options.max_iterations`, terminates only on `next_cursor: None`,
    /// returns [`Error::Validation`] on cap exceeded.
    ///
    /// # Memory
    ///
    /// Accumulates all pages in memory; prefer
    /// [`Self::list_resource_templates`] for very large servers.
    ///
    /// # Errors
    ///
    /// - Any error surfaced by [`Self::list_resource_templates`] propagates unchanged.
    /// - Cap exceeded → `Error::Validation("list_all_resource_templates exceeded max_iterations cap of N pages")`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn ex<T: pmcp::shared::Transport + Send + Sync + 'static>(mut client: pmcp::Client<T>) -> pmcp::Result<()> {
    /// let templates = client.list_all_resource_templates().await?;
    /// println!("discovered {} templates", templates.len());
    /// # Ok(()) }
    /// ```
    pub async fn list_all_resource_templates(&self) -> Result<Vec<ResourceTemplate>> {
        let cap = self.options.max_iterations;
        let mut out: Vec<ResourceTemplate> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..cap {
            let page = self.list_resource_templates(cursor).await?;
            out.extend(page.resource_templates);
            match page.next_cursor {
                None => return Ok(out),
                Some(next) => cursor = Some(next),
            }
        }
        Err(Error::validation(format!(
            "list_all_resource_templates exceeded max_iterations cap of {cap} pages"
        )))
    }

    /// Read a resource.
    ///
    /// Retrieves the content of a specific resource from the server by its URI.
    /// Resources can contain text, binary data, or structured content.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to read
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Read a text resource
    /// let resource = client.read_resource("file://readme.txt".to_string()).await?;
    /// for content in resource.contents {
    ///     match content {
    ///         pmcp::Content::Text { text } => {
    ///             println!("Text content: {}", text);
    ///         }
    ///         pmcp::Content::Resource { uri, .. } => {
    ///             println!("Resource reference: {}", uri);
    ///         }
    ///         _ => println!("Other content type"),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resources
    /// - The resource URI doesn't exist
    /// - Access to the resource is denied
    /// - Network or protocol errors occur
    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/read")?;

        let request = Request::Client(Box::new(ClientRequest::ReadResource(ReadResourceRequest {
            uri,
            _meta: None,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Subscribe to resource updates.
    ///
    /// Subscribes to receive notifications when a resource changes.
    /// The server will send notifications when the subscribed resource is modified.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to subscribe to
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a configuration file
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Now the client will receive notifications when settings.json changes
    /// // Handle notifications in your event loop
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI doesn't exist
    /// - Network or protocol errors occur
    pub async fn subscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/subscribe")?;

        // Check if server supports subscriptions
        if let Some(resources) = &self
            .server_capabilities
            .as_ref()
            .and_then(|c| c.resources.as_ref())
        {
            if !resources.subscribe.unwrap_or(false) {
                return Err(Error::capability(
                    "Server does not support resource subscriptions",
                ));
            }
        }

        let request = Request::Client(Box::new(ClientRequest::Subscribe(SubscribeRequest { uri })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Unsubscribe from resource updates.
    ///
    /// Unsubscribes from notifications for a previously subscribed resource.
    /// After unsubscribing, the client will no longer receive change notifications.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI of the resource to unsubscribe from
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Subscribe to a resource
    /// client.subscribe_resource("file://config/settings.json".to_string()).await?;
    ///
    /// // Later, unsubscribe when no longer needed
    /// client.unsubscribe_resource("file://config/settings.json".to_string()).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support resource subscriptions
    /// - The resource URI was not previously subscribed to
    /// - Network or protocol errors occur
    pub async fn unsubscribe_resource(&self, uri: String) -> Result<()> {
        self.ensure_initialized()?;
        self.assert_capability("resources", "resources/unsubscribe")?;

        let request = Request::Client(Box::new(ClientRequest::Unsubscribe(UnsubscribeRequest {
            uri,
        })));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(_) => Ok(()),
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Request completion from the server.
    ///
    /// Requests auto-completion suggestions from the server for a given context.
    /// This is useful for implementing IDE-like features with contextual suggestions.
    ///
    /// # Arguments
    ///
    /// * `params` - The completion request parameters
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CompleteRequest};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Request completion for partial text
    /// let completion_request = CompleteRequest {
    ///     r#ref: pmcp::CompletionReference::Resource {
    ///         uri: "file://code.rs".to_string(),
    ///     },
    ///     argument: pmcp::CompletionArgument {
    ///         name: "function_name".to_string(),
    ///         value: "calc_".to_string(),
    ///     },
    /// };
    ///
    /// let completions = client.complete(completion_request).await?;
    /// for completion in completions.completion.values {
    ///     println!("Suggestion: {}", completion);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support completions
    /// - The completion context is invalid
    /// - Network or protocol errors occur
    pub async fn complete(&self, params: CompleteRequest) -> Result<CompleteResult> {
        self.ensure_initialized()?;
        self.assert_capability("completions", "completion/complete")?;

        let request = Request::Client(Box::new(ClientRequest::Complete(params)));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Create a message using sampling (for LLM providers).
    ///
    /// Requests the server to generate a message using its language model capabilities.
    /// This is typically used by servers that provide LLM functionality.
    ///
    /// # The "LLM-server pattern" (INVERSE of spec host sampling)
    ///
    /// This method is the **LLM-server pattern**: the *client* asks a *server*
    /// whose [`pmcp::SamplingHandler`](crate::SamplingHandler) runs the LLM. It
    /// is the **inverse** of MCP spec host sampling, where a server requests
    /// sampling and the client answers via a
    /// [`pmcp::client::host::HostSamplingHandler`](crate::client::host::HostSamplingHandler).
    /// Both directions are supported and neither is deprecated — pick the one
    /// that matches who owns the model. This path is unchanged by the client
    /// host surface.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, CreateMessageParams, SamplingMessage};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let mut capabilities = ClientCapabilities::default();
    /// capabilities.sampling = Some(Default::default());
    ///
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(capabilities).await?;
    ///
    /// // Create a message with the LLM
    /// let msg = SamplingMessage::new(
    ///     pmcp::types::Role::User,
    ///     pmcp::types::SamplingMessageContent::Text {
    ///         text: "Explain how to implement a binary search tree".to_string(),
    ///         meta: None,
    ///     },
    /// );
    /// let prefs = pmcp::types::ModelPreferences::new()
    ///     .with_hints(vec![pmcp::types::ModelHint::new("gpt-4")])
    ///     .with_cost_priority(0.5)
    ///     .with_speed_priority(0.3)
    ///     .with_intelligence_priority(0.2);
    /// let mut request = CreateMessageParams::new(vec![msg])
    ///     .with_model_preferences(prefs)
    ///     .with_system_prompt("You are a helpful programming assistant")
    ///     .with_temperature(0.7)
    ///     .with_max_tokens(1000);
    /// request.include_context = pmcp::types::IncludeContext::ThisServer;
    ///
    /// let result = client.create_message(request).await?;
    /// println!("Model: {}", result.model);
    /// println!("Response: {:?}", result.content);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The server doesn't support sampling
    /// - The request parameters are invalid
    /// - Network or protocol errors occur
    pub async fn create_message(&self, params: CreateMessageParams) -> Result<CreateMessageResult> {
        self.ensure_initialized()?;
        self.assert_capability("sampling", "sampling/createMessage")?;

        let request = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(params))));
        let request_id = RequestId::String(Uuid::new_v4().to_string());
        let response = self.send_request(request_id, request).await?;

        match response.payload {
            crate::types::jsonrpc::ResponsePayload::Result(result) => {
                serde_json::from_value(result).map_err(|e| Error::parse(e.to_string()))
            },
            crate::types::jsonrpc::ResponsePayload::Error(error) => {
                Err(Error::from_jsonrpc_error(error))
            },
        }
    }

    /// Send roots list changed notification.
    ///
    /// Notifies the server that the client's root list has changed.
    /// This is typically sent when the workspace or project roots are modified.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport, ClientCapabilities};
    /// use pmcp::types::roots::{ListRootsResult, Root};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// // Roots advertisement is registry-derived (HOST-05): the client must
    /// // register a roots provider for the `roots` capability to reach the
    /// // wire. Build via `ClientBuilder` and register one with `on_roots`.
    /// let transport = StdioTransport::new();
    /// let mut client = ClientBuilder::new(transport)
    ///     .on_roots(|| async {
    ///         Ok(ListRootsResult {
    ///             roots: vec![Root {
    ///                 uri: "file:///workspace".to_string(),
    ///                 name: Some("workspace".to_string()),
    ///             }],
    ///         })
    ///     })
    ///     .build();
    ///
    /// // With a provider registered, a caller-set `list_changed` is preserved,
    /// // so the client advertises that it emits roots-list-changed notices.
    /// let mut capabilities = ClientCapabilities::default();
    /// capabilities.roots = Some(pmcp::RootsCapabilities { list_changed: true });
    /// client.initialize(capabilities).await?;
    ///
    /// // Notify server when project roots change
    /// client.send_roots_list_changed().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - The client doesn't support roots list changed notifications
    /// - Network or protocol errors occur
    pub async fn send_roots_list_changed(&self) -> Result<()> {
        self.ensure_initialized()?;
        if let Some(roots) = &self.capabilities.as_ref().and_then(|c| c.roots.as_ref()) {
            if roots.list_changed {
                // OK, we support it
            } else {
                return Err(Error::capability(
                    "Client does not support roots list changed notifications",
                ));
            }
        }

        self.send_notification(Notification::Client(ClientNotification::RootsListChanged))
            .await
    }

    /// Authenticate with the server.
    ///
    /// Performs authentication using the provided authentication information.
    /// This should be called after initialization if the server requires authentication.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, AuthInfo, AuthScheme};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    ///
    /// // Initialize first
    /// client.initialize(pmcp::ClientCapabilities::default()).await?;
    ///
    /// // Authenticate with bearer token
    /// let auth = AuthInfo {
    ///     scheme: AuthScheme::Bearer,
    ///     token: Some("your-api-token".to_string()),
    ///     oauth: None,
    ///     params: Default::default(),
    /// };
    ///
    /// client.authenticate(&auth)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The client is not initialized
    /// - Authentication fails
    /// - The server doesn't support authentication
    pub fn authenticate(&self, auth_info: &crate::types::AuthInfo) -> Result<()> {
        self.ensure_initialized()?;

        // In a real implementation, this would send an authentication request
        // For now, we'll just validate that we can authenticate
        match auth_info.scheme {
            crate::types::AuthScheme::None => Ok(()),
            crate::types::AuthScheme::Bearer => {
                if auth_info.token.is_none() {
                    return Err(Error::validation("Bearer token required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::OAuth2 => {
                if auth_info.oauth.is_none() {
                    return Err(Error::validation("OAuth information required"));
                }
                Ok(())
            },
            crate::types::AuthScheme::Custom(_) => {
                // Custom auth schemes would be handled here
                Ok(())
            },
        }
    }

    /// Cancel a request.
    ///
    /// Sends a cancellation notification for an active request.
    /// This allows graceful termination of long-running operations.
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to cancel
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, RequestId};
    /// use serde_json::json;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Start a long-running operation
    /// let request_id = RequestId::String("long-operation-123".to_string());
    ///
    /// // Later, cancel the request if needed
    /// client.cancel_request(&request_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the cancellation
    pub async fn cancel_request(&self, request_id: &RequestId) -> Result<()> {
        // Send cancellation notification
        self.send_notification(Notification::Cancelled(
            CancelledNotification::new(request_id.clone())
                .with_reason("User requested cancellation"),
        ))
        .await?;

        // Cancel any local tracking
        let sender = self.active_requests.write().await.remove(request_id);
        if let Some(sender) = sender {
            let _ = sender.send(());
        }

        Ok(())
    }

    /// Send a progress notification.
    ///
    /// Sends a progress update for a long-running operation.
    /// This allows the server or client to track operation progress.
    ///
    /// # Arguments
    ///
    /// * `progress` - The progress notification to send
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{Client, StdioTransport, ClientCapabilities, ProgressNotification, RequestId};
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// client.initialize(ClientCapabilities::default()).await?;
    ///
    /// // Send progress update for a file processing operation
    /// let progress = ProgressNotification::new(
    ///     pmcp::ProgressToken::String("file-processing".to_string()),
    ///     75.0,
    ///     Some("Processing files...".to_string()),
    /// );
    ///
    /// client.send_progress(progress).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network or protocol errors occur while sending the notification
    pub async fn send_progress(&self, progress: ProgressNotification) -> Result<()> {
        self.send_notification(Notification::Progress(progress))
            .await
    }

    /// Emit a `WARN` when a `tasks/*` response fails to deserialize.
    ///
    /// This is the single shared observability helper for all four task
    /// deserialize sites (`tasks/get`, `tasks/result`, `tasks/list`,
    /// `tasks/cancel`). It logs the originating `method`, the available
    /// transport identity, the deserialize `target` type, and the serde
    /// `error` — then the caller still returns `Err` (control flow is
    /// unchanged; this only adds observability, closing TASKDX-03).
    ///
    /// `transport` is [`Transport::transport_type`] (e.g. `"stdio"`,
    /// `"http"`) — the only server identity available here, because the
    /// `Transport` trait exposes no per-instance URL. TASKDX-03 logs this
    /// identity, not a genuine endpoint URL.
    fn log_task_deserialize_error(
        method: &'static str,
        target_type: &'static str,
        transport: &'static str,
        error: &serde_json::Error,
    ) {
        tracing::warn!(
            method = method,
            transport = transport,
            target = target_type,
            error = %error,
            "task response failed to deserialize",
        );
    }

    /// Check if client is initialized.
    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Error::InvalidState("Client not initialized".into()))
        }
    }

    /// Assert that the server has a specific capability.
    fn assert_capability(&self, capability: &str, method: &str) -> Result<()> {
        let has_capability = match capability {
            "tools" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.tools.is_some()),
            "prompts" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.prompts.is_some()),
            "resources" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.resources.is_some()),
            "logging" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.logging.is_some()),
            "completions" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.completions.is_some()),
            "tasks" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.tasks.is_some()),
            // The LLM-server pattern: `create_message` asks a server whose
            // `SamplingHandler` runs the LLM. A pmcp `Server` built with
            // `.sampling(handler)` advertises this by setting
            // `ServerCapabilities.sampling = Some(..)` (see
            // `src/server/mod.rs` `ServerBuilder::sampling`), so the check
            // mirrors that field. Without this arm every `create_message`
            // call fell through to `_ => false` and unconditionally errored.
            "sampling" => self
                .server_capabilities
                .as_ref()
                .is_some_and(|c| c.sampling.is_some()),
            _ => {
                // A capability string reached here without a matching arm. This
                // is a programming error (a new capability was wired without
                // updating this match), not a server-side condition. Make it
                // loud in tests/debug builds while preserving the conservative
                // "not supported" behavior in release.
                tracing::error!(
                    "unknown capability string {capability:?} (required for {method}) — add an arm to assert_capability"
                );
                debug_assert!(
                    false,
                    "unknown capability string {capability:?} (required for {method}) — add an arm to assert_capability"
                );
                false
            },
        };

        if has_capability {
            Ok(())
        } else {
            Err(Error::capability(format!(
                "Server does not support {} (required for {})",
                capability, method
            )))
        }
    }

    /// Send a request and wait for response.
    async fn send_request(
        &self,
        request_id: RequestId,
        request: Request,
    ) -> Result<crate::types::JSONRPCResponse> {
        use crate::shared::protocol_helpers::create_request;

        // Track request for cancellation
        let (cancel_tx, _cancel_rx) = oneshot::channel();
        self.active_requests
            .write()
            .await
            .insert(request_id.clone(), cancel_tx);

        // Create middleware context
        let context = MiddlewareContext::with_request_id(request_id.to_string());

        // Everything after the `active_requests` registration runs inside this
        // inner future so that EVERY error exit funnels through the single
        // cleanup point below (WR-04). On any `Err` — middleware, outbound
        // `send`, inbound `receive`, response middleware, or the host-dispatch
        // reply `send` — the pending entry (and its oneshot cancel sender) is
        // removed before the error propagates, so a `Client` that outlives a
        // failed request never leaks the id or collides with stale state on a
        // later reused id. The happy path still removes the entry inline when
        // the matching response arrives.
        let result = async {
            // Convert to JSONRPC request
            let mut jsonrpc_request = create_request(request_id.clone(), request.clone());

            // Process request through middleware chain (read-only access)
            self.middleware_chain
                .read()
                .await
                .process_request_with_context(&mut jsonrpc_request, &context)
                .await?;

            // Send request through transport
            let message = crate::types::TransportMessage::Request {
                id: request_id.clone(),
                request,
            };

            self.transport.write().await.send(message).await?;

            // Wait for response, dispatching any unsolicited notifications along the way
            loop {
                let response_message = self.transport.write().await.receive().await?;

                match response_message {
                    crate::types::TransportMessage::Response(mut response) => {
                        // Remove from active requests (happy path)
                        self.active_requests.write().await.remove(&request_id);

                        // Process response through middleware chain (read-only access)
                        self.middleware_chain
                            .read()
                            .await
                            .process_response_with_context(&mut response, &context)
                            .await?;
                        return Ok(response);
                    },
                    crate::types::TransportMessage::Notification(notification) => {
                        // Unsolicited notification (e.g., progress, resource changes, SSE events)
                        // Convert to JSONRPC notification for middleware processing
                        use crate::shared::protocol_helpers::create_notification;
                        let mut jsonrpc_notification = create_notification(notification.clone());

                        // Process through protocol middleware chain
                        let notif_context = MiddlewareContext::default();

                        if let Err(e) = self
                            .middleware_chain
                            .write()
                            .await
                            .process_notification_with_context(
                                &mut jsonrpc_notification,
                                &notif_context,
                            )
                            .await
                        {
                            // Log error but don't terminate dispatcher - continue processing
                            tracing::warn!(
                                "Notification middleware processing failed for {}: {}",
                                jsonrpc_notification.method,
                                e
                            );
                        }

                        // Forward to notification handler if registered
                        if let Some(tx) = &self.notification_tx {
                            // Clone the sender because send() requires &mut self
                            #[allow(unused_mut)]
                            let mut tx_clone = tx.clone();
                            if let Err(e) = tx_clone.send(notification).await {
                                tracing::debug!("Notification channel closed: {}", e);
                            }
                        }

                        // Continue loop to wait for the actual response
                    },
                    crate::types::TransportMessage::Request { id, request } => {
                        // Any inbound request at a client is server -> client by
                        // definition (the MCP host direction). Answer it from the
                        // registered host handlers, then keep waiting for the
                        // original response (request_id stays pending). A failed
                        // reply `send` propagates via `?` and is cleaned up at
                        // the single exit point below.
                        let response = self.dispatch_host_request(id, request).await;
                        self.transport
                            .write()
                            .await
                            .send(crate::types::TransportMessage::Response(response))
                            .await?;
                    },
                }
            }
        }
        .await;

        // Single WR-04 exit-cleanup invariant: remove the pending entry on any
        // error path (the happy path already removed it above).
        if result.is_err() {
            self.active_requests.write().await.remove(&request_id);
        }
        result
    }

    /// Answer an inbound server -> client request from the host registry.
    ///
    /// Returns a [`JSONRPCResponse`](crate::types::JSONRPCResponse) that the
    /// caller sends back over the transport. A known request kind with no
    /// registered handler yields `-32601` (method-not-found); a
    /// handler/provider failure yields a sanitized `-32603` (the raw error is
    /// logged locally, never forwarded to the remote server). The connection is
    /// never dropped.
    async fn dispatch_host_request(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        use crate::client::host::{classify_host_request, HostRequestKind};
        match classify_host_request(&request) {
            HostRequestKind::Sampling => self.dispatch_host_sampling(id, request).await,
            HostRequestKind::Elicitation => self.dispatch_host_elicitation(id, request).await,
            HostRequestKind::Roots => self.dispatch_host_roots(id).await,
            // Spec MUST: answer inbound `ping` with an empty-object success
            // result so keepalive pings from servers/proxies do not fail (and
            // do not tear down the connection).
            HostRequestKind::Ping => {
                crate::types::JSONRPCResponse::success(id, serde_json::json!({}))
            },
            HostRequestKind::Unhandled => Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            ),
        }
    }

    /// Route a classified sampling request through the two-stage host approval
    /// model and the registered sampling handler.
    ///
    /// # Policy-denial taxonomy
    ///
    /// Sampling has two host-side access-control stages, both applied ONLY to
    /// the sampling path (never elicitation/roots):
    ///
    /// 1. **Preflight** ([`ClientBuilder::on_sampling_approval`]): an optional
    ///    gate (default-allow) that, when registered, runs BEFORE the handler. A
    ///    [`ApprovalDecision::Deny`] here prevents the LLM call entirely — no
    ///    tokens are billed — genuinely mitigating coerced / denial-of-wallet
    ///    sampling. When no preflight callback is registered, the handler runs
    ///    (default allow).
    /// 2. **Result review** ([`ClientBuilder::on_sampling_result_review`]): an
    ///    optional post-generation stage that sees the produced completion and
    ///    can deny after the fact. Its default (no callback) is pass-through.
    ///
    /// A denial from either stage returns a sanitized `-32603` response with the
    /// GENERIC message `"request denied by host policy"`. The callback's
    /// `Deny(reason)` is logged locally via `tracing::warn!` and is NEVER
    /// forwarded to the remote server (avoids leaking local host policy). The
    /// connection is kept alive — a denial is a normal JSON-RPC error response,
    /// not a transport failure.
    async fn dispatch_host_sampling(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        let Some(handler) = &self.host_registry.sampling else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        let Some(params) = Self::extract_sampling_params(request) else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };

        // (1) PREFLIGHT approval gate — runs BEFORE the handler so a denial
        // prevents the LLM call entirely (no tokens billed).
        if let Some(approval) = &self.host_registry.approval {
            if let ApprovalDecision::Deny(reason) = approval(params.clone()).await {
                tracing::warn!(%reason, "sampling denied by host preflight");
                return Self::host_error(
                    id,
                    crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
                    "request denied by host policy",
                );
            }
        }

        // Capture an owned clone of the params for result review ONLY when a
        // review callback is registered; otherwise the handler consumes
        // `params` below with zero extra clones.
        let review_params = self
            .host_registry
            .result_review
            .is_some()
            .then(|| params.clone());

        // (2) HANDLER — produce the completion.
        let result = match handler.handle_create_message(params).await {
            Ok(result) => result,
            Err(e) => return Self::host_handler_error(id, "sampling/createMessage", &e),
        };

        // (3) RESULT REVIEW — optional post-generation review (default
        // pass-through when no callback is registered). `review_params` is
        // `Some` exactly when `result_review` is `Some`, so the pair matches.
        if let (Some(review), Some(params)) = (&self.host_registry.result_review, review_params) {
            if let ApprovalDecision::Deny(reason) = review(params, result.clone()).await {
                tracing::warn!(%reason, "sampling denied by host result review");
                return Self::host_error(
                    id,
                    crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
                    "request denied by host policy",
                );
            }
        }

        Self::host_ok(id, &result)
    }

    /// Route a classified elicitation request to the registered host handler.
    async fn dispatch_host_elicitation(
        &self,
        id: RequestId,
        request: Request,
    ) -> crate::types::JSONRPCResponse {
        let Some(handler) = &self.host_registry.elicitation else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        // Extract the single elicitation parse variant inline (server-side
        // `elicitation/create`); anything else is not routable here.
        let Request::Server(server) = request else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        let crate::types::ServerRequest::ElicitationCreate(params) = *server else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        match handler.handle_elicitation(*params).await {
            Ok(result) => Self::host_ok(id, &result),
            Err(e) => Self::host_handler_error(id, "elicitation/create", &e),
        }
    }

    /// Answer a classified `roots/list` request from the registered provider.
    async fn dispatch_host_roots(&self, id: RequestId) -> crate::types::JSONRPCResponse {
        let Some(provider) = &self.host_registry.roots else {
            return Self::host_error(
                id,
                crate::error::ErrorCode::METHOD_NOT_FOUND.as_i32(),
                "Method not found",
            );
        };
        match provider().await {
            Ok(result) => Self::host_ok(id, &result),
            Err(e) => Self::host_handler_error(id, "roots/list", &e),
        }
    }

    /// Extract [`CreateMessageParams`] from either inbound sampling parse
    /// variant (client-alias or server), handling the parse ambiguity.
    fn extract_sampling_params(request: Request) -> Option<CreateMessageParams> {
        match request {
            Request::Client(client) => match *client {
                ClientRequest::CreateMessage(params) => Some(*params),
                _ => None,
            },
            Request::Server(server) => match *server {
                crate::types::ServerRequest::CreateMessage(params) => Some(*params),
                _ => None,
            },
        }
    }

    /// Build a successful host response, serializing the handler result.
    fn host_ok<S: serde::Serialize>(id: RequestId, value: &S) -> crate::types::JSONRPCResponse {
        match serde_json::to_value(value) {
            Ok(v) => crate::types::JSONRPCResponse::success(id, v),
            Err(e) => {
                tracing::error!("failed to serialize host response: {e}");
                Self::host_error(
                    id,
                    crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
                    "Internal error handling host request",
                )
            },
        }
    }

    /// Build a JSON-RPC error response that keeps the connection alive.
    ///
    /// All host error responses are sanitized: only the generic `message`
    /// passed here crosses the wire. Raw handler errors and policy-denial
    /// reasons are logged locally by the caller (never forwarded to the remote
    /// server), so local host policy is not leaked. Callers pass the
    /// appropriate [`ErrorCode`](crate::error::ErrorCode) constant:
    /// - `METHOD_NOT_FOUND` for a known request kind with no registered handler,
    /// - `INTERNAL_ERROR` for a sanitized policy denial (a preflight or
    ///   result-review callback returning [`ApprovalDecision::Deny`]) or a
    ///   handler/provider/serialization failure.
    fn host_error(id: RequestId, code: i32, message: &str) -> crate::types::JSONRPCResponse {
        crate::types::JSONRPCResponse::error(
            id,
            crate::types::jsonrpc::JSONRPCError::new(code, message),
        )
    }

    /// Log a handler/provider failure locally and return a sanitized
    /// `INTERNAL_ERROR`.
    fn host_handler_error(
        id: RequestId,
        method: &str,
        err: &Error,
    ) -> crate::types::JSONRPCResponse {
        tracing::error!("host handler for {method} failed: {err}");
        Self::host_error(
            id,
            crate::error::ErrorCode::INTERNAL_ERROR.as_i32(),
            "Internal error handling host request",
        )
    }

    /// Send a notification.
    async fn send_notification(&self, notification: Notification) -> Result<()> {
        let message = crate::types::TransportMessage::Notification(notification);
        self.transport.write().await.send(message).await
    }
}

/// Builder for creating clients with custom configuration.
///
/// # Examples
///
/// ```rust
/// use pmcp::{ClientBuilder, StdioTransport};
///
/// # async fn example() -> Result<(), pmcp::Error> {
/// // Basic client builder
/// let transport = StdioTransport::new();
/// let client = ClientBuilder::new(transport)
///     .enforce_strict_capabilities(true)
///     .build();
///
/// // Client with debounced notifications
/// let transport2 = StdioTransport::new();
/// let debounced_client = ClientBuilder::new(transport2)
///     .debounced_notifications(vec![
///         "notifications/progress".to_string(),
///         "notifications/log".to_string(),
///     ])
///     .enforce_strict_capabilities(false)
///     .build();
///
/// // Chain multiple configurations
/// let transport3 = StdioTransport::new();
/// let configured_client = ClientBuilder::new(transport3)
///     .enforce_strict_capabilities(true)
///     .debounced_notifications(vec!["notifications/resources/changed".to_string()])
///     .build();
/// # Ok(())
/// # }
/// ```
pub struct ClientBuilder<T: Transport> {
    transport: T,
    options: ProtocolOptions,
    middleware_chain: EnhancedMiddlewareChain,
    host_registry: crate::client::host::ClientHostRegistry,
}

impl<T: Transport> std::fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("transport", &"<Transport>")
            .field("options", &self.options)
            .finish()
    }
}

impl<T: Transport> ClientBuilder<T> {
    /// Create a new client builder.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            options: ProtocolOptions::default(),
            middleware_chain: EnhancedMiddlewareChain::new(),
            host_registry: crate::client::host::ClientHostRegistry::default(),
        }
    }

    /// Set whether to enforce strict capabilities.
    pub fn enforce_strict_capabilities(mut self, enforce: bool) -> Self {
        self.options.enforce_strict_capabilities = enforce;
        self
    }

    /// Set debounced notification methods.
    pub fn debounced_notifications(mut self, methods: Vec<String>) -> Self {
        self.options.debounced_notification_methods = methods;
        self
    }

    /// Add middleware to the client.
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_middleware(
        mut self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.middleware_chain.add(middleware);
        self
    }

    /// Add protocol-level middleware to the client.
    ///
    /// This is an alias for `with_middleware()` that provides explicit naming to distinguish
    /// protocol middleware (operates on JSON-RPC messages) from HTTP middleware
    /// (operates on HTTP requests/responses via `StreamableHttpTransportConfigBuilder`).
    ///
    /// Middleware are executed in priority order (Critical → High → Normal → Low → Lowest).
    /// Multiple middleware with the same priority are executed in the order they were added.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::MetricsMiddleware;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .with_protocol_middleware(Arc::new(MetricsMiddleware::new("my-service".to_string())))
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_protocol_middleware(
        self,
        middleware: Arc<dyn crate::shared::AdvancedMiddleware>,
    ) -> Self {
        self.with_middleware(middleware)
    }

    /// Set the entire middleware chain.
    ///
    /// This replaces any previously configured middleware.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::shared::EnhancedMiddlewareChain;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let mut chain = EnhancedMiddlewareChain::new();
    /// // Add middleware to chain...
    ///
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .middleware_chain(chain)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn middleware_chain(mut self, chain: EnhancedMiddlewareChain) -> Self {
        self.middleware_chain = chain;
        self
    }

    /// Register a host sampling handler answering inbound
    /// `sampling/createMessage` requests (the MCP host direction).
    ///
    /// This is the INVERSE of [`Client::create_message`] (the LLM-server
    /// pattern). See [`crate::client::host`] for the full disambiguation.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::{ClientBuilder, StdioTransport};
    /// use pmcp::client::host::HostSamplingHandler;
    /// use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
    /// use pmcp::types::Content;
    /// use async_trait::async_trait;
    ///
    /// struct MyHost;
    /// #[async_trait]
    /// impl HostSamplingHandler for MyHost {
    ///     async fn handle_create_message(
    ///         &self,
    ///         _params: CreateMessageParams,
    ///     ) -> pmcp::Result<CreateMessageResult> {
    ///         Ok(CreateMessageResult::new(Content::text("hi"), "my-model"))
    ///     }
    /// }
    ///
    /// let client = ClientBuilder::new(StdioTransport::new())
    ///     .on_sampling(MyHost)
    ///     .build();
    /// ```
    pub fn on_sampling(mut self, handler: impl host::HostSamplingHandler + 'static) -> Self {
        self.host_registry.sampling = Some(Arc::new(handler));
        self
    }

    /// Register a host elicitation handler answering inbound
    /// `elicitation/create` requests.
    pub fn on_elicitation(mut self, handler: impl host::HostElicitationHandler + 'static) -> Self {
        self.host_registry.elicitation = Some(Arc::new(handler));
        self
    }

    /// Register a roots provider answering inbound `roots/list` requests.
    ///
    /// The provider is generic over any closure returning a future that yields
    /// `Result<ListRootsResult>`, so callers never construct the
    /// [`RootsProvider`] alias by hand.
    pub fn on_roots<F, Fut>(mut self, provider: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<crate::types::roots::ListRootsResult>>
            + Send
            + 'static,
    {
        self.host_registry.roots = Some(Arc::new(move || Box::pin(provider())));
        self
    }

    /// Register an optional pre-handler sampling approval gate.
    ///
    /// The callback is generic over any closure taking owned
    /// [`CreateMessageParams`] and returning a future that yields an
    /// [`ApprovalDecision`]. It is invoked by `dispatch_host_sampling` BEFORE
    /// the sampling handler runs as of this phase, so an
    /// [`ApprovalDecision::Deny`] prevents the LLM
    /// call entirely. The gate is optional and default-allow: when none is
    /// registered, inbound sampling reaches the handler unchallenged.
    pub fn on_sampling_approval<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(CreateMessageParams) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = host::ApprovalDecision> + Send + 'static,
    {
        self.host_registry.approval = Some(Arc::new(move |params| Box::pin(callback(params))));
        self
    }

    /// Register an optional post-handler sampling result review.
    ///
    /// The callback receives the owned request params and the produced
    /// [`CreateMessageResult`] and returns a future yielding an
    /// [`ApprovalDecision`]. It is invoked by `dispatch_host_sampling` AFTER the
    /// sampling handler runs as of this phase, so an
    /// [`ApprovalDecision::Deny`] suppresses the
    /// completion. It is optional and default pass-through: when none is
    /// registered the completion is returned as-is.
    pub fn on_sampling_result_review<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(CreateMessageParams, CreateMessageResult) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = host::ApprovalDecision> + Send + 'static,
    {
        self.host_registry.result_review = Some(Arc::new(move |params, result| {
            Box::pin(callback(params, result))
        }));
        self
    }

    /// Build the client.
    pub fn build(self) -> Client<T> {
        let mut client = Client::with_options(
            self.transport,
            Implementation::new("pmcp-client", env!("CARGO_PKG_VERSION")),
            self.options,
        );
        // Replace the default middleware chain with the configured one
        client.middleware_chain = Arc::new(RwLock::new(self.middleware_chain));
        // Thread the configured host registry onto the client.
        client.host_registry = self.host_registry;
        client
    }
}

impl<T: Transport> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            protocol: self.protocol.clone(),
            middleware_chain: self.middleware_chain.clone(),
            capabilities: self.capabilities.clone(),
            server_capabilities: self.server_capabilities.clone(),
            server_version: self.server_version.clone(),
            instructions: self.instructions.clone(),
            initialized: self.initialized,
            info: self.info.clone(),
            notification_tx: self.notification_tx.clone(),
            active_requests: self.active_requests.clone(),
            options: self.options.clone(),
            host_registry: self.host_registry.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Transport;
    use crate::types::{
        jsonrpc::{JSONRPCError, ResponsePayload},
        JSONRPCResponse, ProgressNotification, ProgressToken, TransportMessage,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    /// Mock transport for testing
    #[derive(Debug)]
    struct MockTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent_messages: Arc<Mutex<Vec<TransportMessage>>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_responses(responses: Vec<TransportMessage>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, response: TransportMessage) {
            self.responses.lock().unwrap().push(response);
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&mut self, message: TransportMessage) -> Result<()> {
            self.sent_messages.lock().unwrap().push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| Error::protocol_msg("No more responses"))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_client_creation() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert!(!client.initialized);
        assert_eq!(client.info.name, "pmcp-client");
    }

    #[test]
    fn test_client_with_info() {
        let transport = MockTransport::new();
        let info = Implementation::new("test-client", "1.0.0");
        let client = Client::with_info(transport, info);
        assert_eq!(client.info.name, "test-client");
        assert_eq!(client.info.version, "1.0.0");
    }

    // === ClientOptions wiring tests ===

    #[test]
    fn test_client_new_uses_default_options() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        assert_eq!(client.options.max_iterations, 100);
    }

    #[test]
    fn test_client_with_client_options_threads_value() {
        let transport = MockTransport::new();
        let opts = ClientOptions {
            max_iterations: 7,
            ..Default::default()
        };
        let client = Client::with_client_options(transport, opts);
        assert_eq!(client.options.max_iterations, 7);
    }

    #[test]
    fn test_client_with_options_preserves_default_client_options() {
        let transport = MockTransport::new();
        let client = Client::with_options(
            transport,
            Implementation::default(),
            ProtocolOptions::default(),
        );
        assert_eq!(client.options.max_iterations, 100);
    }

    // === Client host dispatch unit tests (HOST-01/HOST-05) ===

    struct MockHostSampling;

    #[async_trait]
    impl host::HostSamplingHandler for MockHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            Ok(CreateMessageResult::new(
                crate::types::Content::text("mock host completion"),
                "mock-host-model",
            ))
        }
    }

    struct FailingHostSampling;

    #[async_trait]
    impl host::HostSamplingHandler for FailingHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            Err(Error::protocol_msg(
                "secret path /etc/passwd leaked in error",
            ))
        }
    }

    fn sampling_client_alias_request() -> Request {
        // Inbound sampling parses as the CLIENT variant (parse ambiguity).
        Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
            CreateMessageParams::new(Vec::new()),
        ))))
    }

    #[tokio::test]
    async fn test_dispatch_sampling_alias_reaches_handler() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let id = RequestId::from(1i64);
        let response = client
            .dispatch_host_request(id, sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "inbound sampling (client-alias parse) must reach the host handler, got: {response:?}"
        );
    }

    #[tokio::test]
    async fn test_dispatch_known_unhandled_returns_method_not_found() {
        // No handlers registered; a KNOWN roots/list request must yield -32601.
        let client = Client::new(MockTransport::new());
        let id = RequestId::from(2i64);
        let request = Request::Server(Box::new(crate::types::ServerRequest::ListRoots));
        let response = client.dispatch_host_request(id, request).await;
        match response.payload {
            ResponsePayload::Error(e) => assert_eq!(e.code, -32601),
            ResponsePayload::Result(r) => panic!("expected -32601 error, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_handler_error_is_sanitized_32603() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(FailingHostSampling)
            .build();
        let id = RequestId::from(3i64);
        let response = client
            .dispatch_host_request(id, sampling_client_alias_request())
            .await;
        match response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32603);
                // Sanitized: the raw handler error text must NOT cross the wire.
                assert!(
                    !e.message.contains("/etc/passwd"),
                    "handler error text must be sanitized, got: {}",
                    e.message
                );
            },
            ResponsePayload::Result(r) => panic!("expected -32603 error, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_dispatch_roots_provider_answers() {
        let client = ClientBuilder::new(MockTransport::new())
            .on_roots(|| async { Ok(crate::types::roots::ListRootsResult { roots: Vec::new() }) })
            .build();
        let id = RequestId::from(4i64);
        let request = Request::Server(Box::new(crate::types::ServerRequest::ListRoots));
        let response = client.dispatch_host_request(id, request).await;
        assert!(
            response.is_success(),
            "roots provider must answer roots/list"
        );
    }

    // === Sampling approval (preflight + result-review) unit tests (HOST-04) ===

    /// Host sampling handler that flips an `AtomicBool` when invoked, so tests
    /// can prove whether the LLM call happened.
    struct TrackingHostSampling {
        invoked: Arc<std::sync::atomic::AtomicBool>,
    }

    #[async_trait]
    impl host::HostSamplingHandler for TrackingHostSampling {
        async fn handle_create_message(
            &self,
            _params: CreateMessageParams,
        ) -> Result<CreateMessageResult> {
            self.invoked
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(CreateMessageResult::new(
                crate::types::Content::text("tracked completion"),
                "tracked-model",
            ))
        }
    }

    fn assert_policy_denied(response: &JSONRPCResponse) {
        match &response.payload {
            ResponsePayload::Error(e) => {
                assert_eq!(e.code, -32603, "policy denial must be -32603");
                assert_eq!(
                    e.message, "request denied by host policy",
                    "policy denial message must be the generic sanitized string"
                );
            },
            ResponsePayload::Result(r) => panic!("expected -32603 denial, got result: {r:?}"),
        }
    }

    #[tokio::test]
    async fn test_sampling_no_preflight_runs_handler() {
        // (a) No preflight callback => handler runs, completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(10i64), sampling_client_alias_request())
            .await;
        assert!(response.is_success(), "default (no preflight) must allow");
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must run when no preflight is registered"
        );
    }

    #[tokio::test]
    async fn test_sampling_preflight_deny_skips_handler() {
        // (b) Preflight Deny => handler is NOT called (denial-of-wallet fix) and
        // the raw deny reason must NOT cross the wire.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async {
                host::ApprovalDecision::Deny("local-secret-reason".to_string())
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(11i64), sampling_client_alias_request())
            .await;
        assert_policy_denied(&response);
        assert!(
            !invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must NOT run when preflight denies (no LLM call, no tokens)"
        );
        // The raw deny reason must never be forwarded.
        if let ResponsePayload::Error(e) = &response.payload {
            assert!(
                !e.message.contains("local-secret-reason"),
                "deny reason must be logged locally, not forwarded: {}",
                e.message
            );
        }
    }

    #[tokio::test]
    async fn test_sampling_preflight_allow_runs_handler() {
        // (c) Preflight Allow => completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async { host::ApprovalDecision::Allow })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(12i64), sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "preflight Allow must return completion"
        );
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler must run after preflight Allow"
        );
    }

    #[tokio::test]
    async fn test_sampling_result_review_deny_after_handler() {
        // (d) result_review Deny after an allowed preflight => -32603, but the
        // handler WAS called (generation happened, then was suppressed).
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_result_review(|_params, _result| async {
                host::ApprovalDecision::Deny("post-gen-reason".to_string())
            })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(13i64), sampling_client_alias_request())
            .await;
        assert_policy_denied(&response);
        assert!(
            invoked.load(std::sync::atomic::Ordering::SeqCst),
            "handler runs before result review can deny"
        );
    }

    #[tokio::test]
    async fn test_sampling_result_review_absent_is_passthrough() {
        // (e) result_review absent => pass-through (Allow), completion returned.
        let invoked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(TrackingHostSampling {
                invoked: invoked.clone(),
            })
            .on_sampling_approval(|_params| async { host::ApprovalDecision::Allow })
            .build();
        let response = client
            .dispatch_host_request(RequestId::from(14i64), sampling_client_alias_request())
            .await;
        assert!(
            response.is_success(),
            "absent result_review must pass through"
        );
    }

    // === Capability derivation unit tests (HOST-05) ===

    struct MockHostElicit;

    #[async_trait]
    impl host::HostElicitationHandler for MockHostElicit {
        async fn handle_elicitation(
            &self,
            _params: crate::types::elicitation::ElicitRequestParams,
        ) -> Result<crate::types::elicitation::ElicitResult> {
            Ok(crate::types::elicitation::ElicitResult {
                action: crate::types::elicitation::ElicitAction::Accept,
                content: None,
            })
        }
    }

    #[test]
    fn test_capability_sampling_registered_is_present() {
        // (a) handler registered + default caps => sampling present.
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(caps.sampling.is_some(), "registered sampling => present");
    }

    #[test]
    fn test_capability_sampling_unregistered_default_is_absent() {
        // (b) no handler + default caps => sampling absent.
        let client = Client::new(MockTransport::new());
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(caps.sampling.is_none(), "unregistered sampling => absent");
    }

    #[test]
    fn test_capability_sampling_anti_lie_discards_caller_value() {
        // (c) ANTI-LIE: no handler + caller-set sampling => forced None.
        let client = Client::new(MockTransport::new());
        let mut caps = ClientCapabilities {
            sampling: Some(crate::types::capabilities::SamplingCapabilities::default()),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        assert!(
            caps.sampling.is_none(),
            "caller-set sampling with no handler must be discarded (anti-capability-lie)"
        );
    }

    #[test]
    fn test_capability_sampling_preserves_caller_detail() {
        // (d) PRESERVATION: handler present + caller-configured sub-field =>
        // that exact detail is preserved (not reset to default()).
        let client = ClientBuilder::new(MockTransport::new())
            .on_sampling(MockHostSampling)
            .build();
        let mut caps = ClientCapabilities {
            sampling: Some(crate::types::capabilities::SamplingCapabilities {
                models: Some(vec!["gpt-4o".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        let sampling = caps.sampling.expect("handler present => sampling kept");
        assert_eq!(
            sampling.models,
            Some(vec!["gpt-4o".to_string()]),
            "caller-configured models must be preserved, not reset to default"
        );
    }

    #[test]
    fn test_capability_elicitation_and_roots_parallel() {
        // (e) elicitation + roots follow the same rule.
        // Registered => present.
        let client = ClientBuilder::new(MockTransport::new())
            .on_elicitation(MockHostElicit)
            .on_roots(|| async { Ok(crate::types::roots::ListRootsResult { roots: Vec::new() }) })
            .build();
        let mut caps = ClientCapabilities::default();
        client.derive_host_capabilities(&mut caps);
        assert!(
            caps.elicitation.is_some(),
            "registered elicitation => present"
        );
        assert!(caps.roots.is_some(), "registered roots => present");

        // Unregistered + caller-set => discarded (anti-lie) for both.
        let bare = Client::new(MockTransport::new());
        let mut caps2 = ClientCapabilities {
            elicitation: Some(crate::types::capabilities::ElicitationCapabilities::default()),
            roots: Some(crate::types::capabilities::RootsCapabilities::default()),
            ..Default::default()
        };
        bare.derive_host_capabilities(&mut caps2);
        assert!(caps2.elicitation.is_none(), "elicitation anti-lie");
        assert!(caps2.roots.is_none(), "roots anti-lie");
    }

    #[test]
    fn test_capability_derivation_leaves_tasks_and_experimental_untouched() {
        // (f) tasks / experimental are never modified by host derivation.
        let client = Client::new(MockTransport::new());
        let mut experimental = HashMap::new();
        experimental.insert("custom".to_string(), serde_json::json!(true));
        let mut caps = ClientCapabilities {
            tasks: Some(crate::types::capabilities::ClientTasksCapability::default()),
            experimental: Some(experimental),
            ..Default::default()
        };
        client.derive_host_capabilities(&mut caps);
        assert!(caps.tasks.is_some(), "tasks must be preserved");
        assert_eq!(
            caps.experimental.and_then(|e| e.get("custom").cloned()),
            Some(serde_json::json!(true)),
            "experimental must be preserved"
        );
    }

    // === Typed-helper unit tests ===

    #[tokio::test]
    async fn test_call_tool_typed_serialize_error_maps_to_validation() {
        use serde::Serialize;
        // A type whose Serialize impl always errors.
        struct Bad;
        impl Serialize for Bad {
            fn serialize<S: serde::Serializer>(
                &self,
                _: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("nope"))
            }
        }
        let transport = MockTransport::new();
        let client = Client::new(transport);
        let err = client.call_tool_typed("any", &Bad).await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("call_tool_typed arguments"), "got: {msg}");
        assert!(msg.contains("nope"), "serde error must surface: {msg}");
        assert!(matches!(err, Error::Validation(_)));
    }

    #[tokio::test]
    async fn test_get_prompt_typed_non_object_rejected() {
        let transport = MockTransport::new();
        let client = Client::new(transport);
        // Vec<i32> serializes to Value::Array, which is non-object.
        let err = client
            .get_prompt_typed("p", &vec![1, 2, 3])
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must serialize to a JSON object"),
            "got: {msg}"
        );
        assert!(matches!(err, Error::Validation(_)));
    }

    #[tokio::test]
    async fn test_get_prompt_typed_string_values_not_quoted() {
        use serde::Serialize;
        #[derive(Serialize)]
        struct Args {
            topic: String,
            length: u32,
            verbose: bool,
            ignored: Option<String>,
        }
        // Unit-test the coercion directly by building the intermediate HashMap
        // in-situ. Wire round-trip is covered in tests/list_all_pagination.rs;
        // here we only care that the leaf-coercion rules are honoured.
        let args = Args {
            topic: "rust".into(),
            length: 200,
            verbose: true,
            ignored: None,
        };
        let value = serde_json::to_value(&args).unwrap();
        let obj = value.as_object().unwrap().clone();
        assert_eq!(
            obj.get("topic").unwrap(),
            &serde_json::Value::String("rust".into())
        );
        assert_eq!(obj.get("length").unwrap().to_string(), "200");
        assert_eq!(obj.get("verbose").unwrap().to_string(), "true");
        assert!(matches!(
            obj.get("ignored").unwrap(),
            serde_json::Value::Null
        ));
    }

    #[test]
    fn test_client_builder() {
        let transport = MockTransport::new();
        let client = ClientBuilder::new(transport)
            .enforce_strict_capabilities(true)
            .debounced_notifications(vec!["test".to_string()])
            .build();
        assert!(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(client.protocol.read())
                .options()
                .enforce_strict_capabilities
        );
    }

    #[tokio::test]
    async fn test_client_initialization() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        let caps = ClientCapabilities::minimal();

        let result = client.initialize(caps).await;
        assert!(result.is_ok());
        assert!(client.initialized);
        assert_eq!(client.server_version.as_ref().unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_ping() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let ping_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({})),
        });

        let transport = MockTransport::with_responses(vec![ping_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize first
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Ping
        let result = client.ping().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let tools_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "tools": [{
                    "name": "test-tool",
                    "description": "Test tool",
                    "inputSchema": {}
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![tools_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize with tools capability
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // List tools
        let result = client.list_tools(None).await;
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.tools.len(), 1);
        assert_eq!(tools.tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_error_response() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let error_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Error(JSONRPCError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        });

        let transport = MockTransport::with_responses(vec![error_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should get error
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Method not found"));
    }

    #[tokio::test]
    async fn test_uninitialized_error() {
        let transport = MockTransport::new();
        let client = Client::new(transport);

        // Try to call method without initialization
        let result = client.ping().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not initialized"));
    }

    #[tokio::test]
    async fn test_capability_enforcement() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    // No tools capability
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize without tools capability
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Try to list tools - should fail
        let result = client.list_tools(None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[test]
    fn test_assert_capability_sampling_present_when_server_advertises() {
        // Regression for CR-01: a server advertising `sampling` must satisfy
        // the capability assertion `create_message` performs.
        let mut client = Client::new(MockTransport::new());
        client.server_capabilities = Some(ServerCapabilities {
            sampling: Some(crate::types::SamplingCapabilities::default()),
            ..Default::default()
        });
        assert!(
            client
                .assert_capability("sampling", "sampling/createMessage")
                .is_ok(),
            "sampling capability must be recognized when the server advertises it"
        );
    }

    #[test]
    fn test_assert_capability_sampling_absent_errors() {
        // Negative half of CR-01: no `sampling` advertised => capability error.
        let mut client = Client::new(MockTransport::new());
        client.server_capabilities = Some(ServerCapabilities::default());
        let err = client
            .assert_capability("sampling", "sampling/createMessage")
            .expect_err("missing sampling capability must error");
        assert!(
            err.to_string().contains("does not support sampling"),
            "unexpected error message: {err}"
        );
    }

    /// Transport whose first `send` (the outgoing request) succeeds and whose
    /// second `send` (the host response) fails, returning a single inbound
    /// request from `receive` in between. Drives the WR-04 leak path.
    #[derive(Debug)]
    struct FailSecondSend {
        sends: Arc<Mutex<usize>>,
        inbound: Arc<Mutex<Option<TransportMessage>>>,
    }

    #[async_trait]
    impl Transport for FailSecondSend {
        async fn send(&mut self, _message: TransportMessage) -> Result<()> {
            let mut n = self.sends.lock().unwrap();
            *n += 1;
            if *n >= 2 {
                Err(Error::internal("host response send failed"))
            } else {
                Ok(())
            }
        }

        async fn receive(&mut self) -> Result<TransportMessage> {
            let msg = self.inbound.lock().unwrap().take();
            if let Some(msg) = msg {
                Ok(msg)
            } else {
                Err(Error::internal("no more messages"))
            }
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_host_response_send_failure_cleans_active_requests() {
        // WR-04: when sending the host response fails, the in-flight request's
        // entry (and its oneshot cancel sender) must be removed from
        // active_requests before the error propagates, matching the Response
        // arm's cleanup.
        let inbound = TransportMessage::Request {
            id: RequestId::from("inbound-1".to_string()),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };
        let client = Client::new(FailSecondSend {
            sends: Arc::new(Mutex::new(0)),
            inbound: Arc::new(Mutex::new(Some(inbound))),
        });

        let req_id = RequestId::from("outgoing-1".to_string());
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let result = client.send_request(req_id.clone(), request).await;

        assert!(
            result.is_err(),
            "host-response send failure must propagate as an error"
        );
        assert!(
            !client.active_requests.read().await.contains_key(&req_id),
            "pending entry must be removed when the host response send fails"
        );
    }

    #[tokio::test]
    async fn test_receive_failure_cleans_active_requests() {
        // WR-04: a failure in the inbound `receive` (before any response
        // arrives) must also funnel through the single exit-cleanup point, so
        // the pending entry does not leak. `FailSecondSend` with no inbound
        // message sends the outgoing request successfully (send #1) and then
        // errors on `receive` ("no more messages").
        let client = Client::new(FailSecondSend {
            sends: Arc::new(Mutex::new(0)),
            inbound: Arc::new(Mutex::new(None)),
        });

        let req_id = RequestId::from("outgoing-recv".to_string());
        let request = Request::Client(Box::new(ClientRequest::Ping));
        let result = client.send_request(req_id.clone(), request).await;

        assert!(
            result.is_err(),
            "transport receive failure must propagate as an error"
        );
        assert!(
            !client.active_requests.read().await.contains_key(&req_id),
            "pending entry must be removed when the transport receive fails"
        );
    }

    #[tokio::test]
    async fn test_send_progress() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Send progress
        let progress = ProgressNotification::new(
            ProgressToken::String("test".to_string()),
            50.0,
            Some("Halfway done".to_string()),
        );

        let result = client.send_progress(progress).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "completions": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let complete_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "completion": {
                    "values": ["test1", "test2"]
                }
            })),
        });

        let transport = MockTransport::with_responses(vec![complete_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::default()).await;

        // Complete
        let result = client
            .complete(CompleteRequest {
                r#ref: crate::types::CompletionReference::Resource {
                    uri: "test://test".to_string(),
                },
                argument: crate::types::CompletionArgument {
                    name: "test".to_string(),
                    value: "t".to_string(),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_resource() {
        let init_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "resources": {}
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            })),
        });

        let read_response = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "contents": [{
                    "type": "text",
                    "text": "Hello, world!"
                }]
            })),
        });

        let transport = MockTransport::with_responses(vec![read_response, init_response]);
        let mut client = Client::new(transport);

        // Initialize
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // Read resource
        let result = client.read_resource("test://test".to_string()).await;
        if let Err(e) = &result {
            tracing::error!("Read resource error: {:?}", e);
        }
        assert!(result.is_ok());
        let contents = result.unwrap();
        assert_eq!(contents.contents.len(), 1);
    }

    // === list_all_* in-module tests ===

    fn list_all_init_response() -> TransportMessage {
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {},
                    "prompts": {},
                    "resources": {},
                },
                "serverInfo": { "name": "test-server", "version": "1.0.0" }
            })),
        })
    }

    fn page_response<V: Into<serde_json::Value>>(
        id: i64,
        items_field: &str,
        items: V,
        next_cursor: Option<&str>,
    ) -> TransportMessage {
        let mut payload = serde_json::Map::new();
        payload.insert(items_field.to_string(), items.into());
        if let Some(c) = next_cursor {
            payload.insert("nextCursor".to_string(), json!(c));
        }
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(serde_json::Value::Object(payload)),
        })
    }

    #[tokio::test]
    async fn test_list_all_tools_single_page() {
        let page = page_response(
            2,
            "tools",
            json!([{"name": "only", "description": "t", "inputSchema": {}}]),
            None,
        );
        // MockTransport pops from tail; push reversed + init last.
        let transport = MockTransport::with_responses(vec![page, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "only");
    }

    #[tokio::test]
    async fn test_list_all_tools_three_pages_in_order() {
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "tools",
            json!([{"name": "c", "description": "t", "inputSchema": {}}]),
            None,
        );
        // Reverse-push: pages last-to-first, init last.
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_list_all_tools_cap_enforced() {
        // max_iterations=3, server emits 4 pages all with Some(_).
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "tools",
            json!([{"name": "c", "description": "t", "inputSchema": {}}]),
            Some("p4"),
        );
        let p4 = page_response(
            5,
            "tools",
            json!([{"name": "d", "description": "t", "inputSchema": {}}]),
            Some("p5"),
        );
        let transport =
            MockTransport::with_responses(vec![p4, p3, p2, p1, list_all_init_response()]);
        let opts = ClientOptions {
            max_iterations: 3,
            ..Default::default()
        };
        let mut client = Client::with_client_options(transport, opts);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let err = client.list_all_tools().await.unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(err, Error::Validation(_)), "got: {msg}");
        assert!(msg.contains("list_all_tools"), "method name missing: {msg}");
        assert!(msg.contains('3'), "cap value missing: {msg}");
    }

    #[tokio::test]
    async fn test_list_all_tools_empty_string_cursor_continues() {
        // First page has next_cursor: Some("") — MUST continue the loop.
        let p1 = page_response(
            2,
            "tools",
            json!([{"name": "a", "description": "t", "inputSchema": {}}]),
            Some(""),
        );
        let p2 = page_response(
            3,
            "tools",
            json!([{"name": "b", "description": "t", "inputSchema": {}}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_tools().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec!["a", "b"],
            "Some(\"\") must continue the loop (Pitfall 2)"
        );
    }

    #[tokio::test]
    async fn test_list_all_tools_max_iterations_zero_errors_immediately() {
        // max_iterations=0: loop body must not execute; no tools/list sent.
        // Only the init response is pre-loaded — if the loop ever called
        // list_tools, receive() would then fail with "No more responses"
        // (a Protocol error), not a Validation error.
        let transport = MockTransport::with_responses(vec![list_all_init_response()]);
        let sent_ref = Arc::clone(&transport.sent_messages);
        let opts = ClientOptions {
            max_iterations: 0,
            ..Default::default()
        };
        let mut client = Client::with_client_options(transport, opts);
        let _ = client.initialize(ClientCapabilities::minimal()).await;

        // Snapshot sent count BEFORE the call — initialize() sent its init
        // request. We assert no ADDITIONAL tools/list request is sent.
        let sent_before = sent_ref.lock().unwrap().len();

        let err = client.list_all_tools().await.unwrap_err();
        let msg = format!("{err}");
        assert!(matches!(err, Error::Validation(_)), "got: {msg}");
        assert!(msg.contains('0'), "cap value missing: {msg}");
        assert!(msg.contains("list_all_tools"), "method name missing: {msg}");

        let sent_after = sent_ref.lock().unwrap().clone();
        assert_eq!(
            sent_after.len(),
            sent_before,
            "transport must not receive any tools/list request when max_iterations=0; sent: {sent_after:?}"
        );
    }

    #[tokio::test]
    async fn test_list_all_prompts_three_pages_in_order() {
        let p1 = page_response(2, "prompts", json!([{"name": "p1"}]), Some("p2"));
        let p2 = page_response(3, "prompts", json!([{"name": "p2"}]), Some("p3"));
        let p3 = page_response(3, "prompts", json!([{"name": "p3"}]), None);
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_prompts().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["p1", "p2", "p3"]);
    }

    #[tokio::test]
    async fn test_list_all_resources_three_pages_in_order() {
        let p1 = page_response(
            2,
            "resources",
            json!([{"uri": "file://a", "name": "a"}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "resources",
            json!([{"uri": "file://b", "name": "b"}]),
            Some("p3"),
        );
        let p3 = page_response(
            4,
            "resources",
            json!([{"uri": "file://c", "name": "c"}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p3, p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_resources().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_list_all_resource_templates_two_pages_in_order() {
        let p1 = page_response(
            2,
            "resourceTemplates",
            json!([{"uriTemplate": "file://{a}", "name": "ta"}]),
            Some("p2"),
        );
        let p2 = page_response(
            3,
            "resourceTemplates",
            json!([{"uriTemplate": "file://{b}", "name": "tb"}]),
            None,
        );
        let transport = MockTransport::with_responses(vec![p2, p1, list_all_init_response()]);
        let mut client = Client::new(transport);
        let _ = client.initialize(ClientCapabilities::minimal()).await;
        let all = client.list_all_resource_templates().await.expect("ok");
        let names: Vec<_> = all.into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["ta", "tb"]);
    }

    // === TASKDX-03: WARN on task deserialize failure ===

    /// A single captured tracing event's structured fields.
    #[derive(Debug, Clone, Default)]
    struct CapturedEvent {
        level: String,
        fields: std::collections::HashMap<String, String>,
        message: String,
    }

    /// Minimal in-test recording subscriber that captures events' structured
    /// fields into a shared `Vec`, with NO dependency on `tracing-subscriber`.
    ///
    /// Installed via `tracing::subscriber::with_default` (scoped, never a global
    /// `init()`), so it cannot leak across tests run under `--test-threads=1`.
    #[derive(Clone, Default)]
    struct RecordingSubscriber {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    struct FieldCollector<'a>(&'a mut CapturedEvent);

    impl tracing::field::Visit for FieldCollector<'_> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            let rendered = format!("{value:?}");
            if field.name() == "message" {
                // Debug-rendered messages are wrapped in quotes; strip them.
                self.0.message = rendered.trim_matches('"').to_string();
            } else {
                self.0.fields.insert(
                    field.name().to_string(),
                    rendered.trim_matches('"').to_string(),
                );
            }
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                self.0.message = value.to_string();
            } else {
                self.0
                    .fields
                    .insert(field.name().to_string(), value.to_string());
            }
        }
    }

    impl tracing::Subscriber for RecordingSubscriber {
        fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            let mut captured = CapturedEvent {
                level: event.metadata().level().to_string(),
                ..Default::default()
            };
            event.record(&mut FieldCollector(&mut captured));
            self.events.lock().unwrap().push(captured);
        }

        fn enter(&self, _span: &tracing::span::Id) {}
        fn exit(&self, _span: &tracing::span::Id) {}
    }

    /// Build an init response advertising the `tasks` capability.
    fn tasks_init_response() -> TransportMessage {
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tasks": {} },
                "serverInfo": { "name": "test-server", "version": "1.0.0" }
            })),
        })
    }

    #[test]
    fn test_tasks_get_malformed_response_emits_warn_and_errs() {
        // A flat Task missing the required `task` wrapper — the deliberately
        // wrong shape for GetTaskResult (incident bug #3).
        let malformed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "taskId": "abc",
                "status": "completed"
            })),
        });
        let transport = MockTransport::with_responses(vec![malformed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_get("abc"))
        });

        // Control flow unchanged: still returns Err (a parse error).
        assert!(result.is_err(), "malformed tasks/get must still return Err");

        // Structural WARN assertion (not a substring of the message text).
        let events = sink.lock().unwrap();
        let warn = events
            .iter()
            .find(|e| e.fields.get("method").map(String::as_str) == Some("tasks/get"))
            .expect("a WARN naming method=tasks/get must be captured");
        assert_eq!(warn.level, "WARN", "must be a WARN level event");
        assert!(
            warn.fields.contains_key("error"),
            "WARN must carry the serde error field, got: {:?}",
            warn.fields
        );
        assert!(
            warn.fields.contains_key("transport"),
            "WARN must carry the transport identity field, got: {:?}",
            warn.fields
        );
    }

    #[test]
    fn test_tasks_result_malformed_response_emits_warn_and_errs() {
        // CallToolResult requires `content`; a bare bool is the wrong shape.
        let malformed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!(true)),
        });
        let transport = MockTransport::with_responses(vec![malformed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_result("abc"))
        });

        assert!(
            result.is_err(),
            "malformed tasks/result must still return Err"
        );

        let events = sink.lock().unwrap();
        let warn = events
            .iter()
            .find(|e| e.fields.get("method").map(String::as_str) == Some("tasks/result"))
            .expect("a WARN naming method=tasks/result must be captured");
        assert_eq!(warn.level, "WARN");
        assert!(warn.fields.contains_key("error"), "got: {:?}", warn.fields);
        assert!(
            warn.fields.contains_key("transport"),
            "got: {:?}",
            warn.fields
        );
    }

    #[test]
    fn test_tasks_get_well_formed_response_emits_no_warn() {
        let well_formed = TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(2i64),
            payload: ResponsePayload::Result(json!({
                "task": {
                    "taskId": "abc",
                    "status": "completed",
                    "createdAt": "2026-06-21T00:00:00Z",
                    "lastUpdatedAt": "2026-06-21T00:00:00Z"
                }
            })),
        });
        let transport = MockTransport::with_responses(vec![well_formed, tasks_init_response()]);
        let mut client = Client::new(transport);
        let _ = futures::executor::block_on(client.initialize(ClientCapabilities::default()));

        let recorder = RecordingSubscriber::default();
        let sink = recorder.events.clone();
        let result = tracing::subscriber::with_default(recorder, || {
            futures::executor::block_on(client.tasks_get("abc"))
        });

        assert!(
            result.is_ok(),
            "well-formed tasks/get must succeed: {result:?}"
        );
        let events = sink.lock().unwrap();
        assert!(
            !events
                .iter()
                .any(|e| e.fields.get("method").map(String::as_str) == Some("tasks/get")),
            "no task-deserialize WARN must fire on a well-formed response"
        );
    }
}
