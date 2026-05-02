//! MCP App metadata validation subcommand for cargo-pmcp
//!
//! Validates App-capable tools on an MCP server for correct `_meta` structure,
//! resource cross-references, and host-specific keys.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{AppValidationMode, AppValidator, TestReport, TestStatus};
use pmcp::types::{Content, ReadResourceResult};
use std::collections::HashMap;
use std::time::Duration;

use crate::commands::auth;
use crate::commands::flags::AuthFlags;
use crate::commands::GlobalFlags;

/// Maximum widget body size accepted by the scanner.
///
/// REVISION MED-5: this cap fires AFTER `read_resource` has already loaded
/// the body into memory, so it is OUTPUT/REPORT HYGIENE only — it prevents
/// the validator from emitting reports referencing multi-GB widget bodies
/// and prevents the scanner from spending time on absurdly large inputs.
/// It does NOT protect against transport-layer DoS (an adversarial server
/// could still exhaust cargo-pmcp's heap during the read itself).
///
/// True streaming-level protection requires `ServerTester::read_resource`
/// itself to enforce a byte limit; that is deferred to a follow-up phase or
/// a future revision of this plan.
const MAX_WIDGET_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Execute the `cargo pmcp test apps` command.
pub async fn execute(
    url: String,
    mode: Option<String>,
    tool: Option<String>,
    strict: bool,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let verbose = global_flags.verbose;
    let validation_mode: AppValidationMode = mode
        .as_deref()
        .unwrap_or("standard")
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    print_apps_header(
        &url,
        &validation_mode,
        strict,
        tool.as_deref(),
        global_flags,
    );

    let auth_method = auth_flags.resolve();
    let middleware = auth::resolve_auth_middleware(&url, &auth_method).await?;

    let mut tester = mcp_tester::ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false,
        None,
        transport.as_deref(),
        middleware,
    )
    .context("Failed to create server tester")?;

    run_apps_connectivity(&mut tester, global_flags).await?;

    if global_flags.should_output() {
        println!();
        println!("{}", "2. Discovering tools and resources...".bright_white());
    }

    let tools = list_tools_for_apps(&mut tester, verbose).await?;
    let resources = list_resources_for_apps(&mut tester, verbose).await;

    if global_flags.should_output() {
        println!(
            "   {} {} tools, {} resources discovered",
            "✓".green(),
            tools.len(),
            resources.len()
        );
    }

    // Check for App-capable tools
    let app_count = tools
        .iter()
        .filter(|t| AppValidator::is_app_capable(t))
        .count();

    if app_count == 0 && tool.is_none() {
        if global_flags.should_output() {
            println!();
            println!(
                "   {} No App-capable tools found on this server ({} tools total)",
                "i".bright_cyan(),
                tools.len()
            );
            println!();
        }
        return Ok(());
    }

    if global_flags.should_output() {
        println!(
            "   {} {} App-capable tool{}",
            "i".bright_cyan(),
            app_count,
            if app_count == 1 { "" } else { "s" }
        );
        println!();
        println!("{}", "3. Validating App metadata...".bright_white());
    }

    // REVISION HIGH-4: clone the filter for use both by AppValidator (metadata
    // validation) and the widget-read site below (so --tool restricts which
    // widgets are read). Without the clone, we'd move it into AppValidator::new.
    let tool_filter = tool.clone();

    // Run validation
    let validator = AppValidator::new(validation_mode, tool);
    let mut results = validator.validate_tools(&tools, &resources);

    // REVISION HIGH-4: build app_tools applying the same `tool_filter` semantics
    // AppValidator uses internally (see app_validator.rs lines 76-85). If a
    // filter is set, ONLY tools whose name matches are included; otherwise
    // every App-capable tool is included.
    let app_tools: Vec<&pmcp::types::ToolInfo> = tools
        .iter()
        .filter(|t| match tool_filter.as_deref() {
            Some(name) => t.name == name,
            None => AppValidator::is_app_capable(t),
        })
        .collect();

    // Fetch widget HTML bodies via resources/read for every (filtered)
    // App-capable tool. Emits ERROR-tier results in claude-desktop mode (one
    // per missing handler/signal); standard mode emits ONE summary WARN per
    // widget; chatgpt mode emits zero widget rows (per Plan 01 RESEARCH Q4).
    let (widget_bodies, mut read_failures) =
        read_widget_bodies(&mut tester, &app_tools, verbose).await;
    results.extend(validator.validate_widgets(&widget_bodies));
    results.append(&mut read_failures);

    if results.is_empty() {
        if global_flags.should_output() {
            println!("   {} No validation results", "i".bright_cyan());
            println!();
        }
        return Ok(());
    }

    // Build report
    let mut report = TestReport::new();
    for result in results {
        report.add_test(result);
    }

    if strict {
        report.apply_strict_mode();
    }

    // Print report
    report.print(mcp_tester::OutputFormat::Pretty);

    if report.has_failures() {
        anyhow::bail!("App validation failed - see errors above");
    }

    if global_flags.should_output() {
        println!(
            "{} {}",
            "✓".green().bold(),
            "App validation passed".green().bold()
        );
        println!();
    }

    Ok(())
}

