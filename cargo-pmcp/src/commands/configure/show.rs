//! `cargo pmcp configure show [<name>]` — inspect resolved or raw target config.
//!
//! Default: merged-precedence form with per-field source attribution
//! (env > flag > target > deploy.toml). Plan 06's resolver fills in non-`target` sources;
//! this plan ships the merged form with placeholder source = "target" until then.
//!
//! `--raw`: prints only the stored target block (no precedence merge, no source labels).
//! Per RESEARCH Q6: `show` is an inspection command — D-13 banner does NOT fire here.

use anyhow::{bail, Context, Result};
use clap::Args;

use crate::commands::configure::config::{
    default_user_config_path, TargetConfigV1, TargetEntry,
};
use crate::commands::configure::use_cmd::read_active_marker;
use crate::commands::configure::workspace::find_workspace_root;
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Target name. If omitted, uses the active target (.pmcp/active-target).
    pub name: Option<String>,

    /// Show only the stored target block (no precedence merge, no source attribution).
    #[arg(long)]
    pub raw: bool,
}

/// Execute `cargo pmcp configure show`.
pub fn execute(args: ShowArgs, _gf: &GlobalFlags) -> Result<()> {
    let cfg_path = default_user_config_path();
    let cfg = TargetConfigV1::read(&cfg_path)?;

    let name = match args.name {
        Some(n) => n,
        None => resolve_active_or_fail(&cfg_path)?,
    };

    let entry = cfg.targets.get(&name).with_context(|| {
        format!(
            "target '{}' not found in {} — run `cargo pmcp configure add {}`",
            name,
            cfg_path.display(),
            name
        )
    })?;

    if args.raw {
        print_raw(&name, entry)?;
    } else {
        print_merged_with_attribution(&name, entry);
    }
    Ok(())
}

/// Resolve the active target name when no `<name>` was provided. Order:
/// `PMCP_TARGET` env > `<workspace>/.pmcp/active-target` > error.
fn resolve_active_or_fail(cfg_path: &std::path::Path) -> Result<String> {
    if let Ok(env) = std::env::var("PMCP_TARGET") {
        if !env.trim().is_empty() {
            return Ok(env.trim().to_string());
        }
    }
    let root = find_workspace_root()
        .context("no `<name>` provided and could not locate workspace root")?;
    match read_active_marker(&root)? {
        Some(n) => Ok(n),
        None => bail!(
            "no `<name>` provided and no active target set.\n  \
             Either run `cargo pmcp configure use <name>` first, or pass a name: `cargo pmcp configure show <name>`.\n  \
             (config: {})",
            cfg_path.display()
        ),
    }
}

/// Print the stored target block as TOML — same format the user would write
/// into config.toml. No precedence merge; no source attribution.
fn print_raw(name: &str, entry: &TargetEntry) -> Result<()> {
    // Use owned TargetEntry::clone so the wrapper map can serialize without borrowed-ref complications.
    let mut inner = std::collections::BTreeMap::new();
    inner.insert(name.to_string(), entry.clone());
    let mut top = std::collections::BTreeMap::new();
    top.insert("targets", inner);
    let s = toml::to_string_pretty(&top)?;
    // Data → stdout per Phase 74 D-11.
    println!("{}", s);
    Ok(())
}

/// Print the merged form with per-field source attribution. Field order is fixed
/// per D-13 (api_url, aws_profile, region, then type-specific extras), matching
/// the banner Plan 06 will emit. Currently every value's source is "target"
/// because the full resolver isn't wired yet — Plan 06 will replace this body.
fn print_merged_with_attribution(name: &str, entry: &TargetEntry) {
    println!("→ Target: {} ({})", name, entry.type_tag());

    let (api_url, aws_profile, region, type_specific) = collect_for_display(entry);
    println!(
        "  api_url     = {}{}",
        api_url.as_deref().unwrap_or("<unset>"),
        if api_url.is_some() {
            "  (source: target)"
        } else {
            ""
        }
    );
    println!(
        "  aws_profile = {}{}",
        aws_profile.as_deref().unwrap_or("<unset>"),
        if aws_profile.is_some() {
            "  (source: target)"
        } else {
            ""
        }
    );
    println!(
        "  region      = {}{}",
        region.as_deref().unwrap_or("<unset>"),
        if region.is_some() {
            "  (source: target)"
        } else {
            ""
        }
    );
    for (k, v) in type_specific {
        println!("  {:<11} = {}  (source: target)", k, v);
    }
}

