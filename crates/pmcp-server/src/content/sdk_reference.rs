//! SDK API reference content.

/// Typed tools guide (TypedTool, TypedSyncTool, TypedToolWithOutput).
pub const TYPED_TOOLS: &str = include_str!("../../content/sdk-typed-tools.md");

/// Resource handler patterns and URI conventions.
pub const RESOURCES: &str = include_str!("../../content/sdk-resources.md");

/// Prompt handler patterns and PromptInfo metadata.
pub const PROMPTS: &str = include_str!("../../content/sdk-prompts.md");

/// Authentication middleware (OAuth, API key, JWT).
pub const AUTH: &str = include_str!("../../content/sdk-auth.md");

/// Middleware composition and patterns.
pub const MIDDLEWARE: &str = include_str!("../../content/sdk-middleware.md");

/// MCP Apps extension (widgets, _meta, host layers).
pub const MCP_APPS: &str = include_str!("../../content/sdk-mcp-apps.md");

/// Error handling patterns and error variants.
pub const ERROR_HANDLING: &str = include_str!("../../content/sdk-error-handling.md");
