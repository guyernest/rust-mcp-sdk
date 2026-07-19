//! `approval-mcp` — the dev-grade human-approval MCP server binary.
//!
//! Advertises the two UNNAMESPACED legacy static tools (`resolve_approval`,
//! `get_approval`) plus one `team_approval__ask_<role>` per human role declared
//! in the [`TeamPackage`], over an [`InMemoryTaskStore`](pmcp::server::task_store::InMemoryTaskStore)
//! (observable pending→resolved lifecycle) + an
//! [`ApprovalRepository`](pmcp_team_servers::approval::repository::ApprovalRepository)
//! (approval-domain state). Approvals are SERVICE-OWNED (D-10): any connected
//! client may resolve.
//!
//! HTTP-first (D-02/D-03): built with the `http` feature it serves streamable
//! HTTP by default (the SDK owns the DNS-rebinding/CORS/security-headers stack)
//! or stdio when `--stdio` is passed; built WITHOUT `http` it serves stdio only.
//!
//! Configuration:
//! - `--package` (primary): the captured [`TeamPackage`] providing the human
//!   roster (the ask family is one tool per `human_roles` entry).
//! - `--data-dir`: reserved for CLI parity (the dev stores are in-memory).
//! - `--port`: HTTP bind port (HTTP builds only).
//! - `--stdio`: force stdio even on an `http` build.
//! - `--webhook-url`: opt-in outbound notify-only webhook. cfg-SAFE — if the
//!   binary was built WITHOUT the `webhook` feature, the flag is accepted but the
//!   server falls back to the console channel with a warning (never a compile
//!   error). The optional shared secret is read from the
//!   `PMCP_APPROVAL_WEBHOOK_SECRET` env var (never a CLI arg, so it never appears
//!   in the process table) and is placed ONLY in the outgoing header (V7).

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pmcp_package::TeamPackage;
use pmcp_team_servers::approval::channels::{ApprovalChannel, ConsoleChannel};
use pmcp_team_servers::approval::repository::ApprovalRepository;
use pmcp_team_servers::approval::server::build_approval_mcp_server;

/// CLI arguments for the `approval-mcp` server binary.
#[derive(Debug, Parser)]
#[command(name = "approval-mcp", about = "Dev-grade human-approval MCP server")]
struct Args {
    /// Path to the captured TeamPackage JSON (primary config; human roster).
    #[arg(long)]
    package: PathBuf,

    /// Reserved for CLI parity (the dev stores are in-memory).
    #[arg(
        long,
        env = "PMCP_APPROVAL_MCP_DATA_DIR",
        default_value = "./approval-mcp-data"
    )]
    data_dir: PathBuf,

    /// HTTP bind port (ignored for a stdio build / `--stdio`).
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Force stdio transport (default is HTTP when built with the `http` feature).
    #[arg(long, default_value_t = false)]
    stdio: bool,

    /// Opt-in outbound notify-only webhook URL (cfg-safe without the `webhook`
    /// feature — falls back to the console channel with a warning).
    #[arg(long)]
    webhook_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pmcp_team_servers::dev_bin::init_tracing();

    let args = Args::parse();

    // Load the TeamPackage for the human roster (the ask family derives from it).
    let package_bytes = std::fs::read(&args.package)?;
    let package: TeamPackage = serde_json::from_slice(&package_bytes)?;
    tracing::info!(
        team = %package.name,
        version = %package.version,
        human_roles = package.human_roles.len(),
        data_dir = %args.data_dir.display(),
        "loaded TeamPackage roster context (approval-mcp stores are in-memory)"
    );

    let channel = build_channel(&args);
    // Production uses the random-UUID id seam; conformance builds the repo with
    // the deterministic seam directly.
    let repo = Arc::new(ApprovalRepository::new());
    let server = build_approval_mcp_server(&package.human_roles, channel, repo)?;

    serve(server, &args).await
}

/// Select the notification channel (webhook feature present).
#[cfg(feature = "webhook")]
fn build_channel(args: &Args) -> Arc<dyn ApprovalChannel> {
    use pmcp_team_servers::approval::channels::WebhookChannel;
    if let Some(url) = &args.webhook_url {
        // Secret from env only — never argv (V7: keep it out of the process table).
        let secret = std::env::var("PMCP_APPROVAL_WEBHOOK_SECRET").ok();
        match WebhookChannel::new(url.clone(), secret) {
            Ok(channel) => {
                tracing::info!("approval-mcp using the notify-only webhook channel");
                return Arc::new(channel);
            },
            Err(e) => {
                tracing::warn!(error = %e, "webhook channel build failed; using the console channel");
            },
        }
    }
    Arc::new(ConsoleChannel::new())
}

/// Select the notification channel (webhook feature ABSENT — cfg-safe fallback).
#[cfg(not(feature = "webhook"))]
fn build_channel(args: &Args) -> Arc<dyn ApprovalChannel> {
    if args.webhook_url.is_some() {
        tracing::warn!(
            "--webhook-url supplied but this binary was built without the `webhook` feature; using the console channel"
        );
    }
    Arc::new(ConsoleChannel::new())
}

/// Serve over HTTP by default, or stdio when requested / when built without HTTP.
#[cfg(feature = "http")]
async fn serve(server: pmcp::Server, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
    use pmcp_team_servers::dev_bin::{serve_stdio, serve_streamable_http};

    if args.stdio {
        return serve_stdio(server, "approval-mcp").await;
    }
    serve_streamable_http(
        server,
        "approval-mcp",
        args.port,
        StreamableHttpServerConfig::default(),
    )
    .await
}

/// Stdio-only build: the `http` feature is absent, so serve stdio unconditionally.
#[cfg(not(feature = "http"))]
async fn serve(server: pmcp::Server, _args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    pmcp_team_servers::dev_bin::serve_stdio(server, "approval-mcp").await
}
