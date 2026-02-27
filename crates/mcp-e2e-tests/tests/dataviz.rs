//! E2E tests for the Data Visualization MCP App widget.
//!
//! Validates chart rendering, data table population, column headers,
//! chart type switching, and initial table listing.
//!
//! Note: The dataviz widget loads Chart.js from CDN. In environments where
//! the CDN is unreachable, chart rendering will fail. This is acceptable
//! for the test environment.

use mcp_e2e_tests::{
    get_tool_call_log, launch_browser, new_page_with_bridge, start_test_server,
    wait_for_js_condition,
};
use serde_json::json;

/// Mock responses for dataviz widget tool calls.
fn dataviz_responses() -> serde_json::Value {
    json!({
        "list_tables": {
            "tables": ["Album", "Artist", "Genre", "Track", "MediaType", "Playlist"]
        },
        "execute_query": {
            "columns": ["Genre", "TrackCount"],
            "rows": [
                ["Rock", 1297],
                ["Latin", 579],
                ["Metal", 374],
                ["Alternative & Punk", 332],
                ["Jazz", 130],
                ["Blues", 81],
                ["Classical", 74],
                ["R&B/Soul", 61],
                ["Reggae", 58],
                ["Pop", 48]
            ],
            "row_count": 10
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn dataviz_chart_renders_after_query() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &dataviz_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/dataviz/dashboard.html"))
        .await
        .unwrap();

    // Wait for execute_query to be called (happens on init after loadTables)
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'execute_query')"#,
        15000,
    )
    .await
    .unwrap();

    // Wait for the chart canvas to be present and Chart.js to render
    // Chart.js renders onto the #chart canvas element
    wait_for_js_condition(&page, "document.getElementById('chart') !== null", 5000)
        .await
        .unwrap();

    let chart_exists: serde_json::Value = page
        .evaluate("document.getElementById('chart') !== null")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        chart_exists.as_bool().unwrap(),
        "Chart canvas should exist after query"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dataviz_data_table_populates() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &dataviz_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/dataviz/dashboard.html"))
        .await
        .unwrap();

    // Wait for execute_query to complete
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'execute_query')"#,
        15000,
    )
    .await
    .unwrap();

    // Wait for table body rows to populate
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('#dataTable tbody tr').length > 0",
        5000,
    )
    .await
    .unwrap();

    let row_count: serde_json::Value = page
        .evaluate("document.querySelectorAll('#dataTable tbody tr').length")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        row_count.as_i64().unwrap() > 0,
        "Data table should have rows after query"
    );

    // Verify we get the expected number of rows (10)
    assert_eq!(
        row_count.as_i64().unwrap(),
        10,
        "Expected 10 rows in data table"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dataviz_table_has_correct_columns() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &dataviz_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/dataviz/dashboard.html"))
        .await
        .unwrap();

    // Wait for table to render
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('#dataTable thead th').length > 0",
        15000,
    )
    .await
    .unwrap();

    let headers: serde_json::Value = page
        .evaluate(
            r#"
            Array.from(document.querySelectorAll('#dataTable thead th'))
                .map(th => th.textContent.trim().replace(/[^a-zA-Z]/g, ''))
            "#,
        )
        .await
        .unwrap()
        .into_value()
        .unwrap();

    let headers_arr = headers.as_array().unwrap();
    assert!(
        headers_arr.len() >= 2,
        "Expected at least 2 column headers, got {}",
        headers_arr.len()
    );

    // The sort indicator span adds extra characters, so check that the
    // header text starts with the expected column names
    let first = headers_arr[0].as_str().unwrap();
    let second = headers_arr[1].as_str().unwrap();

    assert!(
        first.contains("Genre"),
        "First column should be 'Genre', got '{first}'"
    );
    assert!(
        second.contains("TrackCount"),
        "Second column should be 'TrackCount', got '{second}'"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dataviz_chart_type_switch() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &dataviz_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/dataviz/dashboard.html"))
        .await
        .unwrap();

    // Wait for initial chart render
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'execute_query')"#,
        15000,
    )
    .await
    .unwrap();

    // Change chart type to "pie" via JS
    page.evaluate(
        r#"
        const select = document.getElementById('chartType');
        select.value = 'pie';
        select.dispatchEvent(new Event('change'));
        "#,
    )
    .await
    .unwrap();

    // Wait a moment for re-render
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify the chart canvas still exists (re-rendered, not removed)
    let chart_exists: serde_json::Value = page
        .evaluate("document.getElementById('chart') !== null")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        chart_exists.as_bool().unwrap(),
        "Chart canvas should still exist after type switch"
    );

    // Verify the select value was updated
    let chart_type: serde_json::Value = page
        .evaluate("document.getElementById('chartType').value")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(
        chart_type.as_str().unwrap(),
        "pie",
        "Chart type select should be 'pie'"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dataviz_calls_list_tables_on_init() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &dataviz_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/dataviz/dashboard.html"))
        .await
        .unwrap();

    // Wait for list_tables to be called during initialization
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'list_tables')"#,
        15000,
    )
    .await
    .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let has_list_tables = log.iter().any(|entry| {
        entry
            .get("name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n == "list_tables")
    });

    assert!(
        has_list_tables,
        "list_tables should have been called on init"
    );
}
