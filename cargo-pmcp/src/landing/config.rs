//! Configuration for MCP server landing pages
//!
//! This module handles parsing and validation of pmcp-landing.toml configuration files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main landing page configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingConfig {
    /// Landing page metadata and content
    pub landing: LandingSection,

    /// Deployment configuration
    #[serde(default)]
    pub deployment: DeploymentSection,

    /// Login-page branding pushed to the Cognito Managed Login hosted UI by
    /// the pmcp.run deploy-landing Lambda. Independent of `landing.branding`,
    /// which styles the landing site chrome — `login` styles the Cognito
    /// hosted pages that end-users see when an MCP client triggers OAuth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginConfig>,
}

/// Cognito Managed Login branding configuration
///
/// Fields here map to the Cognito `UpdateManagedLoginBranding` API and are
/// applied to every (server × MCP client type) pair registered for a server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginConfig {
    /// Primary brand color (hex, #rrggbb or #rgb). Applied to primary buttons
    /// and accent elements in the hosted UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_color: Option<String>,

    /// Page background color (hex, #rrggbb or #rgb). Applied to the hosted UI
    /// page background in light mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,

    /// Logo asset reference. Either:
    ///   - `s3://bucket/key` — fetched from the landing assets bucket
    ///   - `https://<landing-bucket>/path` — same bucket, different URL form
    ///
    /// Any other URL form is rejected by the platform (S3-only logo pattern).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

/// Landing page content and branding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingSection {
    /// MCP server name (required)
    pub server_name: String,

    /// Page title (defaults to server_name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Tagline/subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Hero section configuration
    #[serde(default)]
    pub hero: HeroSection,

    /// Branding configuration
    #[serde(default)]
    pub branding: BrandingSection,

    /// Usage examples
    #[serde(default)]
    pub examples: Vec<ExampleItem>,
}

/// Hero section configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeroSection {
    /// Hero image path (relative to landing directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Call-to-action button text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cta_text: Option<String>,
}

/// Branding configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrandingSection {
    /// Primary brand color (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_color: Option<String>,

    /// Logo path (relative to landing directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

/// Usage example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleItem {
    /// Tool name to demonstrate
    pub tool: String,

    /// Example title
    pub title: String,

    /// Example input (as JSON value)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

/// Deployment configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeploymentSection {
    /// Deployment target
    #[serde(default = "default_target")]
    pub target: String,

    /// MCP server ID (for pmcp.run)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,

    /// Custom endpoint URL (overrides server_id lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

fn default_target() -> String {
    "pmcp.run".to_string()
}

fn validate_hex_color(color: &str, field: &str) -> Result<()> {
    if !color.starts_with('#') || (color.len() != 4 && color.len() != 7) {
        anyhow::bail!(
            "{} must be a valid hex color (e.g., #fff or #ffffff)",
            field
        );
    }
    if !color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!(
            "{} must be a valid hex color (e.g., #fff or #ffffff)",
            field
        );
    }
    Ok(())
}

impl LandingConfig {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: LandingConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;

        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize landing config")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Server name is required
        if self.landing.server_name.is_empty() {
            anyhow::bail!("landing.server_name is required");
        }

        // Validate server name format (alphanumeric + dashes)
        if !self
            .landing
            .server_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!(
                "landing.server_name must contain only alphanumeric characters, dashes, and underscores"
            );
        }

        // Validate primary color if provided
        if let Some(ref color) = self.landing.branding.primary_color {
            validate_hex_color(color, "landing.branding.primary_color")?;
        }

        // Validate login branding colors if provided
        if let Some(ref login) = self.login {
            if let Some(ref color) = login.primary_color {
                validate_hex_color(color, "login.primary_color")?;
            }
            if let Some(ref color) = login.background_color {
                validate_hex_color(color, "login.background_color")?;
            }
        }

        Ok(())
    }

    /// Get the display title (uses title if set, otherwise server_name)
    pub fn display_title(&self) -> &str {
        self.landing
            .title
            .as_deref()
            .unwrap_or(&self.landing.server_name)
    }

    /// Create a default configuration for a server
    pub fn default_for_server(server_name: String) -> Self {
        Self {
            landing: LandingSection {
                server_name: server_name.clone(),
                title: Some(format!("{} MCP Server", server_name)),
                tagline: Some(format!(
                    "Powerful {} capabilities for AI assistants",
                    server_name
                )),
                description: Some(format!(
                    "This MCP server provides {} functionality for Claude and other AI assistants.",
                    server_name
                )),
                hero: HeroSection {
                    image: None,
                    cta_text: Some("Get Started".to_string()),
                },
                branding: BrandingSection {
                    primary_color: Some("#1a1a2e".to_string()),
                    logo: None,
                },
                examples: vec![],
            },
            deployment: DeploymentSection {
                target: "pmcp.run".to_string(),
                server_id: None,
                endpoint: None,
            },
            login: None,
        }
    }
}

