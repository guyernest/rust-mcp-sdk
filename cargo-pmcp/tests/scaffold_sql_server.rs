//! Plan 86-04 TEST-05 — end-to-end scaffold-to-serve integration test.
//!
//! This test exercises the REAL Shape B command surface (SHAP-B-01 / TEST-05,
//! Review M1 / Codex 86-04 MEDIUM: do NOT weaken to an in-process `new::execute`
//! call):
//!
//!   1. `tempfile::tempdir()` — an isolated, auto-cleaned scratch dir.
//!   2. Scaffold via the REAL built binary
//!      `cargo pmcp new --kind sql-server <name>` invoked through
//!      `env!("CARGO_BIN_EXE_cargo-pmcp")` (Cargo sets this for the crate's own
//!      bin in integration tests). This is the ACTUAL command a user runs — not
//!      `new::execute` in-process.
//!   3. Append a `[patch.crates-io]` block to the scaffolded crate's `Cargo.toml`
//!      (via the shared `scaffold_patch::append_crates_io_patch` helper) so the
//!      as-yet-unpublished `pmcp-server-toolkit 0.1.0` (and its transitive
//!      unpublished workspace deps) resolve against their in-repo paths
//!      (RESEARCH Pitfall §1, Assumption A1 — the crate is not yet on crates.io).
//!   4. Spawn a REAL `cargo run` subprocess in that tempdir (SC-1's
//!      "verified end-to-end" promise — not an in-process `run_serving`).
//!   5. Parse the server's MACHINE-READABLE `PMCP_SQL_SERVER_ADDR=` line from
//!      stdout (Plan 02/03 print it — M1, do NOT scrape cargo build output).
//!   6. Poll for HTTP readiness using the verbatim `parity_chinook.rs` 20-attempt
//!      linear-backoff loop, then assert `tools/list` advertises the curated
//!      `list_books` tool and one `tools/call` of `list_books` succeeds.
//!
//! Every spawned child is wrapped in a `ChildGuard` (Drop-kill) so a panic
//! anywhere after spawn cannot leak the `cargo run` server subprocess (M1,
//! threat T-86-04-02). On a readiness timeout the test panics WITH the captured
//! stdout/stderr (M1).
//!
//! # Running
//!
//! This test MUST run single-threaded — it spawns an HTTP server, and the
//! `cargo run` subprocess plus the scaffolded crate's first-build compile the
//! unpublished toolkit (slow; generous read/poll timeouts apply). Per Gemini LOW
//! (CI build perf) it belongs to the `--test-threads=1` group so the heavy
//! tempdir build does not contend:
//!
//! ```sh
//! cargo test -p cargo-pmcp --test scaffold_sql_server -- --test-threads=1
//! ```

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use mcp_tester::report::TestStatus;
use mcp_tester::ServerTester;
use serde_json::json;

// The shared [patch.crates-io] writer + ChildGuard + repo_root, written once and
// reused by deploy_config_only.rs (Plan 06) too (M1).
#[path = "support/scaffold_patch.rs"]
mod scaffold_patch;

use scaffold_patch::{append_crates_io_patch, ChildGuard};

/// The scaffolded crate name (a valid `validate_crate_name` identifier).
const SCAFFOLD_NAME: &str = "scaffold_sql_demo";

/// The curated tool the emitted `config.toml` declares (Plan 03).
const CURATED_TOOL: &str = "list_books";

/// Wall-clock budget for reading the server's `PMCP_SQL_SERVER_ADDR=` line. The
/// `cargo run` subprocess must first compile the unpublished toolkit (cold build
/// in a fresh tempdir target/), which dominates — be generous.
const ADDR_READ_TIMEOUT: Duration = Duration::from_secs(600);

