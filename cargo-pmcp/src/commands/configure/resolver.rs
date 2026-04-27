//! Phase 77 active-target resolver.
//!
//! Field precedence (D-04, REQ-77-06):
//!   ENV > explicit --flag > active target > .pmcp/deploy.toml
//!
//! Active-target selection (D-10):
//!   PMCP_TARGET env > --target CLI flag > .pmcp/active-target > None (D-11 pass-through)
//!
//! On resolution, callers in `main.rs` MAY inject `AWS_PROFILE`, `AWS_REGION`, and
//! `PMCP_API_URL` into the process env (RESEARCH Q5 / Pitfall §6) so deeply-nested
//! callers (CDK subprocess, aws-sdk) see the same values without per-callsite plumbing.
//!
//! NOTE on `set_var` from a library crate: do NOT call `inject_resolved_env_into_process`
//! from `src/lib.rs` paths. Pitfall §8 confines env injection to the binary entry point.
//! This module exposes the helper as `pub`, but documents the constraint in rustdoc.

use anyhow::{bail, Result};
use std::path::Path;

use crate::commands::configure::config::{default_user_config_path, TargetConfigV1, TargetEntry};
use crate::commands::configure::use_cmd::read_active_marker;
use crate::commands::configure::workspace::find_workspace_root;

/// Where a resolved value came from. Used for banner attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetSource {
    /// `PMCP_TARGET` (or `AWS_REGION`, `AWS_PROFILE`, `PMCP_API_URL`) env var.
    Env,
    /// Explicit `--target` (or per-command) CLI flag.
    Flag,
    /// `.pmcp/active-target` workspace marker — used for the active-target name.
    WorkspaceMarker,
    /// A field carried by the resolved target's `~/.pmcp/config.toml` entry.
    Target,
    /// A field carried by `.pmcp/deploy.toml` (Phase 76 fall-through).
    DeployToml,
}

/// A single resolved field with its source attribution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedField {
    /// The resolved value (verbatim from the source).
    pub value: String,
    /// Where the value came from (env / flag / target / deploy.toml).
    pub source: TargetSource,
    /// Target value, if any, when source != Target. `None` when source == Target
    /// or when the target had no value for this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadowed_target_value: Option<String>,
}

impl ResolvedField {
    /// Returns the shadowed target value only when this field is a *real* override —
    /// i.e. source != Target AND the target's value differs from the resolved value.
    /// `None` for source==Target, no-shadow, and benign same-value shadows.
    /// The banner uses this single call to decide whether to warn and what to print.
    pub fn shadowing_target_value(&self) -> Option<&str> {
        self.shadowed_target_value
            .as_deref()
            .filter(|tv| *tv != self.value.as_str())
    }
}

/// Canonical (banner-field, env-var) bindings shared by the resolver (which reads
/// the env vars) and the banner (which warns about overrides). Keep these in lockstep
/// — drift would mean the banner names a different env var than the one the resolver
/// actually consulted.
pub const BANNER_FIELD_ENV_BINDINGS: &[(&str, &str)] = &[
    ("api_url", "PMCP_API_URL"),
    ("aws_profile", "AWS_PROFILE"),
    ("region", "AWS_REGION"),
];

/// Result of resolving the active target and merging fields per D-04.
///
/// **HIGH-3 redesign per 77-REVIEWS.md (Option A):** Fields are stored in a uniform
/// `BTreeMap<String, ResolvedField>` keyed by field name, so per-variant fields
/// (`account_id`, `gcp_project`, `api_token_env`, etc.) all participate in source
/// attribution. The previous shape carried only `api_url` / `aws_profile` / `region`
/// as named struct fields, which left non-pmcp-run variants without an attribution path.
/// Convenience accessors (`region()`, `aws_profile()`, `api_url()`, `account_id()`,
/// `gcp_project()`, `api_token_env()`) keep call sites readable.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    /// Active target name (`None` when no target selected — Phase 76 pass-through path).
    pub name: Option<String>,
    /// Target type tag (`"pmcp-run"`, `"aws-lambda"`, …).
    pub kind: Option<String>,
    /// Resolved scalar fields keyed by canonical name. Every TargetEntry variant's
    /// per-field values land here (after env/flag/target/deploy precedence merge).
    ///
    /// Field names are canonical kebab-case keys: `api_url`, `aws_profile`, `region`,
    /// `account_id`, `gcp_project`, `api_token_env`. Banner display order is enforced
    /// by `banner.rs::FIXED_FIELD_ORDER` (D-13 — api_url / aws_profile / region first;
    /// then variant-specific extras).
    pub fields: std::collections::BTreeMap<String, ResolvedField>,
    /// How the active target NAME was selected (env / flag / marker / none).
    pub name_source: Option<TargetSource>,
}

