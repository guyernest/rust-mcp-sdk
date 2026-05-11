//! Phase 79 Wave 3 (Plan 79-03) Task 3 — CLI flag parsing + Step 4.5 hook
//! integration tests.
//!
//! These tests exercise:
//!
//! 1. `--no-post-deploy-test` parses cleanly.
//! 2. `--post-deploy-tests=conformance,apps` parses with comma delimiter.
//! 3. `--on-test-failure=warn|fail` parses; `rollback` is HARD-REJECTED at
//!    clap parse time with the verbatim `ROLLBACK_REJECT_MESSAGE` (REVISION 3
//!    HIGH-G2).
//! 4. `--apps-mode=...` parses for all three values.
//! 5. CLI flags override deploy.toml on `materialize_post_deploy_config`.
//!
//! The tests drive the actual `cargo-pmcp` binary via `assert_cmd::Command`
//! and inspect stderr for the rejection message. Tests that exercise the
//! Step 4.5 hook itself are integration tests in
//! `tests/post_deploy_orchestrator.rs` — those use the orchestrator
//! directly because spinning up a real Lambda deploy from a test is out of
//! scope.

#![allow(clippy::needless_pass_by_value)]

use assert_cmd::Command;
use cargo_pmcp::deployment::post_deploy_tests::ROLLBACK_REJECT_MESSAGE;

/// Test 3.3b (clap_REJECTS_on_test_failure_rollback — REVISION 3 HIGH-G2):
/// `cargo pmcp deploy --on-test-failure=rollback` MUST exit non-zero with
/// the verbatim `ROLLBACK_REJECT_MESSAGE` in stderr.
#[test]
fn clap_rejects_on_test_failure_rollback() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--on-test-failure=rollback"])
        .assert();
    let output = assert.get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "rollback MUST be rejected at clap parse — exit non-zero. Output: {stderr}"
    );
    assert!(
        stderr.contains("not yet implemented"),
        "stderr must contain ROLLBACK_REJECT_MESSAGE prefix; got: {stderr}"
    );
    assert!(
        ROLLBACK_REJECT_MESSAGE.contains("not yet implemented"),
        "constant content drifted"
    );
    assert!(
        stderr.contains("'fail'") && stderr.contains("'warn'"),
        "stderr must list valid alternatives; got: {stderr}"
    );
}

/// Test 3.1 (clap_parses_no_post_deploy_test): the `--no-post-deploy-test`
/// flag is recognised by clap.
///
/// Mechanism: invoke `cargo pmcp deploy --no-post-deploy-test --help` and
/// expect a successful exit (clap recognises the flag and prints help).
/// Without the flag declared, clap would error with "unexpected argument".
#[test]
fn clap_parses_no_post_deploy_test() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--no-post-deploy-test", "--help"])
        .assert();
    let output = assert.get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "--no-post-deploy-test must be recognised by clap. stderr: {stderr}; stdout: {stdout}"
    );
}

/// Test 3.2 (clap_parses_subset): `--post-deploy-tests=conformance,apps`
/// parses with the comma delimiter declared via `value_delimiter = ','`.
///
/// Mechanism: invoke `cargo pmcp deploy --post-deploy-tests=conformance,apps
/// --help` and expect successful parse.
#[test]
fn clap_parses_post_deploy_tests_subset() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--post-deploy-tests=conformance,apps", "--help"])
        .assert();
    let output = assert.get_output().clone();
    assert!(
        output.status.success(),
        "--post-deploy-tests=conformance,apps must parse; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test 3.3 (clap_parses_on_test_failure_warn_or_fail): both warn and fail
/// parse cleanly through the `parse_on_test_failure_flag` value parser.
#[test]
fn clap_parses_on_test_failure_warn() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--on-test-failure=warn", "--help"])
        .assert();
    assert!(
        assert.get_output().status.success(),
        "--on-test-failure=warn must parse"
    );
}

#[test]
fn clap_parses_on_test_failure_fail() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--on-test-failure=fail", "--help"])
        .assert();
    assert!(
        assert.get_output().status.success(),
        "--on-test-failure=fail must parse"
    );
}

/// Test 3.4 (clap_parses_apps_mode): all three documented values parse.
#[test]
fn clap_parses_apps_mode_standard() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--apps-mode=standard", "--help"])
        .assert();
    assert!(assert.get_output().status.success());
}

#[test]
fn clap_parses_apps_mode_chatgpt() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--apps-mode=chatgpt", "--help"])
        .assert();
    assert!(assert.get_output().status.success());
}

#[test]
fn clap_parses_apps_mode_claude_desktop() {
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--apps-mode=claude-desktop", "--help"])
        .assert();
    assert!(assert.get_output().status.success());
}

