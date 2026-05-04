//! Integration tests for the raw-HTML / CDN-import widget archetype
//! (zero-build MCP Apps use case): a `widgets/` directory containing only
//! `*.html` files and no `package.json` must NOT spawn npm.
//!
//! Tests call `run_widget_build` directly. Proof-of-no-subprocess: the suite
//! passes on a runner with no `npm`/`pnpm`/`yarn`/`bun` on PATH — if the
//! early-return guard regressed, `Command::new("npm").spawn()` would fail.

use cargo_pmcp::deployment::widgets::{run_widget_build, WidgetConfig};
use std::fs;

/// Writes a `widgets/keypad.html` file with a `<script type="module">`
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

/// `node_modules/` and `package-lock.json` must not exist after the call —
/// proof that no `npm install` ran.
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
    // npm walks UP from a manifest-less dir; this asserts no parent-walk write.
    assert!(
        !workspace_root.join("package-lock.json").exists(),
        "no npm install should have walked up — workspace package-lock.json must not exist"
    );
}

/// The early-return guard wins over explicit `npm run build` argv when
/// there is no `package.json` — operator misconfig becomes a no-op skip.
#[tokio::test]
async fn raw_html_widget_explicit_npm_build_argv_friendly_bail() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace_root = tmp.path();
    let widgets_dir = workspace_root.join("widgets");
    write_keypad_html(&widgets_dir);

    let mut widget = raw_html_widget_config();
    widget.build = Some(vec!["npm".into(), "run".into(), "build".into()]);

    let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
        .await
        .expect("raw-HTML early-return should bypass explicit argv");
    assert!(resolved.path.ends_with("widgets"));
    assert!(
        !widgets_dir.join("node_modules").exists(),
        "early-return guard must skip npm install even with explicit npm argv"
    );
}

/// Regression: the Node-project happy path is unchanged by the new guard.
/// A real `package.json` + stub `node_modules/` (so `ensure_node_modules`
/// short-circuits without npm on PATH) + explicit `["true"]` argv runs
/// end-to-end and returns `Ok(resolved)`.
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
    fs::create_dir_all(widgets_dir.join("node_modules")).expect("create node_modules/");
    fs::write(widgets_dir.join("node_modules/.placeholder"), b"").ok();

    let mut widget = raw_html_widget_config();
    widget.build = Some(vec!["true".into()]);

    let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
        .await
        .expect("Node pipeline happy path should still work");
    assert!(resolved.path.ends_with("widgets"));
}
