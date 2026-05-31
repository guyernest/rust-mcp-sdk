//! Command-line surface for the `pmcp-sql-server` binary.
//!
//! Honours the D-03 two-input model: `--config` is the server + operation +
//! code-mode-policy TOML, `--schema` is the (DDL) code-mode schema resource
//! file. `--http` selects the bind address for the streamable-HTTP transport.
//!
//! The struct is parsed by [`Args::parse`] in `main.rs` (Plan 05) and is
//! re-exported from the crate root so both the binary and the test suite reach
//! it via a single path (`pmcp_sql_server::Args`).
//!
//! # Example
//!
//! ```
//! use pmcp_sql_server::Args;
//! use clap::Parser;
//!
//! let args = Args::try_parse_from(["pmcp-sql-server", "--config", "c.toml", "--schema", "s.ddl"])
//!     .expect("required args parse");
//! assert_eq!(args.config.to_str(), Some("c.toml"));
//! assert_eq!(args.schema.to_str(), Some("s.ddl"));
//! assert_eq!(args.http, "127.0.0.1:8080");
//! ```

use std::path::PathBuf;

/// Parsed CLI arguments for the Shape A pure-config SQL MCP server.
///
/// `--config` and `--schema` are required (clap emits a usage error when either
/// is missing); `--http` defaults to `127.0.0.1:8080` (D-disc flag shape —
/// `--http <addr>` chosen over `--transport`/`--bind`, loopback default so the
/// out-of-the-box binary does not expose a public listener).
#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "pmcp-sql-server",
    version,
    about = "Shape A pure-config SQL MCP server — point it at a config.toml + schema and serve a production MCP server with no Rust required"
)]
pub struct Args {
    /// Path to the server `config.toml` (server + `[[tools]]` + `[database]` +
    /// `[code_mode]` declarations). Required.
    #[arg(long)]
    pub config: PathBuf,

    /// Path to the code-mode schema resource file (DDL text served as the schema
    /// resource / code-mode prompt input, D-06). Required.
    #[arg(long)]
    pub schema: PathBuf,

    /// Bind address for the streamable-HTTP transport (`host:port`).
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn parses_required_paths_with_default_http() {
        let args =
            Args::try_parse_from(["pmcp-sql-server", "--config", "c.toml", "--schema", "s.ddl"])
                .expect("required --config/--schema must parse");
        assert_eq!(args.config.to_str(), Some("c.toml"));
        assert_eq!(args.schema.to_str(), Some("s.ddl"));
        assert_eq!(
            args.http, "127.0.0.1:8080",
            "http defaults to loopback:8080"
        );
    }

    #[test]
    fn http_flag_overrides_default() {
        let args = Args::try_parse_from([
            "pmcp-sql-server",
            "--config",
            "c.toml",
            "--schema",
            "s.ddl",
            "--http",
            "0.0.0.0:9000",
        ])
        .expect("explicit --http must parse");
        assert_eq!(args.http, "0.0.0.0:9000");
    }

    #[test]
    fn missing_config_is_a_usage_error() {
        let err = Args::try_parse_from(["pmcp-sql-server", "--schema", "s.ddl"]);
        assert!(err.is_err(), "missing --config must fail clap parsing");
    }

    #[test]
    fn missing_schema_is_a_usage_error() {
        let err = Args::try_parse_from(["pmcp-sql-server", "--config", "c.toml"]);
        assert!(err.is_err(), "missing --schema must fail clap parsing");
    }
}
