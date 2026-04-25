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

    let rows = match select_rows(&cache, args.url.as_deref())? {
        Some(r) => r,
        None => return Ok(()),
    };

    if global_flags.no_color {
        colored::control::set_override(false);
    }

    print_header_row();
    let now = current_unix_secs();
    for (key, entry) in rows {
        print_status_row(&key, entry, now);
    }
    Ok(())
}

/// Resolve the rows to display. Returns:
/// - `Some(rows)` to print one or more entries.
/// - `None` if a specific URL was requested but missing from the cache (caller
///   should return `Ok(())` after printing the "no cached credentials" line).
fn select_rows<'a>(
    cache: &'a TokenCacheV1,
    url: Option<&str>,
) -> Result<Option<Vec<(String, &'a TokenCacheEntry)>>> {
    match url {
        Some(u) => {
            let key = normalize_cache_key(u)?;
            match cache.entries.get(&key) {
                Some(e) => Ok(Some(vec![(key, e)])),
                None => {
                    println!("No cached credentials for {}.", key.bright_yellow());
                    Ok(None)
                },
            }
        },
        None => Ok(Some(
            cache.entries.iter().map(|(k, v)| (k.clone(), v)).collect(),
        )),
    }
}

/// Print the bright-cyan bold header row with column titles.
fn print_header_row() {
    let header = format!(
        "{:<40}  {:<30}  {:<25}  {:<14}  {}",
        "URL", "ISSUER", "SCOPES", "EXPIRES", "REFRESHABLE"
    );
    println!("{}", header.bright_cyan().bold());
}

/// Get current Unix epoch seconds (saturating to 0 on time-skew errors).
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Format the EXPIRES column for a cache entry (pure helper).
///
/// Returns `(formatted_text, is_expired)` so the caller can colorize the row
/// red when expired without the ANSI escape inflating the column width.
fn format_expires_column(expires_at: Option<u64>, now: u64) -> (String, bool) {
    match expires_at {
        Some(exp) if exp > now => (format!("in {}s", exp - now), false),
        Some(_) => ("EXPIRED".to_string(), true),
        None => ("<unknown>".to_string(), false),
    }
}

/// Print one row of the status table, in red when the entry is expired.
fn print_status_row(key: &str, entry: &TokenCacheEntry, now: u64) {
    let issuer = entry.issuer.clone().unwrap_or_else(|| "<unknown>".into());
    let scopes = if entry.scopes.is_empty() {
        "<none>".into()
    } else {
        entry.scopes.join(",")
    };
    let (expires_plain, expires_is_expired) = format_expires_column(entry.expires_at, now);
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
