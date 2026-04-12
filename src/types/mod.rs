//! Core protocol types for MCP.
//!
//! This module contains all the type definitions for the Model Context Protocol,
//! including requests, responses, notifications, and capability definitions.

pub mod auth;
pub mod capabilities;
pub mod completable;
pub mod content;
pub mod elicitation;
pub mod jsonrpc;
pub mod notifications;
pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod sampling;
pub mod tasks;
pub mod tools;

/// UI resources for MCP Apps Extension (SEP-1865)
pub mod ui;

/// MCP Apps Extension types for interactive UI support (ChatGPT Apps, MCP-UI)
#[cfg(feature = "mcp-apps")]
pub mod mcp_apps;

// Re-export transport message type
pub use crate::shared::transport::TransportMessage;

// Re-export protocol version constants
pub use crate::{DEFAULT_PROTOCOL_VERSION, LATEST_PROTOCOL_VERSION, SUPPORTED_PROTOCOL_VERSIONS};

// Re-export commonly used types for flat access.
// protocol/mod.rs re-exports all domain module types, so a single
// `pub use protocol::*` provides `types::X` for every type.
pub use protocol::*;

pub use auth::{AuthInfo, AuthScheme};
pub use capabilities::{
    ClientCapabilities, ClientTasksCapability, CompletionCapabilities, ElicitationCapabilities,
    FormElicitationCapability, LoggingCapabilities, PromptCapabilities, ResourceCapabilities,
    RootsCapabilities, SamplingCapabilities, ServerCapabilities, ServerTasksCapability,
    ToolCapabilities,
};
pub use elicitation::{
    ElicitAction, ElicitRequestParams, ElicitResult, ElicitationCompleteNotification,
};
pub use jsonrpc::{JSONRPCError, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId};
pub use ui::{ToolUIMetadata, UIMimeType, UIResource, UIResourceContents};

// MCP Apps Extension re-exports
#[cfg(feature = "mcp-apps")]
pub use mcp_apps::{
    ChatGptToolMeta, ExtendedUIMimeType, HostType, NotifyLevel, RemoteDomFramework, ToolVisibility,
    UIAction, UIContent, UIDimensions, UIMetadata, WidgetCSP, WidgetMeta, WidgetResponseMeta,
};
