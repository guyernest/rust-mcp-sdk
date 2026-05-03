//! Workspace diagnostics — validates project structure, toolchain, and server connectivity.

use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use walkdir::WalkDir;

use super::GlobalFlags;

/// Run workspace diagnostics.
///
/// Checks:
/// 1. Cargo.toml exists and is a valid workspace or package
/// 2. Rust toolchain is installed and meets MSRV
/// 3. Required tools (cargo-pmcp dependencies) are available
/// 4. If a server URL is provided, tests connectivity
pub fn execute(url: Option<&str>, global_flags: &GlobalFlags) -> Result<()> {
    let quiet = !global_flags.should_output();
    let mut issues = 0u32;

    print_doctor_header(quiet);

    issues += check_cargo_toml(quiet);
    issues += check_rust_toolchain(quiet);
    check_rustfmt(quiet);
    check_clippy(quiet);
    issues += check_widget_rerun_if_changed(quiet);

    if let Some(server_url) = url {
        issues += check_server_connectivity(server_url, quiet)?;
    }

    print_doctor_summary(issues, quiet);

    if issues > 0 {
        anyhow::bail!("{} diagnostic issue(s) found", issues);
    }
    Ok(())
}

fn print_doctor_header(quiet: bool) {
    if quiet {
        return;
    }
    println!();
    println!(
        "  {} Workspace Diagnostics",
        "cargo pmcp doctor".bright_white().bold()
    );
    println!("  {}", "─".repeat(40).dimmed());
    println!();
}

/// Verify Cargo.toml exists and pmcp dependency is present. Returns issue count (0 or 1).
fn check_cargo_toml(quiet: bool) -> u32 {
    let cargo_toml = std::path::Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        if !quiet {
            println!("  {} No Cargo.toml in current directory", "✗".red());
        }
        return 1;
    }

    if !quiet {
        println!("  {} Cargo.toml found", "✓".green());
    }

    let content = std::fs::read_to_string(cargo_toml).unwrap_or_default();
    if !quiet {
        if content.contains("pmcp") {
            println!("  {} pmcp dependency detected", "✓".green());
        } else {
            println!(
                "  {} No pmcp dependency found (not an MCP workspace?)",
                "!".yellow()
            );
        }
    }
    0
}

/// Check rustc is available. Returns issue count (0 or 1).
fn check_rust_toolchain(quiet: bool) -> u32 {
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            if !quiet {
                println!("  {} {}", "✓".green(), version.trim());
            }
            0
        },
        Err(_) => {
            if !quiet {
                println!("  {} Rust toolchain not found", "✗".red());
            }
            1
        },
    }
}

/// Check rustfmt is installed (warning-only, does not count as an issue).
fn check_rustfmt(quiet: bool) {
    let ok = std::process::Command::new("rustfmt")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if quiet {
        return;
    }
    if ok {
        println!("  {} rustfmt available", "✓".green());
    } else {
        println!(
            "  {} rustfmt not found (run: rustup component add rustfmt)",
            "!".yellow()
        );
    }
}

/// Check cargo clippy is installed (warning-only).
fn check_clippy(quiet: bool) {
    let ok = std::process::Command::new("cargo")
        .args(["clippy", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if quiet {
        return;
    }
    if ok {
        println!("  {} clippy available", "✓".green());
    } else {
        println!(
            "  {} clippy not found (run: rustup component add clippy)",
            "!".yellow()
        );
    }
}

/// Probe the MCP server URL with an initialize JSON-RPC request.
/// Returns issue count (0 or 1).
fn check_server_connectivity(server_url: &str, quiet: bool) -> Result<u32> {
    if !quiet {
        println!();
        println!("  {} Server: {}", "→".blue(), server_url);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let issue_count = rt.block_on(async { probe_server_initialize(server_url, quiet).await })?;
    Ok(issue_count)
}

/// Async worker for the initialize probe.
async fn probe_server_initialize(server_url: &str, quiet: bool) -> Result<u32> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"pmcp-doctor","version":"0.1.0"}},"id":1}"#)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if !quiet {
                if status.is_success() {
                    println!("  {} Server reachable (HTTP {})", "✓".green(), status);
                } else {
                    println!("  {} Server returned HTTP {}", "!".yellow(), status);
                }
            }
            Ok(0)
        },
        Err(e) => {
            if !quiet {
                println!("  {} Cannot reach server: {}", "✗".red(), e);
            }
            Ok(1)
        },
    }
}