/// Try to load deployment info from existing pmcp deployment
pub fn load_deployment_info(project_root: &Path) -> Option<(String, String)> {
    // Try to read .pmcp/deployment.toml
    let deployment_file = project_root.join(".pmcp/deployment.toml");
    if !deployment_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&deployment_file).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;

    let server_id = value
        .get("deployment")?
        .get("server_id")?
        .as_str()?
        .to_string();

    let endpoint = value
        .get("deployment")?
        .get("endpoint")?
        .as_str()?
        .to_string();

    Some((server_id, endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LandingConfig::default_for_server("chess".to_string());
        assert_eq!(config.landing.server_name, "chess");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_server_name() {
        let mut config = LandingConfig::default_for_server("valid-name_123".to_string());
        assert!(config.validate().is_ok());

        config.landing.server_name = "invalid name!".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_color() {
        let mut config = LandingConfig::default_for_server("test".to_string());

        config.landing.branding.primary_color = Some("#fff".to_string());
        assert!(config.validate().is_ok());

        config.landing.branding.primary_color = Some("#ffffff".to_string());
        assert!(config.validate().is_ok());

        config.landing.branding.primary_color = Some("ffffff".to_string());
        assert!(config.validate().is_err());

        // Non-hex characters are rejected too
        config.landing.branding.primary_color = Some("#gggggg".to_string());
        assert!(config.validate().is_err());
    }

    /// Regression test for pmcp.run platform CR-01: `[login]` sections were
    /// silently dropped by serde because the struct had no `login` field,
    /// so Cognito `UpdateManagedLoginBranding` was never fired end-to-end
    /// from a real developer deploy.
    #[test]
    fn test_login_section_round_trips_through_toml() {
        let toml = r##"
[landing]
server_name = "example"

[login]
primary_color = "#0972d3"
background_color = "#ffffff"
logo = "s3://pmcp-landings-dev/example/logo.png"
"##;

        let config: LandingConfig = toml::from_str(toml).expect("parse");
        let login = config.login.as_ref().expect("[login] section preserved");
        assert_eq!(login.primary_color.as_deref(), Some("#0972d3"));
        assert_eq!(login.background_color.as_deref(), Some("#ffffff"));
        assert_eq!(
            login.logo.as_deref(),
            Some("s3://pmcp-landings-dev/example/logo.png")
        );

        // Round-trip: serialize back and re-parse, values must survive
        let emitted = toml::to_string(&config).expect("serialize");
        let reparsed: LandingConfig = toml::from_str(&emitted).expect("reparse");
        let login2 = reparsed.login.as_ref().expect("login survives round-trip");
        assert_eq!(login2.primary_color.as_deref(), Some("#0972d3"));
    }

    #[test]
    fn test_login_section_optional_backward_compatible() {
        // Config with no [login] — must still parse and validate cleanly.
        let toml = r#"
[landing]
server_name = "example"
"#;
        let config: LandingConfig = toml::from_str(toml).expect("parse");
        assert!(config.login.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_login_color_validation_rejects_bad_hex() {
        let mut config = LandingConfig::default_for_server("test".to_string());
        config.login = Some(LoginConfig {
            primary_color: Some("not-a-color".to_string()),
            background_color: None,
            logo: None,
        });
        assert!(config.validate().is_err());

        config.login = Some(LoginConfig {
            primary_color: Some("#fff".to_string()),
            background_color: Some("#0972d3".to_string()),
            logo: Some("s3://bucket/logo.png".to_string()),
        });
        assert!(config.validate().is_ok());
    }
}
