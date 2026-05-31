//! SQLite-backed MCP server example — Shape C ≤15-line `main.rs`.
//!
//! Demonstrates building synthesized tools from inline TOML config + an
//! in-memory SQLite connector. Uses the `synthesize_from_config_with_connector`
//! variant from Plan 03 (REVIEWS H3 fix) — NOT the original 1-arg
//! `synthesize_from_config`, which does not accept a connector.
//!
//! Run with:
//! ```sh
//! cargo run --example sqlite_minimal --features sqlite,code-mode -p pmcp-server-toolkit
//! ```

use std::sync::Arc;

use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use pmcp_server_toolkit::tools::synthesize_from_config_with_connector;

const CONFIG: &str = r#"
[server]
name = "sqlite-demo"
version = "0.1.0"

[database]
type = "sqlite"
database = ":memory:"
"#;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG)?;
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory()?);
    let tools = synthesize_from_config_with_connector(&cfg, conn)?;
    println!("sqlite_minimal: synthesized {} tools", tools.len());
    Ok(())
}
