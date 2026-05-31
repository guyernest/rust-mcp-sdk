//! MySQL connector minimal example — Shape C ≤15-line `main`.
//!
//! Uses the in-process [`MysqlMock`] for an offline demonstration (a real
//! MySQL requires credentials + a running server). The mock is reached via the
//! published `dev_mock` feature path (REVIEWS H5) so this example is fully
//! publishable — it depends only on the crate's own `src/` content.
//!
//! Run with:
//! `cargo run -p pmcp-toolkit-mysql --features dev_mock --example mysql_minimal`

use pmcp_server_toolkit::sql::SqlConnector;
use pmcp_toolkit_mysql::dev_mock::MysqlMock;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = MysqlMock::employee_directory();
    let rows = conn.execute("SELECT * FROM employees", &[]).await?;
    println!("mysql_minimal: {} rows", rows.len());
    Ok(())
}