/// Collect display fields in fixed banner order (D-13).
/// Returns `(api_url, aws_profile, region, extras)` where `extras` carries
/// per-variant fields (account_id, gcp_project, api_token_env, …).
fn collect_for_display(
    entry: &TargetEntry,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<(&'static str, String)>,
) {
    let mut extras: Vec<(&'static str, String)> = Vec::new();
    match entry {
        TargetEntry::PmcpRun(e) => (
            e.api_url.clone(),
            e.aws_profile.clone(),
            e.region.clone(),
            extras,
        ),
        TargetEntry::AwsLambda(e) => {
            if let Some(a) = &e.account_id {
                extras.push(("account_id", a.clone()));
            }
            (None, e.aws_profile.clone(), e.region.clone(), extras)
        },
        TargetEntry::GoogleCloudRun(e) => {
            if let Some(p) = &e.gcp_project {
                extras.push(("gcp_project", p.clone()));
            }
            (None, None, e.region.clone(), extras)
        },
        TargetEntry::CloudflareWorkers(e) => {
            extras.push(("account_id", e.account_id.clone()));
            extras.push(("api_token_env", e.api_token_env.clone()));
            (None, None, None, extras)
        },
    }
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::configure::config::{
        CloudflareWorkersEntry, PmcpRunEntry, TargetEntry,
    };
    use serial_test::serial;

    /// Run `f` with HOME and CWD overridden to fresh tempdirs (with a Cargo.toml
    /// in CWD so `find_workspace_root` succeeds when invoked). Restores both after.
    /// Use `#[serial]` on tests using this helper.
    fn run_in_isolated_home<F, R>(f: F) -> R
    where
        F: FnOnce(&std::path::Path) -> R,
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

        let r = f(home_tmp.path());

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

    fn write_target(home: &std::path::Path, name: &str) {
        let path = home.join(".pmcp").join("config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut cfg = TargetConfigV1::read(&path).unwrap_or_else(|_| TargetConfigV1::empty());
        cfg.targets.insert(
            name.into(),
            TargetEntry::PmcpRun(PmcpRunEntry {
                api_url: Some("https://x".into()),
                aws_profile: Some("dev".into()),
                region: Some("us-west-2".into()),
            }),
        );
        cfg.write_atomic(&path).unwrap();
    }

    #[test]
    #[serial]
    fn show_with_explicit_name_succeeds() {
        run_in_isolated_home(|home| {
            write_target(home, "dev");
            let gf = GlobalFlags::default();
            execute(
                ShowArgs {
                    name: Some("dev".into()),
                    raw: false,
                },
                &gf,
            )
            .unwrap();
        });
    }

    #[test]
    #[serial]
    fn show_unknown_target_errors() {
        run_in_isolated_home(|home| {
            write_target(home, "dev");
            let gf = GlobalFlags::default();
            let err = execute(
                ShowArgs {
                    name: Some("nonexistent".into()),
                    raw: false,
                },
                &gf,
            )
            .unwrap_err();
            assert!(err.to_string().contains("not found"), "got: {err}");
        });
    }

    #[test]
    #[serial]
    fn show_no_arg_no_active_errors() {
        run_in_isolated_home(|home| {
            write_target(home, "dev");
            // No active marker, no PMCP_TARGET env (cleared by helper).
            let gf = GlobalFlags::default();
            let err = execute(
                ShowArgs {
                    name: None,
                    raw: false,
                },
                &gf,
            )
            .unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("no") && msg.contains("active"),
                "got: {err}"
            );
        });
    }

    #[test]
    #[serial]
    fn show_raw_succeeds() {
        run_in_isolated_home(|home| {
            write_target(home, "dev");
            let gf = GlobalFlags::default();
            execute(
                ShowArgs {
                    name: Some("dev".into()),
                    raw: true,
                },
                &gf,
            )
            .unwrap();
        });
    }

    #[test]
    fn collect_for_display_pmcp_run_returns_three_main_fields() {
        let e = TargetEntry::PmcpRun(PmcpRunEntry {
            api_url: Some("u".into()),
            aws_profile: Some("p".into()),
            region: Some("r".into()),
        });
        let (a, p, r, x) = collect_for_display(&e);
        assert_eq!(a.as_deref(), Some("u"));
        assert_eq!(p.as_deref(), Some("p"));
        assert_eq!(r.as_deref(), Some("r"));
        assert!(x.is_empty());
    }

    #[test]
    fn collect_for_display_cloudflare_returns_extras() {
        let e = TargetEntry::CloudflareWorkers(CloudflareWorkersEntry {
            account_id: "A".into(),
            api_token_env: "T".into(),
        });
        let (a, p, r, x) = collect_for_display(&e);
        assert!(a.is_none());
        assert!(p.is_none());
        assert!(r.is_none());
        assert_eq!(x.len(), 2);
        assert_eq!(x[0].0, "account_id");
        assert_eq!(x[0].1, "A");
        assert_eq!(x[1].0, "api_token_env");
        assert_eq!(x[1].1, "T");
    }
}
