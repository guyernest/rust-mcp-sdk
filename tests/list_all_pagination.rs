//! Integration tests for `Client::list_all_*` auto-pagination
//! (Phase 73, PARITY-CLIENT-01).
//!
//! Drives the shared `mock_paginated::MockTransport` with scripted multi-page
//! response sequences and asserts:
//!   1. Aggregation across pages preserves order (tools + resource_templates).
//!   2. Termination on `next_cursor: None`.
//!   3. `max_iterations` cap enforcement returns `Error::Validation`
//!      (tools + resource_templates — the latter uses the distinct
//!      `resources/templates/list` capability).

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/mock_paginated.rs"]
mod mock_paginated;

use mock_paginated::{
    build_paginated_responses, init_response, MockTransport, PaginationCapability,
};

use pmcp::{types::ClientCapabilities, Client, ClientOptions, Error};
use serde_json::{json, Value};

// --- Tools family ---

#[tokio::test]
async fn list_all_tools_aggregates_multi_page() {
    let pages: Vec<Vec<Value>> = vec![
        vec![json!({"name": "alpha", "description": "t", "inputSchema": {}})],
        vec![json!({"name": "beta",  "description": "t", "inputSchema": {}})],
        vec![json!({"name": "gamma", "description": "t", "inputSchema": {}})],
    ];
    let responses = build_paginated_responses(init_response(), pages, PaginationCapability::Tools);
    let transport = MockTransport::with_responses(responses);
    let mut client = Client::new(transport);
    client
        .initialize(ClientCapabilities::minimal())
        .await
        .unwrap();
    let all = client.list_all_tools().await.expect("list_all_tools ok");
    let names: Vec<_> = all.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

#[tokio::test]
async fn list_all_tools_terminates_on_none_cursor() {
    let pages: Vec<Vec<Value>> = vec![vec![
        json!({"name": "only", "description": "t", "inputSchema": {}}),
    ]];
    let responses = build_paginated_responses(init_response(), pages, PaginationCapability::Tools);
    let mut client = Client::new(MockTransport::with_responses(responses));
    client
        .initialize(ClientCapabilities::minimal())
        .await
        .unwrap();
    let all = client.list_all_tools().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "only");
}

#[tokio::test]
async fn list_all_tools_rejects_on_max_iterations_exceeded() {
    // cap=2 + 3 pages with Some(_) → error on 3rd iteration.
    let pages: Vec<Vec<Value>> = vec![
        vec![json!({"name": "a", "description": "t", "inputSchema": {}})],
        vec![json!({"name": "b", "description": "t", "inputSchema": {}})],
        vec![json!({"name": "c", "description": "t", "inputSchema": {}})],
    ];
    // build_paginated_responses leaves the LAST page with None; override the
    // last page's cursor by scripting an extra dangling page with Some(_).
    // The simplest path: add one more trailing page so ALL three "in-budget"
    // pages carry Some(_).
    let mut pages = pages;
    pages.push(vec![
        json!({"name": "d", "description": "t", "inputSchema": {}}),
    ]);
    let responses = build_paginated_responses(init_response(), pages, PaginationCapability::Tools);
    let opts = ClientOptions::default().with_max_iterations(2);
    let mut client = Client::with_client_options(MockTransport::with_responses(responses), opts);
    client
        .initialize(ClientCapabilities::minimal())
        .await
        .unwrap();
    let err = client.list_all_tools().await.unwrap_err();
    let msg = format!("{err}");
    assert!(
        matches!(err, Error::Validation(_)),
        "expected Validation, got: {msg}"
    );
    assert!(msg.contains("list_all_tools"), "method name missing: {msg}");
    assert!(msg.contains('2'), "cap value missing: {msg}");
}

// --- Resource templates family (distinct capability path) ---

#[tokio::test]
async fn list_all_resource_templates_aggregates_multi_page() {
    let pages: Vec<Vec<Value>> = vec![
        vec![json!({"uriTemplate": "file://{a}", "name": "a"})],
        vec![json!({"uriTemplate": "file://{b}", "name": "b"})],
    ];
    let responses = build_paginated_responses(
        init_response(),
        pages,
        PaginationCapability::ResourceTemplates,
    );
    let mut client = Client::new(MockTransport::with_responses(responses));
    client
        .initialize(ClientCapabilities::minimal())
        .await
        .unwrap();
    let all = client
        .list_all_resource_templates()
        .await
        .expect("list_all_resource_templates ok");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].name, "a");
    assert_eq!(all[1].name, "b");
}

#[tokio::test]
async fn list_all_resource_templates_rejects_on_max_iterations_exceeded() {
    // cap=2 + 4 pages so the first three stay inside the budget with Some(_).
    let pages: Vec<Vec<Value>> = vec![
        vec![json!({"uriTemplate": "file://{a}", "name": "a"})],
        vec![json!({"uriTemplate": "file://{b}", "name": "b"})],
        vec![json!({"uriTemplate": "file://{c}", "name": "c"})],
        vec![json!({"uriTemplate": "file://{d}", "name": "d"})],
    ];
    let responses = build_paginated_responses(
        init_response(),
        pages,
        PaginationCapability::ResourceTemplates,
    );
    let opts = ClientOptions::default().with_max_iterations(2);
    let mut client = Client::with_client_options(MockTransport::with_responses(responses), opts);
    client
        .initialize(ClientCapabilities::minimal())
        .await
        .unwrap();
    let err = client.list_all_resource_templates().await.unwrap_err();
    let msg = format!("{err}");
    assert!(
        matches!(err, Error::Validation(_)),
        "expected Validation, got: {msg}"
    );
    assert!(
        msg.contains("list_all_resource_templates"),
        "method name missing: {msg}"
    );
    assert!(msg.contains('2'), "cap value missing: {msg}");
}
