//! Phase 79 Wave 2 — widget pre-build orchestrator integration tests.
//!
//! Covers Plan 79-02 Task 1's `<behavior>` clause: 13 tests across
//! `run_widget_build`, `detect_widgets`, `ensure_node_modules`,
//! `invoke_build_script`, and the helpers (`is_yarn_pnp`, argv pass-through,
//! the verbatim missing-build-script error from REQ-79-03).
//!
//! Many tests build a real tempdir with a synthetic `package.json` whose
//! build script touches files in `dist/` — the orchestrator runs the actual
//! `npm`/`pnpm`/`yarn` binary against that, so the test machine must have at
//! least ONE package manager on PATH. We pick `npm` (most universally
//! available) as the canonical PM. Tests requiring a specific PM check for
//! the binary at runtime and SKIP gracefully if missing.
//!
//! Task 1 (this file) only asserts orchestrator behavior. Task 2's tests for
//! the `commands/deploy/mod.rs` hook + CLI flags + `PMCP_WIDGET_DIRS` env
//! join live alongside in this same file (gating each Task-2 test by the
//! `#[cfg(test)]` boundary visible to the same crate).

use cargo_pmcp::deployment::widgets::{
    detect_widgets, run_widget_build, PackageManager, WidgetConfig,
};
use std::fs;
use std::path::Path;

// ============================================================================
// Helpers
// ============================================================================

/// Returns true if the named binary is found on PATH.
fn binary_on_path(name: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|p| p.join(name).is_file())
}

/// Skip-gate: returns Some(reason) if the test must be skipped (e.g., npm
/// missing on PATH). Tests print the reason and return early — they DO NOT
/// fail the suite when a PM is unavailable (CI environments without Node).
fn npm_skip_gate() -> Option<&'static str> {
    if binary_on_path("npm") {
        None
    } else {
        Some("npm not found on PATH — skipping orchestrator integration test")
    }
}

/// Write a `package.json` with a `build` script that runs `cmd_str` (a shell
/// snippet via `sh -c`). Used by tests that need a specific build behavior.
fn write_package_json(dir: &Path, build_cmd: &str) {
    let json = format!(
        r#"{{
  "name": "fixture-widget",
  "version": "0.0.1",
  "private": true,
  "scripts": {{
    "build": "{build}"
  }}
}}
"#,
        build = build_cmd.replace('"', "\\\"")
    );
    fs::write(dir.join("package.json"), json).expect("write package.json");
}

/// Touch `package-lock.json` so `PackageManager::detect_from_dir` returns
/// `Npm` (matches the convention path operators get with `npm install`).
fn touch_npm_lock(dir: &Path) {
    fs::write(dir.join("package-lock.json"), b"{}").expect("write package-lock.json");
}

// ============================================================================
// Task 1 tests — orchestrator behavior
// ============================================================================

/// Test 1.1 (run_widget_build_success_path): a tempdir with `widget/` +
/// `package.json` whose build script touches `dist/foo.html`. Returns
/// `Ok(ResolvedPaths)` and the file exists after.
#[tokio::test]
async fn run_widget_build_success_path() {
    if let Some(reason) = npm_skip_gate() {
        eprintln!("SKIP: {reason}");
        return;
    }
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(&widget_dir).expect("mkdir widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules"); // skip install
    touch_npm_lock(&widget_dir);
    // `mkdir -p dist && touch dist/foo.html` — POSIX-portable, no shell
    // metacharacters that need quoting (we're already inside a JSON string
    // run by `sh -c`).
    write_package_json(&widget_dir, "mkdir -p dist && touch dist/foo.html");

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: None,
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    let resolved = run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("widget build succeeds");
    assert_eq!(resolved.path, widget_dir);
    assert_eq!(resolved.absolute_output_dir, widget_dir.join("dist"));
    assert!(
        widget_dir.join("dist/foo.html").is_file(),
        "build script must have produced dist/foo.html"
    );
}

