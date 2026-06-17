//! Task 1 (Plan 95-02) — server-surface integration test for
//! [`pmcp_workbook_server::build_server`].
//!
//! Builds a [`pmcp::Server`] from the committed synthetic golden bundle
//! (`tax-calc@1.1.0`) through the real `build_server` path the binary uses and
//! asserts the assembled IN-PROCESS surface: all five workbook tools
//! (`calculate` / `explain` / `get_manifest` / `diff_version` /
//! `render_workbook`) are registered via the VERIFIED-stable
//! [`pmcp::Server::get_tool`] inspection API (the same one
//! `pmcp-sql-server`'s `tests/assemble.rs` uses).
//!
//! The `workbook://` render resource's LIVE wire surface (`resources/list`) is
//! additionally asserted in `parity_workbook.rs` — the two tests together cover
//! both the in-process surface and the wire surface (Codex MEDIUM #4). The
//! built [`pmcp::Server`] exposes no public resource-handler accessor, so the
//! resource listability is proven over the wire where it is observable.
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-workbook-server --test assemble -- --test-threads=1
//! ```

use std::path::PathBuf;

use pmcp_workbook_server::{build_server, Args};

/// The five served workbook tools every golden-bundle server must register.
const WORKBOOK_TOOLS: &[&str] = &[
    "calculate",
    "explain",
    "get_manifest",
    "diff_version",
    "render_workbook",
];

/// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
/// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR` so the test is
/// invariant to the cwd.
fn golden_bundle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
}

fn golden_args() -> Args {
    Args {
        bundle_dir: golden_bundle_dir(),
        bundle_id: None,
        http: "127.0.0.1:0".to_string(),
    }
}

#[test]
fn build_server_from_golden_registers_all_five_tools() {
    let server = build_server(&golden_args()).expect("golden bundle assembles a server");

    for name in WORKBOOK_TOOLS {
        assert!(
            server.get_tool(name).is_some(),
            "built server must expose the '{name}' workbook tool"
        );
    }
}

#[test]
fn build_server_with_matching_bundle_id_succeeds() {
    // The golden bundle's BUNDLE.lock bundle_id is "tax-calc".
    let args = Args {
        bundle_id: Some("tax-calc".to_string()),
        ..golden_args()
    };
    let server = build_server(&args).expect("matching --bundle-id assembles a server");
    assert!(
        server.get_tool("calculate").is_some(),
        "the matching-id server still registers the workbook tools"
    );
}
