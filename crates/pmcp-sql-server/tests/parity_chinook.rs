//! REF-02 / SC-3 / SC-4 — the binding **reference-parity** assertion.
//!
//! This test reproduces the Shape A server **through the REAL `--config`
//! `--schema` binary path** (NOT a hand-built / injected connector) and replays
//! the production `generated.yaml` scenario suite against it, asserting every
//! scenario passes.
//!
//! # Why the REAL binary path (Codex HIGH #5)
//!
//! The prior draft proved *assembly* by injecting an in-memory connector into
//! `build_server`. That does NOT prove the pure-config binary (SC-1's "zero
//! Rust" claim). This test instead:
//!
//! 1. copies the vendored DATA-BEARING `tests/fixtures/chinook.db` to a writable
//!    tempfile,
//! 2. writes a temp `config.toml` — the vendored `reference-config.toml` with its
//!    `[database] file_path` overridden to point at the temp DB copy,
//! 3. writes the vendored `chinook.ddl` to a temp `--schema` file,
//! 4. constructs a programmatic [`Args`] (clap-free) and invokes the REAL
//!    [`pmcp_sql_server::run_serving`] entry point — the same path
//!    `cargo run -- --config X --schema Y` takes: `ServerConfig::from_toml*`
//!    → `dispatch` (the `[database] type → connector` seam reading `file_path`)
//!    → `build_server` → `StreamableHttpServer`,
//! 5. replays `generated.yaml` via the `mcp-tester` library and asserts
//!    `result.success`.
//!
//! # What the 29 scenarios prove
//!
//! - **Curated tools on REAL data (REVIEW FIX #1):** `search_tracks` returns
//!   "Rock"/"AC/DC", `get_album_tracks` returns
//!   "For Those About To Rock (We Salute You)", `list_artists` returns "AC/DC" —
//!   value assertions that ONLY pass because `chinook.db` is data-bearing.
//! - **Code-mode policy enforcement (SC-3):** the `validate_code` DELETE / DDL /
//!   `DROP` / no-LIMIT `failure` assertions + the `execute_code` invalid-token
//!   `failure` confirm the static `[code_mode]` policy REJECTS writes / DDL /
//!   forged tokens end-to-end through HTTP.
//! - **All 3 resources (REVIEW FIX #2):** `docs://chinook/schema`,
//!   `docs://chinook/examples`, `code-mode://learnings` all resolve.
//! - **The configured prompt (REVIEW FIX #3):** `start_code_mode` resolves.
//!
//! Passing them all = result parity (SC-4) AND code-mode policy parity (SC-3).
//!
//! # REF-02 scope note (D-01)
//!
//! REF-02's literal "open-images" wording is intentionally satisfied by the
//! SQLite **Chinook** reference that OWNS the `generated.yaml` scenarios — the
//! data-bearing, offline-runnable reference vendored in Plan 03. This is the
//! D-01-approved scope reading (the parity contract lives with Chinook), not a
//! gap.
//!
//! Run with (single-threaded — ephemeral port + per-process env):
//! ```sh
//! cargo test -p pmcp-sql-server --no-default-features --features sqlite \
//!   --test parity_chinook -- --test-threads=1
//! ```

#![cfg(feature = "sqlite")]

use std::time::Duration;

use mcp_tester::{ScenarioExecutor, ServerTester, TestScenario};
use pmcp_sql_server::{run_serving, Args};

/// The token secret the `${CODE_MODE_SECRET}` placeholder in the Chinook config
/// expands to at code-mode wiring time. Must be >= 16 bytes (V6 minimum).
const CODE_MODE_SECRET: &str = "parity-chinook-code-mode-secret-32b";

/// The vendored Lambda-style `file_path` line the temp config overrides.
const LAMBDA_FILE_PATH_LINE: &str = "file_path = \"/var/task/assets/chinook.db\"";

