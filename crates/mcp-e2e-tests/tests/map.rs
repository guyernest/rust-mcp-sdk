//! E2E tests for the Map MCP App widget.
//!
//! Validates map container rendering, city search, marker display,
//! city count, and detail panel visibility.
//!
//! Note: The map widget loads Leaflet.js from CDN. In environments where
//! the CDN is unreachable, Leaflet will fail to load and map-specific
//! assertions will fail. This is acceptable per research notes (Pitfall 4).

use mcp_e2e_tests::{
    get_tool_call_log, launch_browser, new_page_with_bridge, start_test_server, wait_for_element,
    wait_for_js_condition,
};
use serde_json::json;

/// Mock responses for map widget tool calls.
fn map_responses() -> serde_json::Value {
    json!({
        "search_cities": {
            "count": 3,
            "cities": [
                {
                    "id": "tokyo",
                    "name": "Tokyo",
                    "country": "Japan",
                    "category": "capital",
                    "coordinates": { "lat": 35.6762, "lon": 139.6503 },
                    "population": 13960000
                },
                {
                    "id": "paris",
                    "name": "Paris",
                    "country": "France",
                    "category": "capital",
                    "coordinates": { "lat": 48.8566, "lon": 2.3522 },
                    "population": 2161000
                },
                {
                    "id": "new_york",
                    "name": "New York",
                    "country": "United States",
                    "category": "financial",
                    "coordinates": { "lat": 40.7128, "lon": -74.0060 },
                    "population": 8336817
                }
            ]
        },
        "get_city_details": {
            "found": true,
            "city": {
                "id": "tokyo",
                "name": "Tokyo",
                "country": "Japan",
                "category": "capital",
                "population": 13960000,
                "description": "Capital city of Japan and one of the most populous metropolitan areas in the world."
            },
            "suggested_view": {
                "center": { "lat": 35.6762, "lon": 139.6503 },
                "zoom": 12
            }
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn map_container_renders() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &map_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/map/map.html"))
        .await
        .unwrap();

    // Wait for the #map container to be present
    wait_for_element(&page, "#map", 10000).await.unwrap();

    let map_exists: serde_json::Value = page
        .evaluate("document.getElementById('map') !== null")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(map_exists.as_bool().unwrap(), "Map container should exist");
}

#[tokio::test(flavor = "multi_thread")]
async fn map_search_populates_city_list() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &map_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/map/map.html"))
        .await
        .unwrap();

    // Wait for search_cities to be called (happens on init)
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'search_cities')"#,
        10000,
    )
    .await
    .unwrap();

    // Wait for city items to render
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('.city-item').length > 0",
        5000,
    )
    .await
    .unwrap();

    let count: serde_json::Value = page
        .evaluate("document.querySelectorAll('.city-item').length")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        count.as_i64().unwrap() > 0,
        "City list should have items after search"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn map_markers_appear_on_map() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &map_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/map/map.html"))
        .await
        .unwrap();

    // Wait for search_cities to complete
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'search_cities')"#,
        10000,
    )
    .await
    .unwrap();

    // Wait for markers to be rendered
    // Leaflet markers are added as divIcon elements with class "custom-marker"
    // or as standard Leaflet marker elements. Check via the markers object.
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('.leaflet-marker-icon').length > 0",
        5000,
    )
    .await
    .unwrap();

    let marker_count: serde_json::Value = page
        .evaluate("document.querySelectorAll('.leaflet-marker-icon').length")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        marker_count.as_i64().unwrap() > 0,
        "Map should have Leaflet markers after search"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn map_city_count_displays() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &map_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/map/map.html"))
        .await
        .unwrap();

    // Wait for search to complete and city count to update
    wait_for_js_condition(
        &page,
        r#"document.getElementById('cityCount').textContent !== '0'"#,
        10000,
    )
    .await
    .unwrap();

    let count_text: serde_json::Value = page
        .evaluate("document.getElementById('cityCount').textContent")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(
        count_text.as_str().unwrap(),
        "3",
        "City count should show 3 cities"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn map_city_detail_shows_on_click() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();

    // Use a response without suggested_view to avoid map.setView()
    // which triggers Leaflet tile loading that blocks CDP evaluate calls
    // in headless mode due to pending network requests.
    let responses = json!({
        "search_cities": {
            "count": 3,
            "cities": [
                {
                    "id": "tokyo",
                    "name": "Tokyo",
                    "country": "Japan",
                    "category": "capital",
                    "coordinates": { "lat": 35.6762, "lon": 139.6503 },
                    "population": 13960000
                },
                {
                    "id": "paris",
                    "name": "Paris",
                    "country": "France",
                    "category": "capital",
                    "coordinates": { "lat": 48.8566, "lon": 2.3522 },
                    "population": 2161000
                },
                {
                    "id": "new_york",
                    "name": "New York",
                    "country": "United States",
                    "category": "financial",
                    "coordinates": { "lat": 40.7128, "lon": -74.0060 },
                    "population": 8336817
                }
            ]
        },
        "get_city_details": {
            "found": true,
            "city": {
                "id": "tokyo",
                "name": "Tokyo",
                "country": "Japan",
                "category": "capital",
                "population": 13960000,
                "description": "Capital city of Japan and one of the most populous metropolitan areas in the world."
            }
        }
    });

    let page = new_page_with_bridge(&browser, &responses).await.unwrap();

    page.goto(format!("http://{addr}/map/map.html"))
        .await
        .unwrap();

    // Wait for city list to populate after search_cities
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('.city-item').length > 0",
        10000,
    )
    .await
    .unwrap();

    // Invoke getCityDetails directly instead of clicking the city item.
    // Clicking triggers marker.openPopup() which causes Leaflet to pan
    // the map view, loading tiles that block CDP evaluate calls in headless mode.
    page.evaluate("getCityDetails('tokyo')").await.unwrap();

    // Wait for get_city_details to be called
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(function(e) { return e.name === 'get_city_details'; })"#,
        10000,
    )
    .await
    .unwrap();

    // Wait for the detail panel to become visible
    wait_for_js_condition(
        &page,
        r#"document.getElementById('cityDetail').classList.contains('visible')"#,
        10000,
    )
    .await
    .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let has_details = log.iter().any(|entry| {
        entry
            .get("name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n == "get_city_details")
    });

    assert!(has_details, "get_city_details should have been called");
}
