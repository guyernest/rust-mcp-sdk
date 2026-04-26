//! `cargo pmcp configure list` — enumerate defined targets with active marker.
//!
//! Default: plain text with `*` marker on the active target.
//! `--format json`: stable JSON shape with `schema_version`, `active`, `active_source`, `targets[]`.
//! Per Phase 74 D-11: data → stdout; status → stderr.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use crate::commands::configure::config::{
    default_user_config_path, TargetConfigV1, TargetEntry,
};
use crate::commands::configure::use_cmd::read_active_marker;
use crate::commands::configure::workspace::find_workspace_root;
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Output format: `text` (default) or `json`.
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// JSON output shape (REQ-77-01, RESEARCH Q4).
#[derive(Debug, Serialize)]
struct ListJsonOutput {
    schema_version: u32,
    active: Option<String>,
    active_source: ActiveSource,
    targets: Vec<TargetJson>,
}

/// Origin of the active-target value (env, workspace marker, or none).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActiveSource {
    /// Set by `PMCP_TARGET` env var.
    Env,
    /// Read from `<workspace_root>/.pmcp/active-target`.
    WorkspaceMarker,
    /// No active target set.
    None,
}

/// Per-target JSON record.
#[derive(Debug, Serialize)]
struct TargetJson {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    fields: serde_json::Value,
    active: bool,
}

/// Execute `cargo pmcp configure list`.
pub fn execute(args: ListArgs, _gf: &GlobalFlags) -> Result<()> {
    let cfg_path = default_user_config_path();
    let cfg = TargetConfigV1::read(&cfg_path)?;

    let (active, active_source) = compute_active_target()?;

    match args.format.as_str() {
        "json" => print_json(&cfg, active.as_deref(), active_source)?,
        "text" => print_text(&cfg, &cfg_path, active.as_deref(), active_source)?,
        other => anyhow::bail!("unknown --format '{}': expected 'text' or 'json'", other),
    }
    Ok(())
}

/// Computes (active_target_name, source). Order: PMCP_TARGET env > .pmcp/active-target > none.
///
/// Note: this is a simplified version of the full resolver (Plan 06) — it ignores `--target`
/// flag because `list` doesn't take per-invocation overrides; it shows the workspace's
/// active target plus env-override visibility.
fn compute_active_target() -> Result<(Option<String>, ActiveSource)> {
    if let Ok(env) = std::env::var("PMCP_TARGET") {
        if !env.trim().is_empty() {
            return Ok((Some(env.trim().to_string()), ActiveSource::Env));
        }
    }
    // Best-effort workspace lookup; if find_workspace_root fails (e.g. running outside a
    // Cargo workspace), treat as "no active marker" rather than failing list.
    if let Ok(root) = find_workspace_root() {
        if let Some(name) = read_active_marker(&root)? {
            return Ok((Some(name), ActiveSource::WorkspaceMarker));
        }
    }
    Ok((None, ActiveSource::None))
}

fn print_text(
    cfg: &TargetConfigV1,
    cfg_path: &std::path::Path,
    active: Option<&str>,
    active_source: ActiveSource,
) -> Result<()> {
    if cfg.targets.is_empty() {
        // Hint to stderr; no data to stdout.
        eprintln!("no targets defined in {}", cfg_path.display());
        eprintln!("  run `cargo pmcp configure add <name>` to define one");
        return Ok(());
    }

    // Header to stdout (per D-11 — for human-readable text mode we keep the table together
    // on stdout to keep `cargo pmcp configure list` copy-pastable. The `--format json` path
    // is the scriptable channel.)
    println!("{:<6}  {:<22}  {:<20}  fields", "", "NAME", "TYPE");
    for (name, entry) in &cfg.targets {
        let marker = if Some(name.as_str()) == active { "*" } else { " " };
        let type_tag = entry.type_tag();
        let summary = field_summary(entry);
        println!("{:<6}  {:<22}  {:<20}  {}", marker, name, type_tag, summary);
    }
    if matches!(active_source, ActiveSource::Env) {
        eprintln!("note: active target overridden by PMCP_TARGET env var");
    }
    Ok(())
}