/// Print the command header: URL, mode, strict flag, tool filter.
fn print_apps_header(
    url: &str,
    validation_mode: &AppValidationMode,
    strict: bool,
    tool: Option<&str>,
    global_flags: &GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }
    println!();
    println!("{}", "MCP App Validation".bright_cyan().bold());
    println!(
        "{}",
        "────────────────────────────────────────".bright_cyan()
    );
    println!("  URL:  {}", url.bright_white());
    println!("  Mode: {}", validation_mode.to_string().bright_white());
    if strict {
        println!("  Strict: {}", "yes".bright_yellow());
    }
    if let Some(t) = tool {
        println!("  Tool filter: {}", t.bright_white());
    }
    println!();
}

/// Run the connectivity quick-test and print per-test status. Returns an
/// error when any test fails.
async fn run_apps_connectivity(
    tester: &mut mcp_tester::ServerTester,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("{}", "1. Testing connectivity...".bright_white());
    }

    let init_report = tester
        .run_quick_test()
        .await
        .context("Failed to run connectivity test")?;

    if init_report.has_failures() {
        print_connectivity_failures(&init_report.tests);
        anyhow::bail!("Server connectivity check failed - cannot validate App metadata");
    }

    if global_flags.should_output() {
        println!("   {} Connected", "✓".green());
    }
    Ok(())
}

/// Print each connectivity test result with icon + optional error line.
fn print_connectivity_failures(tests: &[mcp_tester::TestResult]) {
    for result in tests {
        let icon = match result.status {
            TestStatus::Passed => "✓".green(),
            TestStatus::Failed => "✗".red(),
            TestStatus::Warning => "⚠".yellow(),
            TestStatus::Skipped => "○".yellow(),
        };
        println!("   {} {}", icon, result.name);
        if let Some(ref error) = result.error {
            println!("      {} {}", "Error:".red(), error);
        }
    }
}

/// List tools with verbose-aware error logging; propagates failure.
async fn list_tools_for_apps(
    tester: &mut mcp_tester::ServerTester,
    verbose: bool,
) -> Result<Vec<pmcp::types::ToolInfo>> {
    match tester.list_tools().await {
        Ok(result) => Ok(result.tools),
        Err(e) => {
            if verbose {
                eprintln!("   {} Tools listing failed: {}", "✗".red(), e);
            }
            anyhow::bail!("Failed to list tools: {e}");
        },
    }
}

/// List resources with verbose-aware warning; best-effort (empty on failure).
async fn list_resources_for_apps(
    tester: &mut mcp_tester::ServerTester,
    verbose: bool,
) -> Vec<pmcp::types::ResourceInfo> {
    match tester.list_resources().await {
        Ok(result) => result.resources,
        Err(e) => {
            if verbose {
                eprintln!(
                    "   {} Resources listing failed (continuing): {}",
                    "⚠".yellow(),
                    e
                );
            }
            Vec::new()
        },
    }
}

