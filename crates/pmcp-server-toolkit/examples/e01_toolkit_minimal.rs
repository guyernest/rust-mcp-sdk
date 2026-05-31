//! Toolkit minimal example — demonstrates Shape C ≤15-line `main.rs` usage.
//!
//! Build with:
//! ```sh
//! cargo run -p pmcp-server-toolkit --example e01_toolkit_minimal --features code-mode
//! ```
//!
//! Per CLAUDE.md ALWAYS requirements + Phase 83 review R3:
//! the imports below are ONE block, crate-root only. NO module-path
//! qualification (`pmcp_server_toolkit::auth::*` etc.). If those need to be
//! added, the D-15 headline DX promise is broken — fix the missing
//! re-export in `lib.rs`, never qualify the import here.

use std::sync::Arc;

use pmcp::Server;
// Per review R3 — the SINGLE crate-root import line that is the binding
// witness of D-15.
use pmcp_server_toolkit::{
    ServerBuilderExt, ServerConfig, StaticAuthProvider, StaticResourceHandler,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var(
        "PMCP_TOOLKIT_TOKEN_SECRET",
        "example-secret-not-for-production-use",
    );

    let config_toml = r#"
[server]
name = "Minimal Toolkit Demo"
version = "0.1.0"

[code_mode]
enabled = true
allow_writes = false
token_secret = "env:PMCP_TOOLKIT_TOKEN_SECRET"

[[tools]]
name = "ping"
description = "Synthetic test tool"
"#;

    let cfg = ServerConfig::from_toml_strict_validated(config_toml)?;

    // Shape C: ≤15-line `main.rs` body. Uses the fallible try_* variants
    // per review R7 so misconfiguration surfaces as `?` rather than panic.
    let _server = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config(&cfg)?
        .try_code_mode_from_config(&cfg)?
        .resources_arc(Arc::new(StaticResourceHandler::from(&cfg)))
        .auth_provider_arc(Arc::new(StaticAuthProvider::new("example-token")))
        .build()?;

    println!(
        "pmcp-server-toolkit example: server built with {} tool(s) from config",
        cfg.tools.len()
    );
    Ok(())
}
