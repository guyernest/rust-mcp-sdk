//! Guided workflow prompt templates.
//!
//! Prompts for common MCP development scenarios:
//! quickstart, create-mcp-server, add-tool, diagnose, setup-auth,
//! debug-protocol-error, and migrate.

pub mod workflows;

pub use workflows::{
    AddToolPrompt, CreateMcpServerPrompt, DebugProtocolErrorPrompt, DiagnosePrompt, MigratePrompt,
    QuickstartPrompt, SetupAuthPrompt,
};
