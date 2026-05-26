//! MySQL connector for pmcp-server-toolkit.
//!
//! Implements `pmcp_server_toolkit::sql::SqlConnector` over `sqlx` (MySQL
//! feature, pure-Rust). Pure-Rust + Lambda-deployable per
//! `feedback_avoid_docker_pure_rust_lambda` memory.
//!
//! # Example
//!
//! ```no_run
//! use pmcp_toolkit_mysql::MysqlConnector;
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = MysqlConnector::connect("mysql://localhost/mydb").await?;
//! # Ok(()) }
//! ```

#![allow(clippy::doc_markdown)]

use pmcp_server_toolkit::sql::ConnectorError;

/// MySQL connector backed by `sqlx` (MySQL feature).
///
/// Fields are intentionally empty in Wave 0 — Plan 06 lands the connection
/// pool and the full [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector)
/// trait impl.
pub struct MysqlConnector {}

impl MysqlConnector {
    /// Connect to a MySQL backend by URL.
    ///
    /// # Errors
    ///
    /// Wave 0 stub: returns [`ConnectorError::Schema`] with an
    /// `unimplemented` marker until Plan 06 lands the real connection logic.
    pub async fn connect(_url: &str) -> Result<Self, ConnectorError> {
        Err(ConnectorError::Schema(format!(
            "{}: unimplemented — Plan 06",
            "MysqlConnector"
        )))
    }
}
