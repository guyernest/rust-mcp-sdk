//! `cargo pmcp loadtest run` command implementation.

use anyhow::Result;
use std::path::PathBuf;

use cargo_pmcp::loadtest::config::LoadTestConfig;
use cargo_pmcp::loadtest::engine::LoadTestEngine;
use cargo_pmcp::loadtest::report::{write_report, LoadTestReport};
use cargo_pmcp::loadtest::summary::render_summary;

use crate::commands::auth;
use crate::commands::flags::{AuthFlags, AuthMethod};
use crate::commands::GlobalFlags;

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
    global_flags: &GlobalFlags,
    auth_flags: &AuthFlags,
) -> Result<()> {
    let no_color = global_flags.no_color;
    let config_file = resolve_config_path(config_path)?;

    if global_flags.should_output() {
        eprintln!("Loading config from: {}", config_file.display());
    }

    let mut config = LoadTestConfig::load(&config_file)
        .map_err(|e| anyhow::anyhow!("Failed to load config '{}': {}", config_file.display(), e))?;

    apply_overrides(&mut config, vus, duration, global_flags);

    let http_middleware_chain = resolve_auth_with_logging(&url, auth_flags, global_flags).await?;

    // Step 3: Build and run the engine
    let mut engine = LoadTestEngine::new(config, url.clone());
    if let Some(n) = iterations {
        engine = engine.with_iterations(n);
    }
    engine = engine.with_no_color(no_color);
    engine = engine.with_http_middleware(http_middleware_chain);

    let result = engine
        .run()
        .await
        .map_err(|e| anyhow::anyhow!("Load test failed: {}", e))?;

    // Step 4: Output k6-style terminal summary
    let summary = render_summary(&result, engine.config(), &url);
    println!("{summary}");

    // Step 5: Write JSON report (unless --no-report)
    if !no_report {
        write_json_report(&result, engine.config(), &url, global_flags);
    }

    Ok(())
}

/// Resolve the config-file path from CLI flag, auto-discovery, or emit a
/// user-facing error.
fn resolve_config_path(config_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = config_path {
        if !path.exists() {
            anyhow::bail!(
                "Config file not found: {}\nUse `cargo pmcp loadtest init` to create one.",
                path.display()
            );
        }
        return Ok(path);
    }

    discover_config().ok_or_else(|| {
        anyhow::anyhow!(
            "No loadtest config found.\n\
             Run `cargo pmcp loadtest init` to create .pmcp/loadtest.toml,\n\
             or use `--config path/to/file.toml` to specify one."
        )
    })
}

/// Resolve the auth middleware chain (OAuth/API-key/none) and log which mode
/// is active.
async fn resolve_auth_with_logging(
    url: &str,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<Option<std::sync::Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>> {
    let auth_method = auth_flags.resolve();
    let is_oauth = matches!(&auth_method, AuthMethod::OAuth { .. });
    let http_middleware_chain = auth::resolve_auth_middleware(url, &auth_method).await?;

    if global_flags.should_output() {
        match &http_middleware_chain {
            Some(_) if is_oauth => eprintln!("Authentication: OAuth 2.0 (token acquired)"),
            Some(_) => eprintln!("Authentication: API key"),
            None => eprintln!("Authentication: none"),
        }
    }

    Ok(http_middleware_chain)
}

/// Write the JSON report for a completed load test. Non-fatal on failure
/// (logs a warning when output is enabled).
fn write_json_report(
    result: &cargo_pmcp::loadtest::engine::LoadTestResult,
    config: &LoadTestConfig,
    url: &str,
    global_flags: &GlobalFlags,
) {
    let report = LoadTestReport::from_result(result, config, url);
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(e) => {
            if global_flags.should_output() {
                eprintln!();
                eprintln!("Warning: Failed to read cwd for report: {}", e);
            }
            return;
        },
    };
    match write_report(&report, &cwd) {
        Ok(path) => {
            if global_flags.should_output() {
                eprintln!();
                eprintln!("Report written to: {}", path.display());
            }
        },
        Err(e) => {
            if global_flags.should_output() {
                eprintln!();
                eprintln!("Warning: Failed to write report: {}", e);
            }
        },
    }
}

/// Apply CLI flag overrides to a loaded config.
///
/// When stages are present, `--vus` is ignored (stages define VU targets)
/// and a warning is logged if not in quiet mode. Duration override still
/// applies as safety ceiling.
fn apply_overrides(
    config: &mut LoadTestConfig,
    vus: Option<u32>,
    duration: Option<u64>,
    global_flags: &GlobalFlags,
) {
    if let Some(v) = vus {
        if config.has_stages() {
            if global_flags.should_output() {
                eprintln!(
                    "Warning: --vus={v} ignored because config contains [[stage]] blocks (stages define VU targets)"
                );
            }
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
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, Some(50), None, &gf);
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
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, None, Some(120), &gf);
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
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, Some(25), Some(300), &gf);
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
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };

        let gf = GlobalFlags {
            verbose: false,
            no_color: false,
            quiet: false,
        };
        apply_overrides(&mut config, None, None, &gf);
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
    }
}
