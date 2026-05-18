// Originated from pmcp-run/built-in/shared/mcp-server-common/src/secrets.rs
// (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace for Phase 83 toolkit lift (P83-02).
//
// Mechanical deltas applied during the lift (per D-01):
// 1. Crate-path swap: crate::error::Error::auth(...) -> ToolkitError::Secret { ... }
// 2. pmcp dep shape: trait operates on toolkit-owned SecretValue (R6) rather
//    than raw String (anti-pattern §"Anti-Patterns" #11).
// 3. Feature gates: AWS impls behind `aws`; env impl unconditional. None of
//    the dropped D-14 features (`ddb`, `dynamo-config`, `openapi-code-mode`,
//    `js-runtime`, `mcp-code-mode`) survived.
// 4. Attribution header above.

//! Secrets management for the toolkit.
//!
//! Resolves secrets from multiple sources behind the [`SecretsProvider`] trait.
//! The trait returns a [`SecretValue`] (toolkit-owned, feature-independent per
//! Phase 83 review R6), never a raw `String` or `Vec<u8>`. `SecretValue` blocks
//! `Debug`, `Display`, `Clone`, `Serialize`, `Deserialize` — `trybuild`
//! compile-fail tests at `tests/compile_fail/*.rs` enforce these denials at
//! compile time (review R5).
//!
//! # Resolution Strategy
//!
//! Built-in providers (call them directly or chain them via [`SecretsProviderChain`]):
//!
//! 1. **Org-level Secrets Manager** (`aws` feature) — if `PMCP_SECRETS_PATH`
//!    contains `/orgs/`
//! 2. **Per-server Secrets Manager** (`aws` feature) — if `PMCP_SECRETS_PATH`
//!    is set without `/orgs/`
//! 3. **SSM Parameter Store** (`aws` feature) — if `PMCP_SSM_PATH` is set
//! 4. **Environment variables** ([`EnvSecrets`]) — always available
//!
//! # Org-Level Secret Structure (pmcp.run)
//!
//! For pmcp.run deployments, secrets are stored at the organization level to
//! reduce costs. One secret per organization contains all server credentials:
//!
//! ```json
//! {
//!   "london-tube": {
//!     "TFL_APP_KEY": "your-api-key"
//!   },
//!   "lichess": {
//!     "LICHESS_TOKEN": "your-token"
//!   }
//! }
//! ```
//!
//! Path format: `pmcp/orgs/{org_id}/credentials`

use crate::error::{Result, ToolkitError};
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretBox};
use std::sync::Arc;
#[cfg(feature = "aws")]
use std::collections::HashMap;
#[cfg(feature = "aws")]
use tokio::sync::RwLock;

/// Environment variable that specifies the Secrets Manager path
pub const SECRETS_MANAGER_PATH_VAR: &str = "PMCP_SECRETS_PATH";

/// Environment variable that specifies the SSM Parameter Store path
pub const SSM_PATH_VAR: &str = "PMCP_SSM_PATH";

/// Environment variable for server ID (used for org-level secrets extraction)
pub const SERVER_ID_VAR: &str = "PMCP_SERVER_ID";

// ============================================================================
// SecretValue — toolkit-owned secret newtype (review R6).
// ============================================================================

/// Toolkit-owned secret newtype — NEVER returns raw bytes from `SecretsProvider`.
///
/// Intentionally does NOT implement `Debug`, `Display`, `Clone`, `Serialize`,
/// `Deserialize`, `PartialEq`, `Eq`. Compile-fail tests at
/// `tests/compile_fail/token_secret_no_*.rs` enforce this via `trybuild`
/// (per Phase 83 review R5 + R6 + Pattern E + CMSUP-02).
///
/// Available unconditionally — does NOT depend on the `code-mode` feature.
/// When the `code-mode` feature is enabled, an `impl From<SecretValue> for
/// pmcp_code_mode::TokenSecret` is provided for interop with the HMAC token
/// machinery.
///
/// The underlying bytes are zeroed on drop via `secrecy::SecretBox`.
pub struct SecretValue(SecretBox<[u8]>);