fn print_json(
    cfg: &TargetConfigV1,
    active: Option<&str>,
    active_source: ActiveSource,
) -> Result<()> {
    let targets: Vec<TargetJson> = cfg
        .targets
        .iter()
        .map(|(name, entry)| {
            let fields = serde_json::to_value(entry).unwrap_or(serde_json::Value::Null);
            TargetJson {
                name: name.clone(),
                kind: entry.type_tag().to_string(),
                fields,
                active: Some(name.as_str()) == active,
            }
        })
        .collect();

    let out = ListJsonOutput {
        schema_version: TargetConfigV1::CURRENT_VERSION,
        active: active.map(String::from),
        active_source,
        targets,
    };
    // JSON → stdout (data channel).
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn field_summary(entry: &TargetEntry) -> String {
    match entry {
        TargetEntry::PmcpRun(e) => format!(
            "api_url={}, region={}, aws_profile={}",
            e.api_url.as_deref().unwrap_or("-"),
            e.region.as_deref().unwrap_or("-"),
            e.aws_profile.as_deref().unwrap_or("-"),
        ),
        TargetEntry::AwsLambda(e) => format!(
            "aws_profile={}, region={}, account_id={}",
            e.aws_profile.as_deref().unwrap_or("-"),
            e.region.as_deref().unwrap_or("-"),
            e.account_id.as_deref().unwrap_or("-"),
        ),
        TargetEntry::GoogleCloudRun(e) => format!(
            "gcp_project={}, region={}",
            e.gcp_project.as_deref().unwrap_or("-"),
            e.region.as_deref().unwrap_or("-"),
        ),
        TargetEntry::CloudflareWorkers(e) => format!(
            "account_id={}, api_token_env={}",
            e.account_id, e.api_token_env
        ),
    }
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::configure::config::{
        AwsLambdaEntry, PmcpRunEntry, TargetEntry,
    };
    use serial_test::serial;

    /// Run `f` with HOME and CWD overridden to fresh tempdirs, with a Cargo.toml
    /// in CWD so `find_workspace_root` succeeds. Restores both after.
    /// Use `#[serial]` on tests using this helper.
    fn run_in_isolated_home_and_workspace<F, R>(f: F) -> R
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
        std::env::set_var("HOME", home_tmp.path());
        std::env::remove_var("PMCP_TARGET");
        std::env::set_current_dir(ws_tmp.path()).unwrap();

        let r = f(home_tmp.path(), ws_tmp.path());

        let _ = std::env::set_current_dir(&saved_cwd);
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_target {
            Some(v) => std::env::set_var("PMCP_TARGET", v),
            None => std::env::remove_var("PMCP_TARGET"),
        }
        r
    }

    fn write_two_targets(home: &std::path::Path) {
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
                aws_profile: Some("prod".into()),
                region: Some("us-east-1".into()),
                account_id: None,
            }),
        );
        cfg.write_atomic(&path).unwrap();
    }

    fn set_active_marker(ws: &std::path::Path, name: &str) {
        let dir = ws.join(".pmcp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("active-target"), format!("{}\n", name)).unwrap();
    }

    #[test]
    #[serial]
    fn list_json_shape_is_parseable() {
        run_in_isolated_home_and_workspace(|home, ws| {
            write_two_targets(home);
            set_active_marker(ws, "dev");
            // Build the same struct print_json would print and verify shape.
            let cfg =
                TargetConfigV1::read(&home.join(".pmcp").join("config.toml")).unwrap();
            let (active, src) = compute_active_target().unwrap();
            assert_eq!(active.as_deref(), Some("dev"));
            assert!(matches!(src, ActiveSource::WorkspaceMarker));
            let targets: Vec<TargetJson> = cfg
                .targets
                .iter()
                .map(|(name, entry)| TargetJson {
                    name: name.clone(),
                    kind: entry.type_tag().to_string(),
                    fields: serde_json::to_value(entry).unwrap(),
                    active: Some(name.as_str()) == active.as_deref(),
                })
                .collect();
            let out = ListJsonOutput {
                schema_version: 1,
                active: active.clone(),
                active_source: src,
                targets,
            };
            let s = serde_json::to_string_pretty(&out).unwrap();
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            assert_eq!(v["schema_version"], 1);
            assert_eq!(v["active"], "dev");
            assert_eq!(v["active_source"], "workspace_marker");
            assert_eq!(v["targets"].as_array().unwrap().len(), 2);
            // Stable order: dev before prod (BTreeMap)
            assert_eq!(v["targets"][0]["name"], "dev");
            assert_eq!(v["targets"][0]["active"], true);
            assert_eq!(v["targets"][1]["name"], "prod");
            assert_eq!(v["targets"][1]["active"], false);
        });
    }

    #[test]
    #[serial]
    fn compute_active_target_env_overrides_marker() {
        run_in_isolated_home_and_workspace(|_home, ws| {
            set_active_marker(ws, "dev");
            std::env::set_var("PMCP_TARGET", "prod");
            let (active, src) = compute_active_target().unwrap();
            std::env::remove_var("PMCP_TARGET");
            assert_eq!(active.as_deref(), Some("prod"));
            assert!(matches!(src, ActiveSource::Env));
        });
    }

    #[test]
    #[serial]
    fn compute_active_target_none_when_neither_set() {
        run_in_isolated_home_and_workspace(|_home, _ws| {
            let (active, src) = compute_active_target().unwrap();
            assert!(active.is_none());
            assert!(matches!(src, ActiveSource::None));
        });
    }

    #[test]
    #[serial]
    fn execute_text_format_succeeds() {
        run_in_isolated_home_and_workspace(|home, ws| {
            write_two_targets(home);
            set_active_marker(ws, "dev");
            let gf = GlobalFlags::default();
            execute(
                ListArgs {
                    format: "text".into(),
                },
                &gf,
            )
            .unwrap();
        });
    }

    #[test]
    #[serial]
    fn execute_unknown_format_errors() {
        run_in_isolated_home_and_workspace(|home, _ws| {
            write_two_targets(home);
            let gf = GlobalFlags::default();
            let err = execute(
                ListArgs {
                    format: "yaml".into(),
                },
                &gf,
            )
            .unwrap_err();
            assert!(
                err.to_string().contains("yaml") || err.to_string().contains("unknown"),
                "got: {err}"
            );
        });
    }

    #[test]
    #[serial]
    fn execute_empty_config_prints_hint() {
        run_in_isolated_home_and_workspace(|_home, _ws| {
            let gf = GlobalFlags::default();
            // No config written — read returns empty; should not error.
            execute(
                ListArgs {
                    format: "text".into(),
                },
                &gf,
            )
            .unwrap();
        });
    }
}
