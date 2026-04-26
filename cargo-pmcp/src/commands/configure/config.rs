//! `~/.pmcp/config.toml` schema and atomic-write helpers.
//!
//! See `cargo-pmcp/src/commands/auth_cmd/cache.rs` for the Phase 74 blueprint.
//! This module clones that pattern, swapping JSON → TOML and `TokenCacheV1` → `TargetConfigV1`.
//!
//! # Concurrency
//! Writes are atomic per file via `tempfile::NamedTempFile::persist`; concurrent
//! `configure add` from two terminals is last-writer-wins.
//!
//! # Permissions (Unix)
//! Parent dir (`~/.pmcp/`) is chmod'd to `0o700` on first write; config file is
//! chmod'd to `0o600` before the atomic rename.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

/// Top-level config schema. Version 1.
///
/// On disk:
/// ```toml
/// schema_version = 1
///
/// [targets.dev]
/// type = "pmcp-run"
/// api_url = "https://dev.example.com"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetConfigV1 {
    /// Schema version. Readers reject any value != 1.
    pub schema_version: u32,
    /// Target name -> entry. `BTreeMap` for deterministic on-disk order
    /// (diff-friendly per RESEARCH Pitfall §3).
    #[serde(default)]
    pub targets: BTreeMap<String, TargetEntry>,
}

impl TargetConfigV1 {
    /// Current on-disk schema version.
    pub const CURRENT_VERSION: u32 = 1;

    /// Construct an empty (zero-target) config at the current schema version.
    pub fn empty() -> Self {
        Self {
            schema_version: Self::CURRENT_VERSION,
            targets: BTreeMap::new(),
        }
    }

    /// Read a config file, returning `empty()` if the file does not exist.
    /// Errors on malformed TOML or unsupported `schema_version`.
    pub fn read(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let v: Self = toml::from_str(&s).with_context(|| {
                    format!("config file corrupt — delete {} to reset", path.display())
                })?;
                if v.schema_version != Self::CURRENT_VERSION {
                    anyhow::bail!(
                        "config schema_version {} unsupported (expected {}); upgrade cargo-pmcp",
                        v.schema_version,
                        Self::CURRENT_VERSION
                    );
                }
                Ok(v)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
            Err(e) => Err(anyhow::anyhow!(
                "failed to read config file {}: {e}",
                path.display()
            )),
        }
    }

    /// Atomic write: tempfile-in-same-dir -> chmod -> persist (rename).
    ///
    /// Cross-platform atomic on modern Linux + Windows per tempfile docs.
    /// Concurrent writers are last-writer-wins (see module rustdoc).
    pub fn write_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("config path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir {}", parent.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }

        let mut tmp = NamedTempFile::new_in(parent)?;
        let body = toml::to_string_pretty(self)?;
        tmp.write_all(body.as_bytes())?;
        tmp.flush()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }

        tmp.persist(path)
            .map_err(|e| anyhow::anyhow!("atomic rename failed: {e}"))?;
        Ok(())
    }
}

/// Per-variant fields for a `pmcp-run` target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PmcpRunEntry {
    /// pmcp-run discovery endpoint URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    /// AWS CLI profile to use for SigV4 signing / credential lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws_profile: Option<String>,
    /// AWS region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

/// Per-variant fields for an `aws-lambda` target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AwsLambdaEntry {
    /// AWS CLI profile to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws_profile: Option<String>,
    /// AWS region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// AWS account ID (for cross-account scenarios).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

/// Per-variant fields for a `google-cloud-run` target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct GoogleCloudRunEntry {
    /// GCP project ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gcp_project: Option<String>,
    /// GCP region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

/// Per-variant fields for a `cloudflare-workers` target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CloudflareWorkersEntry {
    /// Cloudflare account ID.
    pub account_id: String,
    /// Name of the env var that carries the Cloudflare API token.
    /// References-only secrets policy (REQ-77-07).
    pub api_token_env: String,
}

/// One target entry. Tagged enum: TOML `[targets.foo]\ntype = "pmcp-run"\napi_url = "..."`
/// parses to `TargetEntry::PmcpRun(PmcpRunEntry { api_url: Some(...), .. })`.
///
/// `deny_unknown_fields` is enforced PER VARIANT via the wrapped named struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TargetEntry {
    /// pmcp-run platform target.
    PmcpRun(PmcpRunEntry),
    /// AWS Lambda direct deploy target.
    AwsLambda(AwsLambdaEntry),
    /// Google Cloud Run target.
    GoogleCloudRun(GoogleCloudRunEntry),
    /// Cloudflare Workers target.
    CloudflareWorkers(CloudflareWorkersEntry),
}

impl TargetEntry {
    /// Returns the kebab-case type tag (`pmcp-run`, `aws-lambda`, …).
    pub fn type_tag(&self) -> &'static str {
        match self {
            Self::PmcpRun(_) => "pmcp-run",
            Self::AwsLambda(_) => "aws-lambda",
            Self::GoogleCloudRun(_) => "google-cloud-run",
            Self::CloudflareWorkers(_) => "cloudflare-workers",
        }
    }
}

