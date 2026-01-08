//! Secrets management for MCP servers.
//!
//! This module provides a multi-provider architecture for managing secrets
//! across different deployment targets:
//!
//! - **pmcp.run**: Enterprise-grade secret management with organization-level sharing
//! - **AWS Secrets Manager**: Self-hosted AWS deployments
//! - **Local**: Development environment with file-based storage
//!
//! # Secret Naming Convention
//!
//! Secrets are namespaced by server ID to avoid conflicts:
//!
//! ```text
//! {server-id}/{SECRET_NAME}
//!
//! Examples:
//!   chess/ANTHROPIC_API_KEY
//!   london-tube/TFL_APP_KEY
//!   my-api/DATABASE_URL
//! ```
//!
//! # Security
//!
//! - All secret values are wrapped in `SecretValue` which uses the `secrecy` crate
//! - Memory is zeroized when secrets are dropped
//! - Debug/Display output shows `[REDACTED]` instead of actual values
//! - Local secrets are stored with file permissions set to 0600
//!
//! # Example
//!
//! ```ignore
//! use cargo_pmcp::secrets::{ProviderRegistry, SecretValue, SetOptions};
//!
//! // Create provider registry
//! let registry = ProviderRegistry::default();
//!
//! // Get local provider
//! let provider = registry.get("local")?;
//!
//! // Set a secret
//! provider.set(
//!     "my-server/API_KEY",
//!     SecretValue::new("sk-...".to_string()),
//!     SetOptions::default(),
//! ).await?;
//!
//! // Get a secret
//! let value = provider.get("my-server/API_KEY").await?;
//! println!("Secret value: {}", value.expose());
//! ```

pub mod config;
pub mod error;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod value;

// Re-export types used by CLI commands
pub use provider::{ListOptions, SetOptions};
pub use registry::ProviderRegistry;
pub use value::{SecretCharset, SecretValue};
