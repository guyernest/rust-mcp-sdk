//! Postgres connector for pmcp-server-toolkit.
//!
//! Implements `pmcp_server_toolkit::sql::SqlConnector` over `tokio-postgres`
//! + `deadpool-postgres`. Pure-Rust + Lambda-deployable per
//! `feedback_avoid_docker_pure_rust_lambda` memory.
//!
//! # Example
//!
//! ```no_run
//! use pmcp_toolkit_postgres::PostgresConnector;
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = PostgresConnector::connect("postgres://localhost/mydb").await?;
//! # Ok(()) }
//! ```

#![allow(clippy::doc_markdown)]

use pmcp_server_toolkit::sql::ConnectorError;

/// Postgres connector backed by `tokio-postgres` + `deadpool-postgres`.
///
/// Fields are intentionally empty in Wave 0 — Plan 05 lands the connection
/// pool and the full [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector)
/// trait impl.
pub struct PostgresConnector {}

impl PostgresConnector {
    /// Connect to a Postgres backend by URL.
    ///
    /// # Errors
    ///
    /// Wave 0 stub: returns [`ConnectorError::Schema`] with an
    /// `unimplemented` marker until Plan 05 lands the real connection logic.
    pub async fn connect(_url: &str) -> Result<Self, ConnectorError> {
        Err(ConnectorError::Schema(format!(
            "{}: unimplemented — Plan 05",
            "PostgresConnector"
        )))
    }
}
