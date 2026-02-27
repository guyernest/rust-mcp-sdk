//! Load testing engine for MCP servers.
//!
//! Provides typed TOML configuration, an MCP-aware HTTP client,
//! error classification, and HdrHistogram-based metrics.

pub mod client;
pub mod config;
pub mod error;
pub mod metrics;
