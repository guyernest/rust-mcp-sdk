//! Phase 79 Wave 0: integration tests for `--format=json` on
//! `cargo pmcp test {check, conformance, apps}`.
//!
//! These tests exercise the JSON output contract via assert_cmd against the
//! built `cargo-pmcp` binary. Tests that require a passing-mock MCP server
//! are deferred to Plan 79-03 (which will introduce a reusable in-process
//! MCP fixture for the verifier's own integration tests). The tests here
//! cover all paths reachable WITHOUT a working MCP server fixture:
//!
//! - Clap-parse-time validation of `--format` values (Test 2.9)
//! - InfraError JSON emission against an unreachable URL (Test 2.2 partial,
//!   2.11 partial — covers exit code 2 path)
//! - JSON-mode stdout cleanliness (Test 2.10) — single document, no ANSI
//!   leakage when the InfraError path triggers
//! - Pretty-mode default preservation (Test 2.7, 2.8) — `--format=pretty`
//!   and no flag both produce non-JSON output (no PostDeployReport leakage)
//!
//! Tests requiring a PASSING mock MCP server (Tests 2.1, 2.3-2.6) are
//! deferred to Plan 79-03's fixture work — the JSON paths exercised below
//! cover the same serde + PostDeployReport construction logic, and the
//! mcp-tester unit tests in `post_deploy_report.rs` cover the wire-format
//! contract.

use assert_cmd::Command;
use mcp_tester::post_deploy_report::{PostDeployReport, TestCommand as PdrCommand, TestOutcome};

/// Unreachable URL — port 1 on localhost is reserved (TCPMUX) and ~always
/// produces a connection-refused error very quickly. Used to drive the
/// InfraError path of every subcommand without spawning a fixture.
const UNREACHABLE_URL: &str = "http://127.0.0.1:1/mcp";

/// Test 2.9: clap rejects `--format=banana` with an error mentioning the
/// possible values. Locks the value-enum at parse time.
#[test]
fn check_format_unknown_value_rejected_at_clap_parse() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "check", "http://x", "--format=banana"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("pretty") || stderr.contains("json"),
        "clap error must list the valid format values; stderr was:\n{stderr}"
    );
}

/// Test 2.9 (mirror): same for `cargo pmcp test conformance`.
#[test]
fn conformance_format_unknown_value_rejected_at_clap_parse() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "conformance", "http://x", "--format=banana"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("pretty") || stderr.contains("json"),
        "clap error must list the valid format values; stderr was:\n{stderr}"
    );
}

/// Test 2.9 (mirror): same for `cargo pmcp test apps`.
#[test]
fn apps_format_unknown_value_rejected_at_clap_parse() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "apps", "http://x", "--format=banana"])
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("pretty") || stderr.contains("json"),
        "clap error must list the valid format values; stderr was:\n{stderr}"
    );
}

/// Test 2.2 / 2.10 / 2.11 partial: against an unreachable URL,
/// `cargo pmcp test check --format=json` MUST:
///   - exit code 1 (TestFailed) — `run_quick_test` surfaces network errors
///     as failed `TestResult`s INSIDE the report (not as `Err(...)` from
///     the function). Exit code 2 (InfraError) is reserved for failures
///     that prevent the test from running at all (e.g., URL parse error,
///     auth resolve failure) — see `execute_json` in `check.rs`.
///   - stdout is exactly one parseable PostDeployReport JSON document
///   - report.command == Check
///   - report.outcome == TestFailed
///   - report.url matches the supplied URL
///   - report.failures contains at least one entry with a `reproduce`
///     starting with `cargo pmcp test check`
#[test]
fn check_format_json_emits_test_failed_for_unreachable_url() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "check", UNREACHABLE_URL, "--format=json"])
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(1),
        "exit code must be 1 (TestFailed) when run_quick_test reports failed connectivity; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout must be UTF-8");
    let pdr: PostDeployReport =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!("stdout must be a valid PostDeployReport JSON document; serde error: {e}\nstdout:\n{stdout}")
        });
    assert_eq!(pdr.command, PdrCommand::Check);
    assert_eq!(pdr.outcome, TestOutcome::TestFailed);
    assert_eq!(pdr.url, UNREACHABLE_URL);
    assert_eq!(pdr.schema_version, "1");
    assert!(
        !pdr.failures.is_empty(),
        "TestFailed must populate failures with at least one detail"
    );
    let detail = &pdr.failures[0];
    assert!(
        detail.reproduce.starts_with("cargo pmcp test check"),
        "reproduce must include the bare check subcommand; got: {}",
        detail.reproduce
    );
}

/// Test 2.10 (mirror): JSON branch stdout must NOT contain ANSI escape codes
/// or any text outside the single JSON document.
#[test]
fn check_format_json_stdout_has_no_ansi_or_extra_text() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "check", UNREACHABLE_URL, "--format=json"])
        .env("CLICOLOR_FORCE", "0")
        .env("NO_COLOR", "1")
        .assert();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // No ANSI escape sequences (CSI starts with ESC).
    assert!(
        !stdout.contains('\x1b'),
        "stdout must not contain ANSI escapes; raw stdout:\n{stdout:?}"
    );
    // Trim must round-trip into exactly one PostDeployReport — proves no
    // banner/log lines bracket the JSON document.
    let _: PostDeployReport = serde_json::from_str(stdout.trim()).expect(
        "stdout must be exactly one parseable PostDeployReport JSON document — \
         no extra log/banner lines allowed in the JSON branch",
    );
}

