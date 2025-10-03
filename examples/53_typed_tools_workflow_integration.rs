//! Typed Tools + Workflow Prompts Integration Example
//!
//! This example demonstrates the integration of two powerful features:
//! - Typed tools with automatic JSON schema generation
//! - Workflow-based prompts with type-safe handles
//!
//! Key features demonstrated:
//! 1. Using `TypedTool` for automatic schema generation
//! 2. Using `.prompt_workflow()` for validated workflow registration
//! 3. Automatic ExpansionContext building from typed tools
//! 4. Type-safe workflow construction with handles
//!
//! The example creates a code review workflow that uses typed tools.

#![cfg(feature = "schema-generation")]

use pmcp::server::workflow::{InternalPromptMessage, SequentialWorkflow};
use pmcp::types::Role;
use pmcp::{RequestHandlerExtra, Result, Server};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ============================================================================
// Typed Tools with Automatic Schema Generation
// ============================================================================

/// Code analyzer tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AnalyzeCodeInput {
    /// Source code to analyze
    code: String,
    /// Programming language
    #[serde(default = "default_language")]
    language: String,
    /// Analysis depth (1-3)
    #[serde(default = "default_depth")]
    depth: u8,
}

fn default_language() -> String {
    "rust".to_string()
}

fn default_depth() -> u8 {
    2
}

/// Code review tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct ReviewCodeInput {
    /// Code analysis results
    analysis: String,
    /// Review focus areas
    focus: Vec<ReviewFocus>,
}

/// Review focus areas
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ReviewFocus {
    Security,
    Performance,
    Maintainability,
    Style,
}

/// Code formatter tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FormatCodeInput {
    /// Code to format
    code: String,
    /// Formatting style
    #[serde(default)]
    style: FormattingStyle,
}

/// Formatting styles
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum FormattingStyle {
    #[default]
    Standard,
    Compact,
    Verbose,
}

// ============================================================================
// Tool Implementations
// ============================================================================

async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code analysis
    Ok(json!({
        "language": input.language,
        "depth": input.depth,
        "issues_found": 3,
        "complexity_score": 7.5,
        "analysis": format!(
            "Analyzed {} lines of {} code at depth {}. Found 3 potential issues.",
            input.code.lines().count(),
            input.language,
            input.depth
        )
    }))
}

async fn review_code(input: ReviewCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code review
    Ok(json!({
        "review_summary": format!(
            "Reviewed code with focus on: {:?}. {}",
            input.focus,
            input.analysis
        ),
        "recommendations": [
            "Consider refactoring complex functions",
            "Add more inline documentation",
            "Improve error handling"
        ]
    }))
}

async fn format_code(input: FormatCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code formatting
    let formatted = match input.style {
        FormattingStyle::Compact => input.code.replace("    ", "  "),
        FormattingStyle::Verbose => input
            .code
            .lines()
            .map(|l| format!("    {}", l))
            .collect::<Vec<_>>()
            .join("\n"),
        FormattingStyle::Standard => input.code.clone(),
    };

    Ok(json!({
        "formatted_code": formatted,
        "style": format!("{:?}", input.style)
    }))
}

// ============================================================================
// Workflow Creation
// ============================================================================

/// Create a code review workflow using typed tool handles
///
/// This workflow demonstrates:
/// - Type-safe tool references with ToolHandle
/// - Workflow validation at build time
/// - Automatic schema inheritance from typed tools
fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review_workflow",
        "Comprehensive code review with analysis, review, and formatting"
    )
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language (default: rust)", false)
    .instruction(InternalPromptMessage::new(
        Role::System,
        "You are an expert code reviewer. Use the provided tools to analyze, review, and format code."
    ))
    .instruction(InternalPromptMessage::new(
        Role::User,
        "Please review the provided code thoroughly, focusing on security, performance, and maintainability."
    ))
    // Note: The workflow doesn't execute tools - it just provides instructions
    // The actual tool execution is handled by the LLM/client
}

// ============================================================================
// Server Setup
// ============================================================================