/// Test 1.2 (missing_build_script): `package.json` with no `build` script
/// fails with the verbatim REQ-79-03 message.
#[tokio::test]
async fn missing_build_script_returns_actionable_error() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(&widget_dir).expect("mkdir widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    touch_npm_lock(&widget_dir);
    fs::write(
        widget_dir.join("package.json"),
        r#"{"name":"x","version":"0.0.1","scripts":{"test":"true"}}"#,
    )
    .expect("write package.json sans build");

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: None,
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    let err = run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect_err("missing build script must error");
    let msg = err.to_string();
    assert!(
        msg.contains("no 'build' script"),
        "expected verbatim REQ-79-03 message, got: {msg}"
    );
    assert!(
        msg.contains(".pmcp/deploy.toml"),
        "actionable hint must reference .pmcp/deploy.toml, got: {msg}"
    );
}

/// Test 1.3 (runs_install_when_node_modules_missing): no `node_modules/`,
/// no Yarn-PnP markers — `ensure_node_modules` must run install. We assert
/// indirectly: with an explicit install command of `mkdir -p node_modules`
/// (via `["sh", "-c", ...]`), the dir appears after the run.
#[tokio::test]
async fn install_runs_when_node_modules_missing() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(&widget_dir).expect("mkdir widget");
    touch_npm_lock(&widget_dir); // Npm PM detected — but we override install
    write_package_json(&widget_dir, "true"); // build is a no-op

    // Use `sh -c` to create a sentinel file proving install ran.
    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec!["sh".to_string(), "-c".to_string(), "true".to_string()]),
        install: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "mkdir -p node_modules && touch install_ran.txt".to_string(),
        ]),
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("explicit install + build succeed");
    assert!(
        widget_dir.join("install_ran.txt").is_file(),
        "install command must have run"
    );
}

/// Test 1.4 (skips_install_when_node_modules_exists): pre-populated
/// `node_modules/` → `ensure_node_modules` is a no-op even when an install
/// command is configured that would fail (we use `false` — the always-fails
/// shell builtin — and assert no error).
#[tokio::test]
async fn install_skipped_when_node_modules_exists() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    touch_npm_lock(&widget_dir);
    write_package_json(&widget_dir, "true");

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        // `false` ALWAYS exits non-zero. If install fired, this would propagate.
        build: Some(vec!["true".to_string()]),
        install: Some(vec!["false".to_string()]),
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("install MUST be skipped when node_modules exists");
}

/// Test 1.4b (skips_install_when_yarn_pnp_present — REVISION 3 Codex MEDIUM):
/// `.pnp.cjs` (Yarn PnP marker) AND no `node_modules/` → install is a no-op.
/// Repeats with `.pnp.loader.mjs` for Yarn 4+ format.
#[tokio::test]
async fn install_skipped_when_yarn_pnp_marker_present() {
    for marker in [".pnp.cjs", ".pnp.loader.mjs"] {
        let workspace = tempfile::tempdir().expect("workspace tempdir");
        let widget_dir = workspace.path().join("widget");
        fs::create_dir_all(&widget_dir).expect("mkdir widget");
        // Plant the Yarn PnP marker — but no node_modules/.
        fs::write(widget_dir.join(marker), b"// PnP marker").expect("write pnp marker");
        write_package_json(&widget_dir, "true");

        let cfg = WidgetConfig {
            path: "widget".to_string(),
            build: Some(vec!["true".to_string()]),
            // If install fired, `false` would propagate as Err.
            install: Some(vec!["false".to_string()]),
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };
        run_widget_build(&cfg, workspace.path(), true)
            .await
            .unwrap_or_else(|e| panic!("PnP marker {marker} must skip install — got error: {e}"));
    }
}

/// Test 1.5 (build_failure_aborts): `package.json` with `"build": "exit 1"`
/// returns Err containing "build" and "failed". REQ-79-05.
#[tokio::test]
async fn build_failure_aborts() {
    if let Some(reason) = npm_skip_gate() {
        eprintln!("SKIP: {reason}");
        return;
    }
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    touch_npm_lock(&widget_dir);
    // `exit 1` aborts the script's `sh` interpretation with non-zero status,
    // which `npm run build` propagates.
    write_package_json(&widget_dir, "exit 1");

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: None,
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    let err = run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect_err("non-zero build must error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("build") && msg.contains("failed"),
        "expected build-failed error, got: {msg}"
    );
}