/// Test 2.2 (conformance mirror): for an unreachable URL, conformance
/// follows the same trinary contract as `check` — the runner returns a
/// report with failed entries (not an `Err`), so the JSON branch emits
/// `TestFailed` (exit code 1), not `InfraError` (exit code 2).
#[test]
fn conformance_format_json_emits_test_failed_for_unreachable_url() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "pmcp",
            "test",
            "conformance",
            UNREACHABLE_URL,
            "--format=json",
        ])
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(1),
        "exit code must be 1 (TestFailed) on unreachable URL"
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout must be UTF-8");
    let pdr: PostDeployReport =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid PostDeployReport JSON");
    assert_eq!(pdr.command, PdrCommand::Conformance);
    assert_eq!(pdr.outcome, TestOutcome::TestFailed);
    assert_eq!(pdr.url, UNREACHABLE_URL);
    assert_eq!(pdr.schema_version, "1");
    assert!(
        pdr.summary.is_some(),
        "conformance JSON branch always populates summary (5-bucket TestSummary)"
    );
    assert!(!pdr.failures.is_empty());
    assert!(pdr
        .failures
        .iter()
        .any(|f| f.reproduce.starts_with("cargo pmcp test conformance")));
    assert!(pdr
        .failures
        .iter()
        .any(|f| f.reproduce.contains("--domain")));
}

/// Test 2.5 + 2.11 (apps mirror): same InfraError contract for the apps
/// subcommand. The `mode` field is set to the parsed mode string even on
/// the InfraError path so consumers can correlate the failure with the
/// invocation.
#[test]
fn apps_format_json_emits_infra_error_for_unreachable_url() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "pmcp",
            "test",
            "apps",
            UNREACHABLE_URL,
            "--mode",
            "claude-desktop",
            "--format=json",
        ])
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit code must be 2 (InfraError) on unreachable URL; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout must be UTF-8");
    let pdr: PostDeployReport =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid PostDeployReport JSON");
    assert_eq!(pdr.command, PdrCommand::Apps);
    assert_eq!(pdr.outcome, TestOutcome::InfraError);
    assert_eq!(pdr.url, UNREACHABLE_URL);
    assert_eq!(pdr.schema_version, "1");
    // Note: the InfraError path constructed via emit_infra_error_json sets
    // mode: None at the producer side; the apps subcommand triggers it
    // BEFORE parsing the mode through to the report. That preserves the
    // contract that mode is always Some on the success/test-failed paths
    // for `apps`. Verifier consumers MUST be defensive against mode=None
    // on InfraError.
    assert!(pdr
        .failures
        .iter()
        .any(|f| f.reproduce.starts_with("cargo pmcp test apps")));
}

/// Test 2.7 / 2.8: pretty mode is the default. With NO `--format` flag,
/// stdout MUST NOT contain a `"schema_version"` field (which would indicate
/// the JSON branch fired by accident). Same when `--format=pretty` is
/// explicitly passed.
#[test]
fn check_default_format_does_not_emit_post_deploy_json() {
    // Drive the InfraError path so we get deterministic stderr + non-zero
    // exit, but verify stdout does NOT contain JSON markers.
    for args in [
        vec!["pmcp", "test", "check", UNREACHABLE_URL],
        vec!["pmcp", "test", "check", UNREACHABLE_URL, "--format=pretty"],
    ] {
        let assert = Command::cargo_bin("cargo-pmcp")
            .expect("cargo-pmcp binary must be available")
            .args(&args)
            .assert();
        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        assert!(
            !stdout.contains("\"schema_version\""),
            "pretty mode (default) must NOT emit PostDeployReport JSON; stdout was:\n{stdout}"
        );
        assert!(
            !stdout.contains("\"command\": \"check\""),
            "pretty mode (default) must NOT emit PostDeployReport JSON; stdout was:\n{stdout}"
        );
    }
}

/// Test 2.7 / 2.8 (conformance mirror).
#[test]
fn conformance_default_format_does_not_emit_post_deploy_json() {
    for args in [
        vec!["pmcp", "test", "conformance", UNREACHABLE_URL],
        vec![
            "pmcp",
            "test",
            "conformance",
            UNREACHABLE_URL,
            "--format=pretty",
        ],
    ] {
        let assert = Command::cargo_bin("cargo-pmcp")
            .expect("cargo-pmcp binary must be available")
            .args(&args)
            .assert();
        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        assert!(
            !stdout.contains("\"schema_version\""),
            "pretty mode (default) must NOT emit PostDeployReport JSON; stdout was:\n{stdout}"
        );
    }
}

/// Test 2.7 / 2.8 (apps mirror).
#[test]
fn apps_default_format_does_not_emit_post_deploy_json() {
    for args in [
        vec!["pmcp", "test", "apps", UNREACHABLE_URL],
        vec!["pmcp", "test", "apps", UNREACHABLE_URL, "--format=pretty"],
    ] {
        let assert = Command::cargo_bin("cargo-pmcp")
            .expect("cargo-pmcp binary must be available")
            .args(&args)
            .assert();
        let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
        assert!(
            !stdout.contains("\"schema_version\""),
            "pretty mode (default) must NOT emit PostDeployReport JSON; stdout was:\n{stdout}"
        );
    }
}

/// Smoke check: `--help` lists the new `--format` arg with its possible
/// values. Locks the user-facing help contract for downstream docs.
#[test]
fn check_help_lists_format_with_possible_values() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["pmcp", "test", "check", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("--format"),
        "--help must mention --format; got:\n{stdout}"
    );
    assert!(
        stdout.contains("pretty") && stdout.contains("json"),
        "--help must list pretty + json possible values; got:\n{stdout}"
    );
}
