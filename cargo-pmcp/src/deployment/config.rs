use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::deployment::post_deploy_tests::PostDeployTestsConfig;
use crate::deployment::widgets::WidgetsConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub target: TargetConfig,
    pub aws: AwsConfig,
    pub server: ServerConfig,
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub secrets: HashMap<String, String>,
    pub auth: AuthConfig,
    pub observability: ObservabilityConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_gateway: Option<ApiGatewayConfig>,
    /// Assets configuration for bundling files with deployment
    #[serde(default)]
    pub assets: AssetsConfig,

    /// Composition configuration for server-to-server communication
    #[serde(default)]
    pub composition: CompositionConfig,

    /// IAM declarations for the Lambda execution role (tables, buckets, raw statements).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the no-`[iam]`
    /// path so pre-existing `.pmcp/deploy.toml` files round-trip unchanged.
    #[serde(default, skip_serializing_if = "IamConfig::is_empty")]
    pub iam: IamConfig,

    /// Widget pre-build declarations (Phase 79).
    ///
    /// The `skip_serializing_if` guard preserves byte-identity on the
    /// no-`[[widgets]]` path so pre-existing `.pmcp/deploy.toml` files
    /// round-trip unchanged (mirrors the Phase 76 IamConfig D-05 contract).
    #[serde(default, skip_serializing_if = "WidgetsConfig::is_empty")]
    pub widgets: WidgetsConfig,

    /// Post-deploy verification config (Phase 79).
    ///
    /// `Option::is_none` skip preserves byte-identity on the
    /// no-`[post_deploy_tests]` path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_deploy_tests: Option<PostDeployTestsConfig>,

    /// Project root directory (not serialized)
    #[serde(skip)]
    pub project_root: PathBuf,
}

/// Composition configuration for MCP server-to-server communication.
///
/// Enables servers to be composed in a tiered architecture:
/// - `foundation` tier: Core data connectors (databases, CRMs, APIs)
/// - `domain` tier: Business logic servers that call foundation servers
///
/// # Example Configuration
///
/// ```toml
/// [composition]
/// tier = "foundation"
/// allow_composition = true
/// internal_only = false
/// description = "Database explorer providing SQL query execution"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionConfig {
    /// Server tier: "foundation" or "domain"
    #[serde(default = "default_tier")]
    pub tier: String,

    /// Whether this server can be called by other servers
    #[serde(default = "default_true")]
    pub allow_composition: bool,

    /// Whether this server is only available internally (not exposed via API)
    #[serde(default)]
    pub internal_only: bool,

    /// Description for composition discovery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_tier() -> String {
    "foundation".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(rename = "type")]
    pub target_type: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub memory_mb: u32,
    pub timeout_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved_concurrency: Option<u32>,
}

/// OAuth authentication configuration for MCP servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Whether OAuth is enabled
    pub enabled: bool,

    /// OAuth provider type (cognito, oidc, none)
    #[serde(default = "default_oauth_provider")]
    pub provider: String,

    /// Cognito-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cognito: Option<CognitoConfig>,

    /// External OIDC provider configuration (future)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc: Option<OidcConfig>,

    /// Dynamic Client Registration settings
    #[serde(default)]
    pub dcr: DcrConfig,

    /// OAuth scopes configuration
    #[serde(default)]
    pub scopes: ScopesConfig,

    /// Legacy fields for backwards compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default)]
    pub callback_urls: Vec<String>,
}

fn default_oauth_provider() -> String {
    "none".to_string()
}