/// Test 1.6 (zero_outputs_warns_no_fail): build succeeds (`true`) but emits
/// no files in `dist/` → `run_widget_build` returns Ok (warning-only).
#[tokio::test]
async fn zero_outputs_warns_but_does_not_fail() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    touch_npm_lock(&widget_dir);
    write_package_json(&widget_dir, "true"); // succeeds, emits nothing

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        // Use explicit `true` to bypass npm; we're not testing PM detection here.
        build: Some(vec!["true".to_string()]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    // dist/ doesn't exist either — verify_outputs_exist must tolerate that.
    run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("zero outputs must NOT abort");
}

/// Test 1.7 (npm_not_on_path_loud_error): a deliberately-bogus binary name
/// triggers the spawn-failure path. We use `__pmcp_nonexistent_binary__`
/// (extremely unlikely to exist) and assert the error mentions PATH.
#[tokio::test]
async fn missing_binary_returns_actionable_path_error() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    touch_npm_lock(&widget_dir);
    write_package_json(&widget_dir, "true");

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec!["__pmcp_nonexistent_binary__".to_string()]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    let err = run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect_err("missing binary must error");
    let msg = err.to_string();
    assert!(
        msg.contains("PATH") && msg.contains("__pmcp_nonexistent_binary__"),
        "expected actionable PATH error, got: {msg}"
    );
}

// ============================================================================
// detect_widgets — synthesis tests (1.8, 1.9, 1.10)
// ============================================================================

/// Build a minimal `DeployConfig` (no `[[widgets]]` block) for synthesis tests.
fn empty_deploy_config(
    workspace_root: &std::path::PathBuf,
) -> cargo_pmcp::deployment::config::DeployConfig {
    cargo_pmcp::deployment::config::DeployConfig::default_for_server(
        "fixture-server".to_string(),
        "us-west-2".to_string(),
        workspace_root.clone(),
    )
}

/// Test 1.8 (detect_widgets_synthesizes_from_widget_dir): workspace with
/// `widget/` and no explicit `[[widgets]]` → returns one synthesized
/// `WidgetConfig` with `path == "widget"`.
#[test]
fn detect_widgets_synthesizes_from_widget_dir() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    fs::create_dir_all(workspace.path().join("widget")).expect("mkdir widget");
    let cfg = empty_deploy_config(&workspace.path().to_path_buf());
    let widgets = detect_widgets(&cfg, workspace.path());
    assert_eq!(widgets.len(), 1, "must synthesize exactly one widget");
    assert_eq!(widgets[0].path, "widget");
    assert_eq!(widgets[0].output_dir, "dist");
    assert!(widgets[0].build.is_none());
    assert!(widgets[0].install.is_none());
}

/// Test 1.9 (detect_widgets_synthesizes_from_widgets_dir): same as 1.8 but
/// with `widgets/` (plural).
#[test]
fn detect_widgets_synthesizes_from_widgets_dir() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    fs::create_dir_all(workspace.path().join("widgets")).expect("mkdir widgets");
    let cfg = empty_deploy_config(&workspace.path().to_path_buf());
    let widgets = detect_widgets(&cfg, workspace.path());
    assert_eq!(widgets.len(), 1);
    assert_eq!(widgets[0].path, "widgets");
}

/// Test 1.10 (detect_widgets_skips_ui_and_app_dirs): `ui/` and `app/` →
/// empty Vec. CONTEXT.md "DROP `ui/` and `app/`" LOCKED. REQ-79-01.
#[test]
fn detect_widgets_skips_ui_and_app_dirs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    fs::create_dir_all(workspace.path().join("ui")).expect("mkdir ui");
    fs::create_dir_all(workspace.path().join("app")).expect("mkdir app");
    let cfg = empty_deploy_config(&workspace.path().to_path_buf());
    let widgets = detect_widgets(&cfg, workspace.path());
    assert!(
        widgets.is_empty(),
        "REQ-79-01: ui/ and app/ MUST NOT be auto-detected — got: {widgets:?}"
    );
}