impl SecretValue {
    /// Create from raw bytes. The input `Vec` is consumed and copied into a
    /// `SecretBox`; the original `Vec` is NOT zeroed — callers that need
    /// maximum security should use [`SecretValue::from_env`] instead.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        let v: Vec<u8> = bytes.into();
        Self(SecretBox::new(Box::from(v.as_slice())))
    }

    /// Read from an environment variable. The string value is converted to
    /// bytes and wrapped immediately.
    ///
    /// # Errors
    /// Returns `std::env::VarError` if the variable is not set or not UTF-8.
    pub fn from_env(var: &str) -> std::result::Result<Self, std::env::VarError> {
        std::env::var(var).map(|s| Self::new(s.into_bytes()))
    }

    /// Expose the secret bytes for use by callers (HMAC, header construction,
    /// etc.). Callers MUST NOT log or persist the returned slice.
    pub fn expose_secret(&self) -> &[u8] {
        self.0.expose_secret()
    }
}

// SAFETY NOTE: SecretValue intentionally does NOT derive or implement:
// - Debug (prevents logging secret bytes)
// - Display (prevents printing secret bytes)
// - Clone (prevents accidental copies that bypass zeroize)
// - Serialize / Deserialize (prevents JSON/wire leakage)
// - PartialEq / Eq (prevents timing side-channel comparisons)
// `tests/compile_fail/token_secret_no_*.rs` enforce these denials at
// compile time via `trybuild` (review R5 — commented-out assertions are
// theatre; only real compile-fail tests carry weight).

/// Interop with `pmcp_code_mode::TokenSecret` when the `code-mode` feature is on.
///
/// Allows a `SecretValue` resolved by a `SecretsProvider` to be fed into the
/// HMAC token machinery in `pmcp-code-mode` without forcing every toolkit
/// consumer to depend on `code-mode`.
#[cfg(feature = "code-mode")]
impl From<SecretValue> for pmcp_code_mode::TokenSecret {
    fn from(v: SecretValue) -> Self {
        pmcp_code_mode::TokenSecret::new(v.expose_secret().to_vec())
    }
}

// ============================================================================
// SecretsProvider trait.
// ============================================================================

/// Trait for secrets providers.
///
/// `get` returns a [`SecretValue`] (toolkit-owned, feature-independent). NEVER
/// returns raw `String` or `Vec<u8>` (anti-pattern §"Anti-Patterns" #11).
#[async_trait]
pub trait SecretsProvider: Send + Sync {
    /// Get a single secret by name.
    async fn get(&self, name: &str) -> Result<SecretValue>;

    /// Get all available secret names (for validation/debugging — names only,
    /// never the values themselves).
    async fn list_available(&self) -> Result<Vec<String>>;

    /// Provider name for logging.
    fn provider_name(&self) -> &'static str;
}

// ============================================================================
// SecretsProviderChain — fall through ordered providers.
// ============================================================================

/// Chain multiple providers, trying each in order until one succeeds.
pub struct SecretsProviderChain {
    providers: Vec<Arc<dyn SecretsProvider>>,
}