/// REVISION MED-6: deduplicate widget URIs before issuing `read_resource`.
///
/// Multiple tools may share a single widget URI (e.g. a tool family with one
/// shared dashboard). This helper returns a `Vec<(tool_name, uri)>` listing
/// every (tool, uri) pair (so each tool gets its own validator results), AND
/// returns a `HashMap<uri, Vec<tool_name>>` that lets the read loop fetch
/// each unique URI exactly once and then fan the cached HTML back out per
/// tool.
fn dedup_widget_uris(
    app_tools: &[&pmcp::types::ToolInfo],
) -> (Vec<(String, String)>, HashMap<String, Vec<String>>) {
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(app_tools.len());
    let mut by_uri: HashMap<String, Vec<String>> = HashMap::new();
    for tool in app_tools {
        let Some(uri) = AppValidator::extract_resource_uri(tool) else {
            continue;
        };
        pairs.push((tool.name.clone(), uri.clone()));
        by_uri.entry(uri).or_default().push(tool.name.clone());
    }
    (pairs, by_uri)
}

/// Best-effort: fetch widget HTML bodies via `resources/read` for every
/// App-capable tool, deduplicating reads by URI (REVISION MED-6).
/// Returns `(tool_name, uri, html)` triples (REVISION HIGH-4: tool name
/// flows into the validator so error reports can name the tool).
///
/// Per-widget read failures DO NOT abort the run; they are surfaced as
/// `TestStatus::Failed` rows in the returned `read_failures` Vec so the
/// user still sees them in the report (per RESEARCH §Pitfall 4).
async fn read_widget_bodies(
    tester: &mut mcp_tester::ServerTester,
    app_tools: &[&pmcp::types::ToolInfo],
    verbose: bool,
) -> (Vec<(String, String, String)>, Vec<mcp_tester::TestResult>) {
    let (pairs, by_uri) = dedup_widget_uris(app_tools);
    let mut html_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut failures: Vec<mcp_tester::TestResult> = Vec::new();
    // Read each UNIQUE uri exactly once (REVISION MED-6).
    for uri in by_uri.keys() {
        match tester.read_resource(uri).await {
            Ok(result) => match first_text_body(&result) {
                Some(text) => {
                    html_cache.insert(uri.clone(), Some(text));
                },
                None => {
                    if verbose {
                        eprintln!(
                            "   {} read_resource({}) returned non-text/empty body — skipping",
                            "⚠".yellow(),
                            uri
                        );
                    }
                    html_cache.insert(uri.clone(), None);
                    failures.push(make_read_failure_result(uri, "non-text or empty body"));
                },
            },
            Err(e) => {
                if verbose {
                    eprintln!(
                        "   {} read_resource({}) failed (continuing): {}",
                        "⚠".yellow(),
                        uri,
                        e
                    );
                }
                html_cache.insert(uri.clone(), None);
                failures.push(make_read_failure_result(uri, &e.to_string()));
            },
        }
    }
    // Fan the cached body back out per (tool, uri) pair (REVISION MED-6).
    let mut bodies: Vec<(String, String, String)> = Vec::with_capacity(pairs.len());
    for (tool_name, uri) in pairs {
        if let Some(Some(html)) = html_cache.get(&uri) {
            bodies.push((tool_name, uri, html.clone()));
        }
    }
    (bodies, failures)
}

/// Walk `result.contents` and return the first text-bearing body, capped at
/// `MAX_WIDGET_BODY_BYTES`. Anything else (Image, Audio, ResourceLink, oversized) → None.
///
/// Variant coverage (per src/types/content.rs:63-111):
/// - `Text` — always returns the text (subject to size cap)
/// - `Resource { text: Some(t), .. }` — returns the embedded text if exposed
/// - `Image`, `Audio`, `ResourceLink` — fall through to catchall `_ => None`
/// - Catchall `_ => None` is INTENTIONAL: future Content variants don't silently
///   leak through as garbage input to the scanner.
fn first_text_body(result: &ReadResourceResult) -> Option<String> {
    let candidate: Option<String> = result.contents.iter().find_map(|c| match c {
        Content::Text { text, .. } => Some(text.clone()),
        Content::Resource { text: Some(t), .. } => Some(t.clone()),
        _ => None,
    });
    match candidate {
        Some(t) if t.len() <= MAX_WIDGET_BODY_BYTES => Some(t),
        _ => None,
    }
}

