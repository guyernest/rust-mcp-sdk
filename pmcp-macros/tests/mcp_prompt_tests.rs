//! Integration tests for standalone `#[mcp_prompt]` functions.
//!
//! These tests verify full macro expansion: compilation, argument schema generation,
//! prompt handler implementation, and builder registration.

use pmcp::PromptHandler;
use pmcp_macros::mcp_prompt;
use pmcp::types::{Content, GetPromptResult, PromptMessage};
use pmcp::State;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

// === Shared argument types ===

#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    /// The programming language
    language: String,
    /// Code to review
    code: String,
}

// === Test 1: Minimal async prompt with typed args ===

#[mcp_prompt(description = "Review code for issues")]
async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Review this {} code:\n{}",
            args.language, args.code
        )))],
        Some("Code review".to_string()),
    ))
}

#[tokio::test]
async fn test_code_review_handle() {
    let prompt = code_review();
    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert("code".to_string(), "fn main() {}".to_string());
    let extra = pmcp::RequestHandlerExtra::default();
    let result = prompt.handle(args, extra).await.unwrap();
    assert_eq!(result.messages.len(), 1);
}

// === Test 2: Metadata has correct name, description, and arguments ===

#[test]
fn test_code_review_metadata() {
    let prompt = code_review();
    let meta = prompt.metadata().expect("metadata should exist");
    assert_eq!(meta.name, "code_review");
    assert_eq!(
        meta.description.as_deref(),
        Some("Review code for issues")
    );
    let arguments = meta.arguments.as_ref().expect("should have arguments");
    assert_eq!(arguments.len(), 2);

    let arg_names: Vec<&str> = arguments.iter().map(|a| a.name.as_str()).collect();
    assert!(arg_names.contains(&"language"), "should have 'language' arg");
    assert!(arg_names.contains(&"code"), "should have 'code' arg");

    // Both fields are required (non-Option).
    for arg in arguments {
        assert!(arg.required, "arg '{}' should be required", arg.name);
    }

    // Check description derivation from doc comments via JsonSchema.
    let lang_arg = arguments.iter().find(|a| a.name == "language").unwrap();
    assert_eq!(
        lang_arg.description.as_deref(),
        Some("The programming language")
    );
}

// === Test 3: No-arg prompt ===

#[mcp_prompt(description = "Get system status")]
async fn system_status() -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::system(Content::text("System is healthy"))],
        None,
    ))
}

#[tokio::test]
async fn test_no_arg_prompt() {
    let prompt = system_status();
    let extra = pmcp::RequestHandlerExtra::default();
    let result = prompt.handle(HashMap::new(), extra).await.unwrap();
    assert_eq!(result.messages.len(), 1);
}

#[test]
fn test_no_arg_prompt_metadata() {
    let prompt = system_status();
    let meta = prompt.metadata().expect("metadata should exist");
    assert_eq!(meta.name, "system_status");
    assert!(meta.arguments.is_none(), "no-arg prompt should have no arguments");
}

// === Test 4: Optional arguments (Option<T> fields) ===

#[derive(Debug, Deserialize, JsonSchema)]
struct SummarizeArgs {
    /// The text to summarize
    text: String,
    /// Maximum length (optional)
    max_length: Option<String>,
}

#[mcp_prompt(description = "Summarize text")]
async fn summarize(args: SummarizeArgs) -> pmcp::Result<GetPromptResult> {
    let _ = args.max_length;
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Summarize: {}",
            args.text
        )))],
        None,
    ))
}

#[test]
fn test_optional_arg_metadata() {
    let prompt = summarize();
    let meta = prompt.metadata().expect("metadata should exist");
    let arguments = meta.arguments.as_ref().expect("should have arguments");

    let text_arg = arguments.iter().find(|a| a.name == "text").unwrap();
    assert!(text_arg.required, "'text' should be required");

    let max_len_arg = arguments.iter().find(|a| a.name == "max_length").unwrap();
    assert!(
        !max_len_arg.required,
        "'max_length' (Option) should NOT be required"
    );
}

// === Test 5: Name override (D-04) ===

#[mcp_prompt(name = "custom_name", description = "Custom named prompt")]
async fn my_prompt(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!("{}", args.language)))],
        None,
    ))
}