impl ResolvedTarget {
    /// True when no target was selected — caller should fall through to Phase 76 behavior.
    pub fn is_none(&self) -> bool {
        self.name.is_none() && self.fields.is_empty()
    }

    /// Returns the resolved `api_url` field (if any) with source attribution.
    pub fn api_url(&self) -> Option<&ResolvedField> {
        self.fields.get("api_url")
    }
    /// Returns the resolved `aws_profile` field (if any) with source attribution.
    pub fn aws_profile(&self) -> Option<&ResolvedField> {
        self.fields.get("aws_profile")
    }
    /// Returns the resolved `region` field (if any) with source attribution.
    pub fn region(&self) -> Option<&ResolvedField> {
        self.fields.get("region")
    }
    /// Returns the resolved `account_id` field (if any) with source attribution.
    pub fn account_id(&self) -> Option<&ResolvedField> {
        self.fields.get("account_id")
    }
    /// Returns the resolved `gcp_project` field (if any) with source attribution.
    pub fn gcp_project(&self) -> Option<&ResolvedField> {
        self.fields.get("gcp_project")
    }
    /// Returns the resolved `api_token_env` field (if any) with source attribution.
    pub fn api_token_env(&self) -> Option<&ResolvedField> {
        self.fields.get("api_token_env")
    }
}

/// Resolves which named target is "active" for this invocation.
///
/// Order (D-10): `PMCP_TARGET` env > `--target` CLI flag > `.pmcp/active-target` > None.
pub fn resolve_active_target_name(
    cli_flag: Option<&str>,
) -> Result<Option<(String, TargetSource)>> {
    if let Ok(env) = std::env::var("PMCP_TARGET") {
        if !env.trim().is_empty() {
            return Ok(Some((env.trim().to_string(), TargetSource::Env)));
        }
    }
    if let Some(f) = cli_flag {
        if !f.trim().is_empty() {
            return Ok(Some((f.trim().to_string(), TargetSource::Flag)));
        }
    }
    if let Ok(root) = find_workspace_root() {
        if let Some(name) = read_active_marker(&root)? {
            return Ok(Some((name, TargetSource::WorkspaceMarker)));
        }
    }
    Ok(None)
}

