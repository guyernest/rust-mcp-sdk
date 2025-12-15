//! Identity provider implementations.
//!
//! This module provides concrete implementations of the [`IdentityProvider`] trait
//! for popular OAuth/OIDC providers.
//!
//! # Built-in Providers
//!
//! - [`CognitoProvider`] - AWS Cognito user pools
//! - [`GenericOidcProvider`] - Any OIDC-compliant provider (Google, Auth0, Okta, etc.)
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::auth::providers::{CognitoProvider, GenericOidcProvider};
//!
//! // Use Cognito
//! let cognito = CognitoProvider::new("us-east-1", "us-east-1_xxxxx", "client-id").await?;
//!
//! // Use any OIDC provider (Google example)
//! let google = GenericOidcProvider::new(
//!     "google",
//!     "Google Identity",
//!     "https://accounts.google.com",
//!     "your-client-id",
//! ).await?;
//! ```

mod cognito;
mod generic_oidc;

pub use cognito::CognitoProvider;
pub use generic_oidc::{GenericOidcConfig, GenericOidcProvider};
