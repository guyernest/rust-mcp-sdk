//! Embedded axum HTTP server for serving widget files in tests.
//!
//! Resolves the workspace examples directory from `CARGO_MANIFEST_DIR`
//! and serves each widget's `widgets/` directory under its own route prefix.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

/// Start an embedded test server that serves widget HTML files.
///
/// Routes:
/// - `/chess/` -> `examples/mcp-apps-chess/widgets/`
/// - `/map/` -> `examples/mcp-apps-map/widgets/`
/// - `/dataviz/` -> `examples/mcp-apps-dataviz/widgets/`
///
/// Returns the bound socket address and a join handle for the spawned server task.
pub async fn start_test_server() -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let examples_root = resolve_examples_root()?;

    let chess_dir = examples_root.join("mcp-apps-chess/widgets");
    let map_dir = examples_root.join("mcp-apps-map/widgets");
    let dataviz_dir = examples_root.join("mcp-apps-dataviz/widgets");

    for (name, path) in [
        ("chess", &chess_dir),
        ("map", &map_dir),
        ("dataviz", &dataviz_dir),
    ] {
        anyhow::ensure!(
            path.exists(),
            "Widget directory not found for {}: {}",
            name,
            path.display()
        );
    }

    let app = Router::new()
        .nest_service("/chess", ServeDir::new(&chess_dir))
        .nest_service("/map", ServeDir::new(&map_dir))
        .nest_service("/dataviz", ServeDir::new(&dataviz_dir))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((addr, handle))
}

/// Resolve the workspace `examples/` directory from the crate's manifest location.
///
/// The E2E test crate lives at `crates/mcp-e2e-tests/` within the workspace,
/// so the workspace root is two directories up.
fn resolve_examples_root() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let examples = manifest_dir.join("../../examples").canonicalize()?;
    anyhow::ensure!(
        examples.is_dir(),
        "Examples directory not found at {}",
        examples.display()
    );
    Ok(examples)
}
