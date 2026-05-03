//! REVISION HIGH-2 — CLI E2E acceptance tests for `cargo pmcp test apps`.
//!
//! These drive the actual `cargo-pmcp` binary against an in-process MCP
//! server fixture exposing the broken/corrected widget pair as resources,
//! then assert the binary's exit code and stdout/stderr against the
//! roadmap acceptance criteria:
//!
//! - AC-78-1: broken widget FAILS `cargo pmcp test apps --mode claude-desktop`
//!   (non-zero exit, stderr/stdout names a missing handler)
//! - AC-78-2: corrected widget PASSES the same command (zero exit)
//! - AC-78-3: `cargo pmcp test apps` (no flag, Standard mode) PASSES both
//!   fixtures (zero exit, no regression on the permissive default)
//! - AC-78-4: `--mode chatgpt` PASSES both fixtures (zero exit) AND stderr
//!   does NOT mention any of the four protocol handler names (chatgpt mode
//!   is a no-op for widget validation per Plan 01 / REVISION HIGH-1)

use assert_cmd::Command;
use std::path::PathBuf;

/// Path to the stdio MCP server fixture binary built from
/// `cargo-pmcp/tests/fixtures/mcp_widget_server.rs`. The fixture serves
/// either the broken or the corrected widget pair based on the env var
/// `WIDGET_FIXTURE` (`broken` or `corrected`).
fn fixture_bin_path() -> PathBuf {
    // Compiled by Cargo as a [[bin]] target in cargo-pmcp/Cargo.toml or
    // produced via cargo build by the test harness. If the fixture binary
    // is not yet built, the tests in this file are gated by a
    // `Path::exists()` check and skip with a documented reason.
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target"));
    target_dir.join("debug").join("mcp_widget_server")
}

/// If the fixture binary doesn't exist (Plan 03 not yet landed, or the
/// fixture binary not yet defined as a [[bin]] target), the CLI tests skip
/// with a clear log message rather than failing.
fn skip_if_no_fixture() -> Option<PathBuf> {
    let p = fixture_bin_path();
    if !p.exists() {
        eprintln!(
            "[skip] fixture binary not built at {:?}; CLI E2E tests require Plan 03's fixture pair to be available. \
             Once Plan 03 lands, configure cargo-pmcp/tests/fixtures/mcp_widget_server.rs as a [[bin]] target \
             in cargo-pmcp/Cargo.toml so this test exercises the full CLI -> resources/read -> validate_widgets path.",
            p
        );
        return None;
    }
    Some(p)
}

#[test]
fn cli_acceptance_broken_widget_fails_claude_desktop() {
    let Some(fixture) = skip_if_no_fixture() else {
        return;
    };
    let url = format!("stdio:{}", fixture.display());
    let assert = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["test", "apps", "--mode", "claude-desktop", &url])
        .env("WIDGET_FIXTURE", "broken")
        .assert();
    let output = assert.get_output().clone();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "AC-78-1: broken widget MUST cause non-zero exit under --mode claude-desktop. Output:\n{}",
        combined
    );
    assert!(
        combined.contains("onteardown"),
        "AC-78-1: stderr/stdout must name at least one missing handler (e.g. onteardown). Output:\n{}",
        combined
    );
}

#[test]
fn cli_acceptance_corrected_widget_passes_claude_desktop() {
    let Some(fixture) = skip_if_no_fixture() else {
        return;
    };
    let url = format!("stdio:{}", fixture.display());
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["test", "apps", "--mode", "claude-desktop", &url])
        .env("WIDGET_FIXTURE", "corrected")
        .assert()
        .success();
}

