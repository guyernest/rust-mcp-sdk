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
//! - [`error`] - Rich error types with JSON-RPC error code mapping
//! - [`constants`] - Meta key and method name constants

pub mod constants;
pub mod error;
pub mod types;

/// Domain types (TaskRecord, TaskWithVariables) for internal task representation.
pub mod domain;

// Stub for future plans
/// Task store trait and implementations - implemented in Plan 02.
pub mod store {}

// Re-exports for ergonomic access
pub use constants::*;
pub use domain::{TaskRecord, TaskWithVariables};
pub use error::TaskError;
pub use types::*;
