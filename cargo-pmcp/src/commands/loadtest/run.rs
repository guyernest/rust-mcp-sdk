//! `cargo pmcp loadtest run` command implementation.

use anyhow::Result;
use std::io::IsTerminal;
use std::path::PathBuf;

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::engine::LoadTestEngine;
use cargo_pmcp::loadtest::report::{write_report, LoadTestReport};
use cargo_pmcp::loadtest::summary::render_summary;

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
    no_report: bool,
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
        },
        None => match discover_config() {
            Some(path) => path,
            None => {
                anyhow::bail!(
                    "No loadtest config found.\n\
                     Run `cargo pmcp loadtest init` to create .pmcp/loadtest.toml,\n\
                     or use `--config path/to/file.toml` to specify one."
                );
            },
        },
    };

    eprintln!("Loading config from: {}", config_file.display());

    let mut config = LoadTestConfig::load(&config_file)
        .map_err(|e| anyhow::anyhow!("Failed to load config '{}': {}", config_file.display(), e))?;

    // Step 2: Apply CLI overrides
    apply_overrides(&mut config, vus, duration);

    // Step 3: Build and run the engine
    let mut engine = LoadTestEngine::new(config, url.clone());
    if let Some(n) = iterations {
        engine = engine.with_iterations(n);
    }
    engine = engine.with_no_color(no_color);

    let result = engine
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("Load test failed: {}", e))?;

    // Step 4: Output k6-style terminal summary
    // Set color override based on --no-color flag and TTY detection
    if no_color || !std::io::stdout().is_terminal() {
        colored::control::set_override(false);
    }

    let summary = render_summary(&result, engine.config(), &url);
    println!("{summary}");

    // Step 5: Write JSON report (unless --no-report)
    if !no_report {
        let report = LoadTestReport::from_result(&result, engine.config(), &url);
        let cwd = std::env::current_dir()?;
        match write_report(&report, &cwd) {
            Ok(path) => {
                eprintln!();
                eprintln!("Report written to: {}", path.display());
            },
            Err(e) => {
                eprintln!();
                eprintln!("Warning: Failed to write report: {}", e);
                // Non-fatal -- the test still completed successfully
            },
        }
    }

    Ok(())
}

/// Apply CLI flag overrides to a loaded config.
///
/// When stages are present, `--vus` is ignored (stages define VU targets)
/// and a warning is logged. Duration override still applies as safety ceiling.
fn apply_overrides(config: &mut LoadTestConfig, vus: Option<u32>, duration: Option<u64>) {
    if let Some(v) = vus {
        if config.has_stages() {
            eprintln!(
                "Warning: --vus={v} ignored because config contains [[stage]] blocks (stages define VU targets)"
            );
        } else {
            config.settings.virtual_users = v;
        }
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
            stage: vec![],
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
            stage: vec![],
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
            stage: vec![],
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
            stage: vec![],
        };

        apply_overrides(&mut config, None, None);
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
    }
}