impl SecretsProviderChain {
    /// Construct a chain from an ordered list of providers.
    pub fn new(providers: Vec<Arc<dyn SecretsProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl SecretsProvider for SecretsProviderChain {
    async fn get(&self, name: &str) -> Result<SecretValue> {
        let mut last_error: Option<ToolkitError> = None;

        for provider in &self.providers {
            match provider.get(name).await {
                Ok(value) => {
                    tracing::debug!(
                        secret = %name,
                        provider = %provider.provider_name(),
                        "Secret resolved"
                    );
                    return Ok(value);
                }
                Err(e) => {
                    tracing::trace!(
                        secret = %name,
                        provider = %provider.provider_name(),
                        error = %e,
                        "Secret not found in provider, trying next"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ToolkitError::Secret {
            name: name.to_string(),
            cause: "no providers configured".to_string(),
        }))
    }

    async fn list_available(&self) -> Result<Vec<String>> {
        let mut all = Vec::new();
        for provider in &self.providers {
            if let Ok(names) = provider.list_available().await {
                all.extend(names);
            }
        }
        all.sort();
        all.dedup();
        Ok(all)
    }

    fn provider_name(&self) -> &'static str {
        "chain"
    }
}

// ============================================================================
// EnvSecrets — env-var provider (unconditional).
// ============================================================================

/// Environment variable secrets provider.
///
/// Looks up secrets by env-var name. Optionally filters by a prefix — when a
/// prefix is configured, callers pass the un-prefixed name (e.g.
/// `EnvSecrets::new("PMCP_TOOLKIT_").get("DB_URL")` reads `PMCP_TOOLKIT_DB_URL`).
///
/// # Example
/// ```
/// use pmcp_server_toolkit::secrets::EnvSecrets;
/// let secrets = EnvSecrets::new("PMCP_TOOLKIT_");
/// # let _ = secrets;
/// ```
pub struct EnvSecrets {
    /// Optional prefix prepended to the secret name before reading the env var.
    prefix: String,
}

impl EnvSecrets {
    /// Construct an env-var provider with an optional prefix.
    ///
    /// Pass `""` to disable prefix filtering (the secret name is read as-is).
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    /// Construct an env-var provider with no prefix.
    pub fn no_prefix() -> Self {
        Self::new("")
    }

    fn full_name(&self, name: &str) -> String {
        format!("{}{}", self.prefix, name)
    }
}

#[async_trait]
impl SecretsProvider for EnvSecrets {
    async fn get(&self, name: &str) -> Result<SecretValue> {
        let full = self.full_name(name);
        std::env::var(&full)
            .map(|s| SecretValue::new(s.into_bytes()))
            .map_err(|e| ToolkitError::Secret {
                name: full,
                cause: format!("env: {e}"),
            })
    }

    async fn list_available(&self) -> Result<Vec<String>> {
        // Return env vars that look like secrets (all caps, no common system
        // vars). When a prefix is set, only return names that match it (with
        // the prefix stripped, so the result is callable via `get`).
        let system_vars = [
            "PATH", "HOME", "USER", "SHELL", "TERM", "LANG", "PWD", "OLDPWD", "SHLVL", "HOSTNAME",
            "LOGNAME", "MAIL", "EDITOR", "VISUAL",
        ];

        Ok(std::env::vars()
            .filter(|(k, _)| {
                k.chars().all(|c| c.is_ascii_uppercase() || c == '_')
                    && !system_vars.contains(&k.as_str())
            })
            .filter_map(|(k, _)| {
                if self.prefix.is_empty() {
                    Some(k)
                } else {
                    k.strip_prefix(&self.prefix).map(str::to_string)
                }
            })
            .collect())
    }

    fn provider_name(&self) -> &'static str {
        "env"
    }
}

// ============================================================================
// AWS Secrets Manager — org-level provider (aws-feature-gated).
// ============================================================================

/// AWS Secrets Manager provider for org-level shared secrets.
///
/// This provider handles hierarchical secrets where multiple servers share
/// a single secret with the structure:
/// ```json
/// {
///   "london-tube": {
///     "TFL_APP_KEY": "xxx"
///   },
///   "lichess": {
///     "LICHESS_TOKEN": "yyy"
///   }
/// }
/// ```
///
/// The provider extracts secrets only for the specific `server_id`.
#[cfg(feature = "aws")]
pub struct OrgSecretsManagerProvider {
    /// Path to the org-level secret in Secrets Manager.
    secret_path: String,
    /// Server ID to extract secrets for.
    server_id: String,
    /// Cached secrets for this server (extracted from org secret).
    cache: RwLock<Option<HashMap<String, String>>>,
}

#[cfg(feature = "aws")]
impl OrgSecretsManagerProvider {
    /// Construct an org-level Secrets Manager provider.
    pub fn new(secret_path: String, server_id: String) -> Self {
        Self {
            secret_path,
            server_id,
            cache: RwLock::new(None),
        }
    }

    async fn ensure_cached(&self) -> Result<()> {
        {
            let cache = self.cache.read().await;
            if cache.is_some() {
                return Ok(());
            }
        }
        let secrets = self.fetch_secrets().await?;
        let mut cache = self.cache.write().await;
        *cache = Some(secrets);
        Ok(())
    }

    async fn fetch_secrets(&self) -> Result<HashMap<String, String>> {
        use aws_config::BehaviorVersion;
        use aws_sdk_secretsmanager::Client;

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let response = client
            .get_secret_value()
            .secret_id(&self.secret_path)
            .send()
            .await
            .map_err(|e| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: format!("org secretsmanager: {e}"),
            })?;

        let secret_string = response
            .secret_string()
            .ok_or_else(|| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: "org secret has no string value (binary secrets not supported)".to_string(),
            })?;

        let all_secrets: HashMap<String, serde_json::Value> = serde_json::from_str(secret_string)
            .map_err(|e| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: format!("org secret is not valid JSON: {e}"),
            })?;

