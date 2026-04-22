//! `cargo pmcp auth` — manage OAuth credentials for MCP servers.
//!
//! Five subcommands that give developers one-time browser login per server,
//! then transparent bearer-token reuse across every `cargo pmcp test/*`,
//! `connect`, `preview`, `schema`, `dev`, `loadtest/run`, and `pentest`
//! invocation.
//!
//! Per-server token cache: `~/.pmcp/oauth-cache.json` (schema_version: 1).
//!
//! # Concurrency
//!
//! Parallel `auth login` invocations are not safe; prefer sequential logins.
//! The cache file uses last-writer-wins atomic-rename semantics
//! (`tempfile::NamedTempFile::persist`), so concurrent logins to either the
//! same URL or different URLs may result in lost entries. Matches
//! `gh auth login` / `aws sso login` behavior.

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
