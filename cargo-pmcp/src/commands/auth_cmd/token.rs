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
/// Looks up the cached entry, auto-refreshes if near expiry (D-15), then
/// prints the raw token to stdout with a trailing newline. Errors go to
/// stderr via `anyhow`. No banners. (D-11)
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

    // D-15: transparent refresh when within REFRESH_WINDOW_SECS of expiry.
    let token = if is_near_expiry(entry, REFRESH_WINDOW_SECS) {
        // Status message to stderr so stdout stays clean (D-11).
        eprintln!("Refreshing cached token for {}...", key);
        refresh_and_persist(&cache_path, &key, entry).await?
    } else {
        entry.access_token.clone()
    };

    // D-11: raw token to stdout, newline-terminated, NO other output on success.
    println!("{}", token);
    Ok(())
}

#[cfg(test)]
mod tests {
    // stderr/stdout discipline is verified by integration test in
    // cargo-pmcp/tests/auth_integration.rs (Task 2.4).

    #[test]
    fn it_compiles() {
        // placeholder to ensure the module is test-discoverable
    }
}
