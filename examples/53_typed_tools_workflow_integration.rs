//! Typed Tools + Workflow Server-Side Execution Example
//!
//! This example demonstrates the complete integration of typed tools with workflows
//! including **server-side execution** during `prompts/get`.
//!
//! Key features demonstrated:
//! 1. Typed tools with automatic JSON schema generation
//! 2. Workflow-based prompts with server-side tool execution
//! 3. Data binding and flow between workflow steps
//! 4. Conversation trace generation showing execution results
//!
//! **IMPORTANT**: With the new implementation, workflows execute tools SERVER-SIDE
//! during `prompts/get`, not client-side. The server returns a complete conversation
//! trace showing all tool calls and results.

#![cfg(feature = "schema-generation")]

use pmcp::server::workflow::dsl::*;
use pmcp::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};
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
    /// Code analysis results (JSON string)
    analysis: String,
    /// Review focus areas
    focus: Vec<String>,
}

/// Code formatter tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FormatCodeInput {
    /// Code to format
    code: String,
    /// Issues found from review
    issues: Vec<String>,
}

// ============================================================================
// Tool Implementations
// ============================================================================

async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code analysis
    Ok(json!({
        "language": input.language,
        "depth": input.depth,
        "lines_of_code": input.code.lines().count(),
        "issues_found": 3,
        "complexity_score": 7.5,
        "analysis_summary": format!(
            "Analyzed {} lines of {} code at depth {}. Found 3 potential issues.",
            input.code.lines().count(),
            input.language,
            input.depth
        ),
        "issue_details": [
            "Function 'process_data' has high cyclomatic complexity",
            "Missing error handling in 'read_file'",
            "Consider using Result<T> instead of panicking"
        ]
    }))
}

async fn review_code(input: ReviewCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code review based on analysis
    Ok(json!({
        "review_summary": format!(
            "Reviewed code with focus on: {}. Analysis: {}",
            input.focus.join(", "),
            input.analysis
        ),
        "recommendations": [
            "Refactor complex functions into smaller units",
            "Add comprehensive error handling",
            "Improve inline documentation",
            "Add unit tests for edge cases"
        ],
        "priority_issues": input.focus,
        "approval_status": "conditional"
    }))
}

async fn format_code(input: FormatCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Simulate code formatting with issue annotations
    let formatted = input.code.clone();
    let annotations = input
        .issues
        .iter()
        .enumerate()
        .map(|(i, issue)| format!("// TODO (Issue {}): {}", i + 1, issue))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({
        "formatted_code": format!("{}\n\n{}", annotations, formatted),
        "changes_made": "Added TODO comments for identified issues",
        "issues_annotated": input.issues.len()
    }))
}

// ============================================================================
// Workflow Creation with Server-Side Execution
// ============================================================================

/// Create a code review workflow that executes server-side
///
/// This workflow demonstrates:
/// - Server-side tool execution during `prompts/get`
/// - Data binding between workflow steps
/// - Conversation trace generation
///
/// **Execution Flow**:
/// 1. Server receives `prompts/get` request with {code: "...", language: "..."}
/// 2. Server executes all three tools sequentially
/// 3. Server returns conversation trace showing:
///    - User intent
///    - Assistant plan
///    - For each step: Assistant tool call → User result
/// 4. Client/LLM sees complete execution context
fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review_workflow",
        "perform comprehensive code review with analysis and formatting",
    )
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language (default: rust)", false)

    // Step 1: Analyze code (server executes this tool)
    .step(
        WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
            .arg("code", prompt_arg("code"))
            .arg("language", prompt_arg("language"))
            .arg("depth", constant(json!(2)))
            .bind("analysis_result"),  // ← Server stores result
    )

    // Step 2: Review code (uses analysis result from step 1)
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_code"))
            .arg("analysis", field("analysis_result", "analysis_summary"))  // ← Extract field
            .arg("focus", constant(json!(["security", "performance", "maintainability"])))
            .bind("review_result"),  // ← Server stores result
    )

    // Step 3: Format code (uses results from both previous steps)
    .step(
        WorkflowStep::new("format", ToolHandle::new("format_code"))
            .arg("code", prompt_arg("code"))
            .arg("issues", field("review_result", "recommendations"))  // ← Extract field
            .bind("formatted_result"),  // ← Server stores final result
    )
}

