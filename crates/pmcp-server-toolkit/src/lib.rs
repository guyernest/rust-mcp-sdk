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

// === Crate-root re-exports per D-15 (headline DX promise — reviewed R3) ===
//
// A Shape C consumer writes a single one-line crate-root import:
//   use pmcp_server_toolkit::{AuthProvider, StaticAuthProvider,
//                             SecretsProvider, SecretValue, EnvSecrets};
//
// NO `as _` no-name imports (those break the DX promise — review R3).

// Auth — re-export pmcp's trait at the toolkit crate root so consumers don't
// have to write `pmcp::server::auth::AuthProvider`. The toolkit's static impl
// is also re-exported at crate root.
pub use crate::auth::StaticAuthProvider;
pub use pmcp::server::auth::AuthProvider;

// Secrets — toolkit-owned trait + value type + concrete impls. Per review R6
// the secret type `SecretValue` is toolkit-owned (NOT pmcp_code_mode::TokenSecret),
// so it's stable under `--no-default-features`.
pub use crate::secrets::{EnvSecrets, SecretValue, SecretsProvider, SecretsProviderChain};

// AWS-feature-gated secrets impls.
#[cfg(feature = "aws")]
pub use crate::secrets::{OrgSecretsManagerProvider, SecretsManagerSecrets, SsmSecrets};

// Resources (TKIT-04) — Plan 03 headline re-export per D-15 + review R3.
pub use crate::resources::StaticResourceHandler;

// Prompts (TKIT-05) — Plan 03 headline re-export per D-15 + review R3.
pub use crate::prompts::StaticPromptHandler;

// Config (TKIT-01) — Plan 04 headline re-export per D-15 + review R3.
// ServerConfig is THE single top-level config type a Shape C consumer touches.
pub use crate::config::ServerConfig;

// Validation error type also surfaces at the crate root so consumers can
// pattern-match on it without importing from `error` (review R3 headline DX).
pub use crate::error::ConfigValidationError;

// Tools (TKIT-07) — Plan 05 headline re-export per D-15 + review R3.
// `synthesize_from_config` is the one-call entry point Shape A/C consumers
// reach for; lifting it to the crate root keeps the import surface flat.
pub use crate::tools::synthesize_from_config;

// SQL connector trait stub (TKIT-10) — Plan 07 headline re-export per D-15 +
// review R3. MINIMIZED Phase 83 surface per review R2: ONLY `Dialect`,
// `SqlConnector`, and `ConnectorError` are re-exported. `execute()` and
// `translate_placeholders` are intentionally absent — they land in Phase 84
// (pmcp-server-toolkit 0.2.0) once the first real connector validates the
// contract. `MockSqlConnector` stays `pub(crate)` — it's test-only.
pub use crate::sql::{ConnectorError, Dialect, SqlConnector};

// Code-mode prompt assembler (TKIT-10 / D-12) — Plan 07 headline re-export.
// Feature-gated on `code-mode` because it lives in the code_mode module which
// is itself feature-gated (D-16: code-mode is opt-in).
#[cfg(feature = "code-mode")]
pub use crate::code_mode::assemble_code_mode_prompt;

// Why: compile-only assertion proving the headline D-15 / review-R3 crate-root
// DX promise. If any of these paths fails to resolve, the crate fails to
// build — no test runtime required.
#[allow(dead_code)]
const _ROOT_REEXPORT_SMOKE: fn() = || {
    let _: Option<&dyn AuthProvider> = None;
    let _: Option<&dyn SecretsProvider> = None;
    let _: Option<StaticAuthProvider> = None;
    let _: Option<EnvSecrets> = None;
    let _: Option<SecretValue> = None;
    let _: Option<SecretsProviderChain> = None;
    let _: Option<StaticResourceHandler> = None;
    let _: Option<StaticPromptHandler> = None;
    let _: Option<ServerConfig> = None;
    let _: Option<ConfigValidationError> = None;
    // Plan 05 (TKIT-07): synthesize_from_config is fn-typed; reference the
    // function pointer to assert the re-exported path resolves at the crate root.
    let _: fn(&ServerConfig) -> Result<Vec<crate::tools::SynthesizedTool>> = synthesize_from_config;
    // Plan 07 (TKIT-10): SqlConnector trait stub + Dialect enum re-exports.
    let _: Option<Dialect> = None;
    let _: Option<ConnectorError> = None;
    let _: Option<&dyn SqlConnector> = None;
};

// Plan 06 (TKIT-06 + TKIT-09): compile-only assertion that the code_mode
// submodule's re-exports + wiring helpers resolve at `code_mode::*`. Gated on
// `code-mode` because the module itself is feature-gated (D-15 + D-16: the
// headline submodule, not a flattened crate-root surface).
#[cfg(feature = "code-mode")]
#[allow(dead_code)]
const _CODE_MODE_REEXPORT_SMOKE: fn() = || {
    let _: Option<Box<dyn crate::code_mode::CodeExecutor>> = None;
    let _: Option<crate::code_mode::ValidationPipeline> = None;
    let _: Option<crate::code_mode::TokenSecret> = None;
    let _: Option<crate::code_mode::HmacTokenGenerator> = None;
    let _: Option<crate::code_mode::ApprovalToken> = None;
    let _: Option<crate::code_mode::NoopPolicyEvaluator> = None;
    let _: fn(&ServerConfig) -> Result<crate::code_mode::ValidationPipeline> =
        crate::code_mode::validation_pipeline_from_config;
    // Plan 07 (TKIT-10 / D-12): assemble_code_mode_prompt re-exports at crate
    // root under the code-mode feature. The fn returns a `BoxFuture`-ish async
    // surface; reference the function pointer to assert the path resolves.
    let _ = assemble_code_mode_prompt;
};
