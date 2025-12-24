//! Configuration for foundation server connections.

use super::CompositionError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Configuration for connecting to foundation servers.
///
/// This configuration is typically loaded from a `foundations.toml` file
/// that is generated during schema export.
///
/// # Example Configuration File
///
/// ```toml
/// [foundations.calculator]
/// url = "http://localhost:8080"
/// timeout_ms = 30000
///
/// [foundations.database]
/// url = "http://localhost:8081"
/// headers = { "Authorization" = "Bearer token123" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundationConfig {
    /// Map of foundation server ID to endpoint configuration.
    #[serde(default)]
    pub foundations: HashMap<String, FoundationEndpoint>,

    /// Default timeout in milliseconds for all servers.
    /// Individual server timeouts override this.
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Default retry count for failed requests.
    #[serde(default = "default_retries")]
    pub default_retries: u32,
}

fn default_timeout() -> u64 {
    30_000 // 30 seconds
}

fn default_retries() -> u32 {
    3
}

impl Default for FoundationConfig {
    fn default() -> Self {
        Self {
            foundations: HashMap::new(),
            default_timeout_ms: default_timeout(),
            default_retries: default_retries(),
        }
    }
}

impl FoundationConfig {
    /// Create a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = FoundationConfig::from_file("foundations.toml")?;
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, CompositionError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Parse configuration from a TOML string.
    ///
    /// # Arguments
    ///
    /// * `content` - TOML content as a string
    pub fn from_toml(content: &str) -> Result<Self, CompositionError> {
        toml::from_str(content).map_err(Into::into)
    }

    /// Add a foundation endpoint to the configuration.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server
    /// * `endpoint` - The endpoint configuration
    pub fn add_foundation(&mut self, server_id: impl Into<String>, endpoint: FoundationEndpoint) {
        self.foundations.insert(server_id.into(), endpoint);
    }

    /// Create a configuration with a single foundation server.
    ///
    /// This is useful for testing or simple setups.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server
    /// * `url` - The URL of the foundation server
    pub fn with_foundation(server_id: impl Into<String>, url: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.add_foundation(server_id, FoundationEndpoint::new(url));
        config
    }

    /// Get the endpoint configuration for a foundation server.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The ID of the foundation server
    pub fn get_endpoint(&self, server_id: &str) -> Option<&FoundationEndpoint> {
        self.foundations.get(server_id)
    }

    /// Get the timeout for a specific server, falling back to default.
    pub fn timeout_for(&self, server_id: &str) -> std::time::Duration {
        let ms = self
            .foundations
            .get(server_id)
            .and_then(|e| e.timeout_ms)
            .unwrap_or(self.default_timeout_ms);
        std::time::Duration::from_millis(ms)
    }

    /// Get the retry count for a specific server, falling back to default.
    pub fn retries_for(&self, server_id: &str) -> u32 {
        self.foundations
            .get(server_id)
            .and_then(|e| e.retries)
            .unwrap_or(self.default_retries)
    }

    /// Serialize the configuration to TOML.
    pub fn to_toml(&self) -> Result<String, CompositionError> {
        toml::to_string_pretty(self).map_err(|e| CompositionError::Serialization(e.to_string()))
    }

    /// Load configuration from environment variables.
    ///
    /// Environment variables are in the format:
    /// - `PMCP_FOUNDATION_<SERVER_ID>_URL` - Server URL
    /// - `PMCP_FOUNDATION_<SERVER_ID>_TIMEOUT_MS` - Timeout in milliseconds
    ///
    /// # Example
    ///
    /// ```bash
    /// export PMCP_FOUNDATION_CALCULATOR_URL="http://localhost:8080"
    /// export PMCP_FOUNDATION_DATABASE_URL="http://localhost:8081"
    /// ```
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Scan environment for foundation URLs
        for (key, value) in std::env::vars() {
            if let Some(suffix) = key.strip_prefix("PMCP_FOUNDATION_") {
                if let Some(server_id) = suffix.strip_suffix("_URL") {
                    let server_id = server_id.to_lowercase();
                    config.add_foundation(&server_id, FoundationEndpoint::new(&value));

                    // Check for timeout override
                    if let Ok(timeout) = std::env::var(format!(
                        "PMCP_FOUNDATION_{}_TIMEOUT_MS",
                        server_id.to_uppercase()
                    )) {
                        if let Ok(ms) = timeout.parse::<u64>() {
                            if let Some(endpoint) = config.foundations.get_mut(&server_id) {
                                endpoint.timeout_ms = Some(ms);
                            }
                        }
                    }
                }
            }
        }

