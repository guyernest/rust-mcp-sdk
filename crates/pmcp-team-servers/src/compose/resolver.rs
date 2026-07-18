//! The `PackageResolver` seam: resolve a member's [`pmcp_package::ComponentRef`]
//! to its [`pmcp_package::AgentPackage`].
//!
//! team-mcp dispatch (109-05) needs to load the target member's agent package
//! to forward a `tools/call` to it. This trait is that lookup seam; 109-05/109-06
//! provide a concrete local file/dir implementation. Defined atomically here
//! (109-01) as a contract so downstream plans build against a stable signature.

use async_trait::async_trait;

/// Why a [`PackageResolver::resolve_agent`] lookup failed.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// No agent package matched the requested reference.
    #[error("agent package not found for reference: {0}")]
    NotFound(String),
    /// A filesystem/transport error occurred while locating the package.
    #[error("i/o error resolving agent package: {0}")]
    Io(String),
    /// The located package payload failed to deserialize.
    #[error("failed to parse agent package: {0}")]
    Parse(String),
}

/// Resolves a component reference to the concrete agent package it names.
///
/// The `ComponentRef` → `AgentPackage` seam team-mcp dispatch resolves member
/// agents through (109-05/109-06 provide a local file/dir impl).
#[async_trait]
pub trait PackageResolver: Send + Sync {
    /// Resolve a member's component reference to its captured agent package.
    ///
    /// # Errors
    /// - [`ResolveError::NotFound`] — no package matches `r`.
    /// - [`ResolveError::Io`] — a filesystem/transport failure occurred while
    ///   locating the package.
    /// - [`ResolveError::Parse`] — the located package failed to deserialize.
    async fn resolve_agent(
        &self,
        r: &pmcp_package::ComponentRef,
    ) -> Result<pmcp_package::AgentPackage, ResolveError>;
}
