//! Postgres connector minimal example — Shape C ≤15-line `main`.
//!
//! Uses the in-process [`PostgresMock`] for an offline demonstration (a real
//! Postgres requires credentials + a running server). The mock is reached via
//! the published `dev_mock` feature path (REVIEWS H5) so this example is fully
//! publishable — it depends only on the crate's own `src/` content.
//!
//! Run with:
//! `cargo run -p pmcp-toolkit-postgres --example postgres_minimal --features dev_mock`

use pmcp_server_toolkit::sql::SqlConnector;
use pmcp_toolkit_postgres::dev_mock::PostgresMock;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = PostgresMock::employee_directory();
    let rows = conn.execute("SELECT * FROM employees", &[]).await?;
    println!("postgres_minimal: {} rows", rows.len());
    Ok(())
}
