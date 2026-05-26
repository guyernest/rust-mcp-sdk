//! Amazon Athena connector for pmcp-server-toolkit.
//!
//! Implements `pmcp_server_toolkit::sql::SqlConnector` over `aws-sdk-athena`
//! (no `aws-sdk-glue` — `GetTableMetadata` covers schema introspection).
//! Pure-Rust + Lambda-deployable per `feedback_avoid_docker_pure_rust_lambda`
//! memory.
//!
//! # Example
//!
//! ```no_run
//! use pmcp_toolkit_athena::AthenaConnector;
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = AthenaConnector::from_config("us-east-1", "primary").await?;
//! # Ok(()) }
//! ```

#![allow(clippy::doc_markdown)]

use pmcp_server_toolkit::sql::ConnectorError;

/// Amazon Athena connector backed by `aws-sdk-athena`.
///
/// Fields are intentionally empty in Wave 0 — Plan 07 lands the AWS client,
/// the StartQueryExecution → poll → GetQueryResults loop, and the full
/// [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector) trait impl.
pub struct AthenaConnector {}

impl AthenaConnector {
    /// Construct an Athena connector from a region + workgroup.
    ///
    /// # Errors
    ///
    /// Wave 0 stub: returns [`ConnectorError::Schema`] with an
    /// `unimplemented` marker until Plan 07 lands the real client logic.
    pub async fn from_config(_region: &str, _workgroup: &str) -> Result<Self, ConnectorError> {
        Err(ConnectorError::Schema(format!(
            "{}: unimplemented — Plan 07",
            "AthenaConnector"
        )))
    }
}