/// Cognito-specific OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoConfig {
    /// Cognito User Pool ID (created or existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,

    /// User Pool name (used when creating new)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_name: Option<String>,

    /// Resource server identifier for scopes
    #[serde(default = "default_resource_server_id")]
    pub resource_server_id: String,

    /// Social identity providers
    #[serde(default)]
    pub social_providers: Vec<String>,

    /// MFA setting (optional, required, off)
    #[serde(default = "default_mfa")]
    pub mfa: String,

    /// Access token TTL
    #[serde(default = "default_access_token_ttl")]
    pub access_token_ttl: String,

    /// Refresh token TTL
    #[serde(default = "default_refresh_token_ttl")]
    pub refresh_token_ttl: String,

    /// Hosted UI domain prefix (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

fn default_resource_server_id() -> String {
    "mcp".to_string()
}

fn default_mfa() -> String {
    "optional".to_string()
}

fn default_access_token_ttl() -> String {
    "1h".to_string()
}

fn default_refresh_token_ttl() -> String {
    "30d".to_string()
}

/// External OIDC provider configuration.
///
/// Supports multiple OIDC providers via the `provider_type` field:
/// - `google` - Google Identity Platform
/// - `auth0` - Auth0
/// - `okta` - Okta
/// - `entra` - Microsoft Entra ID (Azure AD)
/// - `generic` - Any OIDC-compliant provider
///
/// The SDK's `GenericOidcProvider` is used for token validation at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    /// OIDC provider type (google, auth0, okta, entra, generic)
    #[serde(default = "default_oidc_provider_type")]
    pub provider_type: String,

    /// OIDC issuer URL
    pub issuer: String,

    /// Client ID for this application
    pub client_id: String,

    /// Client secret (optional for public clients)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Expected audience (defaults to client_id if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,

    /// JWKS URL (auto-discovered if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_url: Option<String>,

    /// Custom scopes to request (in addition to openid)
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
}

fn default_oidc_provider_type() -> String {
    "generic".to_string()
}

fn default_oidc_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

/// Dynamic Client Registration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrConfig {
    /// Enable DCR endpoint
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Patterns to auto-detect public clients (no secret required)
    #[serde(default = "default_public_client_patterns")]
    pub public_client_patterns: Vec<String>,

    /// Default scopes for new clients
    #[serde(default = "default_scopes")]
    pub default_scopes: Vec<String>,

    /// Allowed scopes clients can request
    #[serde(default = "default_allowed_scopes")]
    pub allowed_scopes: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_public_client_patterns() -> Vec<String> {
    vec![
        "claude".to_string(),
        "desktop".to_string(),
        "cursor".to_string(),
        "mcp-inspector".to_string(),
        "chatgpt".to_string(),
    ]
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "mcp/read".to_string(),
    ]
}

fn default_allowed_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
        "mcp/read".to_string(),
        "mcp/write".to_string(),
    ]
}

/// OAuth scopes configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScopesConfig {
    /// Custom scope definitions
    #[serde(default)]
    pub custom: std::collections::HashMap<String, String>,
}

impl Default for CognitoConfig {
    fn default() -> Self {
        Self {
            user_pool_id: None,
            user_pool_name: None,
            resource_server_id: default_resource_server_id(),
            social_providers: vec![],
            mfa: default_mfa(),
            access_token_ttl: default_access_token_ttl(),
            refresh_token_ttl: default_refresh_token_ttl(),
            domain: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub log_retention_days: u32,
    pub enable_xray: bool,
    pub create_dashboard: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarms: Option<AlarmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    pub error_threshold: u32,
    pub latency_threshold_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGatewayConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_limit: Option<u32>,
}

/// Assets configuration for bundling files with deployment.
///
/// Assets are files bundled with your MCP server (databases, markdown files, configs).
/// They are accessible at runtime via `pmcp::assets` module.
///
/// # Example Configuration
///
/// ```toml
/// [assets]
/// include = [
///     "chinook.db",
///     "resources/**/*.md",
///     "config/*.toml",
/// ]
/// exclude = ["**/*.tmp", "**/.DS_Store"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsConfig {
    /// Glob patterns for files to include as assets.
    ///
    /// Patterns are relative to the workspace root.
    /// Examples: `"chinook.db"`, `"resources/**/*.md"`, `"config/*.toml"`
    #[serde(default)]
    pub include: Vec<String>,

    /// Glob patterns for files to exclude from assets.
    ///
    /// Applied after include patterns.
    /// Examples: `"**/*.tmp"`, `"**/.DS_Store"`
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Base directory for assets (relative to workspace root).
    ///
    /// If not specified, assets are resolved from workspace root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<String>,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![
                "**/*.tmp".to_string(),
                "**/.DS_Store".to_string(),
                "**/Thumbs.db".to_string(),
            ],
            base_dir: None,
        }
    }
}

