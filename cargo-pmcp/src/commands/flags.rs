//! Shared flag structs for all cargo-pmcp commands.
//!
//! These types provide consistent CLI flags across commands via `#[command(flatten)]`.
//! Using shared structs ensures uniform naming, help text, and behavior.

use clap::{Args, ValueEnum};
use std::fmt;
use std::path::PathBuf;

/// Output format for commands that support structured output.
///
/// Used by commands that can emit either human-readable text or
/// machine-parseable JSON output.
#[derive(Debug, Clone, ValueEnum)]
pub enum FormatValue {
    /// Human-readable text output (default).
    Text,
    /// Machine-parseable JSON output.
    Json,
}

impl fmt::Display for FormatValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatValue::Text => write!(f, "text"),
            FormatValue::Json => write!(f, "json"),
        }
    }
}

/// Flags for commands that write output to a file.
///
/// Provides `--output` / `-o` for redirecting command output to a file path.
#[derive(Debug, Args)]
pub struct OutputFlags {
    /// Write output to a file instead of stdout.
    #[arg(long, short)]
    pub output: Option<PathBuf>,
}

/// Flags for commands that support format selection.
///
/// Provides `--format` with `text` (default) or `json` variants.
#[derive(Debug, Args)]
pub struct FormatFlags {
    /// Output format: text or json.
    #[arg(long, value_enum, default_value = "text")]
    pub format: FormatValue,
}

/// Flags for commands that accept a server target.
///
/// Provides a positional URL argument and `--server` flag for commands
/// that can target either a URL directly or a named pmcp.run server.
/// Used via `#[command(flatten)]` on commands where both URL and server
/// are optional (test run, test generate, schema export).
#[derive(Debug, Args)]
pub struct ServerFlags {
    /// URL of the MCP server (positional argument).
    #[arg(index = 1)]
    pub url: Option<String>,

    /// Named server on pmcp.run (alternative to URL).
    #[arg(long)]
    pub server: Option<String>,
}