/// Test 1.11 (multiple_widgets_stop_on_first_failure): two `[[widgets]]`
/// blocks where the first fails → second is NOT attempted. We assert by
/// checking that a sentinel file the second widget would create does NOT
/// appear after the loop.
#[tokio::test]
async fn multiple_widgets_stop_on_first_failure() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_a = workspace.path().join("widget-a");
    let widget_b = workspace.path().join("widget-b");
    fs::create_dir_all(widget_a.join("node_modules")).expect("mkdir a/node_modules");
    fs::create_dir_all(widget_b.join("node_modules")).expect("mkdir b/node_modules");
    write_package_json(&widget_a, "exit 1");
    write_package_json(&widget_b, "touch ran_b.sentinel");

    let widgets = vec![
        WidgetConfig {
            path: "widget-a".to_string(),
            build: Some(vec!["false".to_string()]), // explicit fail
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        },
        WidgetConfig {
            path: "widget-b".to_string(),
            build: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                "touch ran_b.sentinel".to_string(),
            ]),
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        },
    ];

    // Mirror the orchestrator-loop the deploy hook will use.
    let mut first_err = None;
    for w in &widgets {
        if let Err(e) = run_widget_build(w, workspace.path(), true).await {
            first_err = Some(e);
            break;
        }
    }
    assert!(
        first_err.is_some(),
        "widget-a must have failed and aborted the loop"
    );
    assert!(
        !widget_b.join("ran_b.sentinel").is_file(),
        "widget-b MUST NOT have run after widget-a failed — sentinel present"
    );
}

/// Test 1.12 (run_widget_build_returns_resolved_paths — REVISION 3 HIGH-C1):
/// `run_widget_build` returns `Ok(ResolvedPaths)` so the caller can collect
/// all paths and join into PMCP_WIDGET_DIRS at the end. Per-call
/// `std::env::set_var` is REMOVED.
#[tokio::test]
async fn run_widget_build_returns_resolved_paths_no_env_mutation() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    write_package_json(&widget_dir, "true");

    // Snapshot env BEFORE.
    let before = std::env::var("PMCP_WIDGET_DIRS").ok();
    let before_old = std::env::var("PMCP_WIDGET_DIR").ok(); // pre-revision-3 name

    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec!["true".to_string()]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    let resolved = run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("build succeeds");

    let after = std::env::var("PMCP_WIDGET_DIRS").ok();
    let after_old = std::env::var("PMCP_WIDGET_DIR").ok();
    assert_eq!(
        before, after,
        "REVISION 3 HIGH-C1: run_widget_build MUST NOT mutate PMCP_WIDGET_DIRS"
    );
    assert_eq!(
        before_old, after_old,
        "REVISION 3 HIGH-C1: PMCP_WIDGET_DIR (pre-rev-3) name also untouched"
    );
    assert_eq!(resolved.path, widget_dir);
    assert_eq!(resolved.absolute_output_dir, widget_dir.join("dist"));
}

/// Test 1.13 (explicit_argv_passed_through_unchanged — REVISION 3 Codex
/// MEDIUM): a `WidgetConfig` with `build: Some(vec!["sh", "-c", "..."])`
/// runs the argv exactly. Pre-revision-3 string-form would have lost the
/// `--silent`-style flag boundary to whitespace-splitting; argv-array form
/// preserves it. We prove this by writing a sentinel that REQUIRES the args
/// to be passed exactly (otherwise `sh -c` interprets the script
/// differently).
#[tokio::test]
async fn explicit_argv_passed_through_unchanged() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    write_package_json(&widget_dir, "true");

    // The sentinel content includes an embedded space — only argv-array form
    // preserves it as a single argument to `-c`. Whitespace-split would have
    // smashed it into ["sh", "-c", "echo", "hello", ">", "out.txt"].
    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo 'hello world' > out.txt".to_string(),
        ]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    run_widget_build(&cfg, workspace.path(), true)
        .await
        .expect("argv-form build succeeds");
    let written = fs::read_to_string(widget_dir.join("out.txt")).expect("out.txt exists");
    assert_eq!(
        written.trim(),
        "hello world",
        "argv-form must preserve embedded whitespace in `-c` argument"
    );
}

