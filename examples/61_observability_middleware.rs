//! Observability Middleware Example
//!
//! This example demonstrates how to add observability (tracing, metrics, logging)
//! to an MCP server using the built-in observability middleware.
//!
//! # Features Demonstrated
//!
//! - Console backend for development
//! - CloudWatch EMF backend for production
//! - Custom configuration via TOML or environment variables
//! - Trace context propagation
//! - Automatic request/response logging
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example 61_observability_middleware
//! ```
//!
//! # Configuration
//!
//! You can configure observability via:
//! 1. `.pmcp-config.toml` file
//! 2. Environment variables (e.g., `PMCP_OBSERVABILITY_BACKEND=cloudwatch`)
//!
//! Example TOML:
//! ```toml
//! [observability]
//! enabled = true
//! backend = "console"  # or "cloudwatch"
//! sample_rate = 1.0
//!
//! [observability.console]
//! pretty = true
//! verbose = false
//!
//! [observability.cloudwatch]
//! namespace = "MyApp/MCP"
//! emf_enabled = true
//! ```

use async_trait::async_trait;
use pmcp::{
    server::{
        builder::ServerCoreBuilder,
        observability::{ObservabilityConfig, TraceContext},
    },
    Result, ServerCapabilities, ToolHandler,
};
use serde_json::{json, Value};

/// A simple tool that demonstrates observability middleware in action.
#[derive(Debug)]
struct GetWeatherTool;

#[async_trait]
impl ToolHandler for GetWeatherTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value> {
        let city = args
            .get("city")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Simulate some work
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        Ok(json!({
            "city": city,
            "temperature": 72,
            "conditions": "sunny",
            "humidity": 45
        }))
    }
}

/// A tool that simulates an error to demonstrate error tracking.
#[derive(Debug)]
struct FailingTool;

#[async_trait]
impl ToolHandler for FailingTool {
    async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value> {
        // Simulate some work before failing
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        Err(pmcp::Error::internal("Simulated error for demo"))
    }
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .init();

    println!("=== MCP Observability Middleware Example ===\n");

    // Example 1: Development configuration with console output
    println!("1. Creating server with development observability (console output):");
    let dev_config = ObservabilityConfig::development();
    println!("   - Backend: {}", dev_config.backend);
    println!("   - Pretty output: {}", dev_config.console.pretty);
    println!("   - Sample rate: {}", dev_config.sample_rate);

    let _dev_server = ServerCoreBuilder::new()
        .name("dev-weather-server")
        .version("1.0.0")
        .tool("get_weather", GetWeatherTool)
        .tool("failing_tool", FailingTool)
        .capabilities(ServerCapabilities::tools_only())
        .with_observability(dev_config)
        .build()?;

    println!("   Server built successfully!\n");

    // Example 2: Production configuration with CloudWatch
    println!("2. Creating server with production observability (CloudWatch EMF):");
    let prod_config = ObservabilityConfig::production();
    println!("   - Backend: {}", prod_config.backend);
    println!("   - Namespace: {}", prod_config.cloudwatch.namespace);
    println!("   - EMF enabled: {}", prod_config.cloudwatch.emf_enabled);

    let _prod_server = ServerCoreBuilder::new()
        .name("prod-weather-server")
        .version("1.0.0")
        .tool("get_weather", GetWeatherTool)
        .capabilities(ServerCapabilities::tools_only())
        .with_observability(prod_config)
        .build()?;

    println!("   Server built successfully!\n");

    // Example 3: Custom configuration
    println!("3. Creating server with custom observability configuration:");
    let mut custom_config = ObservabilityConfig::default();
    custom_config.sample_rate = 0.5; // Sample 50% of requests
    custom_config.fields.capture_arguments_hash = true;
    custom_config.fields.capture_response_size = true;
    custom_config.metrics.prefix = "myapp".to_string();

    println!("   - Sample rate: 50%");
    println!("   - Capture arguments hash: true");
    println!("   - Metrics prefix: myapp");

    let _custom_server = ServerCoreBuilder::new()
        .name("custom-weather-server")
        .version("1.0.0")
        .tool("get_weather", GetWeatherTool)
        .capabilities(ServerCapabilities::tools_only())
        .with_observability(custom_config)
        .build()?;

    println!("   Server built successfully!\n");

    // Example 4: Disabled observability
    println!("4. Creating server with observability disabled:");
    let disabled_config = ObservabilityConfig::disabled();
    println!("   - Enabled: {}", disabled_config.enabled);

    let _disabled_server = ServerCoreBuilder::new()
        .name("no-obs-server")
        .version("1.0.0")
        .tool("get_weather", GetWeatherTool)
        .capabilities(ServerCapabilities::tools_only())
        .with_observability(disabled_config)
        .build()?;

    println!("   Server built successfully!\n");

    // Example 5: Load from config file or environment
    println!("5. Loading observability config from file/environment:");
    let loaded_config = ObservabilityConfig::load().unwrap_or_else(|e| {
        println!("   Note: Could not load config ({e}), using defaults");
        ObservabilityConfig::default()
    });
    println!("   - Backend: {}", loaded_config.backend);
    println!("   - Enabled: {}", loaded_config.enabled);

    // Example 6: Trace context usage
    println!("\n6. Trace context demonstration:");
    let root_trace = TraceContext::new_root();
    println!("   Root trace:");
    println!("     - trace_id: {}", root_trace.short_trace_id());
    println!("     - span_id: {}", &root_trace.span_id[..8]);
    println!("     - depth: {}", root_trace.depth);

    let child_trace = root_trace.child();
    println!("   Child trace:");
    println!("     - trace_id: {}", child_trace.short_trace_id());
    println!("     - span_id: {}", &child_trace.span_id[..8]);
    println!(
        "     - parent_span_id: {}",
        &child_trace.parent_span_id.as_ref().unwrap()[..8]
    );
    println!("     - depth: {}", child_trace.depth);

    println!("\n=== Example Complete ===");
    println!("\nWhen using these servers with actual MCP requests,");
    println!("you'll see observability output for each tool call:");
    println!("- Request received with trace context");
    println!("- Response sent with duration and status");
    println!("- Metrics emitted (duration, count, errors)");

    Ok(())
}
