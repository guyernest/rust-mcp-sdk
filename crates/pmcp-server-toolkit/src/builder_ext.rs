// Net-new code for Phase 83 PATTERNS §13 (builder extension surface).
// Hosts the `ServerBuilderExt` trait + `try_*` fallible variants per review R7.

//! Builder extension trait for [`pmcp::ServerBuilder`] — connects config-driven
//! synthesis (Plans 04, 05, 06) to the public Phase 82 builder API.
//!
//! Per CONTEXT.md D-10 + D-11, this is the "common path" surface — power users
//! call [`crate::tools::synthesize_from_config`] +
//! [`crate::code_mode::register_code_mode_tools`] directly. Shape C ≤15-line
//! `main.rs` users compose this trait.
//!
//! Per review R7, each method has a panicking convenience form
//! ([`ServerBuilderExt::tools_from_config`],
//! [`ServerBuilderExt::code_mode_from_config`]) AND a fallible companion
//! ([`ServerBuilderExt::try_tools_from_config`],
//! [`ServerBuilderExt::try_code_mode_from_config`]). The panicking forms
//! delegate to the `try_*` variants with documented panic messages — production
//! servers should prefer the `try_*` shape so misconfiguration surfaces as a
//! `Result`, not a crash.

use std::sync::Arc;

use pmcp::ServerBuilder;

use crate::config::ServerConfig;
use crate::error::Result;
use crate::sql::SqlConnector;

/// Composable builder extensions for config-driven `pmcp` servers.
///
/// Implemented for [`pmcp::ServerBuilder`] (Phase 82's public, `Arc`-aware
/// builder) so config-driven wiring composes with the standard chained-method
/// builder DSL.
pub trait ServerBuilderExt: Sized {
    /// Register every `[[tools]]` entry from `config` as a `tool_arc` handler
    /// (TKIT-07). Panicking convenience wrapping
    /// [`ServerBuilderExt::try_tools_from_config`].
    ///
    /// # Panics
    ///
    /// Panics with `"tools_from_config: ..."` if
    /// [`crate::tools::synthesize_from_config`] returns `Err`. Prefer
    /// [`ServerBuilderExt::try_tools_from_config`] for production servers
    /// where misconfiguration must surface as a `Result`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .tools_from_config(&cfg);
    /// ```
    fn tools_from_config(self, config: &ServerConfig) -> Self;

    /// Fallible companion to [`ServerBuilderExt::tools_from_config`]
    /// (review R7).
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if synthesis fails — typically
    /// [`crate::ToolkitError::Synth`] or [`crate::ToolkitError::Validation`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_tools_from_config(&cfg)?;
    /// # Ok(()) }
    /// ```
    fn try_tools_from_config(self, config: &ServerConfig) -> Result<Self>;

    /// Register every `[[tools]]` entry from `config` as a `tool_arc` handler,
    /// threading `connector` into each handler so `tools/call` executes SQL and
    /// emits `structuredContent` (Phase 84 CONN-01 / D-06). Panicking
    /// convenience wrapping [`ServerBuilderExt::try_tools_from_config_with_connector`].
    ///
    /// This is the Shape A wiring point: production servers with a live
    /// connector use this entry point; the connector-less
    /// [`ServerBuilderExt::tools_from_config`] remains for callers that only
    /// need the synthesized tool schemas (handlers error at runtime if invoked).
    ///
    /// # Panics
    ///
    /// Panics with `"tools_from_config_with_connector: ..."` if
    /// [`crate::tools::synthesize_from_config_with_connector`] returns `Err`.
    /// Prefer [`ServerBuilderExt::try_tools_from_config_with_connector`] for
    /// production servers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    /// use pmcp_server_toolkit::sql::SqlConnector;
    ///
    /// fn build(connector: Arc<dyn SqlConnector>) {
    ///     let cfg = ServerConfig::default();
    ///     let _builder = Server::builder()
    ///         .name("demo")
    ///         .version("0.1.0")
    ///         .tools_from_config_with_connector(&cfg, connector);
    /// }
    /// ```
    fn tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Self;

    /// Fallible companion to
    /// [`ServerBuilderExt::tools_from_config_with_connector`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if synthesis fails — typically
    /// [`crate::ToolkitError::Synth`] or [`crate::ToolkitError::Validation`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    /// use pmcp_server_toolkit::sql::SqlConnector;
    ///
    /// # fn run(connector: Arc<dyn SqlConnector>) -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_tools_from_config_with_connector(&cfg, connector)?;
    /// # Ok(()) }
    /// ```
    fn try_tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self>;

    /// Wire the `[code_mode]` block. Panicking convenience wrapping
    /// [`ServerBuilderExt::try_code_mode_from_config`].
    ///
    /// When the `code-mode` feature is disabled, this is a no-op that emits
    /// a `tracing::warn!` so operators auditing logs can spot the feature gap
    /// (threat T-83-08-02 mitigation).
    ///
    /// # Panics
    ///
    /// Panics if [`ServerBuilderExt::try_code_mode_from_config`] errors —
    /// commonly because `token_secret`'s referenced env var is unset, or an
    /// inline literal `token_secret` was supplied without the dev-only escape
    /// hatch (review R9). Prefer
    /// [`ServerBuilderExt::try_code_mode_from_config`] for production servers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .code_mode_from_config(&cfg);
    /// ```
    fn code_mode_from_config(self, config: &ServerConfig) -> Self;