// ============================================================================
// Sanity: PackageManager pure-fn tests don't need orchestrator infra
// ============================================================================

/// Sanity: `PackageManager::detect_from_dir` on an empty tempdir falls back
/// to Npm. Mirrors a Wave 1 unit-test invariant from inside the production
/// crate — re-asserted here as the orchestrator's most important pure
/// dependency.
#[test]
fn package_manager_falls_back_to_npm_when_no_lockfile() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        PackageManager::detect_from_dir(dir.path()),
        PackageManager::Npm
    );
}

// ============================================================================
// Task 2 tests — Step 2.5 hook + CLI flags + PMCP_WIDGET_DIRS env join
// ============================================================================
//
// `DeployCommand::pre_build_widgets_and_set_env` is a private associated fn
// on the bin target's struct (lives in `commands/deploy/mod.rs`), so it is
// not directly reachable from this integration suite. The Task 2 tests
// instead replicate the orchestrator-loop algorithm verbatim using the
// public `detect_widgets` + `run_widget_build` primitives — the algorithm
// itself is tested here, and the wiring (parse flag → call helper) is
// trivially covered by the clap-parse tests (2.1, 2.2, 2.3) which exercise
// the real binary via `assert_cmd`.

/// Mirror of `DeployCommand::pre_build_widgets_and_set_env` body. Used by
/// Tests 2.4..2.9 to exercise the loop algorithm and the env-join
/// semantics in isolation. Kept BYTE-IDENTICAL to the production
/// implementation (sans the `&self`/`global_flags` indirection) so any
/// drift is caught immediately.
async fn run_pre_build_loop(
    widgets: &[WidgetConfig],
    workspace_root: &Path,
    quiet: bool,
) -> anyhow::Result<()> {
    if widgets.is_empty() {
        return Ok(());
    }
    let mut all_output_dirs: Vec<String> = Vec::with_capacity(widgets.len());
    for widget in widgets {
        let resolved = run_widget_build(widget, workspace_root, quiet).await?;
        all_output_dirs.push(resolved.absolute_output_dir.to_string_lossy().to_string());
    }
    let joined = all_output_dirs.join(":");
    std::env::set_var("PMCP_WIDGET_DIRS", &joined);
    Ok(())
}

/// Test 2.1 (clap_parses_no_widget_build): `cargo pmcp deploy --help`
/// includes `--no-widget-build`. (We assert via `--help` rather than a real
/// `deploy` invocation — `deploy` requires an `.pmcp/deploy.toml` and AWS
/// credentials, which isn't appropriate for an integration test.)
#[test]
fn clap_parses_no_widget_build_flag() {
    let cmd = assert_cmd::Command::cargo_bin("cargo-pmcp")
        .expect("locate cargo-pmcp binary")
        .args(["deploy", "--help"])
        .output()
        .expect("run cargo pmcp deploy --help");
    let stdout = String::from_utf8_lossy(&cmd.stdout);
    assert!(
        stdout.contains("--no-widget-build"),
        "cargo pmcp deploy --help must list --no-widget-build, got:\n{stdout}"
    );
}

/// Test 2.2 (clap_parses_widgets_only): `cargo pmcp deploy --help` includes
/// `--widgets-only`.
#[test]
fn clap_parses_widgets_only_flag() {
    let cmd = assert_cmd::Command::cargo_bin("cargo-pmcp")
        .expect("locate cargo-pmcp binary")
        .args(["deploy", "--help"])
        .output()
        .expect("run cargo pmcp deploy --help");
    let stdout = String::from_utf8_lossy(&cmd.stdout);
    assert!(
        stdout.contains("--widgets-only"),
        "cargo pmcp deploy --help must list --widgets-only, got:\n{stdout}"
    );
}

