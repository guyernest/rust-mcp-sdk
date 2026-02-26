//! E2E browser tests for MCP App widgets.
//!
//! Uses chromiumoxide (CDP) for headless browser automation
//! and axum for embedded widget file serving.
//!
//! # Architecture
//!
//! - **Browser lifecycle**: `launch_browser()` downloads Chromium (cached on disk)
//!   and launches a headless instance with a spawned CDP handler.
//! - **Test server**: `start_test_server()` spins up an axum server on a random
//!   port, serving widget HTML from the workspace examples directories.
//! - **Mock bridge**: `inject_mock_bridge()` injects `window.mcpBridge` via CDP
//!   `addScriptToEvaluateOnNewDocument`, providing canned tool responses.

pub mod bridge;
pub mod server;

pub use bridge::{get_tool_call_log, get_widget_state, inject_mock_bridge};
pub use server::start_test_server;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use chromiumoxide::element::Element;
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use chromiumoxide::page::Page;
use futures::StreamExt;

/// Launch a headless Chromium browser with auto-download.
///
/// Downloads Chromium to a temporary directory on first invocation.
/// Subsequent calls within the same test binary still download (since there
/// is no cross-test-binary shared state), but the fetcher caches the binary
/// on disk so the download is skipped if already present.
pub async fn launch_browser() -> anyhow::Result<Browser> {
    let download_path = std::env::temp_dir().join("mcp-e2e-chromium");
    tokio::fs::create_dir_all(&download_path).await?;

    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&download_path)
            .build()?,
    );
    let info = fetcher.fetch().await?;

    let config = BrowserConfig::builder()
        .chrome_executable(info.executable_path)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .build()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let (browser, mut handler) = Browser::launch(config).await?;
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    Ok(browser)
}

/// Create a new browser page with the mock bridge pre-injected.
///
/// The mock bridge is injected via `evaluate_on_new_document` so it is
/// available before any page scripts execute. The caller should navigate
/// to the desired URL after this function returns.
pub async fn new_page_with_bridge(
    browser: &Browser,
    responses: &serde_json::Value,
) -> anyhow::Result<Page> {
    let page = browser.new_page("about:blank").await?;
    inject_mock_bridge(&page, responses).await?;
    Ok(page)
}

/// Wait for an element matching `selector` to appear in the DOM.
///
/// Polls every 100ms up to `timeout_ms` milliseconds. Returns the element
/// on success or an error describing the timeout.
pub async fn wait_for_element(
    page: &Page,
    selector: &str,
    timeout_ms: u64,
) -> anyhow::Result<Element> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        match page.find_element(selector).await {
            Ok(el) => return Ok(el),
            Err(_) if std::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            },
            Err(e) => {
                anyhow::bail!("Timed out waiting for selector '{}': {}", selector, e);
            },
        }
    }
}

/// Wait for a JS expression to return a truthy value.
///
/// Polls every 100ms up to `timeout_ms` milliseconds.
pub async fn wait_for_js_condition(
    page: &Page,
    expression: &str,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let result: serde_json::Value = page.evaluate(expression).await?.into_value()?;
        if result.as_bool().unwrap_or(false)
            || result.as_i64().is_some_and(|n| n > 0)
            || result.as_str().is_some_and(|s| !s.is_empty())
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("Timed out waiting for JS condition: {}", expression);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Suppress CDP console errors from the page (for cleaner test output).
///
/// Injects a script that overrides `console.error` and `console.warn`
/// to prevent noisy CDN-related errors in headless tests.
pub async fn suppress_console_noise(page: &Page) -> anyhow::Result<()> {
    page.evaluate_on_new_document(AddScriptToEvaluateOnNewDocumentParams::new(
        r#"
        const _origError = console.error;
        const _origWarn = console.warn;
        console.error = function(...args) {
            // Suppress common CDN/network errors in test environment
            const msg = args.map(String).join(' ');
            if (msg.includes('ERR_CONNECTION_REFUSED') ||
                msg.includes('Failed to load resource') ||
                msg.includes('net::ERR')) {
                return;
            }
            _origError.apply(console, args);
        };
        console.warn = function(...args) {
            const msg = args.map(String).join(' ');
            if (msg.includes('ERR_CONNECTION_REFUSED') ||
                msg.includes('Failed to load resource')) {
                return;
            }
            _origWarn.apply(console, args);
        };
        "#,
    ))
    .await?;
    Ok(())
}
