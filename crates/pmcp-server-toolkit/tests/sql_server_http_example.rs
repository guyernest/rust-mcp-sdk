//! Plan 86-02 Task 2 — ChildGuard-protected subprocess integration test for the
//! Shape C `sql_server_http` example, plus the ≤15-line `main`-body assertion.
//!
//! The integration test spawns the example as a subprocess (examples do NOT get
//! a `CARGO_BIN_EXE_<example>` env, so we spawn `cargo run --example`), parses the
//! machine-readable `PMCP_SQL_SERVER_ADDR=` line it prints (M1 — never scrape
//! cargo build output), then drives it over HTTP via the mcp-tester library:
//! initialize (readiness poll) → tools/list (`list_books` present) → tools/call
//! (`list_books` succeeds). A `ChildGuard` (Drop-kill) guarantees the server
//! subprocess is reaped even if an assertion panics, and a readiness timeout
//! fails WITH the captured stdout/stderr.
#![cfg(all(feature = "sqlite", feature = "code-mode", feature = "http"))]

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use mcp_tester::report::TestStatus;
use mcp_tester::ServerTester;
use serde_json::json;

/// Drop-kill guard so a panic anywhere in the test body can NOT leak the spawned
/// server subprocess (M1). `kill` + `wait` reap the child on scope exit.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Drain a piped stream into a String (best-effort) — used to surface the child's
/// captured output in a readiness-timeout panic message (M1).
fn drain<R: Read>(mut r: R) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

#[tokio::test]
async fn test_tools_list_and_call_against_serving_example() {
    // Spawn the example as a subprocess. `cargo run --example` is the portable
    // path (no CARGO_BIN_EXE for examples). The example binds an ephemeral port
    // (127.0.0.1:0) and prints `PMCP_SQL_SERVER_ADDR=http://127.0.0.1:<port>`.
    let mut child = Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "-p",
            "pmcp-server-toolkit",
            "--example",
            "sql_server_http",
            "--features",
            "sqlite,code-mode,http",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `cargo run --example sql_server_http`");

    // Take the pipes BEFORE wrapping in ChildGuard (Drop borrows the child).
    let stdout = child.stdout.take().expect("child stdout piped");
    let stderr = child.stderr.take().expect("child stderr piped");
    let guard = ChildGuard(child);

    // Parse the machine-readable address line from stdout (M1). Read line-by-line
    // so we stop as soon as the example reports its bound address.
    let mut reader = BufReader::new(stdout);
    let mut url = String::new();
    let mut captured_stdout = String::new();
    for _ in 0..200 {
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
        "example never printed PMCP_SQL_SERVER_ADDR=.\n--- captured stdout ---\n{captured_stdout}\n--- captured stderr ---\n{}",
        drain(stderr),
    );

    // Drive the live server over HTTP. Readiness-poll initialize with linear
    // backoff (20 attempts), mirroring parity_chinook.rs.
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned example");

    let mut ready = false;
    for attempt in 0..20u32 {
        if matches!(tester.test_initialize().await.status, TestStatus::Passed) {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(
        ready,
        "MCP initialize never succeeded against {url} (readiness timeout).\n--- captured stdout ---\n{captured_stdout}\n--- captured stderr ---\n{}",
        drain(stderr),
    );

    // tools/list must include the curated `list_books` tool.
    let list = tester.test_tools_list().await;
    assert!(
        matches!(list.status, TestStatus::Passed),
        "tools/list must succeed: {list:?}"
    );
    let tools = tester.get_tools().expect("tools cached after tools/list");
    assert!(
        tools.iter().any(|t| t.name == "list_books"),
        "tools/list must advertise `list_books`; saw: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // tools/call `list_books` must succeed (returns rows / structuredContent).
    let call = tester
        .test_tool("list_books", json!({ "limit": 5 }))
        .await
        .expect("test_tool must not error at the harness level");
    assert!(
        matches!(call.status, TestStatus::Passed),
        "tools/call `list_books` must succeed: {call:?}"
    );

    // Explicit drop reaps the subprocess (Drop also runs at scope end).
    drop(guard);
}

/// M4 line-count definition (SHARED with the plan):
///
/// The `main` body budget is the count of physical, non-blank, non-comment
/// **statement** lines between `main`'s opening `{` and the final `Ok(())`,
/// EXCLUDING:
///   - blank lines,
///   - lines whose trimmed start is `//`,
///   - the `fn`/attribute signature and the closing `}`,
///   - the terminal `Ok(())`,
///   - rustfmt method-chain CONTINUATION lines (trimmed start is `.`) and lone
///     closing-delimiter continuation lines (`)`, `);`, `)?;`, `},` etc.) — a
///     statement rustfmt wraps across several physical lines counts ONCE.
///
/// Budget is 15.
#[test]
fn example_body_is_at_most_15_lines() {
    let src = include_str!("../examples/sql_server_http.rs");

    // Locate `async fn main` and the opening brace of its body.
    let main_idx = src
        .find("async fn main")
        .expect("example must define `async fn main`");
    let body_open = src[main_idx..]
        .find('{')
        .map(|o| main_idx + o + 1)
        .expect("main signature must have an opening brace");
    let body = &src[body_open..];

    let mut count = 0usize;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "Ok(())" {
            break; // terminal expression ends the body budget
        }
        if line == "}" {
            break; // closing brace of main (defensive; Ok(()) precedes it)
        }
        // Skip rustfmt continuation lines of a single statement.
        if line.starts_with('.') {
            continue;
        }
        if is_closing_continuation(line) {
            continue;
        }
        count += 1;
    }

    assert!(
        count <= 15,
        "main body has {count} statement lines (M4 def); budget is 15"
    );
}

/// A lone closing-delimiter continuation produced by rustfmt wrapping a single
/// statement across lines (e.g. `)`, `);`, `)?;`, `},`, `)?,`). These are not
/// independent statements and must not count toward the M4 budget.
fn is_closing_continuation(line: &str) -> bool {
    matches!(
        line,
        ")" | ");" | ")?;" | ")?," | ")," | "}," | "})" | "});"
    )
}