fn main() -> Result<()> {
    println!("=== Typed Tools + Workflow Prompts Integration ===\n");

    // Build server with typed tools and workflow prompt
    let _server = Server::builder()
        .name("code-review-server")
        .version("1.0.0")
        // Register typed tools (automatic schema generation)
        .tool_typed("analyze_code", analyze_code)
        .tool_typed("review_code", review_code)
        .tool_typed("format_code", format_code)
        // Register workflow-based prompt (automatic validation)
        .prompt_workflow(create_code_review_workflow())?
        .build()?;

    println!("✓ Server built successfully");
    println!("\nRegistered typed tools:");
    println!("  - analyze_code: Analyzes source code");
    println!("  - review_code: Reviews code with specific focus areas");
    println!("  - format_code: Formats code with different styles");

    println!("\nRegistered workflow prompts:");
    println!("  - code_review_workflow: Multi-step code review process");

    println!("\n=== Key Features Demonstrated ===\n");

    println!("1. Typed Tools:");
    println!("   - Automatic JSON schema generation from Rust types");
    println!("   - Type-safe input validation");
    println!("   - Enum support for constrained values");
    println!("   - Default values and optional fields");

    println!("\n2. Workflow Prompts:");
    println!("   - Type-safe tool handles (ToolHandle)");
    println!("   - Automatic workflow validation");
    println!("   - Prompt argument definitions");
    println!("   - Instruction message composition");

    println!("\n3. Integration Benefits:");
    println!("   - ExpansionContext automatically built from typed tools");
    println!("   - Tool schemas available for workflow expansion");
    println!("   - Single source of truth for tool definitions");
    println!("   - Compile-time type safety + runtime validation");

    println!("\n=== Server Capabilities ===\n");

    // Demonstrate that tools are properly registered with schemas
    println!("The server provides:");
    println!("  - tools/list: Returns typed tools with auto-generated schemas");
    println!("  - prompts/list: Returns workflow prompts with argument schemas");
    println!("  - tools/call: Executes typed tools with validated inputs");
    println!("  - prompts/get: Returns workflow instructions for LLM");

    println!("\n=== Example Workflow Usage ===\n");

    println!("1. Client calls prompts/get with:");
    println!("   {{");
    println!("     \"name\": \"code_review_workflow\",");
    println!("     \"arguments\": {{");
    println!("       \"code\": \"fn main() {{ println!(\\\"Hello\\\"); }}\",");
    println!("       \"language\": \"rust\"");
    println!("     }}");
    println!("   }}");

    println!("\n2. Server returns workflow instructions to LLM");

    println!("\n3. LLM calls tools in sequence:");
    println!("   - tools/call: analyze_code");
    println!("   - tools/call: review_code");
    println!("   - tools/call: format_code");

    println!("\n4. Each tool call uses the auto-generated schema");
    println!("   for validation and type coercion");

    println!("\n=== Migration Guide ===\n");

    println!("Migrating from plain tools to typed tools + workflows:");
    println!("1. Define input types with serde + schemars derives");
    println!("2. Use .tool_typed() instead of .tool()");
    println!("3. Create workflows with ToolHandle references");
    println!("4. Use .prompt_workflow() for automatic validation");
    println!("5. Remove manual ExpansionContext building");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typed_tool_schemas_generated() {
        let server = Server::builder()
            .name("test")
            .version("1.0.0")
            .tool_typed("analyze_code", analyze_code)
            .build()
            .expect("Server should build");

        // Build registries to verify tool schemas are available
        let (tools, _resources) = server.build_expansion_registries();

        assert!(tools.contains_key("analyze_code"));
        let tool_info = &tools["analyze_code"];
        assert_eq!(tool_info.name, "analyze_code");
        assert!(!tool_info.input_schema.is_null());

        // Verify schema has expected properties
        let schema = &tool_info.input_schema;
        let properties = schema
            .get("properties")
            .expect("Schema should have properties");
        assert!(properties.get("code").is_some());
        assert!(properties.get("language").is_some());
        assert!(properties.get("depth").is_some());
    }

    #[test]
    fn test_workflow_validation_passes() {
        let workflow = create_code_review_workflow();

        // Workflow should validate successfully
        assert!(workflow.validate().is_ok());

        // Verify workflow properties
        assert_eq!(workflow.name(), "code_review_workflow");
        assert_eq!(workflow.arguments().len(), 2);
        assert!(workflow.arguments().contains_key(&"code".into()));
        assert!(workflow.arguments().contains_key(&"language".into()));
    }

    #[test]
    fn test_server_builder_with_workflow() {
        // Test that .prompt_workflow() works correctly
        let result = Server::builder()
            .name("test")
            .version("1.0.0")
            .tool_typed("analyze_code", analyze_code)
            .prompt_workflow(create_code_review_workflow());

        assert!(result.is_ok());

        let server = result.unwrap().build().expect("Server should build");

        // Verify registries are built correctly
        let (tools, _) = server.build_expansion_registries();
        assert!(tools.contains_key("analyze_code"));
    }
}
