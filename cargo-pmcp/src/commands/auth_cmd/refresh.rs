//! `cargo pmcp auth refresh <url>` — force-refresh the cached access_token.

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, normalize_cache_key, refresh_and_persist, TokenCacheV1,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth refresh <url>` — force a refresh of the cached token.
#[derive(Debug, Args)]
pub struct RefreshArgs {
    /// URL of the cached MCP server to force-refresh.
    pub url: String,
}

/// Execute the `refresh` subcommand.
///
/// Calls [`refresh_and_persist`] unconditionally, ignoring expiry. Errors with
/// an actionable message when `entry.refresh_token.is_none()`.
pub async fn execute(args: RefreshArgs, global_flags: &GlobalFlags) -> Result<()> {
    let cache_path = default_multi_cache_path();
    let cache = TokenCacheV1::read(&cache_path)?;
    let key = normalize_cache_key(&args.url)?;
    let entry = cache.entries.get(&key).ok_or_else(|| {
        anyhow::anyhow!(
            "no cached credentials for {}. Run `cargo pmcp auth login {}` first.",
            key,
            key
        )
    })?;

    refresh_and_persist(&cache_path, &key, entry).await?;

    if global_flags.should_output() {
        println!("Refreshed token for {}.", key.bright_green());
    }
    Ok(())
}