impl AssetsConfig {
    /// Create a new assets config with the given include patterns.
    #[allow(dead_code)]
    pub fn new(include: Vec<String>) -> Self {
        Self {
            include,
            ..Default::default()
        }
    }

    /// Check if any assets are configured.
    pub fn has_assets(&self) -> bool {
        !self.include.is_empty()
    }

    /// Resolve asset patterns to actual file paths.
    pub fn resolve_files(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let base = match &self.base_dir {
            Some(dir) => project_root.join(dir),
            None => project_root.to_path_buf(),
        };

        let mut files = Vec::new();

        for pattern in &self.include {
            let full_pattern = base.join(pattern);
            let glob_pattern = full_pattern.to_string_lossy();

            let paths =
                glob::glob(&glob_pattern).context(format!("Invalid glob pattern: {}", pattern))?;

            for entry in paths.flatten() {
                if entry.is_file() && !self.is_excluded(&entry) {
                    files.push(entry);
                }
            }
        }

        Ok(files)
    }

    /// Check if a path matches any exclude pattern.
    fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.exclude {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                if glob_pattern.matches(&path_str) {
                    return true;
                }
            }
        }

        false
    }
}

impl DeployConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".pmcp/deploy.toml");

        if !config_path.exists() {
            anyhow::bail!("Deployment not initialized. Run: cargo pmcp deploy init");
        }

        let config_str =
            std::fs::read_to_string(&config_path).context("Failed to read .pmcp/deploy.toml")?;

        let mut config: Self =
            toml::from_str(&config_str).context("Failed to parse .pmcp/deploy.toml")?;

        // Set the project root
        config.project_root = project_root.to_path_buf();

        // Auto-detect and merge template-required assets
        config.auto_configure_template_assets(project_root);

        Ok(config)
    }

    /// Auto-configure assets based on detected templates in the workspace.
    ///
    /// This scans the workspace config for templates that require specific assets
    /// (e.g., sqlite-explorer requires chinook.db) and adds them to the assets config
    /// if they exist in the workspace and aren't already configured.
    fn auto_configure_template_assets(&mut self, project_root: &Path) {
        // Load workspace config to detect templates
        let workspace_config = match crate::utils::config::WorkspaceConfig::load() {
            Ok(config) => config,
            Err(_) => return, // No workspace config, skip auto-detection
        };

        // Collect template-required assets
        let mut required_assets: Vec<String> = Vec::new();

        for server in workspace_config.servers.values() {
            match server.template.as_str() {
                "sqlite-explorer" | "db-explorer" => {
                    // sqlite-explorer template requires chinook.db
                    if !required_assets.contains(&"chinook.db".to_string()) {
                        required_assets.push("chinook.db".to_string());
                    }
                },
                // Add other templates with asset requirements here
                _ => {},
            }
        }

        // Add required assets that exist and aren't already configured
        for asset in required_assets {
            let asset_path = project_root.join(&asset);
            if asset_path.exists() && !self.assets.include.contains(&asset) {
                self.assets.include.push(asset);
            }
        }
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&config_dir).context("Failed to create .pmcp directory")?;

        let config_path = config_dir.join("deploy.toml");
        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, config_str).context("Failed to write .pmcp/deploy.toml")?;

        Ok(())
    }

    pub fn default_for_server(server_name: String, region: String, project_root: PathBuf) -> Self {
        let mut environment = HashMap::new();
        environment.insert("RUST_LOG".to_string(), "info".to_string());

        Self {
            target: TargetConfig {
                target_type: "aws-lambda".to_string(),
                version: "1.0.0".to_string(),
            },
            aws: AwsConfig {
                region,
                account_id: None,
            },
            server: ServerConfig {
                name: server_name,
                memory_mb: 512,
                timeout_seconds: 30,
                reserved_concurrency: None,
            },
            environment,
            secrets: HashMap::new(),
            auth: AuthConfig::default(),
            observability: ObservabilityConfig {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: Some(AlarmConfig {
                    error_threshold: 10,
                    latency_threshold_ms: 5000,
                }),
            },
            api_gateway: None,
            assets: AssetsConfig::default(),
            composition: CompositionConfig::default(),
            iam: IamConfig::default(),
            widgets: WidgetsConfig::default(),
            post_deploy_tests: None,
            project_root,
        }
    }

    /// Create config with OAuth enabled using Cognito
    pub fn with_cognito_oauth(
        server_name: String,
        region: String,
        project_root: PathBuf,
        cognito_config: CognitoConfig,
    ) -> Self {
        let mut config = Self::default_for_server(server_name, region, project_root);
        config.auth = AuthConfig {
            enabled: true,
            provider: "cognito".to_string(),
            cognito: Some(cognito_config),
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: None,
            callback_urls: vec![],
        };
        config
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "none".to_string(),
            cognito: None,
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: None,
            callback_urls: vec![],
        }
    }
}