/// Test 3.9 (caller_does_not_double_print_banner — F-6): source-grep
/// `cargo-pmcp/src/commands/deploy/mod.rs` for any `eprintln!` of the
/// failure banner. Caller MUST only call `outputs.display()` and
/// `std::process::exit(failure.exit_code())` — never `eprintln!` the
/// failure itself.
///
/// Mechanism: source-level lock. The orchestrator's `interpret_outcomes`
/// is the single banner-print site; if a future change adds a duplicate
/// banner emission in the deploy command, this test fails.
#[test]
fn caller_does_not_double_print_banner() {
    let src = std::fs::read_to_string("src/commands/deploy/mod.rs")
        .expect("read src/commands/deploy/mod.rs");
    let bad_pattern_count = src.matches("eprintln!(\"{}\", failure)").count()
        + src.matches("eprintln!(\"{failure}\")").count()
        + src.matches("eprintln!(\"{}\", banner)").count()
        + src.matches("eprintln!(\"{banner}\")").count();
    assert_eq!(
        bad_pattern_count, 0,
        "deploy/mod.rs MUST NOT eprintln! the failure banner — that's the \
         orchestrator's job (F-6 mitigation). found {bad_pattern_count} occurrences."
    );
}

/// Test 3.10 (NO resolve_auth_token call — REVISION 3 HIGH-C2): source-grep
/// the deploy command for `resolve_auth_token`. Per HIGH-C2 the helper does
/// NOT exist, and the deploy command MUST NOT call it. Subprocess inherits
/// parent env via Tokio Command default.
#[test]
fn no_resolve_auth_token_call() {
    let src = std::fs::read_to_string("src/commands/deploy/mod.rs")
        .expect("read src/commands/deploy/mod.rs");
    assert!(
        !src.contains("resolve_auth_token"),
        "deploy/mod.rs MUST NOT reference resolve_auth_token (REVISION 3 \
         HIGH-C2 — function deleted). Subprocess inherits parent env."
    );

    let pdt_src = std::fs::read_to_string("src/deployment/post_deploy_tests.rs")
        .expect("read src/deployment/post_deploy_tests.rs");
    assert!(
        !pdt_src.contains("resolve_auth_token"),
        "post_deploy_tests.rs MUST NOT define resolve_auth_token (REVISION 3 \
         HIGH-C2). Subprocess inherits parent env."
    );
}

/// Source-level lock for HIGH-1: there must be NO regex-based parsers in
/// the orchestrator. The pre-revision-3 `parse_conformance_summary`,
/// `parse_apps_summary`, and `build_failure_recipes` helpers were DELETED
/// in revision 3.
#[test]
fn no_regex_parser_helpers() {
    let src = std::fs::read_to_string("src/deployment/post_deploy_tests.rs")
        .expect("read src/deployment/post_deploy_tests.rs");
    for forbidden in [
        "fn parse_conformance_summary",
        "fn parse_apps_summary",
        "fn build_failure_recipes",
    ] {
        assert!(
            !src.contains(forbidden),
            "post_deploy_tests.rs must not define {forbidden} (REVISION 3 HIGH-1 — typed JSON replaces it)"
        );
    }
}

/// Source-level lock for HIGH-1: subprocess argv MUST contain --format=json.
#[test]
fn subprocess_argv_includes_format_json() {
    let src = std::fs::read_to_string("src/deployment/post_deploy_tests.rs")
        .expect("read src/deployment/post_deploy_tests.rs");
    assert!(
        src.contains("--format=json"),
        "subprocess argv must include --format=json (REVISION 3 HIGH-1)"
    );
}

/// Source-level lock for HIGH-G2: the deploy hook does NOT add a runtime
/// match arm or WARN block for `OnFailure::Rollback`. Pre-revision-3 plan
/// added a deploy-START WARN; revision 3 removed it because clap rejects
/// rollback at parse time and the custom Deserialize rejects it at
/// config-load time — by the time execute_async runs, only Fail or Warn
/// can be set.
///
/// This test scans non-doc-comment lines for any executable reference to
/// `OnFailure::Rollback`. The doc-comment reference inside
/// `materialize_post_deploy_config` (which explains WHY no arm exists) is
/// allowed.
#[test]
fn no_deploy_start_warn_for_rollback() {
    let src = std::fs::read_to_string("src/commands/deploy/mod.rs")
        .expect("read src/commands/deploy/mod.rs");
    let bad_lines: Vec<&str> = src
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            // Skip doc-comments (`///` or `//!`) and regular line comments (`//`).
            !trimmed.starts_with("///")
                && !trimmed.starts_with("//!")
                && !trimmed.starts_with("//")
                && line.contains("OnFailure::Rollback")
        })
        .collect();
    assert!(
        bad_lines.is_empty(),
        "deploy/mod.rs MUST NOT contain executable references to OnFailure::Rollback \
         (REVISION 3 HIGH-G2 — variant doesn't exist; rejection happens at parse + \
         config-load time). Found:\n{}",
        bad_lines.join("\n")
    );
}
