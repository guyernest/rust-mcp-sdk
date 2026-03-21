//! Tower middleware layers for MCP server security.
//!
//! Provides composable Tower Layers for DNS rebinding protection and
//! security response headers. These layers wrap OUTSIDE the existing
//! `ServerHttpMiddleware` chain.

pub mod dns_rebinding;
pub mod security_headers;

pub use dns_rebinding::{AllowedOrigins, DnsRebindingLayer, DnsRebindingService};
pub use security_headers::{SecurityHeadersLayer, SecurityHeadersService};