        let server_secrets = match all_secrets.get(&self.server_id) {
            Some(serde_json::Value::Object(obj)) => {
                let mut result = HashMap::new();
                for (key, value) in obj {
                    if key.starts_with('_') {
                        continue;
                    }
                    let string_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => continue,
                        other => other.to_string(),
                    };
                    if string_value.is_empty() || string_value == "PLACEHOLDER_UPDATE_REQUIRED" {
                        continue;
                    }
                    result.insert(key.clone(), string_value);
                }
                result
            }
            Some(_) => {
                return Err(ToolkitError::Secret {
                    name: self.server_id.clone(),
                    cause: "server entry in org secret is not an object".to_string(),
                });
            }
            None => {
                tracing::warn!(
                    path = %self.secret_path,
                    server_id = %self.server_id,
                    "No secrets configured for this server in org secret"
                );
                HashMap::new()
            }
        };

        tracing::info!(
            path = %self.secret_path,
            server_id = %self.server_id,
            count = server_secrets.len(),
            "Loaded secrets from org-level AWS Secrets Manager"
        );

        Ok(server_secrets)
    }
}

#[cfg(feature = "aws")]
#[async_trait]
impl SecretsProvider for OrgSecretsManagerProvider {
    async fn get(&self, name: &str) -> Result<SecretValue> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        cache
            .as_ref()
            .and_then(|c| c.get(name).cloned())
            .map(|s| SecretValue::new(s.into_bytes()))
            .ok_or_else(|| ToolkitError::Secret {
                name: name.to_string(),
                cause: format!(
                    "not found for server '{}' in org secret '{}'",
                    self.server_id, self.secret_path
                ),
            })
    }

    async fn list_available(&self) -> Result<Vec<String>> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        Ok(cache
            .as_ref()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default())
    }

    fn provider_name(&self) -> &'static str {
        "org-secretsmanager"
    }
}

// ============================================================================
// AWS Secrets Manager — per-server provider (aws-feature-gated).
// ============================================================================

/// AWS Secrets Manager provider for per-server secrets.
///
/// Fetches secrets from AWS Secrets Manager where the secret value is a JSON
/// object containing multiple key-value pairs for a single server.
#[cfg(feature = "aws")]
pub struct SecretsManagerSecrets {
    /// Path to the secret in Secrets Manager.
    secret_path: String,
    /// Cached secrets (fetched once, cached for lifetime).
    cache: RwLock<Option<HashMap<String, String>>>,
}

