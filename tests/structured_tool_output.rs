//! Structured tool output bridge acceptance gate.
//!
//! Per the MCP spec, a tool that declares an `outputSchema` SHOULD return
//! `structuredContent` conforming to it. Before this gate, the SDK had both
//! halves of the vocabulary and no bridge: `ToolInfo::output_schema` was
//! published in `tools/list`, but the Payload-path dispatchers stringified
//! every result into `content[0].text` and only set `structured_content` for
//! widget tools.
//!
//! These tests prove:
//! 1. `CallToolResult::structured` / `structured_with_text` — the success-side
//!    counterparts of `CallToolResult::rejected`: one value, one call, both
//!    voices (text for text-only clients, `structuredContent` for
//!    structured-aware clients).
//! 2. Both native dispatchers (high-level `Server` over an in-process duplex
//!    transport, and `ServerCore` via a server pump) auto-emit
//!    `structuredContent` for Payload-path tools whose cached `ToolInfo`
//!    declares an `output_schema` (e.g. `TypedToolWithOutput`), round-tripping
//!    the exact handler value in BOTH voices.
//! 3. Tools without a declared `output_schema` keep today's text-only
//!    envelope on both dispatchers (no behavior change for existing code).

#![cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::Arc;

use duplex::{call_via_core, call_via_server};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::typed_tool::{TypedTool, TypedToolWithOutput};
use pmcp::types::{CallToolResult, Content};
use pmcp::{Server, ToolHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Extract the single text block from a result's content.
fn text_of(result: &CallToolResult) -> &str {
    match result
        .content
        .first()
        .expect("result carries at least one content block")
    {
        Content::Text { text } => text,
        other => panic!("expected text content, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Typed fixture tool: declares an outputSchema via TypedToolWithOutput.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct ProposeArgs {
    /// Corpus to propose a schema for.
    corpus: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ProposedSchema {
    /// Entity type names discovered in the corpus.
    entities: Vec<String>,
    /// Total entity count.
    count: u32,
}

fn propose_schema_tool() -> impl ToolHandler {
    TypedToolWithOutput::new("propose_schema", |args: ProposeArgs, _extra| {
        Box::pin(async move {
            let _ = args.corpus;
            Ok(ProposedSchema {
                entities: vec!["Person".to_string(), "Company".to_string()],
                count: 2,
            })
        })
    })
    .with_description("Propose a graph schema for a corpus")
}

/// The exact JSON value `propose_schema_tool` emits, for round-trip asserts.
fn expected_proposed_schema() -> Value {
    json!({ "entities": ["Person", "Company"], "count": 2 })
}

/// A schema-less tool (input typing only) — the regression control.
fn text_only_tool() -> impl ToolHandler {
    TypedTool::new("text_only", |args: ProposeArgs, _extra| {
        Box::pin(async move {
            let _ = args.corpus;
            Ok(json!({ "plain": true }))
        })
    })
}

// ---------------------------------------------------------------------------
// 1. Constructors: structured / structured_with_text.
// ---------------------------------------------------------------------------

#[test]
fn structured_dual_emits_one_value_in_both_voices() {
    let value = expected_proposed_schema();
    let result = CallToolResult::structured(value.clone());

    assert!(!result.is_error, "structured() is a success result");
    assert_eq!(
        result.structured_content,
        Some(value.clone()),
        "structuredContent carries the value verbatim"
    );
    let parsed: Value =
        serde_json::from_str(text_of(&result)).expect("text voice is valid JSON of the value");
    assert_eq!(parsed, value, "text voice round-trips to the same value");
}

#[test]
fn structured_with_text_separates_the_two_voices() {
    let value = expected_proposed_schema();
    let result = CallToolResult::structured_with_text(value.clone(), "Proposed 2 entity types.");

    assert!(!result.is_error);
    assert_eq!(result.structured_content, Some(value));
    assert_eq!(
        text_of(&result),
        "Proposed 2 entity types.",
        "human voice differs from the raw serialization"
    );
}

// ---------------------------------------------------------------------------
// 2. Dispatcher auto-emit: declared outputSchema => structuredContent.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_auto_emits_structured_content_for_declared_output_schema() {
    let server = Server::builder()
        .name("structured-output-server")
        .version("1.0.0")
        .tool("propose_schema", propose_schema_tool())
        .build()
        .expect("server builds");

    let result = call_via_server(server, "propose_schema", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content,
        Some(expected_proposed_schema()),
        "high-level Server bridges declared outputSchema to structuredContent"
    );
    let parsed: Value = serde_json::from_str(text_of(&result)).expect("text voice is JSON");
    assert_eq!(
        parsed,
        expected_proposed_schema(),
        "text voice still round-trips for text-only clients"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_core_auto_emits_structured_content_for_declared_output_schema() {
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("structured-output-core")
            .version("1.0.0")
            .tool("propose_schema", propose_schema_tool())
            .build()
            .expect("core builds"),
    );

    let result = call_via_core(core, "propose_schema", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content,
        Some(expected_proposed_schema()),
        "ServerCore bridges declared outputSchema to structuredContent"
    );
    let parsed: Value = serde_json::from_str(text_of(&result)).expect("text voice is JSON");
    assert_eq!(parsed, expected_proposed_schema());
}

// ---------------------------------------------------------------------------
// 3. Regression: no declared outputSchema => text-only envelope, both paths.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_keeps_text_only_envelope_without_output_schema() {
    let server = Server::builder()
        .name("text-only-server")
        .version("1.0.0")
        .tool("text_only", text_only_tool())
        .build()
        .expect("server builds");

    let result = call_via_server(server, "text_only", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content, None,
        "no declared outputSchema: high-level Server emits text only"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_core_keeps_text_only_envelope_without_output_schema() {
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("text-only-core")
            .version("1.0.0")
            .tool("text_only", text_only_tool())
            .build()
            .expect("core builds"),
    );

    let result = call_via_core(core, "text_only", json!({ "corpus": "docs" })).await;

    assert_eq!(
        result.structured_content, None,
        "no declared outputSchema: ServerCore emits text only"
    );
}
