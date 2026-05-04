//! Plan 79-06 integration tests for the raw-HTML / CDN-import widget
//! archetype (Phase 45 zero-build MCP Apps use case). Closes UAT Test 3 from
//! Phase 79: `widgets/` with only `*.html` files and no `package.json` must
//! NOT spawn npm and must NOT crash with raw `os error 2`.
//!
//! Reference reproduction: `~/projects/mcp/Scientific-Calculator-MCP-App` —
//! a single-file `widgets/keypad.html` importing the SDK from
//! `https://esm.sh/@modelcontextprotocol/ext-apps`, no `package.json`, no
//! lockfile, no `node_modules/`. Prior to Plan 79-06, `cargo pmcp deploy
//! --widgets-only` hard-crashed; npm walked UP the directory tree from the
//! manifest-less widgets/ and audited 1839 packages from a parent workspace
//! before the orchestrator's `read_to_string(package.json)` raised raw
//! `io::Error: No such file or directory (os error 2)`.
//!
//! These tests call `run_widget_build` DIRECTLY (schema-direct, faster, no
//! real-deploy machinery needed). The proof-of-no-subprocess is that the
//! tests pass on a runner with NO npm/pnpm/yarn/bun on PATH — if the
//! early-return guard regressed, the orchestrator would attempt
//! `Command::new("npm").spawn()` and fail with the original os-error-2 (or
//! "npm not found" on bare CI runners).

use cargo_pmcp::deployment::widgets::{run_widget_build, WidgetConfig};
use std::fs;

/// Helper: write the canonical Scientific-Calculator-MCP-App reproduction
/// — a `widgets/keypad.html` file containing a `<script type="module">`
/// importing the SDK from a CDN. No `package.json`, no lockfile.
fn write_keypad_html(widgets_dir: &std::path::Path) {
    fs::create_dir_all(widgets_dir).expect("create widgets/");
    fs::write(
        widgets_dir.join("keypad.html"),
        r#"<!DOCTYPE html>
<html><body>
<script type="module">
import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps";
new App({ /* ... */ });
</script>
</body></html>
"#,
    )
    .expect("write keypad.html");
}

/// Default raw-HTML widget config — `path = "widgets"`, no build/install
/// override, default `output_dir = "dist"`, no embedded crates.
fn raw_html_widget_config() -> WidgetConfig {
    WidgetConfig {
        path: "widgets".to_string(),
        build: None,
        install: None,
        output_dir: "dist".to_string(),
        embedded_in_crates: vec![],
    }
}

/// Plan 79-06 Test I1 (proof-of-fix for UAT Test 3): the documented zero-
/// build raw-HTML / CDN-import widget archetype. After the early-return
/// guard, `run_widget_build` returns `Ok(resolved)` and spawns NO
/// subprocess — `node_modules/` and `package-lock.json` MUST NOT exist
/// after the call (proof that no `npm install` ran).
#[tokio::test]
async fn raw_html_widget_archetype_does_not_spawn_npm() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace_root = tmp.path();
    let widgets_dir = workspace_root.join("widgets");
    write_keypad_html(&widgets_dir);

    let widget = raw_html_widget_config();
    let resolved = run_widget_build(&widget, workspace_root, /* quiet */ false)
        .await
        .expect("raw-HTML widget should not error");

    assert!(
        resolved.path.ends_with("widgets"),
        "path must end with widgets, got {:?}",
        resolved.path
    );
    assert!(
        resolved.absolute_output_dir.ends_with("widgets/dist"),
        "absolute_output_dir must end with widgets/dist, got {:?}",
        resolved.absolute_output_dir,
    );
    assert!(
        !widgets_dir.join("node_modules").exists(),
        "no npm install should have been spawned — node_modules must not exist"
    );
    assert!(
        !widgets_dir.join("package-lock.json").exists(),
        "no npm install should have been spawned — package-lock.json must not exist"
    );
    // Defense-in-depth: also check the workspace root for an accidental
    // `package-lock.json` written by a parent-walking npm install. This was
    // the side-effect observed in UAT Test 3 when npm walked up from the
    // manifest-less widgets/.
    assert!(
        !workspace_root.join("package-lock.json").exists(),
        "no npm install should have walked up — workspace package-lock.json must not exist"
    );
}

/// Plan 79-06 Test I2: explicit `widget.build = ["npm", "run", "build"]`
/// argv against a manifest-less directory. The early-return guard wins
/// over explicit argv when there is no `package.json` — operator
/// misconfiguration becomes a no-op skip rather than a crash. The
/// defense-in-depth guard inside `verify_build_script_exists` is the
/// SECOND layer for cases where the early-return is bypassed.
#[tokio::test]
async fn raw_html_widget_explicit_npm_build_argv_friendly_bail() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace_root = tmp.path();
    let widgets_dir = workspace_root.join("widgets");
    write_keypad_html(&widgets_dir);

    let mut widget = raw_html_widget_config();
    widget.build = Some(vec!["npm".into(), "run".into(), "build".into()]);

    // Early-return wins — no Node pipeline runs at all (the directory has
    // no package.json so there is nothing for `npm run build` to read).
    // This is the chosen design (operator misconfig => no-op skip).
    let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
        .await
        .expect("raw-HTML early-return should bypass explicit argv");
    assert!(resolved.path.ends_with("widgets"));
    assert!(
        !widgets_dir.join("node_modules").exists(),
        "early-return guard must skip npm install even with explicit npm argv"
    );
}

/// Plan 79-06 Test I3 — regression coverage: the Node-project happy path
/// is unchanged by the new guard. A widget with `package.json`, a stub
/// `node_modules/` (so `ensure_node_modules` short-circuits without npm
/// on PATH), and an explicit `widget.build = ["true"]` argv runs end-to-
/// end and returns `Ok(resolved)`.
#[tokio::test]
#[cfg(unix)]
async fn node_project_unchanged_baseline() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace_root = tmp.path();
    let widgets_dir = workspace_root.join("widgets");
    fs::create_dir_all(&widgets_dir).expect("create widgets/");
    fs::write(
        widgets_dir.join("package.json"),
        br#"{"scripts":{"build":"true"}}"#,
    )
    .expect("write package.json");
    // Pre-create node_modules so `ensure_node_modules` short-circuits
    // without requiring npm on PATH.
    fs::create_dir_all(widgets_dir.join("node_modules")).expect("create node_modules/");
    fs::write(widgets_dir.join("node_modules/.placeholder"), b"").ok();

    let mut widget = raw_html_widget_config();
    // Explicit `true` argv exits 0 on every Unix-like CI runner and
    // sidesteps `verify_build_script_exists`.
    widget.build = Some(vec!["true".into()]);

    let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
        .await
        .expect("Node pipeline happy path should still work");
    assert!(resolved.path.ends_with("widgets"));
}