    /// Fallible companion to [`ServerBuilderExt::code_mode_from_config`]
    /// (review R7).
    ///
    /// Tolerant of `config.code_mode = None` (returns the builder unchanged)
    /// — Plan 06 ensures
    /// [`crate::code_mode::register_code_mode_tools`] is a no-op when the
    /// block is absent.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToolkitError`] if code-mode wiring fails — commonly
    /// [`crate::ToolkitError::CodeMode`] (env var missing) or
    /// [`crate::ToolkitError::Validation`] (inline `token_secret` rejected
    /// per review R9).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp::Server;
    /// use pmcp_server_toolkit::{ServerBuilderExt, ServerConfig};
    ///
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = ServerConfig::default();
    /// let _builder = Server::builder()
    ///     .name("demo")
    ///     .version("0.1.0")
    ///     .try_code_mode_from_config(&cfg)?;
    /// # Ok(()) }
    /// ```
    fn try_code_mode_from_config(self, config: &ServerConfig) -> Result<Self>;
}

impl ServerBuilderExt for ServerBuilder {
    fn tools_from_config(self, config: &ServerConfig) -> Self {
        self.try_tools_from_config(config).expect(
            "tools_from_config: synthesize_from_config returned an error — \
             prefer try_tools_from_config to handle this as a Result",
        )
    }

    fn try_tools_from_config(mut self, config: &ServerConfig) -> Result<Self> {
        let synthesized = crate::tools::synthesize_from_config(config)?;
        // T-83-08-02 mitigation: emit a visible signal when the [[tools]]
        // block is empty so an operator notices the gap rather than seeing a
        // silently-empty server.
        if synthesized.is_empty() {
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_tools_from_config: config declared zero [[tools]] entries — \
                 server will expose no tools (set RUST_LOG=warn to surface this)"
            );
        }
        for (name, _info, handler) in synthesized {
            self = self.tool_arc(name, handler);
        }
        Ok(self)
    }

    fn tools_from_config_with_connector(
        self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Self {
        self.try_tools_from_config_with_connector(config, connector)
            .expect(
                "tools_from_config_with_connector: synthesize_from_config_with_connector \
                 returned an error — prefer try_tools_from_config_with_connector to handle \
                 this as a Result",
            )
    }

    fn try_tools_from_config_with_connector(
        mut self,
        config: &ServerConfig,
        connector: Arc<dyn SqlConnector>,
    ) -> Result<Self> {
        let synthesized = crate::tools::synthesize_from_config_with_connector(config, connector)?;
        // T-83-08-02 mitigation: visible signal when the [[tools]] block is
        // empty so an operator notices the gap rather than a silently-empty server.
        if synthesized.is_empty() {
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_tools_from_config_with_connector: config declared zero [[tools]] entries — \
                 server will expose no tools (set RUST_LOG=warn to surface this)"
            );
        }
        for (name, _info, handler) in synthesized {
            self = self.tool_arc(name, handler);
        }
        Ok(self)
    }

    fn code_mode_from_config(self, config: &ServerConfig) -> Self {
        self.try_code_mode_from_config(config).expect(
            "code_mode_from_config: register_code_mode_tools errored — \
             prefer try_code_mode_from_config to handle (e.g. missing env var)",
        )
    }

    fn try_code_mode_from_config(self, config: &ServerConfig) -> Result<Self> {
        #[cfg(feature = "code-mode")]
        {
            return crate::code_mode::register_code_mode_tools(self, config);
        }
        #[cfg(not(feature = "code-mode"))]
        {
            let _ = config;
            tracing::warn!(
                target: "pmcp_server_toolkit::builder_ext",
                "try_code_mode_from_config called but `code-mode` feature is \
                 disabled at compile-time — skipping (T-83-08-02 visibility)"
            );
            Ok(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ServerConfig, ServerSection, ToolDecl};
    use pmcp::Server;

    fn min_cfg() -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools: vec![ToolDecl {
                name: "ping".to_string(),
                description: Some("ping".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn tools_from_config_registers_synthesized_handlers() {
        let cfg = min_cfg();
        let server = Server::builder()
            .name("test")
            .version("0.1.0")
            .tools_from_config(&cfg)
            .build()
            .expect("build");
        assert!(
            server.get_tool("ping").is_some(),
            "tools_from_config must wire each [[tools]] entry via tool_arc (Phase 82)"
        );
    }

    #[test]
    fn try_tools_from_config_returns_ok_on_valid_config() {
        let cfg = min_cfg();
        let builder = Server::builder().name("t").version("0.1.0");
        let result = builder.try_tools_from_config(&cfg);
        assert!(result.is_ok(), "valid config must return Ok");
    }

    #[test]
    fn code_mode_from_config_is_noop_when_block_absent() {
        // Plan 06 Task 2 ensures register_code_mode_tools tolerates
        // config.code_mode = None.
        let cfg = min_cfg();
        let _builder = Server::builder()
            .name("t")
            .version("0.1.0")
            .code_mode_from_config(&cfg);
        // No panic means tolerance works.
    }

    #[test]
    fn try_code_mode_from_config_is_ok_when_block_absent() {
        let cfg = min_cfg();
        let builder = Server::builder().name("t").version("0.1.0");
        let result = builder.try_code_mode_from_config(&cfg);
        assert!(
            result.is_ok(),
            "code_mode = None must produce Ok (no-op) so callers can invoke unconditionally"
        );
    }
}