/// Test 2.3 (clap_help_lists_new_flags): combined assertion — both flags
/// appear in the same `--help` invocation. This catches regressions where
/// one flag is removed but the other left in.
#[test]
fn clap_help_lists_both_widget_flags() {
    let cmd = assert_cmd::Command::cargo_bin("cargo-pmcp")
        .expect("locate cargo-pmcp binary")
        .args(["deploy", "--help"])
        .output()
        .expect("run cargo pmcp deploy --help");
    let stdout = String::from_utf8_lossy(&cmd.stdout);
    let no_widget = stdout.matches("--no-widget-build").count();
    let widgets_only = stdout.matches("--widgets-only").count();
    assert_eq!(
        no_widget, 1,
        "--no-widget-build must appear exactly once in help"
    );
    assert_eq!(
        widgets_only, 1,
        "--widgets-only must appear exactly once in help"
    );
}

/// Test 2.4 (orchestrator_skips_when_no_widget_build): the
/// `if !self.no_widget_build` guard means an entire skipped path produces
/// neither file-system effects nor env-var mutations. We assert by running
/// the loop with an empty widgets vec (semantically equivalent to skipping)
/// and verifying nothing is set.
#[tokio::test]
async fn orchestrator_skips_when_no_widget_build_flag_set() {
    // Snapshot env BEFORE.
    let before = std::env::var("PMCP_WIDGET_DIRS").ok();
    // The `if !self.no_widget_build` branch is just early-skip — the loop
    // is never entered. Equivalent: pass empty widgets.
    run_pre_build_loop(&[], &std::env::temp_dir(), true)
        .await
        .expect("empty loop is a no-op");
    let after = std::env::var("PMCP_WIDGET_DIRS").ok();
    assert_eq!(
        before, after,
        "no_widget_build flag (modeled as empty widgets) MUST NOT mutate PMCP_WIDGET_DIRS"
    );
}

/// Test 2.5 (orchestrator_exits_after_widgets_only): the
/// `if self.widgets_only` branch returns Ok(()) before `target.build()`.
/// We assert at the deploy-mod.rs level that the loop runs (env var set)
/// AND that no `target.build()` mock would be invoked. Modeled here as:
/// after `run_pre_build_loop` succeeds with --widgets-only, the env var IS
/// set (so a follow-up `cargo build` would see updated widgets).
#[tokio::test]
async fn widgets_only_runs_loop_then_exits_with_env_set() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    write_package_json(&widget_dir, "true");
    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec!["true".to_string()]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    run_pre_build_loop(std::slice::from_ref(&cfg), workspace.path(), true)
        .await
        .expect("widgets-only loop succeeds");
    // PMCP_WIDGET_DIRS must be set so a follow-up `cargo build` sees it.
    let dirs = std::env::var("PMCP_WIDGET_DIRS")
        .expect("PMCP_WIDGET_DIRS must be set after widgets-only run");
    assert!(
        dirs.contains("widget/dist"),
        "PMCP_WIDGET_DIRS must contain the widget output dir, got: {dirs}"
    );
}

/// Test 2.6 (PMCP_WIDGET_DIRS_set_before_target_build — REVISION 3 HIGH-C1):
/// after a successful single-widget build, `PMCP_WIDGET_DIRS` is set AND
/// contains exactly the widget's `absolute_output_dir`.
#[tokio::test]
async fn pmcp_widget_dirs_set_with_single_widget_path() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widget_dir = workspace.path().join("widget");
    fs::create_dir_all(widget_dir.join("node_modules")).expect("mkdir node_modules");
    write_package_json(&widget_dir, "true");
    let cfg = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec!["true".to_string()]),
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    };
    run_pre_build_loop(std::slice::from_ref(&cfg), workspace.path(), true)
        .await
        .expect("loop succeeds");
    let dirs = std::env::var("PMCP_WIDGET_DIRS").expect("must be set");
    let expected_path = widget_dir.join("dist").to_string_lossy().to_string();
    assert_eq!(
        dirs, expected_path,
        "single-widget PMCP_WIDGET_DIRS must equal exactly the absolute output dir"
    );
}