/// Build a Failed `TestResult` naming the failed widget URI. Used for both
/// network errors and non-text/empty bodies so the user sees them in the
/// report rather than silent skips.
fn make_read_failure_result(uri: &str, reason: &str) -> mcp_tester::TestResult {
    mcp_tester::TestResult {
        name: format!("[{uri}] read_resource"),
        category: mcp_tester::TestCategory::Apps,
        status: TestStatus::Failed,
        duration: Duration::from_secs(0),
        error: Some(format!("Could not read widget body: {reason}")),
        details: Some(
            "[guide:handlers-before-connect] Widget HTML could not be fetched — the server may not register the widget resource, or the body is binary/empty.".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::types::{Content, ReadResourceResult, ToolInfo};

    fn tool_with_resource_uri(name: &str, uri: &str) -> ToolInfo {
        let mut meta = serde_json::Map::new();
        let mut ui = serde_json::Map::new();
        ui.insert(
            "resourceUri".to_string(),
            serde_json::Value::String(uri.to_string()),
        );
        meta.insert("ui".to_string(), serde_json::Value::Object(ui));
        let mut tool = ToolInfo::new(name, None, serde_json::json!({"type": "object"}));
        tool._meta = Some(meta);
        tool
    }

    #[test]
    fn first_text_body_returns_text_variant() {
        let result = ReadResourceResult::new(vec![Content::Text {
            text: "<html/>".to_string(),
        }]);
        assert_eq!(first_text_body(&result), Some("<html/>".to_string()));
    }

    #[test]
    fn first_text_body_skips_image_variant_returns_none() {
        let result = ReadResourceResult::new(vec![Content::Image {
            data: "iVBORw0KGgo=".to_string(),
            mime_type: "image/png".to_string(),
        }]);
        assert_eq!(first_text_body(&result), None);
    }

    #[test]
    fn first_text_body_skips_audio_variant_returns_none() {
        let result = ReadResourceResult::new(vec![Content::Audio {
            data: "AAAA".to_string(),
            mime_type: "audio/wav".to_string(),
            annotations: None,
            meta: None,
        }]);
        assert_eq!(first_text_body(&result), None);
    }

    #[test]
    fn first_text_body_skips_resourcelink_variant_returns_none() {
        use pmcp::types::content::ResourceLinkContent;
        let link = ResourceLinkContent::new("x", "ui://x");
        let result = ReadResourceResult::new(vec![Content::ResourceLink(Box::new(link))]);
        assert_eq!(first_text_body(&result), None);
    }

    #[test]
    fn over_10mb_body_skipped() {
        let big = "x".repeat(11_000_000);
        let result = ReadResourceResult::new(vec![Content::Text { text: big }]);
        assert_eq!(first_text_body(&result), None);
    }

    #[test]
    fn dedup_widget_uris_collapses_duplicates() {
        let a = tool_with_resource_uri("alpha", "ui://x");
        let b = tool_with_resource_uri("beta", "ui://x");
        let c = tool_with_resource_uri("gamma", "ui://y");
        let tools_vec = [a, b, c];
        let refs: Vec<&ToolInfo> = tools_vec.iter().collect();
        let (pairs, by_uri) = dedup_widget_uris(&refs);
        assert_eq!(pairs.len(), 3, "all 3 (tool, uri) pairs preserved");
        assert_eq!(by_uri.len(), 2, "only 2 unique URIs after dedup");
        let xs = by_uri.get("ui://x").expect("ui://x present");
        let ys = by_uri.get("ui://y").expect("ui://y present");
        assert_eq!(xs.len(), 2, "ui://x maps to 2 tool names");
        assert_eq!(ys.len(), 1, "ui://y maps to 1 tool name");
    }

    #[tokio::test]
    async fn read_widget_bodies_returns_empty_for_no_app_tools() {
        // We cannot easily construct a ServerTester for a unit test, so this
        // case is exercised by an empty input slice via dedup_widget_uris,
        // which is the load-bearing path. The full async helper is exercised
        // by the `apps_helpers.rs` integration test.
        let empty: Vec<&ToolInfo> = Vec::new();
        let (pairs, by_uri) = dedup_widget_uris(&empty);
        assert!(pairs.is_empty());
        assert!(by_uri.is_empty());
    }

    #[test]
    fn make_read_failure_result_emits_failed_status_with_uri_in_name() {
        let r = make_read_failure_result("ui://broken", "boom");
        assert_eq!(r.status, TestStatus::Failed);
        assert!(r.name.contains("ui://broken"));
        assert!(r.error.as_ref().unwrap().contains("boom"));
    }
}
