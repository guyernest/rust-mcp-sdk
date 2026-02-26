//! MCP-aware HTTP client for load testing.
//! Full implementation in Plan 01-02.

/// MCP-aware HTTP client. Each virtual user owns one instance.
pub struct McpClient {
    _private: (),
}