/// Full resolver: returns merged-precedence scalar fields per D-04 (4 levels).
///
/// Precedence per field: ENV > explicit flag > active target > `.pmcp/deploy.toml`
///
/// Returns `Ok(None)` when no target is selected AND `~/.pmcp/config.toml` does not exist
/// AND no `deploy_config` was provided — this is the D-11 zero-touch path.
///
/// **MED-1 fix per 77-REVIEWS.md**: the `explicit_name` parameter lets `configure show <name>`
/// resolve a specific named target's merged-precedence view, even when that name is NOT the
/// currently active target. When `explicit_name = Some(name)`:
///   - active-target resolution (env/flag/marker) is BYPASSED
///   - the named entry is fetched from `~/.pmcp/config.toml`
///   - per-field env/flag/target/deploy.toml precedence still applies
///
/// **HIGH-3 fix per 77-REVIEWS.md (Option A)**: fields are populated into the uniform
/// `BTreeMap<String, ResolvedField>` so all per-variant fields (`account_id`,
/// `gcp_project`, `api_token_env`, etc.) get source attribution.
///
/// `deploy_config` is passed by the caller (typically `main.rs` or the deploy command's
/// dispatch path) which calls `DeployConfig::load(project_root)` once and injects it.
/// The resolver itself does NOT load deploy.toml — it stays path-agnostic and unit-testable.
pub fn resolve_target(
    explicit_name: Option<&str>,
    cli_flag: Option<&str>,
    _project_root: &Path,
    deploy_config: Option<&crate::deployment::config::DeployConfig>,
) -> Result<Option<ResolvedTarget>> {
    let cfg_path = default_user_config_path();
    let cfg_exists = cfg_path.exists();

    // MED-1: explicit_name path bypasses active-target resolution.
    let active = if let Some(name) = explicit_name {
        // Treat the explicit name as having "Flag" source (callers like `configure show <name>`
        // care about per-field attribution, not how the name itself was selected).
        Some((name.to_string(), TargetSource::Flag))
    } else {
        resolve_active_target_name(cli_flag)?
    };

    // D-11 zero-touch: no target selected AND no ~/.pmcp/config.toml AND no deploy.toml available
    // → return None so callers fall through to the legacy Phase 76 behavior.
    if active.is_none() && !cfg_exists && deploy_config.is_none() {
        return Ok(None);
    }

    // Read user config (NotFound → empty per Plan 03 contract).
    let cfg = TargetConfigV1::read(&cfg_path)?;

    // Active selected but user config doesn't contain it → hard error per D-10.
    let entry = match &active {
        Some((name, _src)) => match cfg.targets.get(name) {
            Some(e) => Some(e.clone()),
            None => bail!(
                "target '{}' not found in {} — run `cargo pmcp configure add {}`",
                name,
                cfg_path.display(),
                name
            ),
        },
        None => None,
    };

    // Apply precedence per field: ENV > flag > target > deploy.toml (D-04 / REQ-77-06).
    // HIGH-3: build a uniform field map so non-pmcp-run variants (aws-lambda has account_id;
    // google-cloud-run has gcp_project; cloudflare-workers has account_id+api_token_env)
    // all participate. Field-name keys match the canonical TOML keys used in config.toml.
    let mut fields: std::collections::BTreeMap<String, ResolvedField> =
        std::collections::BTreeMap::new();

    let mut put = |key: &str,
                   env: Option<String>,
                   flag: Option<String>,
                   target: Option<String>,
                   deploy: Option<String>| {
        let target_for_shadow = target.clone();
        if let Some((value, source)) = pick_first_four(env, flag, target, deploy) {
            let shadowed_target_value = if matches!(source, TargetSource::Target) {
                None
            } else {
                target_for_shadow
            };
            fields.insert(
                key.to_string(),
                ResolvedField {
                    value,
                    source,
                    shadowed_target_value,
                },
            );
        }
    };

    // api_url — pmcp-run only at v1.
    put(
        "api_url",
        std::env::var("PMCP_API_URL").ok(),
        None,
        entry.as_ref().and_then(|e| e.api_url().cloned()),
        None, // Phase 76 DeployConfig has no api_url
    );

    // aws_profile — pmcp-run + aws-lambda.
    put(
        "aws_profile",
        std::env::var("AWS_PROFILE").ok(),
        None,
        entry.as_ref().and_then(|e| e.aws_profile().cloned()),
        None, // Phase 76 AwsConfig has no aws_profile
    );

    // region — pmcp-run, aws-lambda, google-cloud-run.
    put(
        "region",
        std::env::var("AWS_REGION")
            .ok()
            .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok()),
        None,
        entry.as_ref().and_then(|e| e.region().cloned()),
        deploy_config
            .map(|d| d.aws.region.clone())
            .filter(|s| !s.is_empty()),
    );

    // account_id — aws-lambda + cloudflare-workers.
    put(
        "account_id",
        None,
        None,
        entry.as_ref().and_then(|e| e.account_id().cloned()),
        deploy_config
            .and_then(|d| d.aws.account_id.clone())
            .filter(|s| !s.is_empty()),
    );

    // gcp_project — google-cloud-run.
    put(
        "gcp_project",
        std::env::var("GOOGLE_CLOUD_PROJECT").ok(),
        None,
        entry.as_ref().and_then(|e| {
            if let TargetEntry::GoogleCloudRun(g) = e {
                g.gcp_project.clone()
            } else {
                None
            }
        }),
        None,
    );

    // api_token_env — cloudflare-workers (an env-var NAME, not value; references-only per D-07).
    put(
        "api_token_env",
        None,
        None,
        entry.as_ref().and_then(|e| {
            if let TargetEntry::CloudflareWorkers(c) = e {
                Some(c.api_token_env.clone())
            } else {
                None
            }
        }),
        None,
    );

    Ok(Some(ResolvedTarget {
        name: active.as_ref().map(|(n, _)| n.clone()),
        kind: entry.as_ref().map(|e| e.type_tag().to_string()),
        fields,
        name_source: active.map(|(_, s)| s),
    }))
}