/// Returns `<home>/.pmcp/config.toml`.
pub fn default_user_config_path() -> PathBuf {
    let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push(".pmcp");
    p.push("config.toml");
    p
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_current_schema_version() {
        let c = TargetConfigV1::empty();
        assert_eq!(c.schema_version, 1);
        assert!(c.targets.is_empty());
    }

    #[test]
    fn roundtrip_pmcp_run_entry() {
        let mut c = TargetConfigV1::empty();
        c.targets.insert(
            "dev".to_string(),
            TargetEntry::PmcpRun(PmcpRunEntry {
                api_url: Some("https://dev.example.com".into()),
                aws_profile: Some("dev-profile".into()),
                region: Some("us-west-2".into()),
            }),
        );
        let s = toml::to_string_pretty(&c).unwrap();
        let back: TargetConfigV1 = toml::from_str(&s).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn read_nonexistent_returns_empty() {
        let p = std::path::PathBuf::from("/nonexistent/path/config.toml");
        let c = TargetConfigV1::read(&p).unwrap();
        assert!(c.targets.is_empty());
        assert_eq!(c.schema_version, 1);
    }

    #[test]
    fn read_rejects_unsupported_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::write(&p, "schema_version = 999\n[targets]\n").unwrap();
        let err = TargetConfigV1::read(&p).unwrap_err();
        assert!(
            err.to_string().contains("schema_version 999 unsupported"),
            "got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600_perms_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".pmcp").join("config.toml");
        TargetConfigV1::empty().write_atomic(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "config file mode = {:o}", mode);
        let dir_mode = std::fs::metadata(p.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "config dir mode = {:o}", dir_mode);
    }

    #[test]
    fn target_entry_pmcp_run_rejects_unknown_field() {
        // Proves M3 fix: per-variant named struct + deny_unknown_fields actually rejects.
        let toml_str = r#"schema_version = 1
[targets.foo]
type = "pmcp-run"
api_url = "https://x"
bogus = "x"
"#;
        let r: Result<TargetConfigV1, _> = toml::from_str(toml_str);
        assert!(
            r.is_err(),
            "unknown field on PmcpRunEntry must reject; got: {:?}",
            r
        );
        let err_msg = format!("{}", r.unwrap_err());
        assert!(
            err_msg.contains("bogus")
                || err_msg.contains("unknown")
                || err_msg.contains("unexpected"),
            "error must mention the unknown field; got: {err_msg}"
        );
    }

    #[test]
    fn target_entry_aws_lambda_rejects_unknown_field() {
        let toml_str = r#"schema_version = 1
[targets.bar]
type = "aws-lambda"
region = "us-east-1"
not_a_field = "x"
"#;
        let r: Result<TargetConfigV1, _> = toml::from_str(toml_str);
        assert!(
            r.is_err(),
            "unknown field on AwsLambdaEntry must reject; got: {:?}",
            r
        );
    }

    #[test]
    fn all_four_variants_parse() {
        let toml_str = r#"schema_version = 1
[targets.a]
type = "pmcp-run"
[targets.b]
type = "aws-lambda"
[targets.c]
type = "google-cloud-run"
[targets.d]
type = "cloudflare-workers"
account_id = "acct-1"
api_token_env = "MY_TOKEN"
"#;
        let c: TargetConfigV1 = toml::from_str(toml_str).unwrap();
        assert_eq!(c.targets.len(), 4);
        assert_eq!(c.targets["a"].type_tag(), "pmcp-run");
        assert_eq!(c.targets["b"].type_tag(), "aws-lambda");
        assert_eq!(c.targets["c"].type_tag(), "google-cloud-run");
        assert_eq!(c.targets["d"].type_tag(), "cloudflare-workers");
    }

    #[test]
    fn default_path_is_home_pmcp_config_toml() {
        let p = default_user_config_path();
        // Last 2 components: <home>/.pmcp/config.toml
        assert_eq!(p.file_name().and_then(|s| s.to_str()), Some("config.toml"));
        assert_eq!(
            p.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str()),
            Some(".pmcp")
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn pmcp_run_targets_roundtrip(
            api in proptest::option::of("[a-z]{3,10}://[a-z0-9.]{3,20}"),
            profile in proptest::option::of("[a-z][a-z0-9_-]{0,15}"),
            region in proptest::option::of("[a-z]{2}-[a-z]{4,8}-[1-9]")
        ) {
            let entry = TargetEntry::PmcpRun(PmcpRunEntry {
                api_url: api,
                aws_profile: profile,
                region,
            });
            let mut c = TargetConfigV1::empty();
            c.targets.insert("t".to_string(), entry);
            let s = toml::to_string_pretty(&c).unwrap();
            let back: TargetConfigV1 = toml::from_str(&s).unwrap();
            prop_assert_eq!(back, c);
        }
    }
}
