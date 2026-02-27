//! Demonstration of the LoadTestEngine.
//!
//! Runs a short load test against an MCP server and prints results.
//!
//! Usage:
//!   cargo run --example engine_demo -- http://localhost:3000/mcp
//!
//! If no URL is provided, defaults to http://localhost:3000/mcp.
//! The server is expected to support MCP Streamable HTTP transport.

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::engine::LoadTestEngine;

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:3000/mcp".to_string());

    let toml_config = r#"
[settings]
virtual_users = 2
duration_secs = 5
timeout_ms = 3000

[[scenario]]
type = "tools/call"
weight = 70
tool = "echo"
arguments = { message = "hello from loadtest" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///example/data.json"
"#;

    let config = LoadTestConfig::from_toml(toml_config).expect("Failed to parse config");

    println!("Starting load test against {}", url);
    println!("  VUs: {}", config.settings.virtual_users);
    println!("  Duration: {}s", config.settings.duration_secs);
    println!("  Scenarios: {} steps", config.scenario.len());
    println!();

    let engine = LoadTestEngine::new(config, url).with_no_color(false);

    match engine.run().await {
        Ok(result) => {
            println!();
            println!("=== Load Test Complete ===");
            println!("  Elapsed:     {:.1}s", result.elapsed.as_secs_f64());
            println!("  Total reqs:  {}", result.snapshot.total_requests);
            println!("  Success:     {}", result.snapshot.success_count);
            println!("  Errors:      {}", result.snapshot.error_count);
            println!("  Error rate:  {:.1}%", result.snapshot.error_rate * 100.0);
            println!("  P50:         {}ms", result.snapshot.p50);
            println!("  P95:         {}ms", result.snapshot.p95);
            println!("  P99:         {}ms", result.snapshot.p99);
            let rps = if result.elapsed.as_secs_f64() > 0.0 {
                result.snapshot.total_requests as f64 / result.elapsed.as_secs_f64()
            } else {
                0.0
            };
            println!("  RPS:         {:.1}", rps);
            println!("  Active VUs:  {}", result.final_active_vus);
        },
        Err(e) => {
            eprintln!("Load test failed: {}", e);
            std::process::exit(1);
        },
    }
}
