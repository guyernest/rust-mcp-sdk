//! # Code Mode GraphQL Example
//!
//! Demonstrates the end-to-end Code Mode flow:
//! 1. Define a server struct with `#[derive(CodeMode)]`
//! 2. Register code mode tools via `register_code_mode_tools(builder)`
//! 3. Validate a GraphQL query (`validate_code`) -- success path
//! 4. Receive an HMAC-signed approval token
//! 5. Execute the validated code with the token (`execute_code`)
//! 6. **Demonstrate rejection**: invalid code that fails validation
//!
//! Uses `NoopPolicyEvaluator` -- **for testing/local development ONLY**.
//! Production servers MUST implement `PolicyEvaluator` with a real
//! authorization backend (e.g., Cedar, AWS Verified Permissions).
//!
//! Run with: `cargo run --example s41_code_mode_graphql --features full`

use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
    ValidationContext, ValidationPipeline,
};
use pmcp_code_mode_derive::CodeMode;
use serde_json::{json, Value};
use std::sync::Arc;

/// A simple GraphQL executor that returns mock data.
///
/// In production, this would execute the GraphQL query against a real backend
/// (e.g., a database, a remote GraphQL service, or an in-process schema).
struct GraphQLExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for GraphQLExecutor {
    async fn execute(
        &self,
        code: &str,
        _variables: Option<&Value>,
    ) -> Result<Value, ExecutionError> {
        // In production, this would execute the GraphQL query against a real backend.
        // For this example, return mock data based on the query.
        Ok(json!({
            "data": {
                "query": code,
                "result": [
                    {"id": "1", "name": "Alice"},
                    {"id": "2", "name": "Bob"},
                ]
            }
        }))
    }
}

/// Server struct annotated with `#[derive(CodeMode)]`.
///
/// The derive macro generates `register_code_mode_tools(builder)` which
/// takes a `ServerCoreBuilder` by value and returns it with `validate_code`
/// and `execute_code` tools registered.
///
/// **Required field names** (v0.1.0 convention):
/// - `code_mode_config`: `CodeModeConfig`
/// - `token_secret`: `TokenSecret`
/// - `policy_evaluator`: `Arc<impl PolicyEvaluator>`
/// - `code_executor`: `Arc<impl CodeExecutor>`
#[derive(CodeMode)]
struct MyGraphQLServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    #[allow(dead_code)]
    // Required by #[derive(CodeMode)] convention; used when policy evaluation is wired
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<GraphQLExecutor>,
}

#[tokio::main]
async fn main() {
    // 1. Create the server with Code Mode configuration
    //
    // NOTE: NoopPolicyEvaluator is used here for demonstration.
    // In production, replace with CedarPolicyEvaluator or your custom evaluator.
    let server = MyGraphQLServer {
        code_mode_config: CodeModeConfig::enabled(),
        token_secret: TokenSecret::new(b"example-secret-key-32-bytes!!!!".to_vec()),
        policy_evaluator: Arc::new(NoopPolicyEvaluator::new()),
        code_executor: Arc::new(GraphQLExecutor),
    };

    // 2. Register code mode tools on the builder
    // (In a real server, you would also add other tools and then build/run the server)
    let builder = pmcp::Server::builder();
    #[allow(deprecated)]
    let _builder = server
        .register_code_mode_tools(builder)
        .expect("Failed to register code mode tools");
    println!("Registered validate_code and execute_code tools on builder.");

    // 3. Demonstrate the validation -> execution round trip
    println!("\n=== Code Mode GraphQL Example ===\n");

    // Create a validation pipeline directly to show the flow
    let pipeline = ValidationPipeline::from_token_secret(
        server.code_mode_config.clone(),
        &server.token_secret,
    )
    .expect("Failed to create validation pipeline — check token secret length");

    let context = ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash");

    // --- SUCCESS PATH ---
    println!("--- Success Path: Valid GraphQL Query ---");
    let query = "query { users { id name } }";
    println!("Query: {query}");

    match pipeline.validate_graphql_query(query, &context) {
        Ok(result) => {
            println!("Validation: PASSED (is_valid={})", result.is_valid);
            println!("Risk level: {}", result.risk_level);
            println!(
                "Approval token: {}...",
                result
                    .approval_token
                    .as_ref()
                    .map(|t| &t[..t.len().min(20)])
                    .unwrap_or("none")
            );
            if let Some(ref explanation) = Some(&result.explanation) {
                println!("Explanation: {explanation}");
            }

            // Execute with the approval token
            if result.approval_token.is_some() {
                let exec_result = server.code_executor.execute(query, None).await;
                match exec_result {
                    Ok(data) => println!(
                        "\nExecution result:\n{}",
                        serde_json::to_string_pretty(&data).expect("JSON serialization")
                    ),
                    Err(e) => println!("\nExecution error: {e:?}"),
                }
            }
        },
        Err(e) => {
            println!("Validation: FAILED - {e:?}");
        },
    }

    // --- REJECTION PATH ---
    // Demonstrates that mutations are rejected when allow_mutations is false (the default).
    println!("\n--- Rejection Path: Mutation Blocked by Config ---");
    let mutation = "mutation { createUser(name: \"evil\") { id } }";
    println!("Query: {mutation}");

    match pipeline.validate_graphql_query(mutation, &context) {
        Ok(result) => {
            if result.is_valid {
                println!("Validation: PASSED (unexpected for mutation with default config)");
            } else {
                println!("Validation: REJECTED (expected)");
                for violation in &result.violations {
                    println!("  Violation: {} - {}", violation.rule, violation.message);
                }
                println!("This demonstrates that mutations do NOT receive an approval token.");
                println!(
                    "Approval token present: {}",
                    result.approval_token.is_some()
                );
            }
        },
        Err(e) => {
            println!("Validation: REJECTED (expected) - {e:?}");
            println!("This demonstrates that invalid code does NOT receive an approval token.");
        },
    }

    println!("\n=== Example Complete ===");
}