/// The policy-rejection scenarios that MUST individually gate this test.
///
/// # Why per-step gating (VERIFICATION Gap 2 — test validity)
///
/// Each of these scenarios carries `continue_on_failure: true` and asserts
/// `type: failure` (the policy must REJECT the operation). `mcp-tester` computes
/// `ScenarioResult.success` (see `crates/mcp-tester/src/scenario_executor.rs`
/// lines 111-118) by EXCLUDING every `continue_on_failure` step:
///
/// ```text
/// let success = scenario_error.is_none()
///     && step_results.iter().all(|r| {
///         r.success
///             || scenario.steps.iter()
///                 .any(|step| step.name == r.step_name && step.continue_on_failure)
///     });
/// ```
///
/// So a genuinely-failing rejection scenario (e.g. the no-LIMIT
/// `SELECT * FROM Artist` before Gap 1's `require_limit` fix landed) is silently
/// dropped from the aggregate — `assert!(result.success)` stays green even when
/// every policy rejection regresses. That makes the SC-3 negative-path parity
/// proof NON-gating.
///
/// The fix lives HERE in the test, not in the fixtures: we assert every
/// `StepResult.success` is `true`. `StepResult.success` is the per-step truth
/// (all of that step's assertions passed) and is computed BEFORE the
/// `continue_on_failure` exclusion, so iterating `result.step_results` gives a
/// `continue_on_failure`-INDEPENDENT gate. A presence guard (below) then ensures
/// a rejection scenario cannot silently vanish from `generated.yaml` and make the
/// per-step gate trivially pass.
///
/// DO NOT "simplify" the assertions back to `assert!(result.success)` — that
/// reintroduces Gap 2.
const REQUIRED_REJECTION_SCENARIOS: &[&str] = &[
    "Validate: DELETE should be rejected",
    "Validate: DDL (CREATE TABLE) should be rejected",
    "Validate: DROP TABLE should be rejected",
    "Validate: SELECT without LIMIT should be rejected",
    "Execute: with invalid token (should fail)",
];

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Build a temp `config.toml` from the vendored reference config with its
/// `[database] file_path` overridden to point at `db_path` (a writable copy of
/// the data-bearing fixture). String-replace (rather than parse→reserialize)
/// keeps the large embedded markdown resource content byte-identical to the
/// production reference — only the one `file_path` line changes.
fn temp_config_with_db(db_path: &std::path::Path) -> String {
    let reference = std::fs::read_to_string(fixtures_dir().join("reference-config.toml"))
        .expect("read reference-config.toml");
    assert!(
        reference.contains(LAMBDA_FILE_PATH_LINE),
        "reference-config.toml must contain the Lambda file_path line to override"
    );
    let overridden = format!("file_path = \"{}\"", db_path.display());
    reference.replace(LAMBDA_FILE_PATH_LINE, &overridden)
}

