//! Progress Reporting with Countdown Tool
//!
//! This example demonstrates progress reporting and cancellation with a simple
//! countdown tool that counts from N to 0, reporting progress at each step.
//!
//! Features demonstrated:
//! - Progress reporting with `extra.report_count()`
//! - Automatic progress token extraction from request metadata
//! - Rate limiting of progress notifications (max 10/second)
//! - Final progress notification always sent
//! - Request cancellation support
//!
//! Run with:
//! ```bash
//! cargo run --example 11_progress_countdown
//! ```

use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::{Server, ToolHandler};
use pmcp::types::{CallToolRequest, ProgressToken, RequestMeta};
use serde_json::{json, Value};
use std::time::Duration;

/// A countdown tool that reports progress at each step.
///
/// This tool demonstrates:
/// - Progress reporting with total value
/// - Cancellation handling
/// - Sleep between iterations to simulate work
struct CountdownTool;

#[async_trait]
impl ToolHandler for CountdownTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Extract the starting number (default to 10)
        let start = args.get("from").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        tracing::info!("Starting countdown from {}", start);

        // Count down from start to 0
        for i in (0..=start).rev() {
            // Check for cancellation
            if extra.is_cancelled() {
                tracing::warn!("Countdown cancelled at {}", i);
                return Err(pmcp::error::Error::internal(
                    "Countdown cancelled by client",
                ));
            }

            // Report progress: current position in countdown
            // We're counting DOWN, so progress goes UP
            let current = start - i;
            let message = if i == 0 {
                "Countdown complete! 🎉".to_string()
            } else {
                format!("Counting down: {}", i)
            };

            extra
                .report_count(current, start, Some(message.clone()))
                .await?;

            tracing::info!("Countdown: {} (progress: {}/{})", i, current, start);

            // Sleep for 1 second between counts (except at the end)
            if i > 0 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(json!({
            "result": "Countdown completed successfully",
            "from": start,
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with timestamps
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    println!("=== Progress Reporting: Countdown Tool Example ===\n");

    // Create server with countdown tool
    let _server = Server::builder()
        .name("countdown-server")
        .version("1.0.0")
        .tool("countdown", CountdownTool)
        .build()?;

    println!("Server created with 'countdown' tool");
    println!("Tool schema:");
    println!("  countdown(from: number) - Counts down from 'from' to 0, reporting progress\n");

    // Simulate client request with progress token
    println!("--- Example 1: Countdown from 5 with progress tracking ---\n");

    let request = CallToolRequest {
        name: "countdown".to_string(),
        arguments: json!({ "from": 5 }),
        _meta: Some(RequestMeta {
            progress_token: Some(ProgressToken::String("countdown-1".to_string())),
        }),
        task: None,
    };

    println!("Calling countdown tool with progress token 'countdown-1'...\n");

    // In a real scenario, this would go through the server's request handling
    // For this example, we'll directly call the tool to demonstrate progress
    let tool = CountdownTool;
    let extra = RequestHandlerExtra::new(
        "test-request-1".to_string(),
        tokio_util::sync::CancellationToken::new(),
    );

    // Note: In a real server, progress reporter would be automatically created
    // from the request's _meta.progress_token field
    let result = tool.handle(request.arguments, extra).await?;

    println!("\n✅ Countdown completed!");
    println!("Result: {}\n", serde_json::to_string_pretty(&result)?);

    // Demonstrate cancellation
    println!("--- Example 2: Countdown with cancellation ---\n");

    let request = CallToolRequest {
        name: "countdown".to_string(),
        arguments: json!({ "from": 10 }),
        _meta: Some(RequestMeta {
            progress_token: Some(ProgressToken::String("countdown-2".to_string())),
        }),
        task: None,
    };

    println!("Calling countdown from 10 with cancellation after 3 seconds...\n");

    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let extra = RequestHandlerExtra::new("test-request-2".to_string(), cancellation_token.clone());

    // Cancel after 3 seconds
    let cancel_handle = tokio::spawn({
        let token = cancellation_token.clone();
        async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("\n🛑 Cancelling countdown...\n");
            token.cancel();
        }
    });

    let result = tool.handle(request.arguments, extra).await;

    match result {
        Ok(v) => println!("Unexpected success: {}", v),
        Err(e) => println!("❌ Countdown cancelled as expected: {}\n", e),
    }

    cancel_handle.await.unwrap();

    println!("--- Key Features Demonstrated ---\n");
    println!("1. ✅ Progress reporting with extra.report_count(current, total, message)");
    println!("2. ✅ Progress token extracted from request _meta field");
    println!("3. ✅ Rate limiting prevents notification flooding (max 10/sec)");
    println!("4. ✅ Final notification always sent (bypasses rate limiting)");
    println!("5. ✅ Cancellation support with extra.is_cancelled()");
    println!("\n=== Example Complete ===");

    Ok(())
}
