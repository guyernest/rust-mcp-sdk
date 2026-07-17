//! Roots wire types (target-agnostic).
//!
//! `Root` and `ListRootsResult` describe directories or files exposed across
//! the MCP roots surface. They live here (rather than under `server::roots`)
//! so both the server roots manager and the client host surface
//! (`client::host`) can name them on **every** target, including
//! `wasm32-unknown-unknown`.
//!
//! The historical path `pmcp::server::roots::{Root, ListRootsResult}` still
//! resolves via a back-compat re-export in [`crate::server::roots`].

use serde::{Deserialize, Serialize};

/// Represents a root directory or file that the server can operate on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Root {
    /// The URI identifying the root. This must start with file:// for now.
    pub uri: String,
    /// An optional name for the root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Result of listing roots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRootsResult {
    /// The list of roots.
    pub roots: Vec<Root>,
}