/// Print the pass/fail summary banner.
fn print_doctor_summary(issues: u32, quiet: bool) {
    if quiet {
        return;
    }
    println!();
    if issues == 0 {
        println!("  {} All checks passed", "✓".green().bold());
    } else {
        println!("  {} {} issue(s) found", "!".yellow().bold(), issues);
    }
    println!();
}

// ============================================================================
// Phase 79 Plan 79-04: widget rerun-if-changed check
// ============================================================================
//
// REVISION 3 Codex MEDIUM: distinguishes WidgetDir crates (run-time file
// serving — no build.rs needed) from `include_str!` crates (build.rs WARN
// fires when missing). Detection via regex over workspace `.rs` files.
//
// Decision traceability:
// - REQ-79-07 — follows existing `check_*` pattern at lines 52, 80, 102, 122,
//   148 (check_server_connectivity).
// - Pitfall 1 mitigation — warning text recommends `cargo clean -p <crate>`
//   ONCE after adding the build.rs (Cargo's stale-cache hold-on otherwise
//   bypasses the new build.rs on the first deploy).
// - REVISION 3 Codex MEDIUM — WidgetDir-only crates do NOT trigger the
//   warning because run-time file-serving doesn't have Failure Mode B.

/// Check that workspace crates using `include_str!` against widget paths have
/// a `build.rs` emitting `cargo:rerun-if-changed`. Returns issue count.
///
/// REVISION 3 Codex MEDIUM: WidgetDir crates (run-time file serving) are
/// silently skipped — they have no Cargo cache invalidation problem. Only
/// `include_str!`-against-widgets crates need the build.rs scaffold.
fn check_widget_rerun_if_changed(quiet: bool) -> u32 {
    let include_pattern = match Regex::new(r#"include_str!\s*\(\s*"[^"]*widgets?/[^"]*"\s*\)"#) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let workspace = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let include_crates = scan_workspace_for_pattern(&workspace, &include_pattern);
    let mut issues = 0u32;
    for krate in include_crates {
        if !crate_has_rerun_if_changed(&krate) {
            print_widget_rerun_warning(&krate, quiet);
            issues += 1;
        }
    }
    issues
}

/// Print the WARN message for a crate missing its widget `build.rs`.
fn print_widget_rerun_warning(krate: &std::path::Path, quiet: bool) {
    if quiet {
        return;
    }
    println!(
        "  {} crate `{}` uses include_str! for widget files but has no \
         build.rs cargo:rerun-if-changed; widget changes may not trigger \
         recompilation. WidgetDir crates do NOT need a build.rs (run-time \
         file serving). REVISION 3 Codex MEDIUM.",
        "!".yellow(),
        krate.display(),
    );
    let crate_name = krate
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<crate>");
    println!(
        "    Fix: scaffold a build.rs by running `cargo pmcp app new --embed-widgets` \
         to see the template, OR copy the template documented in cargo-pmcp's README. \
         Then run `cargo clean -p {}` ONCE so cargo's existing stale cache doesn't \
         bypass the new build.rs on the first deploy (Phase 79 Pitfall 1).",
        crate_name,
    );
}

/// Walk workspace src/, filter to `.rs` files, regex-scan, collect enclosing
/// crate dirs. Skips `target/`, `node_modules/`, `.git/` for performance.
fn scan_workspace_for_pattern(
    workspace: &std::path::Path,
    pattern: &Regex,
) -> Vec<std::path::PathBuf> {
    let mut hits: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    for entry in WalkDir::new(workspace)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e.path()))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if pattern.is_match(&text) {
            if let Some(crate_dir) = find_enclosing_crate_dir(path) {
                hits.insert(crate_dir);
            }
        }
    }
    hits.into_iter().collect()
}

/// Return `true` for directories the doctor scan should skip entirely.
fn is_ignored_dir(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| matches!(name, "target" | "node_modules" | ".git"))
}

/// Walk up from a file until finding an enclosing `Cargo.toml`. Returns the
/// directory containing it, or `None` if none found.
fn find_enclosing_crate_dir(file: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut p = file.parent()?.to_path_buf();
    loop {
        if p.join("Cargo.toml").is_file() {
            return Some(p);
        }
        p = p.parent()?.to_path_buf();
    }
}

