//! Spec-compliant wire types for the MCP Tasks protocol.
//!
//! These types serialize to JSON that matches the MCP 2025-11-25 Tasks
//! specification byte-for-byte. Domain types with PMCP extensions
//! (variables, owner) are in the [`domain`](crate::domain) module.

pub mod capabilities;
pub mod execution;
pub mod notification;
pub mod params;
pub mod task;

pub use capabilities::*;
pub use execution::*;
pub use notification::*;
pub use params::*;
pub use task::*;
