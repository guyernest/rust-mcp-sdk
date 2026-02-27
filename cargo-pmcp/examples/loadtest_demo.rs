//! Demonstrates the loadtest module types: config parsing, error handling, and metrics recording.
//!
//! Run with: `cargo run --example loadtest_demo`

use std::time::Duration;

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::error::McpError;
use cargo_pmcp::loadtest::metrics::{MetricsRecorder, OperationType, RequestSample};

fn main() {
    // 1. Parse a TOML config
    let toml_content = r#"
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 70
tool = "echo"
arguments = { text = "hello" }

[[scenario]]
type = "resources/read"
weight = 20
uri = "file:///data/config.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
"#;

    let config = LoadTestConfig::from_toml(toml_content).expect("Config should parse");

    println!("Parsed config:");
    println!("  Virtual users: {}", config.settings.virtual_users);
    println!("  Duration: {}s", config.settings.duration_secs);
    println!("  Timeout: {:?}", config.settings.timeout_as_duration());
    println!("  Scenario steps: {}", config.scenario.len());

    for (i, step) in config.scenario.iter().enumerate() {
        println!("  Step {}: weight={}", i + 1, step.weight());
    }

    // 2. Demonstrate error classification
    let err = McpError::JsonRpc {
        code: -32601,
        message: "Method not found".to_string(),
    };
    println!("\nError classification:");
    println!("  Error: {err}");
    println!("  Category: {}", err.error_category());
    println!("  Is method not found: {}", err.is_method_not_found());

    // 3. Demonstrate metrics recording
    let mut recorder = MetricsRecorder::new(config.settings.expected_interval_ms);

    // Record some synthetic success samples
    for ms in [10, 15, 20, 25, 30, 35, 40, 45, 50, 100] {
        let sample =
            RequestSample::success(OperationType::ToolsCall, Duration::from_millis(ms), None);
        recorder.record(&sample);
    }

    // Record an error sample
    let err_sample = RequestSample::error(
        OperationType::ToolsCall,
        Duration::from_millis(500),
        McpError::Timeout,
        None,
    );
    recorder.record(&err_sample);

    let snapshot = recorder.snapshot();
    println!("\nMetrics snapshot:");
    println!("  Total requests: {}", snapshot.total_requests);
    println!("  Success: {}", snapshot.success_count);
    println!("  Errors: {}", snapshot.error_count);
    println!("  P50: {}ms", snapshot.p50);
    println!("  P95: {}ms", snapshot.p95);
    println!("  P99: {}ms", snapshot.p99);
    println!("  Error rate: {:.1}%", snapshot.error_rate * 100.0);

    println!("\nLoadtest demo complete.");
}
