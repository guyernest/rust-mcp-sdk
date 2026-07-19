//! `cargo pmcp agent dev` (CLI-02) — run an agent loop against a completion source.
//!
//! Resolves `--source openai-compat|sampling|fixed` (a clap [`ValueEnum`]) to
//! either an [`AgentEngine`] run over a [`CompletionSource`] (openai-compat →
//! Ollama localhost, or fixed offline via the lib-safe [`run`](super::run) seam)
//! or a sampling-hosted [`AgentServer`] served over `pmcp::StdioTransport`. The
//! agent definition is LOADED from an [`AgentPackage`] (`--package`, or the
//! scaffolded `./agent.package.json`, else a built-in demo) — never a hardcoded
//! fixture.
//!
//! Endpoint handling matches the real `pmcp-agent` API: a remote plain-http
//! endpoint fails at source CONSTRUCTION (`CompletionError::Decode`) → an
//! actionable error naming `--allow-insecure-http`; `AgentEngine::run` returns a
//! [`RunOutcome`] (NOT a `Result`), so a non-`Completed` outcome is mapped to an
//! actionable message naming `--endpoint` / `--source fixed`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use colored::Colorize;

use pmcp_agent::sources::{HttpSourceOptions, OpenAiCompatSource, SecretString};
use pmcp_agent::{
    resolve_agent, AgentEngine, AgentServer, CompletionError, CompletionSourceFactory,
    EnvVarResolver, InMemoryStore, ResolvedAgentConfig, RunOutcome, SamplingSourceFactory,
};
use pmcp_package::{AgentPackage, ConfigSlot, SlotType};

use crate::commands::agent::run::{run_fixed_source, NoopInvoker};
use crate::commands::GlobalFlags;

/// The default OpenAI-compatible endpoint (local Ollama — D-03a, explicit, no
/// auto-detect).
const DEFAULT_ENDPOINT: &str = "http://localhost:11434/v1";

/// The completion source driving the agent loop (D-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SourceKind {
    /// An OpenAI-compatible HTTP endpoint (default → local Ollama).
    OpenaiCompat,
    /// A sampling-hosted `AgentServer` served over stdio (an MCP host provides
    /// the LLM via `sampling/createMessage`).
    Sampling,
    /// A scripted, offline fixed source (no network) — the lib-safe runner seam.
    Fixed,
}

/// Arguments for `cargo pmcp agent dev`.
#[derive(Debug, Args)]
pub struct DevArgs {
    /// Completion source driving the agent loop.
    #[arg(long, value_enum, default_value_t = SourceKind::OpenaiCompat)]
    pub source: SourceKind,
    /// Endpoint URL for the completion source (openai-compat only; defaults to
    /// the local Ollama endpoint).
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Path to the agent package to run (defaults to `./agent.package.json`, else
    /// a built-in demo).
    #[arg(long)]
    pub package: Option<PathBuf>,
    /// Environment variable holding the API key (openai-compat only).
    #[arg(long)]
    pub api_key_env: Option<String>,
    /// Allow a plain-HTTP (non-TLS) endpoint (openai-compat only).
    #[arg(long)]
    pub allow_insecure_http: bool,
    /// Model id passed to the completion source (openai-compat only).
    #[arg(long, default_value = "llama3.2")]
    pub model: String,
}

/// Run an agent loop, resolving `--source` to the corresponding path.
pub async fn execute(args: DevArgs, global_flags: &GlobalFlags) -> Result<()> {
    let pkg = load_package(args.package.as_deref())?;
    let config = resolve_agent(&pkg, &EnvVarResolver::new())
        .await
        .context("resolve the agent package into a runnable config")?;

    match args.source {
        SourceKind::Fixed => run_fixed(config, global_flags).await,
        SourceKind::OpenaiCompat => run_openai_compat(&args, config, global_flags).await,
        SourceKind::Sampling => run_sampling(pkg, config, global_flags).await,
    }
}

/// Load the [`AgentPackage`]: an explicit `--package`, else `./agent.package.json`
/// (connecting `agent new` → `agent dev`), else a built-in demo.
fn load_package(explicit: Option<&Path>) -> Result<AgentPackage> {
    if let Some(path) = explicit {
        return read_package(path);
    }
    let default_path = Path::new("agent.package.json");
    if default_path.exists() {
        return read_package(default_path);
    }
    Ok(builtin_demo_package())
}

fn read_package(path: &Path) -> Result<AgentPackage> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("read agent package from {}", path.display()))?;
    serde_json::from_str(&json)
        .with_context(|| format!("parse {} as an AgentPackage", path.display()))
}

/// The offline fallback package (mirrors the `agent new` scaffold's starter shape).
fn builtin_demo_package() -> AgentPackage {
    AgentPackage {
        name: "demo-agent".to_string(),
        version: semver::Version::new(1, 0, 0),
        instructions: "You are a concise, helpful assistant. Use tools when helpful.".to_string(),
        llm: ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "llama3.2".to_string(),
            },
        },
        max_tokens: 100_000,
        max_iterations: 5,
        connectors: vec![],
        tool_selection: None,
        input_schema: None,
        output_schema: None,
        importance: None,
        finalizer_role: None,
        budget_defaults: vec![],
    }
}

