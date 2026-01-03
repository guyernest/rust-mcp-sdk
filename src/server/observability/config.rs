//! Observability configuration.
//!
//! Configuration can be loaded from:
//! 1. TOML file (`.pmcp-config.toml`)
//! 2. Environment variables (with `PMCP_OBSERVABILITY_` prefix)
//!
//! Environment variables override TOML configuration.
//!
//! # Example TOML Configuration
//!
//! ```toml
//! [observability]
//! enabled = true
//! backend = "cloudwatch"
//! max_depth = 10
//! sample_rate = 1.0
//!
//! [observability.fields]
//! capture_tool_name = true
//! capture_arguments_hash = false
//! capture_client_ip = false
//!
//! [observability.cloudwatch]
//! namespace = "PMCP/Servers"
//! emf_enabled = true
//! ```

use super::backend::CloudWatchConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main observability configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    /// Master switch for observability.
    pub enabled: bool,

    /// Backend selection: "console", "cloudwatch", "null".
    pub backend: String,

    /// Maximum composition depth (loop prevention).
    pub max_depth: u32,

    /// Sampling rate (0.0 - 1.0, for high-volume servers).
    pub sample_rate: f64,

    /// Tracing configuration.
    pub tracing: TracingConfig,

    /// Field capture configuration.
    pub fields: FieldsConfig,

    /// Metrics configuration.
    pub metrics: MetricsConfig,

    /// CloudWatch-specific configuration.
    pub cloudwatch: CloudWatchConfig,

    /// Console-specific configuration.
    pub console: ConsoleConfig,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "console".to_string(),
            max_depth: 10,
            sample_rate: 1.0,
            tracing: TracingConfig::default(),
            fields: FieldsConfig::default(),
            metrics: MetricsConfig::default(),
            cloudwatch: CloudWatchConfig::default(),
            console: ConsoleConfig::default(),
        }
    }
}

