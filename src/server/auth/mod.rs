//! Server-side authentication providers and middleware.
//!
//! This module provides a provider-agnostic authentication system for MCP servers.
//! The core design principle is that **your MCP server code should never know about
//! OAuth providers, tokens, or authentication flows - it only sees [`AuthContext`]**.
//!
//! # Quick Start
//!
//! ```rust
//! use pmcp::server::auth::{AuthContext, TokenValidatorConfig, ClaimMappings};
//!
//! // Configure JWT validation for your OAuth provider
//! let config = TokenValidatorConfig::jwt(
//!     "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
//!     "your-app-client-id",
//! );
//!
//! // Use provider-specific claim mappings
//! let mappings = ClaimMappings::cognito();
//! ```
//!
//! # Provider Support
//!
//! The authentication system supports multiple OAuth providers through configuration:
//! - AWS Cognito ([`ClaimMappings::cognito`])
//! - Microsoft Entra ID ([`ClaimMappings::entra`])
//! - Google Identity ([`ClaimMappings::google`])
//! - Okta ([`ClaimMappings::okta`])
//! - Auth0 ([`ClaimMappings::auth0`])
//! - Generic OIDC (custom [`ClaimMappings`])

pub mod config;
pub mod jwt;
pub mod middleware;
pub mod mock;
pub mod oauth2;
pub mod proxy;
pub mod traits;

// Re-export core traits and types
pub use traits::{
    AuthContext, AuthProvider, ClaimMappings, ScopeBasedAuthorizer, SessionManager, TokenValidator,
    ToolAuthorizer,
};

// Re-export configuration types
pub use config::TokenValidatorConfig;

// Re-export JWT validator
pub use jwt::JwtValidator;

// Re-export mock validator for testing
pub use mock::{MockAuthContextBuilder, MockValidator};

// Re-export proxy providers
pub use proxy::{NoOpAuthProvider, OptionalAuthProvider, ProxyProvider, ProxyProviderConfig};

// Keep existing OAuth2 exports for compatibility
pub use oauth2::{
    AccessToken, AuthorizationCode, AuthorizationRequest, GrantType, InMemoryOAuthProvider,
    OAuthClient, OAuthError, OAuthMetadata, OAuthProvider, ProxyOAuthProvider, ResponseType,
    RevocationRequest, TokenInfo, TokenRequest, TokenType,
};
