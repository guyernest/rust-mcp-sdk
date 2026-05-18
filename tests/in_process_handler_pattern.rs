//! BLDR-03 regression anchor: handler-level testing pattern.
//!
//! This integration test exercises the documented handler-level testing
//! pattern (CONTEXT.md D-02 part (b)) against a real built `pmcp::Server`,
//! driving it through the new public `tool_arc` + `get_tool` surface and
//! the existing `prompt_arc` + `get_prompt` surface end-to-end. It is the
//! regression anchor: if a future refactor breaks the pattern that
//! external toolkit authors rely on (Phase 83+ `pmcp-server-toolkit`),
//! this test fails.
//!
//! The pattern shape is:
//!
//! ```text
//! Server::builder().*_arc(name, Arc::new(handler)).build()
//!     -> server.get_*(name).expect(...)
//!     -> handler.handle(args, RequestHandlerExtra::default()).await
//!     -> assert on the result
//! ```
//!
//! This test deliberately bypasses the private JSONRPC dispatch entry
//! point on `Server` because CONTEXT.md D-01 forbids exposing or
//! depending on a public dispatch surface in Phase 82. The pattern
//! exercises handler logic only — the JSONRPC dispatch path that runs
//! `auth_provider`, `tool_authorizer`, and `tool_middleware` is
//! bypassed. The negative acceptance grep targets actual invocation
//! sites and import statements for the private dispatch entry point,
//! not prose descriptions of the design decision.

#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use pmcp::types::{Content, GetPromptResult, PromptMessage, Role};
use pmcp::{PromptHandler, RequestHandlerExtra, Server, ToolHandler};
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Echo tool used by every test in this file.
///
/// Returns `{ "echoed": <args> }` so a property test (added in Task 2)
/// can assert on byte-equality of `handle(...)` outputs across the
/// `tool()` and `tool_arc()` registration paths.
struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "echoed": args }))
    }
}

/// Echo prompt used by the prompt-side round trip and the compose test.
///
/// Emits a single `Role::User` message with `Content::Text { text: "hello" }`
/// so the round-trip test can pattern-match the content variant.
struct EchoPrompt;

#[async_trait]
impl PromptHandler for EchoPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(
            vec![PromptMessage::new(Role::User, Content::text("hello"))],
            Some("echo".to_string()),
        ))
    }
}

/// Behavior 1 + 3 + 4 + 5: Tool round-trip via the new `tool_arc` +
/// `get_tool` surface, including Arc-identity check.
#[tokio::test]
async fn tool_arc_get_tool_handle_round_trip() {
    let tool: Arc<dyn ToolHandler> = Arc::new(EchoTool);
    let retained = Arc::clone(&tool);

    let server = Server::builder()
        .name("test")
        .version("0")
        .tool_arc("echo", tool)
        .build()
        .expect("server build");

    let registered = server.get_tool("echo").expect("registered above");
    // The registered Arc must be the exact one we passed in (no clone-on-insert).
    assert!(Arc::ptr_eq(registered, &retained));

    let result = registered
        .handle(json!({ "msg": "hi" }), RequestHandlerExtra::default())
        .await
        .expect("handle ok");
    assert_eq!(result, json!({ "echoed": { "msg": "hi" } }));

    // Negative: unknown tools return None.
    assert!(server.get_tool("nope").is_none());
}

/// Behavior 2 + 3 + 4 + 5: Prompt round-trip via the new `prompt_arc` +
/// existing `get_prompt` surface, including Arc-identity check.
#[tokio::test]
async fn prompt_arc_get_prompt_handle_round_trip() {
    let prompt: Arc<dyn PromptHandler> = Arc::new(EchoPrompt);
    let retained = Arc::clone(&prompt);

    let server = Server::builder()
        .name("test")
        .version("0")
        .prompt_arc("echo", prompt)
        .build()
        .expect("server build");

    let registered = server.get_prompt("echo").expect("registered above");
    assert!(Arc::ptr_eq(registered, &retained));

    let result = registered
        .handle(HashMap::new(), RequestHandlerExtra::default())
        .await
        .expect("handle ok");

    assert_eq!(result.description.as_deref(), Some("echo"));
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].role, Role::User);
    match &result.messages[0].content {
        Content::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Content::Text, got {other:?}"),
    }

    // Negative: unknown prompts return None.
    assert!(server.get_prompt("nope").is_none());
}

/// Proves the two registration paths don't interfere: register BOTH a
/// tool via `tool_arc` AND a prompt via `prompt_arc` on a single builder,
/// build once, and verify BOTH accessors return their handlers and both
/// handlers produce their expected outputs.
#[tokio::test]
async fn tool_arc_and_prompt_arc_compose_on_same_builder() {
    let server = Server::builder()
        .name("compose")
        .version("0")
        .tool_arc("echo", Arc::new(EchoTool) as Arc<dyn ToolHandler>)
        .prompt_arc("echo", Arc::new(EchoPrompt) as Arc<dyn PromptHandler>)
        .build()
        .expect("server build");

    let tool = server.get_tool("echo").expect("tool registered");
    let prompt = server.get_prompt("echo").expect("prompt registered");

    let tool_result = tool
        .handle(json!({ "k": "v" }), RequestHandlerExtra::default())
        .await
        .expect("tool handle ok");
    assert_eq!(tool_result, json!({ "echoed": { "k": "v" } }));

    let prompt_result = prompt
        .handle(HashMap::new(), RequestHandlerExtra::default())
        .await
        .expect("prompt handle ok");
    assert_eq!(prompt_result.messages.len(), 1);
    match &prompt_result.messages[0].content {
        Content::Text { text } => assert_eq!(text, "hello"),
        other => panic!("expected Content::Text, got {other:?}"),
    }

    // Public sanity checks: both registries report presence.
    assert!(server.has_tool("echo"));
    assert!(server.has_prompt("echo"));
}

// Property test (Task 2): observational equivalence of `tool()` and
// `tool_arc()`. The two registration paths must produce servers whose
// `get_tool(name).handle(args, extra)` outputs are byte-equal for the
// same `args`. The internal `capabilities` field is private on `Server`
// and is therefore not observable from this integration test; the
// capability-shape equivalence invariant lives in Plan 01 Task 3's
// crate-internal `#[cfg(test)]` unit test (which has access to private
// fields).
//
// proptest = "1.7" is confirmed in Cargo.toml [dev-dependencies] — this
// test depends on it as a hard precondition (no alternative path).
proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn tool_and_tool_arc_produce_observable_equivalence(value in any::<String>()) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let server_a = Server::builder()
                .name("a")
                .version("0")
                .tool("echo", EchoTool)
                .build()
                .unwrap();
            let server_b = Server::builder()
                .name("b")
                .version("0")
                .tool_arc("echo", Arc::new(EchoTool))
                .build()
                .unwrap();

            let args = json!({ "v": value.clone() });
            let result_a = server_a
                .get_tool("echo")
                .unwrap()
                .handle(args.clone(), RequestHandlerExtra::default())
                .await
                .unwrap();
            let result_b = server_b
                .get_tool("echo")
                .unwrap()
                .handle(args, RequestHandlerExtra::default())
                .await
                .unwrap();

            prop_assert_eq!(result_a, result_b);
            // Public sanity: both servers must report has_tool("echo").
            prop_assert!(server_a.has_tool("echo"));
            prop_assert!(server_b.has_tool("echo"));
            Ok(())
        })?;
    }
}
