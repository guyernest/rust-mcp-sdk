//! Command-line surface for the `pmcp-openapi-server` binary.
//!
//! Honours the D-03 input model: `--config` is the server + operation +
//! code-mode-policy TOML (required), `--spec` is the OPTIONAL OpenAPI document
//! (JSON or YAML) served as the `api_schema` code-mode resource. A curated-only
//! server boots with NO `--spec` (D-03). `--http` selects the bind address for
//! the streamable-HTTP transport.
//!
//! The struct is parsed by [`Args::parse`] in `main.rs` and is re-exported from
//! the crate root so both the binary and the test suite reach it via a single
//! path (`pmcp_openapi_server::Args`).
//!
//! # Example
//!
//! ```
//! use pmcp_openapi_server::Args;
//! use clap::Parser;
//!
//! // --spec is OPTIONAL (D-03): a curated-only config boots with no spec.
//! let args = Args::try_parse_from(["pmcp-openapi-server", "--config", "c.toml"])
//!     .expect("required --config parses");
//! assert_eq!(args.config.to_str(), Some("c.toml"));
//! assert!(args.spec.is_none(), "spec is optional at runtime");
//! assert_eq!(args.http, "127.0.0.1:8080");
//! ```

use std::path::PathBuf;

/// Parsed CLI arguments for the Shape A pure-config OpenAPI MCP server.
///
/// `--config` is required (clap emits a usage error when it is missing);
/// `--spec` is OPTIONAL (D-03 — a curated-only server, or a Code-Mode server
/// without the OpenAPI contract resource, boots with no spec); `--http` defaults
/// to `127.0.0.1:8080` (loopback default so the out-of-the-box binary does not
/// expose a public listener).
#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "pmcp-openapi-server",
    version,
    about = "Shape A pure-config OpenAPI MCP server — point it at a config.toml (+ optional OpenAPI spec) and serve a production MCP server with no Rust required"
)]
pub struct Args {
    /// Path to the server `config.toml` (server + `[[tools]]` + `[backend]` +
    /// `[code_mode]` declarations). Required.
    #[arg(long)]
    pub config: PathBuf,

    /// Path to the OpenAPI document (JSON or YAML) served as the `api_schema`
    /// code-mode resource. OPTIONAL at runtime (D-03): a curated-only server
    /// boots without it. Code Mode without a spec runs WITHOUT the `api_schema`
    /// resource (a warning is emitted; the LLM generates scripts without the
    /// OpenAPI contract). Typically supplied at scaffold time but droppable for
    /// curated-only deployments.
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Bind address for the streamable-HTTP transport (`host:port`).
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn cli_parses_config_only_with_no_spec_and_default_http() {
        // D-03 proof: --spec is optional; a curated-only config parses.
        let args = Args::try_parse_from(["pmcp-openapi-server", "--config", "c.toml"])
            .expect("required --config must parse");
        assert_eq!(args.config.to_str(), Some("c.toml"));
        assert!(args.spec.is_none(), "spec is optional (D-03)");
        assert_eq!(
            args.http, "127.0.0.1:8080",
            "http defaults to loopback:8080"
        );
    }

    #[test]
    fn cli_accepts_optional_spec() {
        let args = Args::try_parse_from([
            "pmcp-openapi-server",
            "--config",
            "c.toml",
            "--spec",
            "s.yaml",
        ])
        .expect("--config + --spec must parse");
        assert_eq!(
            args.spec.as_deref().and_then(|p| p.to_str()),
            Some("s.yaml")
        );
    }

    #[test]
    fn cli_http_flag_overrides_default() {
        let args = Args::try_parse_from([
            "pmcp-openapi-server",
            "--config",
            "c.toml",
            "--http",
            "0.0.0.0:9000",
        ])
        .expect("explicit --http must parse");
        assert_eq!(args.http, "0.0.0.0:9000");
    }

    #[test]
    fn cli_missing_config_is_a_usage_error() {
        let err = Args::try_parse_from(["pmcp-openapi-server", "--spec", "s.yaml"]);
        assert!(err.is_err(), "missing --config must fail clap parsing");
    }
}
