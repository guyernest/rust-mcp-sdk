//! `cargo pmcp auth token <url>` — raw access_token to stdout (gh-style).

use anyhow::Result;
use clap::Args;

use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, is_near_expiry, normalize_cache_key, refresh_and_persist,
    TokenCacheV1, REFRESH_WINDOW_SECS,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth token <url>` — print the cached token raw to stdout.
#[derive(Debug, Args)]
pub struct TokenArgs {
    /// URL of the cached MCP server.
    pub url: String,
}

/// Execute the `token` subcommand.
///
/// Prints the cached token raw to stdout with a trailing newline, auto-refreshing
/// when within `REFRESH_WINDOW_SECS` of expiry. Status messages go to stderr so
/// stdout stays scriptable (`TOKEN=$(cargo pmcp auth token URL)`).
pub async fn execute(args: TokenArgs, _global_flags: &GlobalFlags) -> Result<()> {
    let cache_path = default_multi_cache_path();
    let cache = TokenCacheV1::read(&cache_path)?;
    let key = normalize_cache_key(&args.url)?;
    let entry = cache.entries.get(&key).ok_or_else(|| {
        anyhow::anyhow!(
            "no cached token for {}. Run `cargo pmcp auth login {}` first.",
            key,
            key
        )
    })?;

    let token = if is_near_expiry(entry, REFRESH_WINDOW_SECS) {
        eprintln!("Refreshing cached token for {}...", key);
        refresh_and_persist(&cache_path, &key, entry).await?
    } else {
        entry.access_token.clone()
    };

    println!("{}", token);
    Ok(())
}