/// `pick_first_four(env, flag, target, deploy)` returns the first non-empty `Some` with
/// its `TargetSource` label. Source order matches D-04 / REQ-77-06:
/// **ENV > flag > target > deploy.toml** (4 levels).
fn pick_first_four(
    env: Option<String>,
    flag: Option<String>,
    target: Option<String>,
    deploy: Option<String>,
) -> Option<(String, TargetSource)> {
    if let Some(v) = env.filter(|s| !s.is_empty()) {
        return Some((v, TargetSource::Env));
    }
    if let Some(v) = flag.filter(|s| !s.is_empty()) {
        return Some((v, TargetSource::Flag));
    }
    if let Some(v) = target.filter(|s| !s.is_empty()) {
        return Some((v, TargetSource::Target));
    }
    if let Some(v) = deploy.filter(|s| !s.is_empty()) {
        return Some((v, TargetSource::DeployToml));
    }
    None
}

/// Inject `AWS_PROFILE`, `AWS_REGION`, `PMCP_API_URL` into the process env from a resolved
/// target's TARGET-source values (env-source values are already in the env by definition;
/// flag-source has no representation here yet).
///
/// **MUST be called only from the binary entry point (`src/main.rs`).** Library callers
/// must not call this fn — see Pitfall §8.
pub fn inject_resolved_env_into_process(resolved: &ResolvedTarget) {
    if let Some(f) = resolved.api_url() {
        if f.source == TargetSource::Target {
            std::env::set_var("PMCP_API_URL", &f.value);
        }
    }
    if let Some(f) = resolved.aws_profile() {
        if f.source == TargetSource::Target {
            std::env::set_var("AWS_PROFILE", &f.value);
        }
    }
    if let Some(f) = resolved.region() {
        if f.source == TargetSource::Target {
            std::env::set_var("AWS_REGION", &f.value);
        }
    }
}

impl TargetEntry {
    /// Extracts the api_url scalar (only PmcpRun has one).
    pub fn api_url(&self) -> Option<&String> {
        match self {
            TargetEntry::PmcpRun(e) => e.api_url.as_ref(),
            _ => None,
        }
    }
    /// Extracts the aws_profile scalar (PmcpRun + AwsLambda).
    pub fn aws_profile(&self) -> Option<&String> {
        match self {
            TargetEntry::PmcpRun(e) => e.aws_profile.as_ref(),
            TargetEntry::AwsLambda(e) => e.aws_profile.as_ref(),
            _ => None,
        }
    }
    /// Extracts the region scalar (PmcpRun + AwsLambda + GoogleCloudRun).
    pub fn region(&self) -> Option<&String> {
        match self {
            TargetEntry::PmcpRun(e) => e.region.as_ref(),
            TargetEntry::AwsLambda(e) => e.region.as_ref(),
            TargetEntry::GoogleCloudRun(e) => e.region.as_ref(),
            TargetEntry::CloudflareWorkers(_) => None,
        }
    }
    /// Extracts the account_id scalar (AwsLambda + CloudflareWorkers).
    pub fn account_id(&self) -> Option<&String> {
        match self {
            TargetEntry::AwsLambda(e) => e.account_id.as_ref(),
            TargetEntry::CloudflareWorkers(e) => Some(&e.account_id),
            _ => None,
        }
    }
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::configure::config::{
        AwsLambdaEntry, PmcpRunEntry, TargetConfigV1, TargetEntry,
    };
    use serial_test::serial;

    /// Helper to create an isolated test environment: HOME + CWD overridden, PMCP_TARGET
    /// + AWS_* + PMCP_API_URL cleared, all restored on return. Use `#[serial]` on tests.
    fn run_isolated<F, R>(f: F) -> R
    where
        F: FnOnce(&std::path::Path, &std::path::Path) -> R,
    {
        let home_tmp = tempfile::tempdir().unwrap();
        let ws_tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            ws_tmp.path().join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.0.0\"\n",
        )
        .unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_cwd = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        let saved_target = std::env::var_os("PMCP_TARGET");
        let saved_aws_profile = std::env::var_os("AWS_PROFILE");
        let saved_aws_region = std::env::var_os("AWS_REGION");
        let saved_aws_default_region = std::env::var_os("AWS_DEFAULT_REGION");
        let saved_api = std::env::var_os("PMCP_API_URL");
        let saved_gcp = std::env::var_os("GOOGLE_CLOUD_PROJECT");
        std::env::set_var("HOME", home_tmp.path());
        for k in [
            "PMCP_TARGET",
            "AWS_PROFILE",
            "AWS_REGION",
            "AWS_DEFAULT_REGION",
            "PMCP_API_URL",
            "GOOGLE_CLOUD_PROJECT",
        ] {
            std::env::remove_var(k);
        }
        std::env::set_current_dir(ws_tmp.path()).unwrap();
        let r = f(home_tmp.path(), ws_tmp.path());
        let _ = std::env::set_current_dir(&saved_cwd);
        for (k, v) in [
            ("HOME", saved_home),
            ("PMCP_TARGET", saved_target),
            ("AWS_PROFILE", saved_aws_profile),
            ("AWS_REGION", saved_aws_region),
            ("AWS_DEFAULT_REGION", saved_aws_default_region),
            ("PMCP_API_URL", saved_api),
            ("GOOGLE_CLOUD_PROJECT", saved_gcp),
        ] {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        r
    }