impl AuthConfig {
    /// Convert auth config to environment variables for the server.
    ///
    /// These environment variables are read by the server_common template
    /// to initialize the appropriate `IdentityProvider` at runtime.
    #[allow(dead_code)]
    pub fn to_env_vars(&self, region: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if !self.enabled {
            vars.insert("AUTH_PROVIDER".to_string(), "none".to_string());
            return vars;
        }

        match self.provider.as_str() {
            "cognito" => {
                vars.insert("AUTH_PROVIDER".to_string(), "cognito".to_string());
                vars.insert("AUTH_REGION".to_string(), region.to_string());

                // Use cognito config if available, fall back to legacy fields
                if let Some(cognito) = &self.cognito {
                    if let Some(user_pool_id) = &cognito.user_pool_id {
                        vars.insert("AUTH_USER_POOL_ID".to_string(), user_pool_id.clone());
                    }
                } else if let Some(user_pool_id) = &self.user_pool_id {
                    vars.insert("AUTH_USER_POOL_ID".to_string(), user_pool_id.clone());
                }

                // Client ID from cognito config or legacy field
                if let Some(client_id) = &self.client_id {
                    vars.insert("AUTH_CLIENT_ID".to_string(), client_id.clone());
                }
            },
            "oidc" | "google" | "auth0" | "okta" | "entra" => {
                if let Some(oidc) = &self.oidc {
                    vars.insert("AUTH_PROVIDER".to_string(), oidc.provider_type.clone());
                    vars.insert("AUTH_ISSUER".to_string(), oidc.issuer.clone());
                    vars.insert("AUTH_CLIENT_ID".to_string(), oidc.client_id.clone());

                    if let Some(secret) = &oidc.client_secret {
                        vars.insert("AUTH_CLIENT_SECRET".to_string(), secret.clone());
                    }
                } else {
                    // Fallback to generic OIDC with provider as type
                    vars.insert("AUTH_PROVIDER".to_string(), self.provider.clone());
                }
            },
            _ => {
                vars.insert("AUTH_PROVIDER".to_string(), "none".to_string());
            },
        }

        vars
    }

