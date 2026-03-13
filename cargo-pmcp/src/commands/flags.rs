//! Shared flag structs for all cargo-pmcp commands.
//!
//! These types provide consistent CLI flags across commands via `#[command(flatten)]`.
//! Using shared structs ensures uniform naming, help text, and behavior.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::fmt;

/// Output format for commands that support structured output.
///
/// Used by commands that can emit either human-readable text or
/// machine-parseable JSON output.
#[derive(Debug, Clone, ValueEnum)]
pub enum FormatValue {
    /// Human-readable text output (default).
    Text,
    /// Machine-parseable JSON output.
    Json,
}

impl fmt::Display for FormatValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatValue::Text => write!(f, "text"),
            FormatValue::Json => write!(f, "json"),
        }
    }
}

/// Flags for commands that accept a server target.
///
/// Provides a positional URL argument and `--server` flag for commands
/// that can target either a URL directly or a named pmcp.run server.
/// Used via `#[command(flatten)]` on commands where both URL and server
/// are optional (test run, test generate, schema export).
#[derive(Debug, Args)]
pub struct ServerFlags {
    /// URL of the MCP server (positional argument).
    #[arg(index = 1)]
    pub url: Option<String>,

    /// Named server on pmcp.run (alternative to URL).
    #[arg(long)]
    pub server: Option<String>,
}

impl ServerFlags {
    /// Resolve to a concrete URL from the url/server pair.
    ///
    /// If `url` is provided, returns it directly. If only `server` is set,
    /// constructs a localhost URL using the given `port`. Errors if neither
    /// is provided.
    pub fn resolve_url(self, port: u16) -> Result<(String, Option<String>)> {
        let url = if let Some(url) = self.url {
            url
        } else if self.server.is_some() {
            format!("http://0.0.0.0:{}", port)
        } else {
            anyhow::bail!("Either a URL or --server must be specified");
        };
        Ok((url, self.server))
    }
}

/// Resolved authentication method from CLI flags.
///
/// Produced by [`AuthFlags::resolve()`] after parsing. Handlers match on this
/// enum instead of inspecting raw flag fields, ensuring consistent auth logic
/// across all server-connecting commands.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// No authentication configured.
    None,
    /// Static bearer token (sent as `Authorization: Bearer <key>`).
    ApiKey(String),
    /// OAuth 2.0 PKCE flow with optional token caching.
    OAuth {
        /// OAuth client identifier.
        client_id: String,
        /// OAuth issuer URL for discovery.
        issuer: Option<String>,
        /// Requested OAuth scopes (defaults to `["openid"]`).
        scopes: Vec<String>,
        /// Disable token caching when true.
        no_cache: bool,
        /// Localhost port for the OAuth redirect callback.
        redirect_port: u16,
    },
}

/// Authentication flags shared across all server-connecting commands.
///
/// Provides `--api-key` and OAuth flags via `#[command(flatten)]`.
/// Use [`AuthFlags::resolve()`] to convert parsed flags into an [`AuthMethod`]
/// enum for uniform downstream handling.
///
/// `--api-key` and `--oauth-client-id` are mutually exclusive at parse time
/// via clap's `conflicts_with` attribute.
#[derive(Debug, Args)]
pub struct AuthFlags {
    /// API key for authentication (sent as Bearer token).
    #[arg(long, env = "MCP_API_KEY", conflicts_with = "oauth_client_id")]
    pub api_key: Option<String>,

    /// OAuth client ID (triggers OAuth PKCE flow).
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,

    /// OAuth issuer URL for OIDC discovery.
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    pub oauth_issuer: Option<String>,

    /// OAuth scopes (comma-separated).
    #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    pub oauth_scopes: Option<Vec<String>>,

    /// Disable OAuth token caching.
    #[arg(long)]
    pub oauth_no_cache: bool,

    /// Localhost port for the OAuth redirect callback.
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}