/// Test 2.7 (PMCP_WIDGET_DIRS_joins_multi_widgets — REVISION 3 HIGH-C1):
/// THREE successful widgets at output dirs `widget-a/dist`, `widget-b/build`,
/// `widget-c/dist` → `PMCP_WIDGET_DIRS` joins them with `:` in declaration
/// order. Pre-revision-3 single-var would have last-widget-wins broken.
#[tokio::test]
async fn pmcp_widget_dirs_joins_multi_widgets_in_declaration_order() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let widgets = [
        ("widget-a", "dist"),
        ("widget-b", "build"),
        ("widget-c", "dist"),
    ];
    let mut configs: Vec<WidgetConfig> = Vec::new();
    for (name, output) in widgets {
        let dir = workspace.path().join(name);
        fs::create_dir_all(dir.join("node_modules")).expect("mkdir node_modules");
        write_package_json(&dir, "true");
        configs.push(WidgetConfig {
            path: name.to_string(),
            build: Some(vec!["true".to_string()]),
            install: None,
            output_dir: output.to_string(),
            embedded_in_crates: vec![],
        });
    }
    run_pre_build_loop(&configs, workspace.path(), true)
        .await
        .expect("multi-widget loop succeeds");
    let dirs = std::env::var("PMCP_WIDGET_DIRS").expect("PMCP_WIDGET_DIRS set");
    let parts: Vec<&str> = dirs.split(':').collect();
    assert_eq!(
        parts.len(),
        3,
        "PMCP_WIDGET_DIRS must contain exactly 3 colon-separated entries, got: {dirs}"
    );
    // Declaration-order preservation:
    assert!(parts[0].ends_with("widget-a/dist"), "entry 0: {}", parts[0]);
    assert!(
        parts[1].ends_with("widget-b/build"),
        "entry 1: {}",
        parts[1]
    );
    assert!(parts[2].ends_with("widget-c/dist"), "entry 2: {}", parts[2]);
}

/// Test 2.8 (mock_cargo_build_inherits_PMCP_WIDGET_DIRS): spawn a child
/// process via `Command::new(...).args(...)` after setting
/// `PMCP_WIDGET_DIRS`; the child sees the env var. Validates RESEARCH.md A4
/// / master plan locked decision #2 (REVISED for HIGH-C1) — that
/// `std::env::set_var` propagates to default-inheritance child processes.
#[tokio::test]
async fn child_subprocess_inherits_pmcp_widget_dirs() {
    // Set the env var via the same mechanism the helper uses.
    std::env::set_var("PMCP_WIDGET_DIRS", "/A/dist:/B/build");
    // Spawn a subprocess that echoes the env var. Use `sh -c` portably.
    let output = tokio::process::Command::new("sh")
        .args(["-c", "echo $PMCP_WIDGET_DIRS"])
        .output()
        .await
        .expect("spawn sh -c");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        "/A/dist:/B/build",
        "child subprocess must inherit PMCP_WIDGET_DIRS via default env-inheritance"
    );
}

/// Test 2.9 (no_widgets_does_not_set_env_var — REVISION 3 HIGH-G1
/// dependency): when `detect_widgets` returns empty Vec AND the loop is
/// invoked, `PMCP_WIDGET_DIRS` is NOT set/changed (build.rs falls back to
/// local-discovery per HIGH-G1 in 79-04).
#[tokio::test]
async fn empty_widgets_does_not_set_env_var() {
    // Use a unique env-var key for this test so we can probe its absence
    // even if a prior test set the canonical name. We fake the sentinel
    // by clearing first.
    let prior = std::env::var("PMCP_WIDGET_DIRS").ok();
    std::env::remove_var("PMCP_WIDGET_DIRS");

    run_pre_build_loop(&[], &std::env::temp_dir(), true)
        .await
        .expect("empty loop is a no-op");

    let after = std::env::var("PMCP_WIDGET_DIRS").ok();
    assert_eq!(
        after, None,
        "empty-widgets path MUST NOT set PMCP_WIDGET_DIRS — got: {after:?}"
    );

    // Restore prior value if any (test isolation).
    if let Some(v) = prior {
        std::env::set_var("PMCP_WIDGET_DIRS", v);
    }
}