#[tokio::test]
async fn chinook_reference_parity_through_real_binary_path() {
    // V6: the Chinook config's token_secret = "${CODE_MODE_SECRET}" is resolved
    // at code-mode wiring time — set it (>= 16 bytes) BEFORE spawning.
    std::env::set_var("CODE_MODE_SECRET", CODE_MODE_SECRET);

    let dir = fixtures_dir();

    // (1) Copy the DATA-BEARING chinook.db to a writable tempfile so the connector
    // has a real, populated, writable DB (INSERT validate-only scenarios still
    // never mutate it, but a writable path is the production shape).
    let tmp = tempfile::tempdir().expect("create tempdir");
    let db_copy = tmp.path().join("chinook.db");
    std::fs::copy(dir.join("chinook.db"), &db_copy).expect("copy data-bearing chinook.db");

    // (2) Temp config.toml: the reference config with file_path → temp DB copy.
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, temp_config_with_db(&db_copy)).expect("write temp config.toml");

    // (3) Temp --schema: the vendored chinook.ddl.
    let schema_path = tmp.path().join("chinook.ddl");
    std::fs::copy(dir.join("chinook.ddl"), &schema_path).expect("copy chinook.ddl");

    // (4) The REAL binary path: programmatic Args → run_serving (ServerConfig::from_toml*
    // → dispatch reads file_path → build_server → StreamableHttpServer). Ephemeral
    // loopback port; capture the REAL bound addr.
    let args = Args {
        config: config_path,
        schema: schema_path,
        http: "127.0.0.1:0".to_string(),
    };
    let (bound, handle) = run_serving(&args)
        .await
        .expect("REAL --config --schema binary path must assemble + serve");

    // (5) Replay generated.yaml via the mcp-tester library against the live HTTP
    // server. Poll readiness via test_initialize() with backoff (it sets up the
    // reusable pmcp client the executor needs) before executing the scenario.
    let url = format!("http://{bound}");
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,        // insecure
        None,         // api_key
        Some("http"), // force_transport
        None,         // http_middleware_chain
    )
    .expect("construct ServerTester for the spawned HTTP server");

    let mut initialized = false;
    for attempt in 0..20u32 {
        let result = tester.test_initialize().await;
        if matches!(result.status, mcp_tester::report::TestStatus::Passed) {
            initialized = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50 * u64::from(attempt + 1))).await;
    }
    assert!(
        initialized,
        "MCP initialize must succeed against the spawned server (readiness)"
    );

    let scenario = TestScenario::from_file(dir.join("generated.yaml"))
        .expect("load the 29-scenario parity contract");

    let mut exec = ScenarioExecutor::new(&mut tester, true /* detailed */);
    let result = exec
        .execute(scenario)
        .await
        .expect("scenario execution must complete without a harness error");

    // SC-4 (result parity) + SC-3 (code-mode policy parity, via the failure
    // assertions). We do NOT gate on `result.success`: it EXCLUDES every
    // `continue_on_failure` step (scenario_executor.rs:111-118), which would mask
    // a regressed policy rejection (VERIFICATION Gap 2). Instead we gate on EVERY
    // `step_results[i].success` — the per-step truth, computed before that
    // exclusion — so each rejection scenario is individually binding.

    // (a) PRESENCE GUARD: every required policy-rejection scenario must still be
    // present in generated.yaml. Without this, deleting/renaming a rejection
    // scenario would silently shrink the suite and make the per-step gate (b)
    // trivially pass. `StepResult.step_name` is set verbatim from the scenario
    // step's `name` (scenario_executor.rs:154), so we match on it directly.
    let present_names: Vec<&str> = result
        .step_results
        .iter()
        .map(|s| s.step_name.as_str())
        .collect();
    let missing_rejections: Vec<&str> = REQUIRED_REJECTION_SCENARIOS
        .iter()
        .copied()
        .filter(|name| !present_names.contains(name))
        .collect();
    assert!(
        missing_rejections.is_empty(),
        "policy-rejection scenarios must be present in generated.yaml so they cannot \
         silently disappear (VERIFICATION Gap 2). Missing: {missing_rejections:#?}. \
         Present steps: {present_names:#?}",
    );

    // (b) PER-STEP GATE: every step — INCLUDING the `continue_on_failure`
    // rejection scenarios — must have passed all of its own assertions. A
    // rejection scenario whose `type: failure` assertion did NOT fire (the exact
    // Gap-2 masking, e.g. the pre-fix no-LIMIT `SELECT * FROM Artist`) now fails
    // the test instead of being dropped from the aggregate.
    let failed: Vec<_> = result
        .step_results
        .iter()
        .filter(|s| !s.success)
        .map(|s| {
            (
                &s.step_name,
                &s.error,
                s.assertion_results
                    .iter()
                    .filter(|a| !a.passed)
                    .map(|a| (&a.assertion, &a.actual_value, &a.message))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert!(
        failed.is_empty(),
        "every reference-parity step must pass its own assertions — rejection \
         scenarios are individually gating, so a `continue_on_failure` step that \
         failed its `type: failure` assertion is no longer masked (VERIFICATION \
         Gap 2). {}/{} steps completed; error={:?}; failed steps={failed:#?}",
        result.steps_completed,
        result.steps_total,
        result.error,
    );

    handle.abort();
}
