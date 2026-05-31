//! Athena connector minimal example — Shape C ≤15-line `main`.
//!
//! Uses the `dev_mock` cargo feature for offline demonstration (REVIEWS H5):
//! the mock is reached via the published `dev_mock` path, NOT a `#[path]`
//! include into `tests/`.
//!
//! Run: `cargo run -p pmcp-toolkit-athena --features dev_mock --example athena_minimal`

use pmcp_server_toolkit::sql::SqlConnector;
use pmcp_toolkit_athena::dev_mock::AthenaMock;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = AthenaMock::open_images_fixture();
    let rows = conn.execute("SELECT * FROM images", &[]).await?;
    println!("athena_minimal: {} rows", rows.len());
    Ok(())
}