    /// Create auth config for Cognito.
    #[allow(dead_code)]
    pub fn cognito(_region: &str, user_pool_id: &str, client_id: &str) -> Self {
        Self {
            enabled: true,
            provider: "cognito".to_string(),
            cognito: Some(CognitoConfig {
                user_pool_id: Some(user_pool_id.to_string()),
                user_pool_name: None,
                resource_server_id: default_resource_server_id(),
                social_providers: vec![],
                mfa: default_mfa(),
                access_token_ttl: default_access_token_ttl(),
                refresh_token_ttl: default_refresh_token_ttl(),
                domain: None,
            }),
            oidc: None,
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: Some(user_pool_id.to_string()),
            client_id: Some(client_id.to_string()),
            callback_urls: vec![],
        }
    }

    /// Create auth config for generic OIDC.
    #[allow(dead_code)]
    pub fn oidc(provider_type: &str, issuer: &str, client_id: &str) -> Self {
        Self {
            enabled: true,
            provider: "oidc".to_string(),
            cognito: None,
            oidc: Some(OidcConfig {
                provider_type: provider_type.to_string(),
                issuer: issuer.to_string(),
                client_id: client_id.to_string(),
                client_secret: None,
                audience: None,
                jwks_url: None,
                scopes: default_oidc_scopes(),
            }),
            dcr: DcrConfig::default(),
            scopes: ScopesConfig::default(),
            user_pool_id: None,
            client_id: Some(client_id.to_string()),
            callback_urls: vec![],
        }
    }
}

impl Default for DcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            public_client_patterns: default_public_client_patterns(),
            default_scopes: default_scopes(),
            allowed_scopes: default_allowed_scopes(),
        }
    }
}

impl Default for CompositionConfig {
    fn default() -> Self {
        Self {
            tier: default_tier(),
            allow_composition: true,
            internal_only: false,
            description: None,
        }
    }
}

/// IAM declarations for the deployed Lambda's execution role.
///
/// Translated by [`crate::deployment::iam::render_iam_block`] into
/// `mcpFunction.addToRolePolicy(...)` calls in the generated CDK `stack.ts`.
/// An empty `IamConfig` is elided from serialised TOML via the
/// `skip_serializing_if = "IamConfig::is_empty"` guard on `DeployConfig::iam`,
/// so files without an `[iam]` section round-trip byte-identically.
///
/// See [`TablePermission`], [`BucketPermission`], and [`IamStatement`] for
/// the per-kind details.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IamConfig {
    /// DynamoDB table permissions (sugar form — see [`TablePermission`]).
    #[serde(default)]
    pub tables: Vec<TablePermission>,

    /// S3 bucket permissions (sugar form — object-level ARNs only — see
    /// [`BucketPermission`]).
    #[serde(default)]
    pub buckets: Vec<BucketPermission>,

    /// Raw IAM policy statements (passthrough after the validator).
    #[serde(default)]
    pub statements: Vec<IamStatement>,
}

impl IamConfig {
    /// Returns `true` when the IAM config contains no declarations
    /// (all three vectors are empty).
    ///
    /// Used by `DeployConfig`'s
    /// `#[serde(skip_serializing_if = "IamConfig::is_empty")]` to preserve
    /// byte-identity for configs without an `[iam]` section (D-05).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.buckets.is_empty() && self.statements.is_empty()
    }
}

/// DynamoDB table permission (sugar form).
///
/// `actions` is a subset of `{"read", "write", "readwrite"}`. Wave 3's
/// translation (`cargo_pmcp::deployment::iam::render_table`) expands each
/// sugar keyword into the 4-action DynamoDB list per 76-CONTEXT.md D-02:
/// - `read` → `GetItem`, `Query`, `Scan`, `BatchGetItem`
/// - `write` → `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem`
/// - `readwrite` → union (8 actions)
///
/// When `include_indexes = true`, the resource list additionally grants
/// `arn:aws:dynamodb:${this.region}:${this.account}:table/NAME/index/*`
/// (GSI/LSI access).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermission {
    /// Table name (AWS region + account inherited from the deploy context).
    pub name: String,

    /// Sugar keywords — subset of `{"read", "write", "readwrite"}`. Unknown
    /// values are rejected by Wave 4's validator.
    pub actions: Vec<String>,

    /// When `true`, grants access to `table/NAME/index/*` (GSI/LSI).
    #[serde(default)]
    pub include_indexes: bool,
}