impl ObservabilityConfig {
    /// Load configuration from file and environment.
    ///
    /// Priority (highest to lowest):
    /// 1. Environment variables
    /// 2. TOML configuration file
    /// 3. Default values
    pub fn load() -> Result<Self, ConfigError> {
        // Load from .pmcp-config.toml if it exists, otherwise use defaults
        let mut config = if let Ok(contents) = std::fs::read_to_string(".pmcp-config.toml") {
            Self::from_toml(&contents)?
        } else {
            Self::default()
        };

        // Override with environment variables
        config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a specific file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
            path: path.as_ref().display().to_string(),
            error: e.to_string(),
        })?;
        let mut config = Self::from_toml(&contents)?;
        config.apply_env_overrides();
        Ok(config)
    }

    /// Parse configuration from TOML content.
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        // Try to parse the full config structure first
        #[derive(Deserialize)]
        struct FullConfig {
            #[serde(default)]
            observability: ObservabilityConfig,
        }

        let full: FullConfig =
            toml::from_str(content).map_err(|e| ConfigError::Parse(e.to_string()))?;

        Ok(full.observability)
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        // Master switch
        if let Ok(enabled) = std::env::var("PMCP_OBSERVABILITY_ENABLED") {
            if let Ok(v) = enabled.parse() {
                self.enabled = v;
            }
        }

        // Backend selection
        if let Ok(backend) = std::env::var("PMCP_OBSERVABILITY_BACKEND") {
            self.backend = backend;
        }

        // Max depth
        if let Ok(max_depth) = std::env::var("PMCP_OBSERVABILITY_MAX_DEPTH") {
            if let Ok(v) = max_depth.parse() {
                self.max_depth = v;
            }
        }

        // Sample rate
        if let Ok(sample_rate) = std::env::var("PMCP_OBSERVABILITY_SAMPLE_RATE") {
            if let Ok(v) = sample_rate.parse() {
                self.sample_rate = v;
            }
        }

        // Field capture overrides
        if let Ok(v) = std::env::var("PMCP_OBSERVABILITY_CAPTURE_TOOL_NAME") {
            if let Ok(b) = v.parse() {
                self.fields.capture_tool_name = b;
            }
        }
        if let Ok(v) = std::env::var("PMCP_OBSERVABILITY_CAPTURE_ARGUMENTS_HASH") {
            if let Ok(b) = v.parse() {
                self.fields.capture_arguments_hash = b;
            }
        }
        if let Ok(v) = std::env::var("PMCP_OBSERVABILITY_CAPTURE_CLIENT_IP") {
            if let Ok(b) = v.parse() {
                self.fields.capture_client_ip = b;
            }
        }
        if let Ok(v) = std::env::var("PMCP_OBSERVABILITY_CAPTURE_RESPONSE_SIZE") {
            if let Ok(b) = v.parse() {
                self.fields.capture_response_size = b;
            }
        }

        // CloudWatch overrides
        if let Ok(namespace) = std::env::var("PMCP_CLOUDWATCH_NAMESPACE") {
            self.cloudwatch.namespace = namespace;
        }
        if let Ok(emf) = std::env::var("PMCP_CLOUDWATCH_EMF_ENABLED") {
            if let Ok(v) = emf.parse() {
                self.cloudwatch.emf_enabled = v;
            }
        }

        // Console overrides
        if let Ok(pretty) = std::env::var("PMCP_CONSOLE_PRETTY") {
            if let Ok(v) = pretty.parse() {
                self.console.pretty = v;
            }
        }
    }

    /// Check if sampling should capture this request.
    ///
    /// Uses the configured sample rate to randomly decide.
    /// Uses a simple time-based entropy source to avoid requiring
    /// the `rand` crate.
    pub fn should_sample(&self) -> bool {
        if self.sample_rate >= 1.0 {
            return true;
        }
        if self.sample_rate <= 0.0 {
            return false;
        }
        // Simple sampling using time-based entropy
        // This is not cryptographically secure but sufficient for sampling
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let random_value = (nanos as f64) / (u32::MAX as f64);
        random_value < self.sample_rate
    }

    /// Create a disabled configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a development configuration with console output.
    pub fn development() -> Self {
        Self {
            enabled: true,
            backend: "console".to_string(),
            console: ConsoleConfig {
                pretty: true,
                verbose: false,
            },
            ..Default::default()
        }
    }

    /// Create a production configuration with `CloudWatch`.
    pub fn production() -> Self {
        Self {
            enabled: true,
            backend: "cloudwatch".to_string(),
            cloudwatch: CloudWatchConfig::default(),
            ..Default::default()
        }
    }
}

/// Tracing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TracingConfig {
    /// Enable distributed tracing.
    pub enabled: bool,

    /// Header name for HTTP trace propagation.
    pub trace_header: String,

    /// Field name for Lambda payload trace propagation.
    pub trace_field: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trace_header: "X-Trace-ID".to_string(),
            trace_field: "_trace".to_string(),
        }
    }
}

/// Field capture configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct FieldsConfig {
    /// Capture tool names in logs.
    pub capture_tool_name: bool,

    /// Capture resource URIs in logs.
    pub capture_resource_uri: bool,

    /// Capture prompt names in logs.
    pub capture_prompt_name: bool,

    /// Capture argument hashes (for correlation without exposing data).
    pub capture_arguments_hash: bool,

    /// Capture full arguments (privacy-sensitive, use with caution).
    pub capture_full_arguments: bool,

    /// Capture user ID from `AuthContext`.
    pub capture_user_id: bool,

    /// Capture client type.
    pub capture_client_type: bool,

    /// Capture client version.
    pub capture_client_version: bool,

    /// Capture client IP (privacy-sensitive, default off).
    pub capture_client_ip: bool,

    /// Capture session ID.
    pub capture_session_id: bool,

    /// Capture response size.
    pub capture_response_size: bool,

    /// Capture error details.
    pub capture_error_details: bool,
}

