//! TOML-based load test scenario configuration.
//!
//! Defines typed structs for parsing load test scenario definitions
//! from TOML config files with weighted mixes of MCP operations.
//!
//! The configuration supports three MCP operation types as first-class
//! scenario steps: `tools/call`, `resources/read`, and `prompts/get`.
//! Each step carries a weight for proportional scheduling.
//!
//! # Example TOML
//!
//! ```toml
//! [settings]
//! virtual_users = 10
//! duration_secs = 60
//! timeout_ms = 5000
//!
//! [[scenario]]
//! type = "tools/call"
//! weight = 60
//! tool = "calculate"
//! arguments = { expression = "2+2" }
//!
//! [[scenario]]
//! type = "resources/read"
//! weight = 30
//! uri = "file:///data/config.json"
//!
//! [[scenario]]
//! type = "prompts/get"
//! weight = 10
//! prompt = "summarize"
//! arguments = { text = "Hello world" }
//! ```
//!
//! Note: The target server URL is NOT part of the config file. It is provided
//! via the `--url` CLI flag per user decision.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::error::LoadTestError;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "hello" }
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
        assert_eq!(config.settings.timeout_ms, 5000);
        assert_eq!(config.scenario.len(), 1);

        match &config.scenario[0] {
            ScenarioStep::ToolCall {
                weight,
                tool,
                arguments,
            } => {
                assert_eq!(*weight, 100);
                assert_eq!(tool, "echo");
                assert_eq!(arguments["text"], "hello");
            }
            other => panic!("Expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_weighted_mix() {
        let toml_str = r#"
[settings]
virtual_users = 20
duration_secs = 120
timeout_ms = 3000

[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.scenario.len(), 3);

        assert!(matches!(
            &config.scenario[0],
            ScenarioStep::ToolCall { weight: 60, .. }
        ));
        assert!(matches!(
            &config.scenario[1],
            ScenarioStep::ResourceRead { weight: 30, .. }
        ));
        assert!(matches!(
            &config.scenario[2],
            ScenarioStep::PromptGet { weight: 10, .. }
        ));
    }

    #[test]
    fn test_parse_default_expected_interval() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000

[[scenario]]
type = "tools/call"
weight = 1
tool = "ping"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.expected_interval_ms, 100);
    }

    #[test]
    fn test_parse_custom_expected_interval() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000
expected_interval_ms = 50

[[scenario]]
type = "tools/call"
weight = 1
tool = "ping"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.expected_interval_ms, 50);
    }

    #[test]
    fn test_validate_empty_scenario_fails() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigValidation { .. }
        ));
    }

    #[test]
    fn test_validate_zero_total_weight_fails() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![
                ScenarioStep::ToolCall {
                    weight: 0,
                    tool: "echo".to_string(),
                    arguments: serde_json::Value::Null,
                },
                ScenarioStep::ResourceRead {
                    weight: 0,
                    uri: "file:///data".to_string(),
                },
            ],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigValidation { .. }
        ));
    }

    #[test]
    fn test_validate_valid_config_passes() {
        let config = LoadTestConfig {
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
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_load_from_file() {
        let toml_content = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
"#;
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        tmpfile.write_all(toml_content.as_bytes()).unwrap();
        tmpfile.flush().unwrap();

        let config = LoadTestConfig::load(tmpfile.path()).unwrap();
        assert_eq!(config.settings.virtual_users, 5);
        assert_eq!(config.scenario.len(), 1);
    }

    #[test]
    fn test_load_missing_file_fails() {
        let result = LoadTestConfig::load(std::path::Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigIo { .. }
        ));
    }

    #[test]
    fn test_timeout_as_duration() {
        let settings = Settings {
            virtual_users: 10,
            duration_secs: 60,
            timeout_ms: 5000,
            expected_interval_ms: 100,
        };
        assert_eq!(settings.timeout_as_duration(), Duration::from_millis(5000));
    }
}