/// S3 bucket permission (sugar form — object-level ARN only).
///
/// Wave 3's translation (`cargo_pmcp::deployment::iam::render_bucket`)
/// expands sugar keywords per 76-CONTEXT.md D-02:
/// - `read` → `GetObject`
/// - `write` → `PutObject`, `DeleteObject`
/// - `readwrite` → union (3 actions)
///
/// The resource ARN is `arn:aws:s3:::NAME/*` (object-level only).
/// Bucket-level operations (e.g. `s3:ListBucket`) must be declared via
/// `[[iam.statements]]` per CR scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketPermission {
    /// Bucket name.
    pub name: String,

    /// Sugar keywords — subset of `{"read", "write", "readwrite"}`.
    pub actions: Vec<String>,
}

/// Raw IAM `PolicyStatement` (passthrough after Wave 4 validation).
///
/// Emitted verbatim as
/// `new iam.PolicyStatement({ effect, actions, resources })` in the generated
/// `stack.ts`. Wave 4's validator enforces:
/// - `effect` is `"Allow"` or `"Deny"`
/// - `actions` is non-empty, each matching `^[a-z0-9-]+:[A-Za-z0-9*]+$`
/// - `resources` is non-empty (ARN or `*`)
/// - Rejects Allow `*:*` with resources `*`
/// - Warns on cross-account ARNs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamStatement {
    /// `"Allow"` or `"Deny"` (validated in Wave 4).
    pub effect: String,

    /// Non-empty list of `service:action` strings; `*` permitted as an
    /// action-name wildcard.
    pub actions: Vec<String>,

    /// Non-empty list of ARNs (or `"*"`). Cross-account ARNs emit a validator
    /// warning in Wave 4.
    pub resources: Vec<String>,
}

#[cfg(test)]
mod iam_wave1_tests {
    use super::*;

    #[test]
    fn iam_config_default_is_empty() {
        assert!(
            IamConfig::default().is_empty(),
            "IamConfig::default() must report empty in Wave 1"
        );
    }

    #[test]
    fn deploy_config_without_iam_block_parses() {
        // Round-trip: serialise a default DeployConfig (no [iam]) and re-parse it.
        // The `skip_serializing_if` guard must elide the [iam] table from the
        // emitted TOML so that subsequent parses do not require the section.
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave1"),
        );
        let serialised = toml::to_string(&cfg).expect("DeployConfig serialises");
        let parsed: DeployConfig =
            toml::from_str(&serialised).expect("round-trip DeployConfig parses");
        assert!(
            parsed.iam.is_empty(),
            "round-tripped DeployConfig must have empty IamConfig"
        );
    }

    #[test]
    fn deploy_config_serializes_without_iam_when_empty() {
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave1"),
        );
        let out = toml::to_string(&cfg).expect("DeployConfig serialises");
        assert!(
            !out.contains("[iam]"),
            "empty IamConfig must not emit [iam] table (D-05 backward-compat) — got:\n{out}"
        );
    }
}

#[cfg(test)]
mod iam_wave2_tests {
    //! Wave 2 (phase 76 Plan 02) — full `IamConfig` schema coverage.
    //!
    //! These tests drive the replacement of the Wave 1 zero-sized stub with the
    //! three-vector schema described in CONTEXT.md (§Scope) and
    //! `CLI_IAM_CHANGE_REQUEST.md`. They co-exist with `iam_wave1_tests` — the
    //! Wave 1 invariants (default-empty, skip-serialize-empty, round-trip) are
    //! still enforced there.
    //!
    //! Integration-level coverage (serde roundtrip through
    //! `cargo_pmcp::deployment::config::*`) lives in
    //! `cargo-pmcp/tests/iam_config.rs`. Because `cargo_pmcp`'s library surface
    //! does not re-export `deployment::config` (same lib-boundary constraint
    //! documented in Wave 1's summary, Rule 3 deviation #1), the struct-level
    //! assertions live in-crate here where `super::*` makes the private types
    //! directly accessible.