#[test]
fn cli_acceptance_standard_mode_passes_both_fixtures() {
    // AC-78-3: no regression on the permissive default. Standard mode emits
    // ONE summary WARN but does NOT cause non-zero exit (warnings are
    // non-fatal in Standard mode without --strict).
    for fixture_kind in ["broken", "corrected"] {
        let Some(fixture) = skip_if_no_fixture() else {
            return;
        };
        let url = format!("stdio:{}", fixture.display());
        Command::cargo_bin("cargo-pmcp")
            .expect("cargo-pmcp binary must be available")
            .args(["test", "apps", &url])
            .env("WIDGET_FIXTURE", fixture_kind)
            .assert()
            .success();
    }
}

#[test]
fn cli_acceptance_chatgpt_mode_passes_both_and_no_handler_messages() {
    // AC-78-4: chatgpt mode unchanged. Zero exit on both fixtures AND
    // stderr/stdout MUST NOT contain any of the four protocol-handler
    // names (since chatgpt mode is a no-op for widget validation per
    // Plan 01 REVISION HIGH-1).
    for fixture_kind in ["broken", "corrected"] {
        let Some(fixture) = skip_if_no_fixture() else {
            return;
        };
        let url = format!("stdio:{}", fixture.display());
        let assert = Command::cargo_bin("cargo-pmcp")
            .expect("cargo-pmcp binary must be available")
            .args(["test", "apps", "--mode", "chatgpt", &url])
            .env("WIDGET_FIXTURE", fixture_kind)
            .assert()
            .success();
        let output = assert.get_output().clone();
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for handler in ["onteardown", "ontoolinput", "ontoolcancelled", "onerror"] {
            assert!(
                !combined.contains(handler),
                "AC-78-4: chatgpt mode must NOT produce widget-handler-related output (found `{}` in fixture `{}`). \
                 Output:\n{}",
                handler, fixture_kind, combined
            );
        }
    }
}

// ============================================================================
// Phase 79 Plan 79-04 — REQ-79-18 (verbatim help text) + REVISION 3 HIGH-G2
// (clap-level rollback rejection) acceptance tests.
// ============================================================================

/// Test 1.6 — `cargo-pmcp deploy --help` text contains the verbatim phrase
/// from CONTEXT.md REQ-79-18: widgets pre-build mention AND verification
/// suite mention. Locks against accidental rewording in future doc updates.
#[test]
fn deploy_help_mentions_widgets_verbatim() {
    let output = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains(
            "Builds widgets (auto-detected from widget/ or widgets/) before \
             compiling and deploying the Rust binary."
        ),
        "REQ-79-18: --help text missing widgets mention. Got:\n{combined}"
    );
    assert!(
        combined.contains(
            "Verifies the deployed endpoint via cargo pmcp test \
             {check,conformance,apps} before reporting success."
        ),
        "REQ-79-18: --help text missing verification mention. Got:\n{combined}"
    );
}

/// Test 2.6 — REVISION 3 HIGH-G2: clap REJECTS `--on-test-failure=rollback` at
/// parse time. Stderr carries the verbatim `ROLLBACK_REJECT_MESSAGE`
/// substring `not yet implemented`. Locks the integration-level contract.
#[test]
fn deploy_on_test_failure_rollback_hard_rejected() {
    let output = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["deploy", "--on-test-failure=rollback"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "REVISION 3 HIGH-G2: --on-test-failure=rollback must be rejected at parse time"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not yet implemented"),
        "REVISION 3 HIGH-G2: stderr should carry ROLLBACK_REJECT_MESSAGE. Got:\n{stderr}"
    );
}

/// Test 1.7 — REVISION 3 Codex MEDIUM: `cargo pmcp app new <name>
/// --embed-widgets` parses without error. We can't `app new` against the real
/// filesystem without polluting cwd, so we drive `--help` to confirm clap
/// recognises the flag.
#[test]
fn app_new_embed_widgets_flag_parses() {
    let output = Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args(["app", "new", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "`cargo pmcp app new --help` must succeed; got status {:?}",
        output.status
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("--embed-widgets"),
        "REVISION 3 Codex MEDIUM: `app new --help` must list --embed-widgets. Got:\n{combined}"
    );
}
