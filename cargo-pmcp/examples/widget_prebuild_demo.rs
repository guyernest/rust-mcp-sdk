//! Phase 79 Plan 79-04: schema-direct demo of the widget pre-build orchestrator.
//!
//! Runs the Wave-2 orchestrator (`run_widget_build`) end-to-end against a
//! tempdir + fake `package.json` fixture WITHOUT performing a real deploy.
//! Exercises:
//!   1. Lockfile-driven `PackageManager` detection (npm via `package-lock.json`).
//!   2. `node_modules/` install-skip heuristic (we pre-create the dir).
//!   3. The build script invocation (synthesized as `mkdir -p dist && echo ...`).
//!   4. The `PMCP_WIDGET_DIRS` env-var contract (REVISION 3 HIGH-C1) — set as a
//!      list-of-one for the single-widget case here; multi-widget projects
//!      colon-join all `[[widgets]]` entries' `absolute_output_dir` paths.
//!
//! Runs cleanly on any developer box (no AWS / Docker / Node runtime needed —
//! the synthesized `package.json` build script is plain `sh`).
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example widget_prebuild_demo
//!
//! Expected exit code: 0.

use cargo_pmcp::deployment::widgets::{run_widget_build, PackageManager, WidgetConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Phase 79 — Widget pre-build orchestrator (isolation demo, REVISION 3) ===\n");

    // --- 1. Tempdir setup ---
    let temp = tempfile::tempdir()?;
    let workspace_root = temp.path().to_path_buf();
    let widget_dir = workspace_root.join("widget");
    std::fs::create_dir_all(&widget_dir)?;

    // npm lockfile drives PackageManager::Npm detection.
    std::fs::write(widget_dir.join("package-lock.json"), "{}")?;

    // package.json with a `build` script that produces a single output file
    // under `dist/`. Plain `sh` so the demo runs without Node installed.
    let pkg_json = r#"{
        "name": "demo-widget",
        "version": "0.0.1",
        "scripts": {
            "build": "mkdir -p dist && echo '<html>demo</html>' > dist/foo.html"
        }
    }"#;
    std::fs::write(widget_dir.join("package.json"), pkg_json)?;

    // Pre-create node_modules so `ensure_node_modules` skips the install step
    // (no real `npm install` happens in this demo).
    std::fs::create_dir_all(widget_dir.join("node_modules"))?;

    println!("--- 1. Tempdir setup ---");
    println!("  Workspace root: {}", workspace_root.display());
    println!("  Widget dir:     {}", widget_dir.display());

    // --- 2. Lockfile-driven PM detection ---
    let pm = PackageManager::detect_from_dir(&widget_dir);
    println!("\n--- 2. Lockfile-driven PM detection ---");
    println!("  Detected: {pm:?} (expected: Npm)");
    assert_eq!(pm, PackageManager::Npm);

    // --- 3. Run the widget pre-build orchestrator ---
    // REVISION 3 Codex MEDIUM: build/install are Option<Vec<String>>. Override
    // the default `npm run build` argv with a direct `sh -c` so this demo
    // doesn't require `npm` on PATH.
    let widget = WidgetConfig {
        path: "widget".to_string(),
        build: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "mkdir -p dist && echo '<html>demo</html>' > dist/foo.html".to_string(),
        ]),
        install: None, // node_modules pre-exists → install is skipped
        output_dir: "dist".to_string(),
        embedded_in_crates: vec!["demo-bin".to_string()],
    };
    println!("\n--- 3. Running widget pre-build orchestrator ---");
    let resolved = run_widget_build(&widget, &workspace_root, false).await?;

    // --- 4. Verify build output ---
    let output_path = widget_dir.join("dist").join("foo.html");
    println!("\n--- 4. Verify build output ---");
    println!(
        "  {} exists: {}",
        output_path.display(),
        output_path.exists()
    );
    assert!(
        output_path.exists(),
        "build script should have produced dist/foo.html"
    );

    // --- 5. PMCP_WIDGET_DIRS contract demonstration (REVISION 3 HIGH-C1) ---
    // For a single widget the env value is the absolute_output_dir as a
    // list-of-one (no colon). Multi-widget projects colon-join all entries —
    // this is what the deploy hook (`pre_build_widgets_and_set_env`) does
    // before invoking `target.build(&config).await?`.
    std::env::set_var(
        "PMCP_WIDGET_DIRS",
        resolved.absolute_output_dir.to_string_lossy().to_string(),
    );
    let env_dirs = std::env::var("PMCP_WIDGET_DIRS")?;
    println!("\n--- 5. PMCP_WIDGET_DIRS contract (REVISION 3 HIGH-C1) ---");
    println!("  Set to: {env_dirs}");
    assert_eq!(PathBuf::from(&env_dirs), widget_dir.join("dist"));

    println!("\n=== Example complete — Phase 79 build half verified end-to-end ===");
    Ok(())
}