    use super::*;

    /// Builds a cost-coach-shaped TOML fixture by serialising a valid
    /// `default_for_server` baseline and appending the three `[[iam.*]]`
    /// blocks from 76-CONTEXT.md (§Scope). This sidesteps the fragility of
    /// hand-crafting every required field (several structs — `ServerConfig`,
    /// `ObservabilityConfig`, `TargetConfig` — have no `#[serde(default)]`)
    /// and keeps the fixture locked to whatever non-IAM defaults the crate
    /// currently ships.
    fn cost_coach_deploy_toml() -> String {
        let baseline = DeployConfig::default_for_server(
            "cost-coach".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-wave2-fixture"),
        );
        let mut out = toml::to_string(&baseline).expect("baseline serialises");
        out.push_str(
            r#"
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
"#,
        );
        out
    }

    #[test]
    fn iam_config_default_has_three_empty_vectors() {
        let iam = IamConfig::default();
        assert!(iam.tables.is_empty());
        assert!(iam.buckets.is_empty());
        assert!(iam.statements.is_empty());
        assert!(iam.is_empty());
    }

    #[test]
    fn iam_config_is_empty_flips_when_any_vector_is_populated() {
        // Populating any one vector MUST flip is_empty to false — this is the
        // exact condition that toggles the D-05 `skip_serializing_if` guard.
        let mut iam = IamConfig::default();
        iam.tables.push(TablePermission {
            name: "t1".into(),
            actions: vec!["read".into()],
            include_indexes: false,
        });
        assert!(
            !iam.is_empty(),
            "populated tables vector must flip is_empty"
        );

        let mut iam = IamConfig::default();
        iam.buckets.push(BucketPermission {
            name: "b1".into(),
            actions: vec!["read".into()],
        });
        assert!(
            !iam.is_empty(),
            "populated buckets vector must flip is_empty"
        );

        let mut iam = IamConfig::default();
        iam.statements.push(IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["arn:aws:s3:::b/*".into()],
        });
        assert!(
            !iam.is_empty(),
            "populated statements vector must flip is_empty"
        );
    }

    #[test]
    fn cost_coach_shaped_toml_parses_into_populated_iam_config() {
        let fixture = cost_coach_deploy_toml();
        let cfg: DeployConfig = toml::from_str(&fixture).expect("cost-coach TOML parses");

        assert_eq!(cfg.iam.tables.len(), 1);
        assert_eq!(cfg.iam.tables[0].name, "cost-coach-tenants");
        assert_eq!(cfg.iam.tables[0].actions, vec!["readwrite".to_string()]);
        assert!(cfg.iam.tables[0].include_indexes);

        assert_eq!(cfg.iam.buckets.len(), 1);
        assert_eq!(cfg.iam.buckets[0].name, "cost-coach-snapshots");
        assert_eq!(cfg.iam.buckets[0].actions, vec!["readwrite".to_string()]);

        assert_eq!(cfg.iam.statements.len(), 1);
        assert_eq!(cfg.iam.statements[0].effect, "Allow");
        assert_eq!(
            cfg.iam.statements[0].actions,
            vec!["secretsmanager:GetSecretValue".to_string()]
        );
        assert_eq!(
            cfg.iam.statements[0].resources,
            vec!["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*".to_string()]
        );

        assert!(!cfg.iam.is_empty());
    }

    #[test]
    fn include_indexes_defaults_false_when_omitted() {
        let baseline = DeployConfig::default_for_server(
            "demo".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-wave2-include-indexes"),
        );
        let mut toml_str = toml::to_string(&baseline).expect("baseline serialises");
        // Append a table entry WITHOUT `include_indexes` — default must be false.
        toml_str.push_str(
            r#"
[[iam.tables]]
name = "t1"
actions = ["read"]
"#,
        );
        let cfg: DeployConfig = toml::from_str(&toml_str).expect("parses");
        assert_eq!(cfg.iam.tables.len(), 1);
        assert!(
            !cfg.iam.tables[0].include_indexes,
            "include_indexes must default to false when TOML omits it"
        );
    }

    #[test]
    fn populated_iam_roundtrips_losslessly_through_toml() {
        // Round-trip: parse → serialise → re-parse → structural equality
        // (IamConfig intentionally derives no PartialEq per PATTERNS.md §S1, so
        // compare each field individually).
        let fixture = cost_coach_deploy_toml();
        let orig: DeployConfig = toml::from_str(&fixture).expect("cost-coach TOML parses");
        let serialised = toml::to_string(&orig).expect("DeployConfig serialises");
        let reparsed: DeployConfig = toml::from_str(&serialised)
            .unwrap_or_else(|e| panic!("reparse failed — serialised:\n{serialised}\nerror: {e}"));

        assert_eq!(orig.iam.tables.len(), reparsed.iam.tables.len());
        assert_eq!(orig.iam.tables[0].name, reparsed.iam.tables[0].name);
        assert_eq!(orig.iam.tables[0].actions, reparsed.iam.tables[0].actions);
        assert_eq!(
            orig.iam.tables[0].include_indexes,
            reparsed.iam.tables[0].include_indexes
        );

        assert_eq!(orig.iam.buckets.len(), reparsed.iam.buckets.len());
        assert_eq!(orig.iam.buckets[0].name, reparsed.iam.buckets[0].name);
        assert_eq!(orig.iam.buckets[0].actions, reparsed.iam.buckets[0].actions);

        assert_eq!(orig.iam.statements.len(), reparsed.iam.statements.len());
        assert_eq!(
            orig.iam.statements[0].effect,
            reparsed.iam.statements[0].effect
        );
        assert_eq!(
            orig.iam.statements[0].actions,
            reparsed.iam.statements[0].actions
        );
        assert_eq!(
            orig.iam.statements[0].resources,
            reparsed.iam.statements[0].resources
        );
    }

    #[test]
    fn d05_empty_iam_still_elides_every_iam_header() {
        // D-05 backward-compat guard, refined: ensure that the post-Wave-2
        // struct still elides ALL iam-related table headers when every vector
        // is empty (not just the bare `[iam]` header that Wave 1 tested).
        let cfg = DeployConfig::default_for_server(
            "demo-server".to_string(),
            "us-west-2".to_string(),
            std::path::PathBuf::from("/tmp/phase76-iam-wave2"),
        );
        let out = toml::to_string(&cfg).expect("DeployConfig serialises");
        for header in [
            "[iam]",
            "[[iam.tables]]",
            "[[iam.buckets]]",
            "[[iam.statements]]",
        ] {
            assert!(
                !out.contains(header),
                "empty IamConfig must not emit {header} header (D-05) — got:\n{out}"
            );
        }
    }

    #[test]
    fn sub_struct_types_are_constructable() {
        // Sanity: the sub-structs are `pub` and their fields are `pub` — Wave 3
        // (render_iam_block) and Wave 4 (validator) will need to build these by
        // hand for test fixtures.
        let _tp = TablePermission {
            name: "t".into(),
            actions: vec!["read".into()],
            include_indexes: false,
        };
        let _bp = BucketPermission {
            name: "b".into(),
            actions: vec!["write".into()],
        };
        let _st = IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["arn:aws:s3:::bucket/*".into()],
        };
        let _iam = IamConfig::default();
    }
}
