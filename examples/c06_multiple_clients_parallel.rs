//! Multiple Parallel Clients Example
//!
//! This example demonstrates how to:
//! 1. Create multiple MCP clients in parallel (simulated)
//! 2. Each client performs independent operations
//! 3. Track results from each client separately
//! 4. Handle errors per client
//!
//! Based on TypeScript SDK's multipleClientsParallel.ts concept
//! Note: This is a simplified version due to current SDK limitations
//!
//! Run with: cargo run --example 47_multiple_clients_parallel --features full

use pmcp::{Error, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
struct ClientConfig {
    id: String,
    name: String,
    operation: String,
    data: Value,
}

/// Simulate a client operation
/// In a real implementation, this would be actual MCP client calls
async fn simulate_client_operation(config: ClientConfig) -> (String, Result<Value>) {
    let client_id = config.id.clone();
    info!("[{}] Starting operation: {}", client_id, config.name);

    let result = async {
        // Simulate processing time
        let delay = 100 + (std::ptr::addr_of!(config) as usize % 900);
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;

        // Simulate different types of operations
        match config.operation.as_str() {
            "calculate" => {
                let a = config.data.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let b = config.data.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let op = config
                    .data
                    .get("op")
                    .and_then(|v| v.as_str())
                    .unwrap_or("add");

                let result = match op {
                    "add" => a + b,
                    "multiply" => a * b,
                    "divide" => {
                        if b == 0.0 {
                            return Err(Error::validation("Division by zero"));
                        }
                        a / b
                    },
                    _ => return Err(Error::validation("Unknown operation")),
                };

                Ok(json!({
                    "result": result,
                    "expression": format!("{} {} {} = {}", a, op, b, result)
                }))
            },
            "text_process" => {
                let text = config
                    .data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let operation = config
                    .data
                    .get("operation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("uppercase");

                if text.is_empty() {
                    return Err(Error::validation("Text cannot be empty"));
                }

                let result = match operation {
                    "uppercase" => text.to_uppercase(),
                    "lowercase" => text.to_lowercase(),
                    "reverse" => text.chars().rev().collect(),
                    "length" => text.len().to_string(),
                    _ => return Err(Error::validation("Unknown text operation")),
                };

                Ok(json!({
                    "original": text,
                    "result": result,
                    "operation": operation
                }))
            },
            "data_fetch" => {
                let resource = config
                    .data
                    .get("resource")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");

                // Simulate fetching different types of data
                let data = match resource {
                    "weather" => {
                        let temp = 15 + (resource.len() % 16) as i32;
                        let conditions = ["sunny", "cloudy", "rainy"];
                        let condition = conditions[resource.len() % conditions.len()];
                        let humidity = 30 + (resource.len() % 61) as u32;
                        json!({
                            "temperature": temp,
                            "condition": condition,
                            "humidity": humidity
                        })
                    },
                    "time" => json!({
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "timezone": "UTC"
                    }),
                    "status" => {
                        let uptime = 1000 + (resource.len() % 999000) as u32;
                        json!({
                            "status": "healthy",
                            "uptime": uptime,
                            "version": "1.0.0"
                        })
                    },
                    _ => return Err(Error::validation("Unknown resource type")),
                };

                Ok(json!({
                    "resource": resource,
                    "data": data,
                    "fetched_at": chrono::Utc::now().to_rfc3339()
                }))
            },
            _ => Err(Error::validation("Unknown operation type")),
        }
    }
    .await;

    match &result {
        Ok(_) => info!("[{}] Operation completed successfully", client_id),
        Err(e) => error!("[{}] Operation failed: {}", client_id, e),
    }

    (client_id, result)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Multiple Parallel Clients Example (Simulated)");
    info!("===============================================");

    // Define multiple client configurations
    let client_configs = vec![
        ClientConfig {
            id: "client-1".to_string(),
            name: "Math Calculator Client".to_string(),
            operation: "calculate".to_string(),
            data: json!({
                "a": 15,
                "b": 3,
                "op": "multiply"
            }),
        },
        ClientConfig {
            id: "client-2".to_string(),
            name: "Text Processing Client".to_string(),
            operation: "text_process".to_string(),
            data: json!({
                "text": "Hello World",
                "operation": "reverse"
            }),
        },
        ClientConfig {
            id: "client-3".to_string(),
            name: "Weather Data Client".to_string(),
            operation: "data_fetch".to_string(),
            data: json!({
                "resource": "weather"
            }),
        },
        ClientConfig {
            id: "client-4".to_string(),
            name: "Time Service Client".to_string(),
            operation: "data_fetch".to_string(),
            data: json!({
                "resource": "time"
            }),
        },
        ClientConfig {
            id: "client-5".to_string(),
            name: "Status Check Client".to_string(),
            operation: "data_fetch".to_string(),
            data: json!({
                "resource": "status"
            }),
        },
    ];

    // Create and run all clients in parallel
    let mut join_set = JoinSet::new();

    for config in client_configs {
        join_set.spawn(simulate_client_operation(config));
    }

    // Collect all results
    let mut results: HashMap<String, Result<Value>> = HashMap::new();
    let mut completed = 0;
    let total = join_set.len();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((client_id, operation_result)) => {
                results.insert(client_id.clone(), operation_result);
                completed += 1;
                info!(
                    "✅ Client {} completed ({}/{})",
                    client_id, completed, total
                );
            },
            Err(join_error) => {
                error!("❌ Join error: {}", join_error);
                completed += 1;
            },
        }
    }

    // Display results summary
    info!("\n📊 Results Summary");
    info!("==================");

    for (client_id, result) in &results {
        match result {
            Ok(data) => {
                info!("✅ {}: Success", client_id);
                // Pretty print the result (truncated for readability)
                let result_str = serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| "Invalid JSON".to_string());
                let truncated = if result_str.len() > 200 {
                    format!("{}...", &result_str[..200])
                } else {
                    result_str
                };
                info!("   Result: {}", truncated);
            },
            Err(e) => {
                error!("❌ {}: Error - {}", client_id, e);
            },
        }
    }

    // Statistics
    let successful = results.values().filter(|r| r.is_ok()).count();
    let failed = results.len() - successful;

    info!("\n📈 Statistics");
    info!("=============");
    info!("Total clients: {}", results.len());
    info!("Successful: {}", successful);
    info!("Failed: {}", failed);
    info!(
        "Success rate: {:.1}%",
        (successful as f64 / results.len() as f64) * 100.0
    );

    if successful > 0 {
        info!("✨ Multiple parallel clients simulation completed successfully!");
        info!("💡 This demonstrates parallel operation handling patterns for MCP clients");
    } else {
        warn!("⚠️  All client operations failed");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_creation() {
        let config = ClientConfig {
            id: "test-1".to_string(),
            name: "Test Client".to_string(),
            operation: "calculate".to_string(),
            data: json!({"a": 1, "b": 2, "op": "add"}),
        };

        assert_eq!(config.id, "test-1");
        assert_eq!(config.name, "Test Client");
        assert_eq!(config.operation, "calculate");
    }

    #[tokio::test]
    async fn test_calculate_operation() {
        let config = ClientConfig {
            id: "test-calc".to_string(),
            name: "Test Calculator".to_string(),
            operation: "calculate".to_string(),
            data: json!({"a": 10, "b": 5, "op": "add"}),
        };

        let (client_id, result) = simulate_client_operation(config).await;
        assert_eq!(client_id, "test-calc");
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data["result"], 15);
    }

    #[tokio::test]
    async fn test_text_operation() {
        let config = ClientConfig {
            id: "test-text".to_string(),
            name: "Test Text".to_string(),
            operation: "text_process".to_string(),
            data: json!({"text": "hello", "operation": "uppercase"}),
        };

        let (client_id, result) = simulate_client_operation(config).await;
        assert_eq!(client_id, "test-text");
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data["result"], "HELLO");
    }

    #[tokio::test]
    async fn test_data_fetch_operation() {
        let config = ClientConfig {
            id: "test-fetch".to_string(),
            name: "Test Fetch".to_string(),
            operation: "data_fetch".to_string(),
            data: json!({"resource": "time"}),
        };

        let (client_id, result) = simulate_client_operation(config).await;
        assert_eq!(client_id, "test-fetch");
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data["resource"], "time");
        assert!(data["data"]["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_invalid_operation() {
        let config = ClientConfig {
            id: "test-invalid".to_string(),
            name: "Test Invalid".to_string(),
            operation: "invalid_op".to_string(),
            data: json!({}),
        };

        let (client_id, result) = simulate_client_operation(config).await;
        assert_eq!(client_id, "test-invalid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_division_by_zero() {
        let config = ClientConfig {
            id: "test-div-zero".to_string(),
            name: "Test Division".to_string(),
            operation: "calculate".to_string(),
            data: json!({"a": 10, "b": 0, "op": "divide"}),
        };

        let (client_id, result) = simulate_client_operation(config).await;
        assert_eq!(client_id, "test-div-zero");
        assert!(result.is_err());
    }
}
