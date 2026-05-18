// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as a public SDK crate for Phase 83.

//! Runtime library for config-driven MCP servers.
//!
//! `pmcp-server-toolkit` lifts the operational glue that pmcp-run servers
//! share — auth providers, secrets resolution, static resources/prompts,
//! a `[[tools]]` synthesizer, and code-mode wiring — into the public SDK.
//!
//! Phase 83 ships the empty crate skeleton; subsequent plans land the
//! functionality across these modules:
//!
//! - [`auth`] — `AuthProvider` implementations (`StaticAuthProvider`, `BearerAuthProvider`).
//! - [`secrets`] — `SecretsProvider` trait + env/AWS implementations and the
//!   `SecretValue` newtype that never leaks via `Debug`/`Display`/`Serialize`.
//! - [`config`] — `ServerConfig` types with `#[serde(deny_unknown_fields)]` strictness.
//! - [`prompts`] — `StaticPromptHandler` adapter for static prompt templates.
//! - [`resources`] — `StaticResourceHandler` adapter for shipped resources.
//! - [`tools`] — `synthesize_from_config` builder turning `[[tools]]` into runtime handlers.
//! - [`sql`] — `SqlConnector` trait + dialect enum for backend-agnostic SQL toolkits.
//! - [`builder_ext`] — `ServerBuilderExt` extension methods on `pmcp::ServerBuilder`.
//! - [`code_mode`] *(feature `code-mode`)* — re-exports from `pmcp-code-mode` plus toolkit glue.
//! - [`error`] — `ToolkitError` enum and the crate-level `Result<T>` alias.
//!
//! The public module set is locked by Phase 83 decision D-15. See the
//! `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/` design log for
//! the architectural responsibility map and review notes.

pub mod auth;
pub mod builder_ext;
pub mod config;
pub mod error;
pub mod prompts;
pub mod resources;
pub mod secrets;
pub mod sql;
pub mod tools;

#[cfg(feature = "code-mode")]
pub mod code_mode;

pub use error::{Result, ToolkitError};
