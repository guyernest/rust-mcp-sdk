// MCP protocol uses underscore-prefixed fields (_meta, _task_id) that are required by spec.
#![allow(clippy::used_underscore_binding)]

//! MCP Tasks support for the PMCP SDK (experimental).
//!
//! This crate implements the MCP 2025-11-25 Tasks specification, providing
//! spec-compliant wire types, state machine validation, error types, and
//! constants for managing long-running task lifecycles in the Model Context
//! Protocol.
//!
//! # Overview
//!
//! Tasks are coordination primitives for long-running activities between
//! client and server. A task progresses through a state machine
//! (`working` -> `completed`/`failed`/`cancelled`, with `input_required`
//! as an intermediate state) and supports TTL-based expiration, polling,
//! and status notifications.
//!
//! # Module Organization
//!
//! - [`types`] - Spec-compliant wire types (Task, params, capabilities, etc.)
//! - [`domain`] - Internal domain types (`TaskRecord`, `TaskWithVariables`)
//! - [`store`] - `TaskStore` async trait, supporting types, and backend implementations
//! - [`security`] - Security configuration and owner resolution
//! - [`error`] - Rich error types with JSON-RPC error code mapping
//! - [`constants`] - Meta key and method name constants

pub mod constants;
pub mod error;
pub mod types;

/// Domain types (TaskRecord, TaskWithVariables) for internal task representation.
pub mod domain;

/// Task store trait and supporting types (StoreConfig, ListTasksOptions, TaskPage).
pub mod store;

/// Security configuration and owner resolution.
pub mod security;

/// Ergonomic task context for tool handlers.
pub mod context;

/// TaskRouter implementation bridging pmcp's TaskRouter trait to TaskStore.
pub mod router;

// Re-exports for ergonomic access
pub use constants::*;
pub use context::TaskContext;
pub use domain::{TaskRecord, TaskWithVariables};
pub use error::TaskError;
pub use router::TaskRouterImpl;
pub use security::{resolve_owner_id, TaskSecurityConfig, DEFAULT_LOCAL_OWNER};
pub use store::memory::InMemoryTaskStore;
pub use store::{ListTasksOptions, StoreConfig, TaskPage, TaskStore};
pub use types::*;