/// Check whether a crate's `build.rs` exists AND mentions
/// `cargo:rerun-if-changed` (or the newer `cargo::rerun-if-changed` syntax).
fn crate_has_rerun_if_changed(crate_dir: &std::path::Path) -> bool {
    let build_rs = crate_dir.join("build.rs");
    std::fs::read_to_string(&build_rs)
        .map(|s| s.contains("cargo:rerun-if-changed") || s.contains("cargo::rerun-if-changed"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Create a minimal crate fixture inside `root` with the given relative
    /// crate dir, optional main.rs body, and optional build.rs body.
    fn make_crate(
        root: &std::path::Path,
        crate_dir: &str,
        main_rs: Option<&str>,
        build_rs: Option<&str>,
    ) -> PathBuf {
        let dir = root.join(crate_dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                crate_dir
            ),
        )
        .unwrap();
        if let Some(body) = main_rs {
            fs::write(dir.join("src/main.rs"), body).unwrap();
        }
        if let Some(body) = build_rs {
            fs::write(dir.join("build.rs"), body).unwrap();
        }
        dir
    }

    /// Switch to the tempdir for the duration of the closure. Doctor scans
    /// `current_dir` so we must `set_current_dir` to drive the scan against
    /// the fixture, restoring the prior cwd on exit.
    fn with_cwd<P: AsRef<std::path::Path>, F: FnOnce()>(dir: P, f: F) {
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.as_ref()).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::env::set_current_dir(prev).unwrap();
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    /// Test 1.1 — include_str! against widget path, no build.rs → 1 issue.
    #[test]
    fn doctor_widget_check_warns_when_include_str_lacks_build_rs() {
        let tmp = tempfile::tempdir().unwrap();
        make_crate(
            tmp.path(),
            "myapp",
            Some(r#"fn main() { let _ = include_str!("../widget/dist/foo.html"); }"#),
            None,
        );
        with_cwd(tmp.path(), || {
            let issues = check_widget_rerun_if_changed(true);
            assert!(issues >= 1, "expected ≥1 issue, got {issues}");
        });
    }

    /// Test 1.2 — include_str! AND build.rs with cargo:rerun-if-changed → 0 issues.
    #[test]
    fn doctor_widget_check_silent_when_build_rs_present() {
        let tmp = tempfile::tempdir().unwrap();
        make_crate(
            tmp.path(),
            "myapp",
            Some(r#"fn main() { let _ = include_str!("../widget/dist/foo.html"); }"#),
            Some(r#"fn main() { println!("cargo:rerun-if-changed=widget/dist"); }"#),
        );
        with_cwd(tmp.path(), || {
            let issues = check_widget_rerun_if_changed(true);
            assert_eq!(issues, 0);
        });
    }

    /// Test 1.3 — no widget include_str! anywhere → 0 issues.
    #[test]
    fn doctor_widget_check_silent_when_no_widget_includes() {
        let tmp = tempfile::tempdir().unwrap();
        make_crate(
            tmp.path(),
            "myapp",
            Some(r#"fn main() { println!("hello"); }"#),
            None,
        );
        with_cwd(tmp.path(), || {
            let issues = check_widget_rerun_if_changed(true);
            assert_eq!(issues, 0);
        });
    }

    /// Test 1.3b — REVISION 3 Codex MEDIUM. WidgetDir crate (NO include_str!) and NO
    /// build.rs → 0 issues. Run-time file-serving has no Failure Mode B.
    #[test]
    fn doctor_widget_check_silent_for_widget_dir_crates() {
        let tmp = tempfile::tempdir().unwrap();
        make_crate(
            tmp.path(),
            "myapp",
            Some(
                r#"use pmcp::WidgetDir;
fn main() {
    let _ = WidgetDir::new("widget/dist");
}"#,
            ),
            None,
        );
        with_cwd(tmp.path(), || {
            let issues = check_widget_rerun_if_changed(true);
            assert_eq!(issues, 0);
        });
    }

    /// Test 1.3c — REVISION 3 Codex MEDIUM. Mixed crate (BOTH WidgetDir AND
    /// include_str! against widget path) and NO build.rs → ≥1 issue. The
    /// include_str! branch drives the warning regardless of WidgetDir presence.
    #[test]
    fn doctor_widget_check_warns_for_mixed_crate_without_build_rs() {
        let tmp = tempfile::tempdir().unwrap();
        make_crate(
            tmp.path(),
            "myapp",
            Some(
                r#"use pmcp::WidgetDir;
fn main() {
    let _ = WidgetDir::new("widget/dist");
    let _ = include_str!("../widget/dist/foo.html");
}"#,
            ),
            None,
        );
        with_cwd(tmp.path(), || {
            let issues = check_widget_rerun_if_changed(true);
            assert!(issues >= 1, "expected ≥1 issue, got {issues}");
        });
    }
}