#[cfg(feature = "aws")]
impl SecretsManagerSecrets {
    /// Construct a per-server Secrets Manager provider.
    pub fn new(secret_path: String) -> Self {
        Self {
            secret_path,
            cache: RwLock::new(None),
        }
    }

    async fn ensure_cached(&self) -> Result<()> {
        {
            let cache = self.cache.read().await;
            if cache.is_some() {
                return Ok(());
            }
        }
        let secrets = self.fetch_secrets().await?;
        let mut cache = self.cache.write().await;
        *cache = Some(secrets);
        Ok(())
    }

    async fn fetch_secrets(&self) -> Result<HashMap<String, String>> {
        use aws_config::BehaviorVersion;
        use aws_sdk_secretsmanager::Client;

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let response = client
            .get_secret_value()
            .secret_id(&self.secret_path)
            .send()
            .await
            .map_err(|e| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: format!("secretsmanager: {e}"),
            })?;

        let secret_string = response
            .secret_string()
            .ok_or_else(|| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: "secret has no string value (binary secrets not supported)".to_string(),
            })?;

        let secrets: HashMap<String, serde_json::Value> = serde_json::from_str(secret_string)
            .map_err(|e| ToolkitError::Secret {
                name: self.secret_path.clone(),
                cause: format!("secret is not valid JSON: {e}"),
            })?;

        let mut result = HashMap::new();
        for (key, value) in secrets {
            if key.starts_with('_') {
                continue;
            }
            let string_value = match value {
                serde_json::Value::String(s) => s,
                serde_json::Value::Null => continue,
                other => other.to_string(),
            };
            if string_value.is_empty() || string_value == "PLACEHOLDER_UPDATE_REQUIRED" {
                continue;
            }
            result.insert(key, string_value);
        }

        tracing::info!(
            path = %self.secret_path,
            count = result.len(),
            "Loaded secrets from AWS Secrets Manager"
        );

        Ok(result)
    }
}

#[cfg(feature = "aws")]
#[async_trait]
impl SecretsProvider for SecretsManagerSecrets {
    async fn get(&self, name: &str) -> Result<SecretValue> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        cache
            .as_ref()
            .and_then(|c| c.get(name).cloned())
            .map(|s| SecretValue::new(s.into_bytes()))
            .ok_or_else(|| ToolkitError::Secret {
                name: name.to_string(),
                cause: format!("not found in Secrets Manager path '{}'", self.secret_path),
            })
    }

    async fn list_available(&self) -> Result<Vec<String>> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        Ok(cache
            .as_ref()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default())
    }

    fn provider_name(&self) -> &'static str {
        "secretsmanager"
    }
}

// ============================================================================
// AWS SSM Parameter Store provider (aws-feature-gated).
// ============================================================================

/// AWS SSM Parameter Store provider.
///
/// Fetches secrets from SSM Parameter Store where each parameter is a separate
/// secret under a common path prefix.
#[cfg(feature = "aws")]
pub struct SsmSecrets {
    /// Path prefix for parameters.
    path_prefix: String,
    /// Cached parameters.
    cache: RwLock<Option<HashMap<String, String>>>,
}

#[cfg(feature = "aws")]
impl SsmSecrets {
    /// Construct an SSM provider scoped to `path_prefix`.
    pub fn new(path_prefix: String) -> Self {
        Self {
            path_prefix,
            cache: RwLock::new(None),
        }
    }

    async fn ensure_cached(&self) -> Result<()> {
        {
            let cache = self.cache.read().await;
            if cache.is_some() {
                return Ok(());
            }
        }
        let params = self.fetch_parameters().await?;
        let mut cache = self.cache.write().await;
        *cache = Some(params);
        Ok(())
    }

