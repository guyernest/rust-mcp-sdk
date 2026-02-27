//! Workflow-based prompt system
//!
//! This module provides a type-safe, ergonomic API for building prompts as workflows.
//! It supports both loose (text-only) and strict (handle-based) modes for easy migration
//! and maximum type safety.
//!
//! # Examples
//!
//! ## Loose Mode (Easy Migration)
//!
//! ```
//! use pmcp::server::workflow::{PromptContent, InternalPromptMessage};
//! use pmcp::types::Role;
//!
//! let message = InternalPromptMessage::user("Please review this code");
//! ```
//!
//! ## Strict Mode (Type-Safe)
//!
//! ```
//! use pmcp::server::workflow::{ToolHandle, PromptContent, InternalPromptMessage};
//! use pmcp::types::Role;
//!
//! let tool = ToolHandle::new("greet");
//! let message = InternalPromptMessage::new(Role::Assistant, tool);
//! ```

pub mod conversion;
pub mod data_source;
pub mod dsl;
pub mod error;
pub mod handles;
pub mod into_prompt_content;
pub mod newtypes;
pub mod prompt_content;
pub mod prompt_handler;
pub mod sequential;
pub mod task_prompt_handler;
pub mod workflow_step;

// Re-export commonly used types
pub use conversion::{ExpansionContext, ResourceInfo, ToolInfo};
pub use data_source::DataSource;
pub use error::WorkflowError;
pub use handles::{ResourceHandle, ToolHandle};
pub use into_prompt_content::IntoPromptContent;
pub use newtypes::{ArgName, BindingName, StepName, Uri};
pub use prompt_content::{InternalPromptMessage, PromptContent};
pub use prompt_handler::WorkflowPromptHandler;
pub use sequential::{ArgumentSpec, SequentialWorkflow};
pub use task_prompt_handler::TaskWorkflowPromptHandler;
pub use workflow_step::WorkflowStep;