/// `--source fixed`: run the loop offline via the shared lib-safe runner seam.
async fn run_fixed(config: ResolvedAgentConfig, global_flags: &GlobalFlags) -> Result<()> {
    let outcome = run_fixed_source(config).await;
    report_success(&outcome, "fixed", global_flags);
    Ok(())
}

/// `--source openai-compat`: build the HTTP source at `--endpoint`, run the loop,
/// and map the outcome/construction errors to actionable messages.
async fn run_openai_compat(
    args: &DevArgs,
    config: ResolvedAgentConfig,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let endpoint = args
        .endpoint
        .clone()
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
    let key = resolve_api_key(args.api_key_env.as_deref());
    let source = build_openai_source(&endpoint, &args.model, key, args.allow_insecure_http)?;

    if global_flags.should_output() {
        println!("Running agent against {}", endpoint.bright_cyan());
    }

    let outcome = AgentEngine::new(source, NoopInvoker, InMemoryStore::new(), config)
        .run("agent-dev-run")
        .await;
    finish_engine_outcome(outcome, &endpoint, global_flags)
}

/// Resolve the API key from `--api-key-env <VAR>` (env-backed, no plaintext CLI
/// secret); default a placeholder for local unauthenticated Ollama. Never logged.
fn resolve_api_key(api_key_env: Option<&str>) -> SecretString {
    match api_key_env {
        Some(var) => SecretString::new(std::env::var(var).unwrap_or_default()),
        None => SecretString::new("ollama"),
    }
}

/// Build the openai-compat source, mapping the construction contract to
/// actionable errors (T-110-03-01): a remote plain-http endpoint returns
/// `CompletionError::Decode`; a client-build failure returns
/// `CompletionError::Transport`.
fn build_openai_source(
    endpoint: &str,
    model: &str,
    key: SecretString,
    allow_insecure_http: bool,
) -> Result<OpenAiCompatSource> {
    let options = HttpSourceOptions {
        allow_insecure_http,
        ..Default::default()
    };
    match OpenAiCompatSource::with_options(endpoint, model, key, options) {
        Ok(source) => Ok(source),
        Err(CompletionError::Decode(_)) => bail!(
            "remote non-HTTPS endpoint {endpoint} is blocked by default — use an https:// URL \
             or pass --allow-insecure-http"
        ),
        Err(err) => bail!(
            "failed to build the completion source for {endpoint} — check --endpoint \
             (or use --source fixed): {err}"
        ),
    }
}

/// Map the engine's [`RunOutcome`] to a process result (T-110-03-03). `.run()`
/// returns a `RunOutcome` — it never surfaces a `CompletionError` — so a
/// non-`Completed` outcome is the unreachable/failed signal.
fn finish_engine_outcome(
    outcome: RunOutcome,
    endpoint: &str,
    global_flags: &GlobalFlags,
) -> Result<()> {
    match outcome {
        RunOutcome::Completed { .. } => {
            report_success(&outcome, "openai-compat", global_flags);
            Ok(())
        },
        RunOutcome::Failed { error } => bail!(
            "agent run did not complete (endpoint {endpoint} may be unreachable) — \
             check --endpoint <url> or use --source fixed: {error}"
        ),
        _ => bail!(
            "agent run did not complete (endpoint {endpoint} may be unreachable) — \
             check --endpoint <url> or use --source fixed"
        ),
    }
}

/// `--source sampling`: serve the agent over stdio; the host provides the LLM.
async fn run_sampling(
    pkg: AgentPackage,
    config: ResolvedAgentConfig,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let factory: Arc<dyn CompletionSourceFactory> = Arc::new(SamplingSourceFactory::new());
    let agent = AgentServer::builder(
        pkg,
        config,
        factory,
        Arc::new(NoopInvoker),
        Arc::new(InMemoryStore::new()),
    )
    .build()
    .context("build the sampling-hosted agent server")?;

    if global_flags.should_output() {
        println!(
            "Serving agent tool '{}' over stdio.",
            agent.tool_name().bright_green()
        );
        println!("An MCP host provides the LLM via sampling/createMessage.");
    }

    agent
        .run(pmcp::StdioTransport::new())
        .await
        .context("serve the agent over stdio")
}

/// Print a success line for a terminal outcome (redaction-safe — no key, no raw
/// endpoint secret).
fn report_success(outcome: &RunOutcome, source: &str, global_flags: &GlobalFlags) {
    if !global_flags.should_output() {
        return;
    }
    let tag = match outcome {
        RunOutcome::Completed { .. } => "Completed",
        RunOutcome::LimitReached => "LimitReached",
        RunOutcome::RetryRequired { .. } => "RetryRequired",
        RunOutcome::Failed { .. } => "Failed",
        _ => "Unknown",
    };
    println!(
        "{} agent run ({source}) finished: {tag}",
        "✓".green().bold()
    );
}
