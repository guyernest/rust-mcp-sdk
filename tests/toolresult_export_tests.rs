//! Tests for `ToolResult` type alias export functionality
//!
//! This module contains comprehensive tests for the `ToolResult` type alias that was added
//! to resolve GitHub issue #37 where users could not import `ToolResult` from the crate root.

use pmcp::{CallToolResult, Content, ToolResult};

/// Test that `ToolResult` can be imported directly from crate root
#[test]
fn test_toolresult_import() {
    // This test verifies the fix for GitHub issue #37
    // ToolResult should be importable as: use pmcp::ToolResult;

    let result: ToolResult = ToolResult::new(vec![Content::text("test result")]);

    assert_eq!(result.content.len(), 1);
    assert!(!result.is_error);
}

/// Test that `ToolResult` is identical to `CallToolResult`
#[test]
fn test_toolresult_type_equivalence() {
    // Create using CallToolResult
    let call_result = CallToolResult::new(vec![Content::text("equivalent test")]);

    // Create using ToolResult alias
    let tool_result: ToolResult = ToolResult::new(vec![Content::text("equivalent test")]);

    // They should serialize identically
    let call_json = serde_json::to_value(&call_result).unwrap();
    let tool_json = serde_json::to_value(&tool_result).unwrap();

    assert_eq!(call_json, tool_json);
}

/// Test `ToolResult` with various content types
#[test]
fn test_toolresult_content_types() {
    // Test with text content
    let text_result = ToolResult::new(vec![Content::text("text content")]);

    // Test with resource content
    let resource_result = ToolResult::new(vec![Content::resource_with_text(
        "file://test.txt",
        "resource content",
        "text/plain",
    )]);

    assert!(!text_result.content.is_empty());
    assert!(!resource_result.content.is_empty());
}

/// Test `ToolResult` error handling
#[test]
fn test_toolresult_error_cases() {
    let error_result = ToolResult::error(vec![Content::text("An error occurred")]);

    assert!(error_result.is_error);
}

/// Test `ToolResult` serialization and deserialization
#[test]
fn test_toolresult_serde() {
    let original = ToolResult::new(vec![Content::text("serialization test")]);

    // Serialize to JSON
    let json_str = serde_json::to_string(&original).unwrap();

    // Deserialize back
    let deserialized: ToolResult = serde_json::from_str(&json_str).unwrap();

    assert_eq!(original.content.len(), deserialized.content.len());
    assert_eq!(original.is_error, deserialized.is_error);
}

/// Test that function accepting `CallToolResult` also accepts `ToolResult`
#[test]
fn test_toolresult_function_compatibility() {
    fn process_result(result: &CallToolResult) -> bool {
        !result.content.is_empty()
    }

    let tool_result = ToolResult::new(vec![Content::text("compatibility test")]);

    // This should work because ToolResult is an alias for CallToolResult
    assert!(process_result(&tool_result));
}

/// Test default construction of `ToolResult`
#[test]
fn test_toolresult_default() {
    let default_result = ToolResult::default();

    assert!(default_result.content.is_empty());
    assert!(!default_result.is_error);
}

/// Test that `ToolResult` works in generic contexts
#[test]
fn test_toolresult_generic() {
    fn wrap_in_option<T>(value: T) -> T {
        value
    }

    let result = ToolResult::new(vec![Content::text("generic test")]);

    let wrapped = wrap_in_option(result);
    // Test that the wrapping function works
    assert_eq!(wrapped.content.len(), 1);
}