// ============================================================================
// Server Setup and Demo
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Typed Tools + Workflow Server-Side Execution ===\n");

    // Build server with typed tools and workflow
    let server = Server::builder()
        .name("code-review-server")
        .version("1.0.0")
        // Register typed tools (automatic schema generation)
        .tool_typed("analyze_code", analyze_code)
        .tool_typed("review_code", review_code)
        .tool_typed("format_code", format_code)
        // Register workflow (enables server-side execution)
        .prompt_workflow(create_code_review_workflow())?
        .build()?;

    println!("✓ Server built successfully\n");

    // =================================================================
    // Demonstrate Server-Side Execution
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 Simulating prompts/get Request");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("Client sends:");
    println!("{{");
    println!("  \"method\": \"prompts/get\",");
    println!("  \"params\": {{");
    println!("    \"name\": \"code_review_workflow\",");
    println!("    \"arguments\": {{");
    println!("      \"code\": \"fn main() {{ println!(\\\"Hello\\\"); }}\",");
    println!("      \"language\": \"rust\"");
    println!("    }}");
    println!("  }}");
    println!("}}\n");

    // Get the workflow prompt handler
    let prompt_handler = server
        .get_prompt("code_review_workflow")
        .expect("Workflow should be registered");

    let mut args = std::collections::HashMap::new();
    args.insert(
        "code".to_string(),
        "fn main() { println!(\"Hello\"); }".to_string(),
    );
    args.insert("language".to_string(), "rust".to_string());

    let extra = pmcp::server::cancellation::RequestHandlerExtra {
        cancellation_token: Default::default(),
        request_id: "demo-request".to_string(),
        session_id: None,
        auth_info: None,
        auth_context: None,
    };

    // Execute workflow server-side
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚙️  Server Executing Workflow (Server-Side)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let result = prompt_handler
        .handle(args, extra)
        .await
        .expect("Workflow execution should succeed");

    println!(
        "Server executed {} tools and generated {} messages\n",
        3,
        result.messages.len()
    );

    // =================================================================
    // Display Conversation Trace
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💬 Generated Conversation Trace (Returned to Client)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    for (i, msg) in result.messages.iter().enumerate() {
        println!("Message {} [{:?}]:", i + 1, msg.role);
        if let pmcp::types::MessageContent::Text { text } = &msg.content {
            // Truncate long messages for display
            let display_text = if text.len() > 200 {
                format!("{}... (truncated)", &text[..200])
            } else {
                text.clone()
            };
            println!("{}\n", display_text);
        }
    }

    // =================================================================
    // Explain the Architecture
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🏗️  Architecture: How It Works");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("1. Client Request:");
    println!("   → Client calls prompts/get with workflow name + arguments\n");

    println!("2. Server-Side Execution:");
    println!("   → Server receives request");
    println!("   → Server executes each workflow step:");
    println!("      • Step 1: analyze_code → stores in 'analysis_result' binding");
    println!("      • Step 2: review_code → uses 'analysis_result', stores in 'review_result'");
    println!("      • Step 3: format_code → uses 'review_result', stores in 'formatted_result'");
    println!("   → Server builds conversation trace showing all executions\n");

    println!("3. Server Response:");
    println!("   → Returns {} messages:", result.messages.len());
    println!("      • Message 1: User intent");
    println!("      • Message 2: Assistant plan");
    println!("      • Messages 3-8: Tool calls + results (3 steps × 2 messages each)");
    println!("   → Client/LLM sees complete execution context\n");

    println!("4. Benefits:");
    println!("   ✓ Single round-trip (fast)");
    println!("   ✓ Deterministic execution");
    println!("   ✓ Complete context for LLM");
    println!("   ✓ Data flow via bindings");
    println!("   ✓ Error handling at each step\n");

    // =================================================================
    // Key Differences from Old Approach
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔄 Key Difference: Server-Side vs Client-Side Execution");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("❌ OLD (Guidance-Only):");
    println!("   → prompts/get returns: 'Use these tools in this order'");
    println!("   → Client/LLM calls tools one by one");
    println!("   → 6+ round trips for 3-step workflow");
    println!("   → No guaranteed execution order\n");

    println!("✅ NEW (Server-Side Execution):");
    println!("   → prompts/get EXECUTES all tools server-side");
    println!("   → Returns complete conversation trace");
    println!("   → 1 round trip total");
    println!("   → Deterministic, sequential execution");
    println!("   → Data flow enforced via bindings\n");

    // =================================================================
    // Summary
    // =================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📚 Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("This example demonstrated:");
    println!("  1. Typed tools with automatic JSON schema generation");
    println!("  2. Workflow with .step() definitions");
    println!("  3. Server-side tool execution during prompts/get");
    println!("  4. Data binding between steps (field extraction)");
    println!("  5. Conversation trace generation");
    println!("  6. Complete workflow execution in single round-trip\n");

    println!("Workflows enable:");
    println!("  ✓ Multi-step orchestration with type safety");
    println!("  ✓ Automatic data flow via bindings");
    println!("  ✓ Efficient server-side execution");
    println!("  ✓ Complete execution context for LLMs");
    println!("  ✓ Deterministic, reproducible workflows\n");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_executes_server_side() {
        let server = Server::builder()
            .name("test")
            .version("1.0.0")
            .tool_typed("analyze_code", analyze_code)
            .tool_typed("review_code", review_code)
            .tool_typed("format_code", format_code)
            .prompt_workflow(create_code_review_workflow())
            .expect("Should register workflow")
            .build()
            .expect("Server should build");

        let prompt_handler = server
            .get_prompt("code_review_workflow")
            .expect("Workflow should be registered");

        let mut args = std::collections::HashMap::new();
        args.insert("code".to_string(), "fn test() {}".to_string());
        args.insert("language".to_string(), "rust".to_string());

        let extra = pmcp::server::cancellation::RequestHandlerExtra {
            cancellation_token: Default::default(),
            request_id: "test".to_string(),
            session_id: None,
            auth_info: None,
            auth_context: None,
        };

        let result = prompt_handler
            .handle(args, extra)
            .await
            .expect("Workflow should execute");

        // Should have 8 messages:
        // 1. User intent
        // 2. Assistant plan
        // 3-4. Step 1 (analyze): call + result
        // 5-6. Step 2 (review): call + result
        // 7-8. Step 3 (format): call + result
        assert_eq!(result.messages.len(), 8, "Should have 8 messages in trace");

        // Verify first message is user intent
        assert_eq!(result.messages[0].role, pmcp::types::Role::User);

        // Verify second message is assistant plan
        assert_eq!(result.messages[1].role, pmcp::types::Role::Assistant);

        // Verify tool results are present
        if let pmcp::types::MessageContent::Text { text } = &result.messages[3].content {
            assert!(text.contains("Tool result"));
        }
    }
}