/// Drain a piped stream into a String (best-effort) — used to surface the child's
/// captured output in a readiness-timeout panic message (M1).
fn drain<R: Read>(mut r: R) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tools_list_and_call_against_scaffolded_server() {
    // (1) Isolated, auto-cleaned scratch dir for the scaffold + its build.
    let tmp = tempfile::tempdir().expect("create tempdir");

    // (2) Scaffold via the REAL built binary (M1 — the actual command surface).
    //     `cargo pmcp new --kind sql-server <name>` is invoked as the bin's own
    //     `new` subcommand. Cargo sets CARGO_BIN_EXE_cargo-pmcp for integration
    //     tests. We do NOT call `new::execute` in-process.
    let scaffold_status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "sql-server", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .status()
        .expect("spawn the real cargo-pmcp binary to scaffold");
    assert!(
        scaffold_status.success(),
        "`cargo pmcp new --kind sql-server {SCAFFOLD_NAME}` must succeed (exit {scaffold_status:?})"
    );

    let crate_dir = tmp.path().join(SCAFFOLD_NAME);
    assert!(
        crate_dir.join("Cargo.toml").is_file(),
        "scaffold must emit Cargo.toml at {}",
        crate_dir.display()
    );

    // (3) Make the unpublished `pmcp-server-toolkit 0.1.0` (+ transitive
    //     unpublished workspace crates) resolve via a [patch.crates-io] override
    //     pointing at the in-repo paths (Pitfall §1, via the shared helper).
    append_crates_io_patch(&crate_dir);

    // (4) Spawn the REAL `cargo run` subprocess in the scaffolded crate dir. The
    //     scaffold reads config.toml/schema.sql relative to cwd (H1 — no
    //     PMCP_ASSETS_DIR needed) and bootstraps SQLite at the local demo.db.
    let mut child = Command::new(env!("CARGO"))
        .args(["run", "--quiet"])
        .current_dir(&crate_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `cargo run` in the scaffolded crate dir");

    // Take the pipes BEFORE wrapping in ChildGuard (Drop borrows the child).
    let stdout = child.stdout.take().expect("child stdout piped");
    let stderr = child.stderr.take().expect("child stderr piped");
    // IMMEDIATELY wrap so a panic anywhere below cannot leak the process (M1).
    let guard = ChildGuard(child);

    // (5) Parse the machine-readable address line from stdout (M1 — never scrape
    //     cargo build output). Read line-by-line under a wall-clock budget so the
    //     cold first-build (compiling the unpublished toolkit) does not wedge us.
    let mut reader = BufReader::new(stdout);
    let mut url = String::new();
    let mut captured_stdout = String::new();
    let read_start = Instant::now();
    loop {
        if read_start.elapsed() > ADDR_READ_TIMEOUT {
            break; // wall-clock timeout — fall through to the assert with output
        }
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        if n == 0 {
            break; // EOF — child exited before printing the address
        }
        captured_stdout.push_str(&line);
        if let Some(addr) = line.trim().strip_prefix("PMCP_SQL_SERVER_ADDR=") {
            url = addr.to_string();
            break;
        }
    }
    assert!(
        !url.is_empty(),
        "scaffolded server never printed PMCP_SQL_SERVER_ADDR= within {}s.\n\
         --- captured stdout ---\n{captured_stdout}\n--- captured stderr ---\n{}",
        ADDR_READ_TIMEOUT.as_secs(),
        drain(stderr),
    );

    // (6) Drive the live server over HTTP. Readiness-poll initialize with linear
    //     backoff (20 attempts), VERBATIM from parity_chinook.rs (lines 182-194).
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned scaffold server");

    let mut initialized = false;
    for attempt in 0..20u32 {
        if matches!(tester.test_initialize().await.status, TestStatus::Passed) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    // (7) On a readiness timeout, FAIL with the captured stdout/stderr (M1).
    assert!(
        initialized,
        "MCP initialize never succeeded against {url} (readiness timeout).\n\
         --- captured stdout ---\n{captured_stdout}\n--- captured stderr ---\n{}",
        drain(stderr),
    );

    // (8) tools/list must advertise the curated `list_books` tool.
    let list = tester.test_tools_list().await;
    assert!(
        matches!(list.status, TestStatus::Passed),
        "tools/list must succeed: {list:?}"
    );
    let tools = tester.get_tools().expect("tools cached after tools/list");
    assert!(
        tools.iter().any(|t| t.name == CURATED_TOOL),
        "tools/list must advertise `{CURATED_TOOL}`; saw: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // (9) tools/call `list_books` must succeed (returns rows / structuredContent).
    let call = tester
        .test_tool(CURATED_TOOL, json!({ "limit": 5 }))
        .await
        .expect("test_tool must not error at the harness level");
    assert!(
        matches!(call.status, TestStatus::Passed),
        "tools/call `{CURATED_TOOL}` must succeed: {call:?}"
    );

    // (10) ChildGuard Drop kills+waits the subprocess; tempdir auto-cleans. The
    //      explicit drop documents the reap point (Drop also runs at scope end).
    drop(guard);
}