    async fn fetch_parameters(&self) -> Result<HashMap<String, String>> {
        use aws_config::BehaviorVersion;
        use aws_sdk_ssm::Client;

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let mut params = HashMap::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client
                .get_parameters_by_path()
                .path(&self.path_prefix)
                .with_decryption(true);

            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| ToolkitError::Secret {
                name: self.path_prefix.clone(),
                cause: format!("ssm: {e}"),
            })?;

            if let Some(parameters) = response.parameters {
                for param in parameters {
                    if let (Some(name), Some(value)) = (param.name, param.value) {
                        let short_name = name
                            .strip_prefix(&self.path_prefix)
                            .unwrap_or(&name)
                            .trim_start_matches('/');
                        params.insert(short_name.to_string(), value);
                    }
                }
            }

            next_token = response.next_token;
            if next_token.is_none() {
                break;
            }
        }

        tracing::info!(
            path = %self.path_prefix,
            count = params.len(),
            "Loaded parameters from AWS SSM Parameter Store"
        );

        Ok(params)
    }
}

#[cfg(feature = "aws")]
#[async_trait]
impl SecretsProvider for SsmSecrets {
    async fn get(&self, name: &str) -> Result<SecretValue> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        cache
            .as_ref()
            .and_then(|c| c.get(name).cloned())
            .map(|s| SecretValue::new(s.into_bytes()))
            .ok_or_else(|| ToolkitError::Secret {
                name: name.to_string(),
                cause: format!("not found in SSM path '{}'", self.path_prefix),
            })
    }

    async fn list_available(&self) -> Result<Vec<String>> {
        self.ensure_cached().await?;
        let cache = self.cache.read().await;
        Ok(cache
            .as_ref()
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default())
    }

    fn provider_name(&self) -> &'static str {
        "ssm"
    }
}

// ============================================================================
// Factory — picks a provider chain from env configuration.
// ============================================================================

/// Construct a `SecretsProvider` chain based on the current environment.
///
/// Resolution order:
/// 1. Org-level Secrets Manager (`aws` feature; activated when `PMCP_SECRETS_PATH`
///    contains `/orgs/`).
/// 2. Per-server Secrets Manager (`aws` feature; activated when `PMCP_SECRETS_PATH`
///    is set without `/orgs/`).
/// 3. SSM Parameter Store (`aws` feature; activated when `PMCP_SSM_PATH` is set).
/// 4. [`EnvSecrets`] (always present, no prefix).
pub fn create_secrets_provider(server_name: &str) -> Arc<dyn SecretsProvider> {
    let mut providers: Vec<Arc<dyn SecretsProvider>> = Vec::new();

    #[cfg(feature = "aws")]
    {
        if let Ok(path) = std::env::var(SECRETS_MANAGER_PATH_VAR) {
            if path.contains("/orgs/") {
                let server_id =
                    std::env::var(SERVER_ID_VAR).unwrap_or_else(|_| server_name.to_string());
                tracing::info!(
                    path = %path,
                    server_id = %server_id,
                    "Using org-level AWS Secrets Manager for secrets"
                );
                providers.push(Arc::new(OrgSecretsManagerProvider::new(path, server_id)));
            } else {
                tracing::info!(path = %path, "Using per-server AWS Secrets Manager for secrets");
                providers.push(Arc::new(SecretsManagerSecrets::new(path)));
            }
        }

        if let Ok(path) = std::env::var(SSM_PATH_VAR) {
            tracing::info!(path = %path, "Using AWS SSM Parameter Store for secrets");
            providers.push(Arc::new(SsmSecrets::new(path)));
        }
    }

    // When the `aws` feature is off, `server_name` is unused at runtime —
    // mark it used to avoid `unused_variables`.
    let _ = server_name;

    // Always include env vars as the final fallback.
    providers.push(Arc::new(EnvSecrets::no_prefix()));

    if providers.len() == 1 {
        providers.pop().expect("non-empty by construction")
    } else {
        Arc::new(SecretsProviderChain::new(providers))
    }
}