    /// Synthesize a minimal DeployConfig from a TOML literal. Only `aws.region` /
    /// `aws.account_id` matter for the resolver path.
    fn make_test_deploy_config_with_region(
        region: &str,
    ) -> crate::deployment::config::DeployConfig {
        let toml_str = format!(
            r#"
[target]
type = "aws-lambda"
version = "1.0"

[aws]
region = "{}"

[server]
name = "test"
memory_mb = 128
timeout_seconds = 30

[environment]

[auth]
enabled = false
provider = "none"

[observability]
log_retention_days = 7
enable_xray = false
create_dashboard = false
"#,
            region
        );
        let mut cfg: crate::deployment::config::DeployConfig =
            toml::from_str(&toml_str).expect("deploy.toml literal must parse for tests");
        cfg.project_root = std::path::PathBuf::from("/tmp");
        cfg
    }

    #[test]
    #[serial]
    fn resolve_active_target_name_env_wins() {
        run_isolated(|_h, ws| {
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "marker_target\n").unwrap();
            std::env::set_var("PMCP_TARGET", "env_target");
            let r = resolve_active_target_name(Some("flag_target")).unwrap();
            std::env::remove_var("PMCP_TARGET");
            assert_eq!(r, Some(("env_target".to_string(), TargetSource::Env)));
        });
    }

    #[test]
    #[serial]
    fn resolve_active_target_name_flag_when_no_env() {
        run_isolated(|_h, ws| {
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "marker_target\n").unwrap();
            let r = resolve_active_target_name(Some("flag_target")).unwrap();
            assert_eq!(r, Some(("flag_target".to_string(), TargetSource::Flag)));
        });
    }

    #[test]
    #[serial]
    fn resolve_active_target_name_marker_when_no_env_no_flag() {
        run_isolated(|_h, ws| {
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "marker_target\n").unwrap();
            let r = resolve_active_target_name(None).unwrap();
            assert_eq!(
                r,
                Some(("marker_target".to_string(), TargetSource::WorkspaceMarker))
            );
        });
    }

    #[test]
    #[serial]
    fn resolve_active_target_name_none() {
        run_isolated(|_h, _ws| {
            let r = resolve_active_target_name(None).unwrap();
            assert!(r.is_none());
        });
    }

    #[test]
    #[serial]
    fn resolve_target_returns_none_when_config_missing_and_no_selection() {
        run_isolated(|_h, ws| {
            let r = resolve_target(None, None, ws, None).unwrap();
            assert!(
                r.is_none(),
                "no config + no selection + no deploy.toml should be D-11 zero-touch"
            );
        });
    }

    #[test]
    #[serial]
    fn resolve_target_errors_when_named_target_not_in_config() {
        run_isolated(|home, ws| {
            let path = home.join(".pmcp").join("config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let cfg = TargetConfigV1::empty();
            cfg.write_atomic(&path).unwrap();
            let err = resolve_target(None, Some("nonexistent"), ws, None).unwrap_err();
            assert!(err.to_string().contains("not found"), "got: {err}");
        });
    }

    #[test]
    #[serial]
    fn resolve_target_returns_target_source_for_target_fields() {
        run_isolated(|home, ws| {
            let path = home.join(".pmcp").join("config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut cfg = TargetConfigV1::empty();
            cfg.targets.insert(
                "dev".into(),
                TargetEntry::PmcpRun(PmcpRunEntry {
                    api_url: Some("https://x".into()),
                    aws_profile: Some("dev-profile".into()),
                    region: Some("us-west-2".into()),
                }),
            );
            cfg.write_atomic(&path).unwrap();
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "dev\n").unwrap();
            let r = resolve_target(None, None, ws, None)
                .unwrap()
                .expect("must resolve");
            assert_eq!(r.name.as_deref(), Some("dev"));
            assert_eq!(r.kind.as_deref(), Some("pmcp-run"));
            assert_eq!(
                r.api_url(),
                Some(&ResolvedField {
                    value: "https://x".into(),
                    source: TargetSource::Target,
                    shadowed_target_value: None,
                })
            );
            assert_eq!(
                r.aws_profile(),
                Some(&ResolvedField {
                    value: "dev-profile".into(),
                    source: TargetSource::Target,
                    shadowed_target_value: None,
                })
            );
            assert_eq!(
                r.region(),
                Some(&ResolvedField {
                    value: "us-west-2".into(),
                    source: TargetSource::Target,
                    shadowed_target_value: None,
                })
            );
        });
    }

    #[test]
    fn pick_first_four_env_wins() {
        let r = pick_first_four(
            Some("E".into()),
            Some("F".into()),
            Some("T".into()),
            Some("D".into()),
        );
        assert_eq!(r, Some(("E".into(), TargetSource::Env)));
    }

    #[test]
    fn pick_first_four_flag_when_no_env() {
        let r = pick_first_four(None, Some("F".into()), Some("T".into()), Some("D".into()));
        assert_eq!(r, Some(("F".into(), TargetSource::Flag)));
    }

    #[test]
    fn pick_first_four_target_when_no_env_no_flag() {
        let r = pick_first_four(None, None, Some("T".into()), Some("D".into()));
        assert_eq!(r, Some(("T".into(), TargetSource::Target)));
    }

    #[test]
    fn pick_first_four_deploy_toml_when_only_source() {
        // B2 fix: 4-level precedence — when env, flag, and target are all None,
        // the resolver MUST fall back to the deploy.toml value labeled DeployToml.
        let r = pick_first_four(None, None, None, Some("D".into()));
        assert_eq!(r, Some(("D".into(), TargetSource::DeployToml)));
    }

    #[test]
    fn pick_first_four_none_when_all_none() {
        let r = pick_first_four(None, None, None, None);
        assert_eq!(r, None);
    }

    #[test]
    fn pick_first_four_skips_empty_string() {
        let r = pick_first_four(Some("".into()), Some("F".into()), None, None);
        assert_eq!(r, Some(("F".into(), TargetSource::Flag)));
    }

    #[test]
    #[serial]
    fn resolve_target_explicit_name_bypasses_active_marker() {
        // MED-1 fix per 77-REVIEWS.md: `configure show <name>` must resolve the requested
        // target even when a different target is active.
        run_isolated(|home, ws| {
            let path = home.join(".pmcp").join("config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut cfg = TargetConfigV1::empty();
            cfg.targets.insert(
                "dev".into(),
                TargetEntry::PmcpRun(PmcpRunEntry {
                    api_url: Some("https://dev".into()),
                    aws_profile: None,
                    region: Some("us-west-2".into()),
                }),
            );
            cfg.targets.insert(
                "prod".into(),
                TargetEntry::AwsLambda(AwsLambdaEntry {
                    aws_profile: Some("prod-profile".into()),
                    region: Some("us-east-1".into()),
                    account_id: Some("123456789012".into()),
                }),
            );
            cfg.write_atomic(&path).unwrap();
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "dev\n").unwrap();

            let r = resolve_target(Some("prod"), None, ws, None)
                .unwrap()
                .expect("must resolve");
            assert_eq!(r.name.as_deref(), Some("prod"));
            assert_eq!(r.kind.as_deref(), Some("aws-lambda"));
            assert_eq!(r.region().map(|f| f.value.as_str()), Some("us-east-1"));
            assert_eq!(
                r.account_id().map(|f| f.value.as_str()),
                Some("123456789012")
            );
            assert_eq!(
                r.aws_profile().map(|f| f.value.as_str()),
                Some("prod-profile")
            );
            // prod (aws-lambda) has no api_url; the dev target's api_url must NOT leak in.
            assert!(
                r.api_url().is_none(),
                "prod (aws-lambda) has no api_url; got: {:?}",
                r.api_url()
            );
        });
    }

    #[test]
    #[serial]
    fn resolve_target_account_id_falls_back_to_deploy_toml() {
        // HIGH-3: account_id must participate in source attribution; the deploy.toml fall-through
        // path must work for it just like it does for region.
        run_isolated(|home, ws| {
            let path = home.join(".pmcp").join("config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut cfg = TargetConfigV1::empty();
            cfg.targets.insert(
                "prod".into(),
                TargetEntry::AwsLambda(AwsLambdaEntry {
                    aws_profile: Some("p".into()),
                    region: Some("us-east-1".into()),
                    account_id: None, // ← intentionally absent — should fall back to deploy.toml
                }),
            );
            cfg.write_atomic(&path).unwrap();
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "prod\n").unwrap();

            let mut deploy = make_test_deploy_config_with_region("us-east-1");
            deploy.aws.account_id = Some("999888777666".into());

            let r = resolve_target(None, None, ws, Some(&deploy))
                .unwrap()
                .expect("must resolve");
            let acct = r
                .account_id()
                .expect("account_id must be present via deploy.toml fall-through");
            assert_eq!(acct.value, "999888777666");
            assert_eq!(acct.source, TargetSource::DeployToml);
        });
    }

    #[test]
    #[serial]
    fn resolve_target_falls_back_to_deploy_toml_for_region() {
        // B2 fix: full integration test — entry has no region, deploy_config does.
        // Resolver must label the resolved region with TargetSource::DeployToml.
        run_isolated(|home, ws| {
            let path = home.join(".pmcp").join("config.toml");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut cfg = TargetConfigV1::empty();
            cfg.targets.insert(
                "dev".into(),
                TargetEntry::PmcpRun(PmcpRunEntry {
                    api_url: Some("https://x".into()),
                    aws_profile: None,
                    region: None, // ← intentionally absent
                }),
            );
            cfg.write_atomic(&path).unwrap();
            std::fs::create_dir_all(ws.join(".pmcp")).unwrap();
            std::fs::write(ws.join(".pmcp").join("active-target"), "dev\n").unwrap();

            let deploy = make_test_deploy_config_with_region("us-west-2");
            let r = resolve_target(None, None, ws, Some(&deploy))
                .unwrap()
                .expect("must resolve");
            assert_eq!(
                r.region(),
                Some(&ResolvedField {
                    value: "us-west-2".into(),
                    source: TargetSource::DeployToml,
                    shadowed_target_value: None,
                })
            );
        });
    }

    #[test]
    #[serial]
    fn inject_aws_env_sets_profile_and_region() {
        run_isolated(|_h, _ws| {
            let mut fields = std::collections::BTreeMap::new();
            fields.insert(
                "aws_profile".into(),
                ResolvedField {
                    value: "p1".into(),
                    source: TargetSource::Target,
                    shadowed_target_value: None,
                },
            );
            fields.insert(
                "region".into(),
                ResolvedField {
                    value: "us-west-2".into(),
                    source: TargetSource::Target,
                    shadowed_target_value: None,
                },
            );
            let r = ResolvedTarget {
                name: Some("dev".into()),
                kind: Some("pmcp-run".into()),
                fields,
                name_source: Some(TargetSource::WorkspaceMarker),
            };
            inject_resolved_env_into_process(&r);
            assert_eq!(std::env::var("AWS_PROFILE").unwrap(), "p1");
            assert_eq!(std::env::var("AWS_REGION").unwrap(), "us-west-2");
        });
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn precedence_holds(
            env in proptest::option::of("[a-z]{1,5}"),
            flag in proptest::option::of("[a-z]{1,5}"),
            target in proptest::option::of("[a-z]{1,5}"),
            deploy in proptest::option::of("[a-z]{1,5}")
        ) {
            // B2 fix: property covers all 4 D-04 precedence levels (env, flag, target, deploy).
            let r = pick_first_four(env.clone(), flag.clone(), target.clone(), deploy.clone());
            let expected = if let Some(e) = env.as_ref().filter(|s| !s.is_empty()) {
                Some((e.clone(), TargetSource::Env))
            } else if let Some(f) = flag.as_ref().filter(|s| !s.is_empty()) {
                Some((f.clone(), TargetSource::Flag))
            } else if let Some(t) = target.as_ref().filter(|s| !s.is_empty()) {
                Some((t.clone(), TargetSource::Target))
            } else if let Some(d) = deploy.as_ref().filter(|s| !s.is_empty()) {
                Some((d.clone(), TargetSource::DeployToml))
            } else {
                None
            };
            prop_assert_eq!(r, expected);
        }
    }
}
