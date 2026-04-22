//! `cargo pmcp auth status [<url>]` — tabular cache inspection.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, normalize_cache_key, TokenCacheEntry, TokenCacheV1,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth status [<url>]` — print a 5-column cache table.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// URL to inspect. If absent, prints a table of all cached servers.
    pub url: Option<String>,
}

/// Execute the `status` subcommand.
///
/// Renders a tabular view of the multi-server cache: URL | ISSUER | SCOPES |
/// EXPIRES | REFRESHABLE. Never prints the raw access_token.
pub async fn execute(args: StatusArgs, global_flags: &GlobalFlags) -> Result<()> {
    let cache = TokenCacheV1::read(&default_multi_cache_path())?;
    if cache.entries.is_empty() {
        println!("No cached credentials. Run `cargo pmcp auth login <url>` to authenticate.");
        return Ok(());
    }

    let rows: Vec<(String, &TokenCacheEntry)> = match args.url {
        Some(u) => {
            let key = normalize_cache_key(&u)?;
            match cache.entries.get(&key) {
                Some(e) => vec![(key, e)],
                None => {
                    println!("No cached credentials for {}.", key.bright_yellow());
                    return Ok(());
                },
            }
        },
        None => cache.entries.iter().map(|(k, v)| (k.clone(), v)).collect(),
    };

    // Apply color after width formatting — ANSI escapes would otherwise be
    // counted by `{:<N}` and miscompute column padding.
    if global_flags.no_color {
        colored::control::set_override(false);
    }

    let header_plain = format!(
        "{:<40}  {:<30}  {:<25}  {:<14}  {}",
        "URL", "ISSUER", "SCOPES", "EXPIRES", "REFRESHABLE"
    );
    println!("{}", header_plain.bright_cyan().bold());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    for (key, entry) in rows {
        let issuer = entry.issuer.clone().unwrap_or_else(|| "<unknown>".into());
        let scopes = if entry.scopes.is_empty() {
            "<none>".into()
        } else {
            entry.scopes.join(",")
        };
        let (expires_plain, expires_is_expired) = match entry.expires_at {
            Some(exp) if exp > now => (format!("in {}s", exp - now), false),
            Some(_) => ("EXPIRED".to_string(), true),
            None => ("<unknown>".to_string(), false),
        };
        let refreshable_plain = if entry.refresh_token.is_some() {
            "yes"
        } else {
            "no"
        };

        let row_plain = format!(
            "{:<40}  {:<30}  {:<25}  {:<14}  {}",
            key, issuer, scopes, expires_plain, refreshable_plain
        );
        if expires_is_expired {
            println!("{}", row_plain.bright_red());
        } else {
            println!("{}", row_plain);
        }
    }
    Ok(())
}
