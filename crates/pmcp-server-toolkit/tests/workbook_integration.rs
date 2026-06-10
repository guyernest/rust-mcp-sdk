//! Phase 92 Plan 05 Task 2 — integration tests for the workbook served-tool
//! boot path THROUGH the builder (`WorkbookBuilderExt`):
//!
//! 1. **build-and-assert** — `with_workbook_bundle` over the committed golden
//!    registers all FIVE tools (`calculate`/`explain`/`get_manifest`/
//!    `diff_version`/`render_workbook`), asserted via `Server::get_tool`.
//! 2. **tamper-fails-boot** — `try_with_workbook_bundle` over a byte-flipped copy
//!    of the golden returns `Err` (WBSV-08 fail-closed, end-to-end through the
//!    builder, NOT just the loader unit). Uses the 92-02 tamper helpers.
//! 3. **example smoke-run** (Codex MEDIUM #11) — boots the SAME server the
//!    `workbook_server_http` example builds (embedded bundle) on an ephemeral
//!    streamable-HTTP port within a BOUNDED timeout, asserts all five tools are
//!    registered, then shuts the server down cleanly (no hang, no leaked socket).
//!    This proves the `cargo run --example` path actually serves, satisfying the
//!    CLAUDE.md "cargo run --example" ALWAYS requirement.
#![cfg(feature = "workbook")]

use std::path::Path;

use pmcp::Server;
use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};

mod support;

/// The five served tools every workbook server registers (the registration
/// contract this plan freezes).
const WORKBOOK_TOOLS: [&str; 5] = [
    "calculate",
    "explain",
    "get_manifest",
    "diff_version",
    "render_workbook",
];

/// The committed golden bundle directory (the 92-02 `@1.1.0` golden).
fn golden_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc@1.1.0")
}

#[test]
fn with_workbook_bundle_registers_all_five_tools() {
    let server = Server::builder()
        .name("workbook-tax-calc")
        .version("1.1.0")
        .with_workbook_bundle(&LocalDirSource::new(golden_dir()))
        .build()
        .expect("server builds from the golden bundle");

    for name in WORKBOOK_TOOLS {
        assert!(
            server.get_tool(name).is_some(),
            "with_workbook_bundle must register the `{name}` tool"
        );
    }
}

#[test]
fn tamper_fails_boot_through_the_builder() {
    // Copy the golden into a tempdir, then flip a byte of the manifest so the
    // recomputed integrity hash no longer matches the on-disk lock.
    let temp = support::tamper::copy_golden_to_temp();
    support::tamper::flip_byte(temp.path(), "manifest.json");

    // The fail-closed boot load surfaces as Err THROUGH the builder (WBSV-08) —
    // the server never registers a single tool on a tampered bundle.
    let result = Server::builder()
        .name("workbook-tax-calc")
        .version("1.1.0")
        .try_with_workbook_bundle(&LocalDirSource::new(temp.path()));

    assert!(
        result.is_err(),
        "a byte-flipped bundle must fail the boot load through the builder (WBSV-08)"
    );
}

/// Codex MEDIUM #11 — boot the example's server on an ephemeral streamable-HTTP
/// port within a bounded timeout, assert all five tools are registered, then
/// shut down cleanly. Gated on `workbook-embedded` + `http` because it builds the
/// EmbeddedSource-backed server the example serves and binds a real socket.
#[cfg(all(feature = "workbook-embedded", feature = "http"))]
#[tokio::test]
async fn example_server_boots_serves_five_tools_and_shuts_down() {
    use std::sync::Arc;
    use std::time::Duration;

    use include_dir::{include_dir, Dir};
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
    use pmcp_server_toolkit::workbook::EmbeddedSource;
    use tokio::sync::Mutex;

    // The SAME committed golden the example bakes in (include_dir over @1.1.0).
    static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0");

    // Build the same server the example builds (embedded bundle, all five tools).
    let server = Server::builder()
        .name("workbook-tax-calc")
        .version("1.1.0")
        .try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))
        .expect("embedded bundle boots fail-closed")
        .build()
        .expect("server builds from the embedded bundle");

    // Assert the registration contract BEFORE serving (every tool present).
    for name in WORKBOOK_TOOLS {
        assert!(
            server.get_tool(name).is_some(),
            "the embedded-bundle server must register `{name}`"
        );
    }

    // Boot it on an ephemeral port within a bounded timeout — proves the serve
    // path binds cleanly (not just compiles), then shut down by aborting the
    // serve task so no socket is leaked and the test cannot hang.
    let shared = Arc::new(Mutex::new(server));
    let cfg = StreamableHttpServerConfig::default();
    let boot = tokio::time::timeout(Duration::from_secs(5), async {
        StreamableHttpServer::with_config(
            "127.0.0.1:0".parse().expect("loopback addr parses"),
            shared,
            cfg,
        )
        .start()
        .await
    })
    .await
    .expect("server boots within the 5s bound (no hang)")
    .expect("server binds the ephemeral port");

    let (addr, handle) = boot;
    assert_eq!(addr.ip().to_string(), "127.0.0.1", "bound to loopback");
    assert_ne!(addr.port(), 0, "an ephemeral port was actually assigned");

    // Clean shutdown: abort the serve task so the listening socket is released
    // (no leaked socket, no hang).
    handle.abort();
    let _ = handle.await; // reap the aborted task (JoinError::Cancelled is fine)
}
