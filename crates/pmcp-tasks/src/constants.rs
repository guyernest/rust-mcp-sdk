//! Constants for MCP Tasks protocol meta keys and method names.
//!
//! Implemented in Task 2 (full set). Core constants provided here
//! for use by Task 1 wire types.

/// Meta key for related-task metadata on `tasks/result` responses.
///
/// Per the MCP spec, this key links a tool result back to its
/// originating task: `{ "io.modelcontextprotocol/related-task": { "taskId": "..." } }`.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";