#[test]
fn test_name_override() {
    let prompt = my_prompt();
    let meta = prompt.metadata().expect("metadata should exist");
    assert_eq!(meta.name, "custom_name");
}

// === Test 6: State<T> injection ===

struct Config {
    prefix: String,
}

#[mcp_prompt(description = "Stateful prompt")]
async fn stateful(args: ReviewArgs, config: State<Config>) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "{}: {}",
            config.prefix, args.language
        )))],
        None,
    ))
}

#[tokio::test]
async fn test_state_injection() {
    let prompt = stateful().with_state(Config {
        prefix: "Review".into(),
    });
    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert("code".to_string(), "fn main() {}".to_string());
    let extra = pmcp::RequestHandlerExtra::default();
    let result = prompt.handle(args, extra).await.unwrap();
    assert_eq!(result.messages.len(), 1);
}

// === Test 7: Sync prompt (fn not async fn) ===

#[mcp_prompt(description = "Sync prompt")]
fn sync_prompt() -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(vec![], None))
}

#[tokio::test]
async fn test_sync_prompt() {
    let prompt = sync_prompt();
    let extra = pmcp::RequestHandlerExtra::default();
    let result = prompt.handle(HashMap::new(), extra).await.unwrap();
    assert!(result.messages.is_empty());
}

// === Test 8: RequestHandlerExtra opt-in (D-03) ===

#[mcp_prompt(description = "With extra")]
async fn with_extra(args: ReviewArgs, extra: pmcp::RequestHandlerExtra) -> pmcp::Result<GetPromptResult> {
    let _ = extra;
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(args.language))],
        None,
    ))
}

#[tokio::test]
async fn test_prompt_with_extra() {
    let prompt = with_extra();
    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert("code".to_string(), "fn main() {}".to_string());
    let extra = pmcp::RequestHandlerExtra::default();
    let result = prompt.handle(args, extra).await.unwrap();
    assert_eq!(result.messages.len(), 1);
}

// === Test 9: Registration on ServerBuilder ===

#[test]
fn test_builder_registration() {
    let builder = pmcp::ServerBuilder::new()
        .name("test")
        .version("1.0.0")
        .prompt("code_review", code_review());
    // This compiles = PromptHandler impl is correct
    drop(builder);
}

// === Compile-fail tests (trybuild) ===

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/mcp_prompt_missing_description.rs");
}

// === Property tests (proptest) ===

use proptest::prelude::*;

fn make_test_extra() -> pmcp::RequestHandlerExtra {
    pmcp::RequestHandlerExtra::default()
}

proptest! {
    /// Invariant: missing required args always returns an error, never panics
    #[test]
    fn prop_missing_required_arg_returns_error(
        language in ".*",
        // code is intentionally omitted
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let prompt = code_review();
        // Only provide 'language', omit required 'code'
        let mut args = HashMap::new();
        args.insert("language".to_string(), language);
        let result = rt.block_on(prompt.handle(args, make_test_extra()));
        prop_assert!(result.is_err(), "Missing required arg 'code' must error");
    }

    /// Invariant: metadata arguments mirror struct fields exactly
    #[test]
    fn prop_metadata_mirrors_struct_fields(_seed in 0u32..1000) {
        let prompt = code_review();
        let meta = prompt.metadata().unwrap();
        let arg_names: Vec<&str> = meta.arguments.as_ref().unwrap()
            .iter().map(|a| a.name.as_str()).collect();
        // ReviewArgs has exactly 'language' and 'code'
        prop_assert!(arg_names.contains(&"language"));
        prop_assert!(arg_names.contains(&"code"));
        prop_assert_eq!(arg_names.len(), 2);
    }

    /// Invariant: no panics on arbitrary string input
    #[test]
    fn prop_no_panic_on_arbitrary_input(
        keys in prop::collection::vec("[a-z_]{1,20}", 0..10),
        values in prop::collection::vec(".*", 0..10),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let prompt = code_review();
        let len = keys.len().min(values.len());
        let args: HashMap<String, String> = keys.into_iter().take(len)
            .zip(values.into_iter().take(len))
            .collect();
        // Must not panic -- error is acceptable
        let _ = rt.block_on(prompt.handle(args, make_test_extra()));
    }
}