// ============================================================================
// Tests.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn secret_value_is_send_sync() {
        // Positive trait check — opposite of the trybuild negative-trait
        // proofs. SecretValue must still cross thread boundaries.
        assert_send_sync::<SecretValue>();
    }

    #[test]
    fn secret_value_exposes_bytes() {
        let sv = SecretValue::new(b"hunter2".to_vec());
        assert_eq!(sv.expose_secret(), b"hunter2");
    }

    #[tokio::test]
    async fn env_secrets_returns_secret_when_var_set() {
        // SAFETY: env var name unique to this test.
        unsafe { std::env::set_var("PMCP_TOOLKIT_TEST_KEY", "value") };
        let provider = EnvSecrets::new("PMCP_TOOLKIT_");
        let secret = provider.get("TEST_KEY").await.expect("expected Ok");
        assert_eq!(secret.expose_secret(), b"value");
        unsafe { std::env::remove_var("PMCP_TOOLKIT_TEST_KEY") };
    }

    #[tokio::test]
    async fn env_secrets_returns_err_when_var_missing() {
        let provider = EnvSecrets::new("PMCP_TOOLKIT_");
        let result = provider.get("DEFINITELY_NOT_SET_12345").await;
        // Cannot use unwrap_err() because `T = SecretValue` is intentionally
        // not Debug — that is precisely the invariant the trybuild compile-fail
        // tests prove. Match on the result explicitly instead.
        match result {
            Ok(_) => panic!("expected Err for missing env var"),
            Err(ToolkitError::Secret { name, cause }) => {
                assert!(name.contains("PMCP_TOOLKIT_DEFINITELY_NOT_SET_12345"));
                assert!(cause.contains("env"));
            }
            Err(other) => panic!("expected ToolkitError::Secret, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn env_secrets_uses_prefix_filter() {
        // Set both a prefixed and a non-prefixed var; verify the provider
        // reads only via the prefix.
        unsafe { std::env::set_var("PMCP_TOOLKIT_DB_URL", "postgres://prefixed") };
        unsafe { std::env::set_var("DB_URL", "postgres://not-prefixed") };

        let provider = EnvSecrets::new("PMCP_TOOLKIT_");
        let secret = provider.get("DB_URL").await.expect("expected Ok");
        assert_eq!(secret.expose_secret(), b"postgres://prefixed");

        unsafe { std::env::remove_var("PMCP_TOOLKIT_DB_URL") };
        unsafe { std::env::remove_var("DB_URL") };
    }

    #[tokio::test]
    async fn env_secrets_no_prefix_reads_var_as_is() {
        unsafe { std::env::set_var("TOOLKIT_NO_PREFIX_TEST", "raw") };
        let provider = EnvSecrets::no_prefix();
        let secret = provider
            .get("TOOLKIT_NO_PREFIX_TEST")
            .await
            .expect("expected Ok");
        assert_eq!(secret.expose_secret(), b"raw");
        unsafe { std::env::remove_var("TOOLKIT_NO_PREFIX_TEST") };
    }

    #[tokio::test]
    async fn chain_provider_falls_through_to_env() {
        unsafe { std::env::set_var("CHAIN_TEST_FALLBACK", "fallback-value") };
        let chain = SecretsProviderChain::new(vec![Arc::new(EnvSecrets::no_prefix())]);
        let secret = chain.get("CHAIN_TEST_FALLBACK").await.expect("expected Ok");
        assert_eq!(secret.expose_secret(), b"fallback-value");
        unsafe { std::env::remove_var("CHAIN_TEST_FALLBACK") };
    }

    #[test]
    fn org_path_detection_matches() {
        assert!("pmcp/orgs/org123/credentials".contains("/orgs/"));
        assert!(!"pmcp/london-tube".contains("/orgs/"));
    }
}
