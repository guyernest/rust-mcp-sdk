//! `cargo pmcp auth` — manage OAuth credentials for MCP servers.
//!
//! Provides five subcommands that together give developers one-time browser
//! login per server, then transparent bearer-token reuse across every
//! `cargo pmcp test/*`, `connect`, `preview`, `schema`, `dev`, `loadtest/run`,
//! and `pentest` invocation.
//!
//! Per-server token cache: `~/.pmcp/oauth-cache.json` (schema_version: 1).
//! See `.planning/phases/74-.../74-CONTEXT.md` D-06..D-16 for command semantics.
//!
//! # Concurrency (review MED-4)
//!
//! Parallel `auth login` invocations are not safe; prefer sequential logins.
//! The cache file uses last-writer-wins atomic-rename semantics
//! (`tempfile::NamedTempFile::persist`), which means BOTH same-URL and
//! different-URL concurrent `auth login` calls may result in lost entries.
//! This matches `gh auth login` / `aws sso login` behavior and is an
//! accepted tradeoff — genuine simultaneous browser logins are rare during
//! initial developer setup. If this becomes a real friction point, a
//! future phase will introduce advisory file locking or a read-merge-retry
//! loop. See T-74-F in the phase threat model.

pub mod cache;
pub mod login;
pub mod logout;
pub mod refresh;
pub mod status;
pub mod token;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp auth <subcommand>`
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Log in to an OAuth-protected MCP server (PKCE, optionally with DCR)
    Login(login::LoginArgs),
    /// Remove cached credentials for a server (or all servers)
    Logout(logout::LogoutArgs),
    /// Show cached credential status
    Status(status::StatusArgs),
    /// Print the cached access token to stdout (raw, gh-style)
    Token(token::TokenArgs),
    /// Force-refresh the cached access token using the cached refresh_token
    Refresh(refresh::RefreshArgs),
}

impl AuthCommand {
    /// Execute the selected auth subcommand, blocking on the internal async
    /// runtime.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        match self {
            AuthCommand::Login(args) => runtime.block_on(login::execute(args, global_flags)),
            AuthCommand::Logout(args) => runtime.block_on(logout::execute(args, global_flags)),
            AuthCommand::Status(args) => runtime.block_on(status::execute(args, global_flags)),
            AuthCommand::Token(args) => runtime.block_on(token::execute(args, global_flags)),
            AuthCommand::Refresh(args) => runtime.block_on(refresh::execute(args, global_flags)),
        }
    }
}
