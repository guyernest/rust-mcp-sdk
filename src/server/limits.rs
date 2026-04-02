//! Configurable payload and resource limits for MCP servers.
//!
//! These limits protect against resource exhaustion attacks by bounding
//! the size of incoming requests and tool arguments. Defaults are tuned
//! for AWS Lambda (4 MB API Gateway limit) but can be adjusted for
//! other deployment targets.

/// Payload and resource limits for the MCP server.
///
/// Applied at multiple layers:
/// - **HTTP body**: rejects oversized requests before JSON parsing
/// - **Tool arguments**: rejects oversized tool call arguments before dispatch
///
/// # Examples
///
/// ```rust
/// use pmcp::server::limits::PayloadLimits;
///
/// // Use defaults (4 MB request, 1 MB args)
/// let limits = PayloadLimits::default();
///
/// // Custom limits for a high-throughput server
/// let limits = PayloadLimits::default()
///     .with_max_request_bytes(16 * 1024 * 1024)  // 16 MB
///     .with_max_tool_args_bytes(4 * 1024 * 1024); // 4 MB
///
/// // Disable limits (not recommended)
/// let limits = PayloadLimits::unlimited();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PayloadLimits {
    /// Maximum HTTP request body size in bytes.
    ///
    /// Requests exceeding this limit are rejected with HTTP 413 before
    /// any JSON parsing occurs. Default: 4 MB (matches AWS API Gateway).
    pub max_request_bytes: usize,

    /// Maximum size of serialized tool call arguments in bytes.
    ///
    /// Checked after JSON-RPC parsing but before tool handler dispatch.
    /// Default: 1 MB.
    pub max_tool_args_bytes: usize,
}

/// Default: 4 MB request body (matches AWS API Gateway).
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// Default: 1 MB tool arguments.
pub const DEFAULT_MAX_TOOL_ARGS_BYTES: usize = 1024 * 1024;

impl Default for PayloadLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_tool_args_bytes: DEFAULT_MAX_TOOL_ARGS_BYTES,
        }
    }
}

impl PayloadLimits {
    /// Create limits with no upper bounds.
    ///
    /// **Not recommended for production.** Use only when an external
    /// reverse proxy already enforces payload limits.
    pub fn unlimited() -> Self {
        Self {
            max_request_bytes: usize::MAX,
            max_tool_args_bytes: usize::MAX,
        }
    }

    /// Set the maximum HTTP request body size.
    pub fn with_max_request_bytes(mut self, max: usize) -> Self {
        self.max_request_bytes = max;
        self
    }

    /// Set the maximum tool argument payload size.
    pub fn with_max_tool_args_bytes(mut self, max: usize) -> Self {
        self.max_tool_args_bytes = max;
        self
    }
}
