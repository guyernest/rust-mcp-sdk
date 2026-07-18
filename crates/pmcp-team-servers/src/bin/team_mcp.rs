//! `team-mcp` — the dev-grade member-dispatch MCP server binary.
//!
//! Advertises one `team_mcp__<member>` tool per roster member of the captured
//! [`TeamPackage`], forwarding a `tools/call` to each member agent under the
//! depth / self-call / ancestor-cycle guards, with guard state carried as
//! namespaced `_meta` (locked D-14, route A).
//!
//! # Member wiring (the shared path)
//!
//! For each `TeamMember`, the binary resolves its `ComponentRef` → `AgentPackage`
//! via a [`LocalDirPackageResolver`] (dev impl of the 109-01 `PackageResolver`
//! seam), resolves the member's MANDATORY llm slot via an
//! [`EnvVarResolver`](pmcp_agent::EnvVarResolver) `SlotResolver`
//! ([`resolve_member_factory`] with `None` — real slots resolve; there is no
//! override in the binary), builds the member `AgentServer`, and wraps it as a
//! [`MemberHandle`]. This is the SAME member-wiring path `TeamRuntime` (109-06)
//! uses.
//!
//! # HTTP-first + the depth-header edge map (D-14)
//!
//! Built with the `http` feature it serves streamable HTTP by default (the SDK
//! owns the DNS-rebinding/CORS/security-headers stack), or stdio when `--stdio`
//! is passed; built WITHOUT `http` it serves stdio only. At the HTTP edge an
//! [`ServerHttpMiddleware`](pmcp::server::http_middleware::ServerHttpMiddleware)
//! maps an incoming `x-pmcp-team-depth` header into the `tools/call` request's
//! namespaced `_meta` BEFORE dispatch, so the guards read depth identically
//! in-memory and over HTTP.

use std::path::PathBuf;

use clap::Parser;

use pmcp_agent::{resolve_agent, EnvVarResolver};
use pmcp_package::TeamPackage;
use pmcp_team_servers::compose::resolver::{LocalDirPackageResolver, PackageResolver};
use pmcp_team_servers::team::identity::{MemberId, MemberTaskForwarding};
use pmcp_team_servers::team::member::{resolve_member_factory, MemberHandle};
use pmcp_team_servers::team::server::build_team_mcp_server;

/// CLI arguments for the `team-mcp` server binary.
#[derive(Debug, Parser)]
#[command(name = "team-mcp", about = "Dev-grade member-dispatch MCP server")]
struct Args {
    /// Path to the captured TeamPackage JSON (primary config; member roster +
    /// `limits.max_team_depth`).
    #[arg(long)]
    package: PathBuf,

    /// The AgentPackage resolver root: `<data-dir>/<member-name>.json` files
    /// each hold a member's captured `AgentPackage`.
    #[arg(
        long,
        env = "PMCP_TEAM_MCP_DATA_DIR",
        default_value = "./team-mcp-data"
    )]
    data_dir: PathBuf,

    /// HTTP bind port (ignored for a stdio build / `--stdio`).
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Force stdio transport (default is HTTP when built with the `http` feature).
    #[arg(long, default_value_t = false)]
    stdio: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Load the TeamPackage: the member roster + the recursion bound.
    let package_bytes = std::fs::read(&args.package)?;
    let package: TeamPackage = serde_json::from_slice(&package_bytes)?;
    let max_team_depth = package.limits.max_team_depth;
    tracing::info!(
        team = %package.name,
        version = %package.version,
        members = package.members.len(),
        max_team_depth,
        data_dir = %args.data_dir.display(),
        "loaded TeamPackage roster"
    );

    // Resolvers built ONCE: ComponentRef -> AgentPackage (local dir) and the
    // member llm SlotResolver (env-var convention).
    let pkg_resolver = LocalDirPackageResolver::new(&args.data_dir);
    let slot_resolver = EnvVarResolver::new();

    // Wire each member: resolve its package, resolve its mandatory llm slot into
    // a concrete factory (None => no injected override), build + spawn it.
    let mut members = Vec::with_capacity(package.members.len());
    let mut roster = Vec::with_capacity(package.members.len());
    for member in &package.members {
        let id = MemberId::from_ref(&member.agent);
        let agent_pkg = pkg_resolver.resolve_agent(&member.agent).await?;
        let config = resolve_agent(&agent_pkg, &slot_resolver).await?;
        let factory = resolve_member_factory(&agent_pkg, &slot_resolver, None).await?;
        let handle = MemberHandle::spawn_from_package(
            id.clone(),
            agent_pkg,
            config,
            factory,
            MemberTaskForwarding::Synthesize,
        )
        .await?;
        tracing::info!(member = %id, "wired team member");
        roster.push(id);
        members.push(handle);
    }

    let server = build_team_mcp_server(members, max_team_depth, roster)?;
    serve(server, &args).await
}