impl Default for FieldsConfig {
    fn default() -> Self {
        Self {
            capture_tool_name: true,
            capture_resource_uri: true,
            capture_prompt_name: true,
            capture_arguments_hash: false,
            capture_full_arguments: false,
            capture_user_id: true,
            capture_client_type: true,
            capture_client_version: true,
            capture_client_ip: false, // Privacy-sensitive
            capture_session_id: true,
            capture_response_size: true,
            capture_error_details: true,
        }
    }
}

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct MetricsConfig {
    /// Emit request count metrics.
    pub request_count: bool,

    /// Emit request duration metrics.
    pub request_duration: bool,

    /// Emit error rate metrics.
    pub error_rate: bool,

    /// Emit per-tool usage metrics.
    pub tool_usage: bool,

    /// Emit per-resource usage metrics.
    pub resource_usage: bool,

    /// Emit per-prompt usage metrics.
    pub prompt_usage: bool,

    /// Custom metric prefix.
    pub prefix: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            request_count: true,
            request_duration: true,
            error_rate: true,
            tool_usage: true,
            resource_usage: true,
            prompt_usage: true,
            prefix: "mcp".to_string(),
        }
    }
}

/// Console backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsoleConfig {
    /// Pretty print (human-readable format).
    pub pretty: bool,

    /// Include full event details (verbose mode).
    pub verbose: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            pretty: true,
            verbose: false,
        }
    }
}

/// Configuration errors.
#[derive(Debug)]
pub enum ConfigError {
    /// IO error reading configuration file.
    Io {
        /// Path to the configuration file.
        path: String,
        /// Error message.
        error: String,
    },
    /// Parse error in configuration.
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(f, "Failed to read config file '{path}': {error}")
            },
            Self::Parse(e) => write!(f, "Failed to parse config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ObservabilityConfig::default();

        assert!(config.enabled);
        assert_eq!(config.backend, "console");
        assert_eq!(config.max_depth, 10);
        assert!((config.sample_rate - 1.0).abs() < f64::EPSILON);
        assert!(config.fields.capture_tool_name);
        assert!(!config.fields.capture_client_ip);
    }

    #[test]
    fn test_from_toml() {
        let toml = r#"
            [observability]
            enabled = true
            backend = "cloudwatch"
            max_depth = 5
            sample_rate = 0.5

            [observability.fields]
            capture_tool_name = true
            capture_client_ip = true

            [observability.cloudwatch]
            namespace = "MyApp/MCP"
            emf_enabled = true
        "#;

        let config = ObservabilityConfig::from_toml(toml).unwrap();

        assert!(config.enabled);
        assert_eq!(config.backend, "cloudwatch");
        assert_eq!(config.max_depth, 5);
        assert!((config.sample_rate - 0.5).abs() < f64::EPSILON);
        assert!(config.fields.capture_tool_name);
        assert!(config.fields.capture_client_ip);
        assert_eq!(config.cloudwatch.namespace, "MyApp/MCP");
    }

    #[test]
    fn test_disabled_config() {
        let config = ObservabilityConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_development_config() {
        let config = ObservabilityConfig::development();
        assert!(config.enabled);
        assert_eq!(config.backend, "console");
        assert!(config.console.pretty);
    }

    #[test]
    fn test_production_config() {
        let config = ObservabilityConfig::production();
        assert!(config.enabled);
        assert_eq!(config.backend, "cloudwatch");
    }

    #[test]
    fn test_should_sample_always() {
        let config = ObservabilityConfig {
            sample_rate: 1.0,
            ..Default::default()
        };

        // Should always sample at 100%
        for _ in 0..100 {
            assert!(config.should_sample());
        }
    }

    #[test]
    fn test_should_sample_never() {
        let config = ObservabilityConfig {
            sample_rate: 0.0,
            ..Default::default()
        };

        // Should never sample at 0%
        for _ in 0..100 {
            assert!(!config.should_sample());
        }
    }

    #[test]
    fn test_tracing_config_defaults() {
        let config = TracingConfig::default();

        assert!(config.enabled);
        assert_eq!(config.trace_header, "X-Trace-ID");
        assert_eq!(config.trace_field, "_trace");
    }

    #[test]
    fn test_metrics_config_defaults() {
        let config = MetricsConfig::default();

        assert!(config.request_count);
        assert!(config.request_duration);
        assert!(config.error_rate);
        assert_eq!(config.prefix, "mcp");
    }
}
