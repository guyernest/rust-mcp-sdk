//! Shape C smoke example — build a `pmcp-sql-server` from config + connector in
//! library form (no CLI, no transport), in a ≤15-line `main` body.
//!
//! This is the ALWAYS-matrix **runnable example** for the binary crate (TEST /
//! SHAP-A-01): it demonstrates that the same `build_server` the binary uses can
//! be driven directly from a few lines of Rust against an in-memory SQLite
//! connector. It is deliberately a SMOKE example — the strict Shape C ≤15-line
//! *library contract* (and its scaffolding/deploy siblings) is owned by Phase 86;
//! do not grow this into that contract.
//!
//! Run with:
//! ```sh
//! cargo run -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite
//! ```

use std::sync::Arc;

use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use pmcp_server_toolkit::ServerConfig;
use pmcp_sql_server::build_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The config's `${CODE_MODE_SECRET}` token_secret resolves at wiring time.
    std::env::set_var("CODE_MODE_SECRET", "sql-server-min-example-secret-32b");
    let connector: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory()?);
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG)?;
    let server = build_server(&cfg, connector, "-- schema served as a resource --".into())?;
    println!(
        "pmcp-sql-server example: built '{}' with {} curated tool(s) from config",
        cfg.server.name,
        cfg.tools.len()
    );
    let _ = server; // a real binary would `serve(server, addr).await?` here.
    Ok(())
}

/// Inline minimal config: one SQLite-backed curated tool + Code Mode enabled.
const CONFIG: &str = r#"
[server]
name = "SQL Server Min Demo"
version = "0.1.0"
type = "sql-api"

[database]
type = "sqlite"
file_path = ":memory:"

[code_mode]
enabled = true
allow_writes = false
token_secret = "${CODE_MODE_SECRET}"

[[tools]]
name = "list_artists"
description = "List all artists"
sql = "SELECT ArtistId, Name FROM Artist ORDER BY Name LIMIT :limit"

[[tools.parameters]]
name = "limit"
type = "integer"
description = "Maximum number of artists"
required = false
default = 20
"#;