/// Serve over HTTP by default, or stdio when requested / when built without HTTP.
#[cfg(feature = "http")]
async fn serve(server: pmcp::Server, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;

    use pmcp::server::http_middleware::ServerHttpMiddlewareChain;
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};

    if args.stdio {
        return serve_stdio(server).await;
    }

    // Edge map (D-14): x-pmcp-team-depth header -> request _meta BEFORE dispatch.
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(edge::TeamDepthHeaderMiddleware));
    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    let shared = Arc::new(tokio::sync::Mutex::new(server));
    let http = StreamableHttpServer::with_config(addr, shared, config);
    let (bound, handle) = http.start().await?;
    tracing::info!(%bound, "team-mcp serving streamable HTTP (x-pmcp-team-depth mapped into _meta)");
    handle.await?;
    Ok(())
}

/// Stdio-only build: the `http` feature is absent, so serve stdio unconditionally.
#[cfg(not(feature = "http"))]
async fn serve(server: pmcp::Server, _args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    serve_stdio(server).await
}

async fn serve_stdio(server: pmcp::Server) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("team-mcp serving over stdio");
    server.run(pmcp::StdioTransport::new()).await?;
    Ok(())
}

/// HTTP-edge middleware: map the `x-pmcp-team-depth` header into the request's
/// namespaced `_meta` so the guards read depth identically over HTTP and
/// in-memory (D-14).
#[cfg(feature = "http")]
mod edge {
    use async_trait::async_trait;

    use pmcp::server::http_middleware::{
        ServerHttpContext, ServerHttpMiddleware, ServerHttpRequest,
    };
    use pmcp::Result;
    use pmcp_team_servers::team::guards::META_DEPTH;
    use serde_json::Value;

    /// The HTTP header carrying the caller's team depth (D-14).
    const DEPTH_HEADER: &str = "x-pmcp-team-depth";

    /// Reads `x-pmcp-team-depth` off the request and, for a `tools/call`,
    /// injects it (verbatim, as a string) into `params._meta[META_DEPTH]`.
    ///
    /// The value is left as a STRING so the downstream strict parser
    /// (`parse_depth_strict`) rejects garbage — the edge never trusts or
    /// pre-parses it.
    pub struct TeamDepthHeaderMiddleware;

    #[async_trait]
    impl ServerHttpMiddleware for TeamDepthHeaderMiddleware {
        async fn on_request(
            &self,
            request: &mut ServerHttpRequest,
            _context: &ServerHttpContext,
        ) -> Result<()> {
            let Some(depth) = request.get_header(DEPTH_HEADER).map(str::to_string) else {
                return Ok(());
            };
            // Parse the JSON-RPC body; leave it untouched on any anomaly.
            let Ok(mut body) = serde_json::from_slice::<Value>(&request.body) else {
                return Ok(());
            };
            if body.get("method").and_then(Value::as_str) != Some("tools/call") {
                return Ok(());
            }
            let Some(obj) = body.as_object_mut() else {
                return Ok(());
            };
            // Ensure params is an object, then params._meta is an object.
            let params = obj
                .entry("params")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(params_obj) = params.as_object_mut() {
                let meta = params_obj
                    .entry("_meta")
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                if let Some(meta_obj) = meta.as_object_mut() {
                    meta_obj.insert(META_DEPTH.to_string(), Value::String(depth));
                    request.body =
                        serde_json::to_vec(&body).unwrap_or_else(|_| request.body.clone());
                }
            }
            Ok(())
        }

        fn priority(&self) -> i32 {
            // Run early — before dispatch reads the request _meta.
            10
        }
    }
}
