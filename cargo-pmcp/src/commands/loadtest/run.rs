//! `cargo pmcp loadtest run` command implementation.

use anyhow::Result;
use std::path::PathBuf;

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::engine::LoadTestEngine;

/// Execute the `loadtest run` command.
///
/// Loads config (via explicit path or auto-discovery), applies CLI overrides,
/// builds and runs the load test engine, and prints a basic results summary.
pub async fn execute_run(
    url: String,
    config_path: Option<PathBuf>,
    vus: Option<u32>,
    duration: Option<u64>,
    iterations: Option<u64>,
    _no_report: bool,
    no_color: bool,
) -> Result<()> {
    // Step 1: Load config
    let config_file = match config_path {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!(
                    "Config file not found: {}\nUse `cargo pmcp loadtest init` to create one.",
                    path.display()
                );
            }
            path
        }
        None => match discover_config() {
            Some(path) => path,
            None => {
                anyhow::bail!(
                    "No loadtest config found.\n\
                     Run `cargo pmcp loadtest init` to create .pmcp/loadtest.toml,\n\
                     or use `--config path/to/file.toml` to specify one."
                );
            }
        },
    };

    eprintln!("Loading config from: {}", config_file.display());

    let mut config = LoadTestConfig::load(&config_file).map_err(|e| {
        anyhow::anyhow!("Failed to load config '{}': {}", config_file.display(), e)
    })?;

    // Step 2: Apply CLI overrides
    apply_overrides(&mut config, vus, duration);

    // Step 3: Build and run the engine
    let mut engine = LoadTestEngine::new(config, url);
    if let Some(n) = iterations {
        engine = engine.with_iterations(n);
    }
    engine = engine.with_no_color(no_color);

    let result = engine
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("Load test failed: {}", e))?;

    // Step 4: Output results
    // Plan 03-02 will add the terminal summary renderer here.
    // Plan 03-03 will add the JSON report writer here.
    eprintln!();
    eprintln!("Load test complete.");
    eprintln!("  Total requests: {}", result.snapshot.total_requests);
    eprintln!(
        "  Success: {} | Errors: {}",
        result.snapshot.success_count, result.snapshot.error_count
    );
    eprintln!(
        "  Latency P50: {}ms | P95: {}ms | P99: {}ms",
        result.snapshot.p50, result.snapshot.p95, result.snapshot.p99
    );
    eprintln!("  Duration: {:.1}s", result.elapsed.as_secs_f64());

    Ok(())
}

/// Apply CLI flag overrides to a loaded config.
fn apply_overrides(config: &mut LoadTestConfig, vus: Option<u32>, duration: Option<u64>) {
    if let Some(v) = vus {
        config.settings.virtual_users = v;
    }
    if let Some(d) = duration {
        config.settings.duration_secs = d;
    }
}

/// Discover `.pmcp/loadtest.toml` by walking parent directories.
///
/// Starts from the current working directory and walks up until either
/// the file is found or the filesystem root is reached. This matches
/// `.git` directory discovery semantics.
fn discover_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".pmcp").join("loadtest.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargo_pmcp::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};

    #[test]
    fn test_discover_config_returns_none_when_no_config() {
        // In the test environment, there's no .pmcp/loadtest.toml
        // at the filesystem root, so this should return None eventually.
        // We verify the function doesn't panic.
        let _ = discover_config();
    }

    #[test]
    fn test_apply_overrides_vus() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        };

        apply_overrides(&mut config, Some(50), None);
        assert_eq!(config.settings.virtual_users, 50);
        assert_eq!(config.settings.duration_secs, 60);
    }

    #[test]
    fn test_apply_overrides_duration() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        };

        apply_overrides(&mut config, None, Some(120));
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 120);
    }

    #[test]
    fn test_apply_overrides_both() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        };

        apply_overrides(&mut config, Some(25), Some(300));
        assert_eq!(config.settings.virtual_users, 25);
        assert_eq!(config.settings.duration_secs, 300);
    }

    #[test]
    fn test_apply_overrides_none() {
        let mut config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        };

        apply_overrides(&mut config, None, None);
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
    }
}