        config
    }
}

/// Configuration for a single foundation server endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundationEndpoint {
    /// The URL of the foundation server.
    pub url: String,

    /// Optional timeout override in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    /// Optional retry count override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,

    /// Optional extra headers to include in requests.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// Optional authentication token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// Whether to enable JSON responses (vs SSE).
    #[serde(default = "default_json_response")]
    pub enable_json_response: bool,
}

fn default_json_response() -> bool {
    true // Default to JSON for simplicity in composition
}

impl FoundationEndpoint {
    /// Create a new endpoint with just a URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            timeout_ms: None,
            retries: None,
            headers: HashMap::new(),
            auth_token: None,
            enable_json_response: true,
        }
    }

    /// Set the timeout for this endpoint.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set the retry count for this endpoint.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Add a header to this endpoint.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Set the authentication token for this endpoint.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Set whether to enable JSON responses.
    pub fn with_json_response(mut self, enable: bool) -> Self {
        self.enable_json_response = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FoundationConfig::default();
        assert!(config.foundations.is_empty());
        assert_eq!(config.default_timeout_ms, 30_000);
        assert_eq!(config.default_retries, 3);
    }

    #[test]
    fn test_with_foundation() {
        let config = FoundationConfig::with_foundation("calculator", "http://localhost:8080");
        assert_eq!(config.foundations.len(), 1);
        assert!(config.get_endpoint("calculator").is_some());
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
            default_timeout_ms = 60000

            [foundations.calculator]
            url = "http://localhost:8080"
            timeout_ms = 30000

            [foundations.database]
            url = "http://localhost:8081"
            headers = { "X-API-Key" = "secret" }
        "#;

        let config = FoundationConfig::from_toml(toml).unwrap();
        assert_eq!(config.default_timeout_ms, 60_000);
        assert_eq!(config.foundations.len(), 2);

        let calc = config.get_endpoint("calculator").unwrap();
        assert_eq!(calc.url, "http://localhost:8080");
        assert_eq!(calc.timeout_ms, Some(30_000));

        let db = config.get_endpoint("database").unwrap();
        assert_eq!(db.url, "http://localhost:8081");
        assert_eq!(db.headers.get("X-API-Key"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_timeout_for() {
        let config = FoundationConfig::from_toml(
            r#"
            default_timeout_ms = 60000
            [foundations.fast]
            url = "http://localhost:8080"
            timeout_ms = 5000
            [foundations.slow]
            url = "http://localhost:8081"
        "#,
        )
        .unwrap();

        assert_eq!(
            config.timeout_for("fast"),
            std::time::Duration::from_millis(5000)
        );
        assert_eq!(
            config.timeout_for("slow"),
            std::time::Duration::from_millis(60000)
        );
        assert_eq!(
            config.timeout_for("unknown"),
            std::time::Duration::from_millis(60000)
        );
    }

    #[test]
    fn test_endpoint_builder() {
        let endpoint = FoundationEndpoint::new("http://localhost:8080")
            .with_timeout(5000)
            .with_retries(5)
            .with_header("Authorization", "Bearer token")
            .with_auth_token("my-token");

        assert_eq!(endpoint.url, "http://localhost:8080");
        assert_eq!(endpoint.timeout_ms, Some(5000));
        assert_eq!(endpoint.retries, Some(5));
        assert_eq!(
            endpoint.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(endpoint.auth_token, Some("my-token".to_string()));
    }
}