impl AuthFlags {
    /// Resolve parsed CLI flags into a typed [`AuthMethod`].
    ///
    /// Priority: `--api-key` wins over OAuth flags (though clap's
    /// `conflicts_with` prevents both from being specified simultaneously).
    /// If no auth flags are provided, returns [`AuthMethod::None`].
    pub fn resolve(&self) -> AuthMethod {
        if let Some(ref key) = self.api_key {
            return AuthMethod::ApiKey(key.clone());
        }
        if let Some(ref client_id) = self.oauth_client_id {
            return AuthMethod::OAuth {
                client_id: client_id.clone(),
                issuer: self.oauth_issuer.clone(),
                scopes: self
                    .oauth_scopes
                    .clone()
                    .unwrap_or_else(|| vec!["openid".to_string()]),
                no_cache: self.oauth_no_cache,
                redirect_port: self.oauth_redirect_port,
            };
        }
        AuthMethod::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Helper CLI struct for testing AuthFlags via clap parsing.
    #[derive(Debug, Parser)]
    #[command(name = "test-cli")]
    struct TestCli {
        #[command(flatten)]
        auth: AuthFlags,
    }

    #[test]
    fn resolve_no_auth_returns_none() {
        let flags = AuthFlags {
            api_key: None,
            oauth_client_id: None,
            oauth_issuer: None,
            oauth_scopes: None,
            oauth_no_cache: false,
            oauth_redirect_port: 8080,
        };
        assert_eq!(flags.resolve(), AuthMethod::None);
    }

    #[test]
    fn resolve_api_key_returns_api_key_variant() {
        let flags = AuthFlags {
            api_key: Some("sk-test-key-123".to_string()),
            oauth_client_id: None,
            oauth_issuer: None,
            oauth_scopes: None,
            oauth_no_cache: false,
            oauth_redirect_port: 8080,
        };
        assert_eq!(
            flags.resolve(),
            AuthMethod::ApiKey("sk-test-key-123".to_string())
        );
    }

    #[test]
    fn resolve_oauth_returns_oauth_variant() {
        let flags = AuthFlags {
            api_key: None,
            oauth_client_id: Some("my-client".to_string()),
            oauth_issuer: Some("https://auth.example.com".to_string()),
            oauth_scopes: Some(vec!["read".to_string(), "write".to_string()]),
            oauth_no_cache: false,
            oauth_redirect_port: 9090,
        };
        assert_eq!(
            flags.resolve(),
            AuthMethod::OAuth {
                client_id: "my-client".to_string(),
                issuer: Some("https://auth.example.com".to_string()),
                scopes: vec!["read".to_string(), "write".to_string()],
                no_cache: false,
                redirect_port: 9090,
            }
        );
    }

    #[test]
    fn resolve_oauth_defaults_scopes_to_openid() {
        let flags = AuthFlags {
            api_key: None,
            oauth_client_id: Some("my-client".to_string()),
            oauth_issuer: None,
            oauth_scopes: None,
            oauth_no_cache: false,
            oauth_redirect_port: 8080,
        };
        assert_eq!(
            flags.resolve(),
            AuthMethod::OAuth {
                client_id: "my-client".to_string(),
                issuer: None,
                scopes: vec!["openid".to_string()],
                no_cache: false,
                redirect_port: 8080,
            }
        );
    }

    #[test]
    fn resolve_oauth_propagates_custom_scopes() {
        let flags = AuthFlags {
            api_key: None,
            oauth_client_id: Some("my-client".to_string()),
            oauth_issuer: None,
            oauth_scopes: Some(vec!["profile".to_string(), "email".to_string()]),
            oauth_no_cache: false,
            oauth_redirect_port: 8080,
        };
        let method = flags.resolve();
        match method {
            AuthMethod::OAuth { scopes, .. } => {
                assert_eq!(scopes, vec!["profile".to_string(), "email".to_string()]);
            },
            other => panic!("Expected OAuth variant, got {other:?}"),
        }
    }

    #[test]
    fn resolve_oauth_no_cache_propagated() {
        let flags = AuthFlags {
            api_key: None,
            oauth_client_id: Some("my-client".to_string()),
            oauth_issuer: None,
            oauth_scopes: None,
            oauth_no_cache: true,
            oauth_redirect_port: 8080,
        };
        let method = flags.resolve();
        match method {
            AuthMethod::OAuth { no_cache, .. } => assert!(no_cache),
            other => panic!("Expected OAuth variant, got {other:?}"),
        }
    }

    #[test]
    fn clap_rejects_api_key_with_oauth_client_id() {
        let result = TestCli::try_parse_from([
            "test-cli",
            "--api-key",
            "my-key",
            "--oauth-client-id",
            "my-client",
        ]);
        assert!(
            result.is_err(),
            "Expected clap parse error for conflicting --api-key and --oauth-client-id"
        );
    }
}
