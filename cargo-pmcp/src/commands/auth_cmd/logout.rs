//! `cargo pmcp auth logout [<url> | --all]` — remove cached credentials.

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, normalize_cache_key, TokenCacheV1,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth logout [<url> | --all]` — remove one entry or wipe the cache.
#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// URL of the MCP server to log out from (mutually exclusive with --all).
    #[arg(conflicts_with = "all")]
    pub url: Option<String>,

    /// Log out from every cached server.
    #[arg(long)]
    pub all: bool,
}

/// Execute the `logout` subcommand.
///
/// With no args: errors out. `--all` clears all entries; a positional URL removes one.
pub async fn execute(args: LogoutArgs, global_flags: &GlobalFlags) -> Result<()> {
    if args.url.is_none() && !args.all {
        anyhow::bail!("specify a server URL or --all to log out of everything");
    }

    let cache_path = default_multi_cache_path();
    let mut cache = TokenCacheV1::read(&cache_path)?;

    if args.all {
        let count = cache.entries.len();
        cache.entries.clear();
        cache.write_atomic(&cache_path)?;
        if global_flags.should_output() {
            println!("Logged out of {} cached server(s).", count);
        }
        return Ok(());
    }

    let raw_url = args.url.as_deref().expect("url set (checked above)");
    let key = normalize_cache_key(raw_url)?;
    match cache.entries.remove(&key) {
        Some(_) => {
            cache.write_atomic(&cache_path)?;
            if global_flags.should_output() {
                println!("Logged out of {}.", key.bright_green());
            }
        },
        None => {
            if global_flags.should_output() {
                println!(
                    "No cached credentials for {} (nothing to do).",
                    key.bright_yellow()
                );
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::GlobalFlags;

    fn gf() -> GlobalFlags {
        GlobalFlags {
            verbose: false,
            no_color: true,
            quiet: true,
        }
    }

    #[tokio::test]
    async fn no_args_errors() {
        let err = execute(
            LogoutArgs {
                url: None,
                all: false,
            },
            &gf(),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("specify a server URL or --all"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn clap_rejects_url_with_all() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct T {
            #[command(flatten)]
            a: LogoutArgs,
        }
        let r = T::try_parse_from(["t", "https://x.example", "--all"]);
        assert!(r.is_err());
    }
}
