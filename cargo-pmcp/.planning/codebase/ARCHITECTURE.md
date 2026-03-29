# Architecture

**Analysis Date:** 2026-02-26

## Pattern Overview

**Overall:** Command-dispatcher CLI with pluggable subsystems

`cargo-pmcp` is a Cargo subcommand (invoked as `cargo pmcp`) built on a flat command-dispatcher pattern. The `main.rs` entry point parses a top-level `Commands` enum via `clap` and delegates directly to module-level `execute` functions or methods. Async execution is bridged explicitly with `tokio::runtime::Runtime::new()?.block_on(...)` per command rather than via a global async runtime.

Two orthogonal plugin registries provide extensibility:
- **`TargetRegistry`** (`src/deployment/registry.rs`) — manages `DeploymentTarget` trait objects
- **`ProviderRegistry`** (`src/secrets/registry.rs`) — manages `SecretProvider` trait objects

**Key Characteristics:**
- Synchronous `main()` with explicit per-command Tokio runtimes (not a single `#[tokio::main]`)
- Trait-object registries keyed by string ID (`Arc<dyn Trait>` stored in `HashMap<String, ...>`)
- `async_trait` used on all async traits to work around Rust's lack of async-in-trait
- `DeployConfig` is the central data bag; loaded from `.pmcp/deploy.toml` and threaded through all deployment operations
- Code generation via in-binary string templates (no external template engine); see `src/templates/`

## Layers

**CLI Layer:**
- Purpose: Argument parsing, command routing, output formatting
- Location: `src/main.rs`, `src/commands/`
- Contains: `Commands` enum, `AddCommands` enum, one module per subcommand
- Depends on: All subsystems below
- Used by: End user via `cargo pmcp <subcommand>`

**Command Modules:**
- Purpose: Orchestrate subsystem calls for a specific user action
- Location: `src/commands/{new,add,dev,deploy/,landing/,test/,schema,validate,connect,app,preview,secret/}.rs`
- Contains: `execute()` / `execute_async()` fns, `DeployCommand`, `AppCommand`, etc.
- Depends on: `deployment`, `secrets`, `templates`, `landing`, `publishing`, `utils`
- Used by: CLI layer dispatch in `execute_command()`

**Deployment Subsystem:**
- Purpose: Multi-target cloud deployment lifecycle (build → deploy → manage)
- Location: `src/deployment/`
- Contains: `DeploymentTarget` trait, `TargetRegistry`, `DeployConfig`, `BinaryBuilder`, per-target implementations
- Depends on: `utils`, external tooling (`cargo-lambda`, CDK, `wrangler`)
- Used by: `src/commands/deploy/`

**Secrets Subsystem:**
- Purpose: Multi-provider secret management with namespacing by server ID
- Location: `src/secrets/`
- Contains: `SecretProvider` trait, `ProviderRegistry`, `SecretValue` (secrecy-wrapped), per-provider implementations
- Depends on: `secrecy`, `zeroize`, optionally `aws-sdk-secretsmanager`
- Used by: `src/commands/secret/`, deployment targets

**Templates Subsystem:**
- Purpose: In-memory code generation for scaffolded projects
- Location: `src/templates/`
- Contains: Rust source strings for workspace, server, server-common, calculator, mcp_app, oauth templates
- Depends on: `std::fs`
- Used by: `src/commands/new.rs`, `src/commands/add.rs`, `src/commands/app.rs`

**Landing Subsystem:**
- Purpose: Landing page lifecycle (config, template rendering)
- Location: `src/landing/`
- Contains: `LandingConfig`, template rendering helpers
- Depends on: `src/publishing/`, Next.js template files at `templates/landing/nextjs/`
- Used by: `src/commands/landing/`

**Publishing Subsystem:**
- Purpose: MCP Apps project detection, manifest generation, landing page HTML generation
- Location: `src/publishing/`
- Contains: `detect::detect_project()`, `manifest::generate_manifest()`, `landing::generate_landing()`
- Depends on: `std::fs`, `serde_json`
- Used by: `src/commands/app.rs`

**Utils:**
- Purpose: Shared configuration utilities
- Location: `src/utils/`
- Contains: `WorkspaceConfig` (`.pmcp-config.toml` tracking server ports and templates)
- Depends on: `toml`, `serde`
- Used by: `deployment`, `commands/new`, `commands/add`

## Data Flow

**New Workspace Flow:**

1. User: `cargo pmcp new <name>`
2. `main.rs` → `execute_command(Commands::New { name, path })`
3. `commands::new::execute()` calls `create_workspace_structure()` to create dirs
4. Calls `templates::workspace::generate()`, `templates::server_common::generate()` to write Rust source files to disk
5. Outputs next-step instructions

**Deploy Flow:**

1. User: `cargo pmcp deploy [--target <id>]`
2. `commands::deploy::DeployCommand::execute()` → `execute_async()`
3. `DeployConfig::load()` reads `.pmcp/deploy.toml` from project root (walks up dirs to find `Cargo.toml`)
4. `TargetRegistry::new()` instantiates all four built-in targets; `registry.get(target_id)` resolves the active one
5. `target.build(&config)` → cross-compiles via `cargo-lambda` or uploads to pmcp.run; returns `BuildArtifact`
6. `target.deploy(&config, artifact)` → calls platform API (AWS CDK, Cloudflare `wrangler`, GCR `gcloud`, pmcp.run GraphQL)
7. `DeploymentOutputs` returned and displayed; `save_deployment_info()` writes `.pmcp/deployment.toml` for pmcp-run

**Secret Management Flow:**

1. User: `cargo pmcp secret set chess/API_KEY`
2. `commands::secret::SecretCommand::execute()` resolves target provider from `--provider` flag or config
3. `ProviderRegistry::new()` instantiates local + pmcp + aws providers
4. `SecretProvider::set()` called on the resolved provider; `SecretValue` wraps value with `secrecy`
5. Local provider stores in `.pmcp/secrets/` with 0600 permissions; pmcp.run provider calls REST API

**MCP Schema Export Flow:**

1. User: `cargo pmcp schema export --server <id>`
2. `commands::schema::SchemaCommand::Execute` → `export()` async fn
3. Sends MCP JSON-RPC `initialize` + `tools/list` + `resources/list` + `prompts/list` to endpoint via `reqwest`
4. Assembles `McpSchema` struct; serializes to JSON; writes to `schemas/<server_id>.json`

**MCP App Preview Flow:**

1. User: `cargo pmcp preview --url <server_url> --open`
2. `commands::preview::execute()` delegates to `mcp-preview` crate
3. Launches local HTTP server on `--port` (default 8765); optionally opens browser

**State Management:**
- No in-process mutable global state; each command creates its own runtime objects
- Persistent state lives in files: `.pmcp/deploy.toml` (deployment config), `.pmcp-config.toml` (workspace server registry), `.pmcp/secrets/` (local secrets), `.pmcp/deployment.toml` (last deploy outputs)
- `tokio::runtime::Runtime` created fresh per async command invocation

## Key Abstractions

**`DeploymentTarget` trait (`src/deployment/trait.rs`):**
- Purpose: Uniform interface for cloud platform operations
- Examples: `src/deployment/targets/aws_lambda/mod.rs`, `src/deployment/targets/pmcp_run/mod.rs`, `src/deployment/targets/cloudflare/mod.rs`, `src/deployment/targets/google_cloud_run/mod.rs`
- Pattern: `async_trait` — methods include `build()`, `deploy()`, `destroy()`, `logs()`, `metrics()`, `secrets()`, `test()`, `rollback()`. `destroy_async()` has a default impl. Optional `supports_async_operations()` for targets (pmcp-run) that support polling.

**`SecretProvider` trait (`src/secrets/provider.rs`):**
- Purpose: Uniform interface for secret storage backends
- Examples: `src/secrets/providers/local.rs`, `src/secrets/providers/pmcp_run.rs`, `src/secrets/providers/aws.rs`
- Pattern: `async_trait` — methods: `list()`, `get()`, `set()`, `delete()`, `health_check()`. Names follow `server-id/SECRET_NAME` format enforced by `parse_secret_name()`.

**`DeployConfig` (`src/deployment/config.rs`):**
- Purpose: Centralized deployment configuration loaded from `.pmcp/deploy.toml`
- Pattern: `serde` deserialization with nested structs (`TargetConfig`, `AwsConfig`, `ServerConfig`, `AuthConfig`, `AssetsConfig`, `CompositionConfig`). `DeployConfig::load()` also calls `auto_configure_template_assets()`.

**`TargetRegistry` / `ProviderRegistry`:**
- Purpose: Dynamic dispatch registries — look up implementations by string ID at runtime
- Pattern: `HashMap<String, Arc<dyn Trait>>` — registries pre-populate in `::new()` with all built-in implementations. Callers call `.get("aws-lambda")` etc.

**`BuildArtifact` enum (`src/deployment/trait.rs`):**
- Purpose: Discriminated union for build outputs
- Variants: `Binary { path, size, deployment_package }`, `Wasm { path, size, deployment_package }`, `Custom { path, artifact_type, deployment_package }`

**`BinaryBuilder` (`src/deployment/builder.rs`):**
- Purpose: Orchestrates `cargo-lambda` cross-compilation for AWS Lambda targets; handles asset bundling into zip deployment packages
- Pattern: Synchronous struct with `build()` method that shells out to `cargo lambda build` and `zip`

## Entry Points

**`main()` (`src/main.rs`):**
- Location: `src/main.rs:223`
- Triggers: `cargo pmcp <args>` or `cargo-pmcp <args>`
- Responsibilities: Strips the `pmcp` argv token when invoked as a cargo subcommand, sets `PMCP_VERBOSE` env var, delegates to `execute_command()`

**`execute_command()` (`src/main.rs`):**
- Location: `src/main.rs:249`
- Triggers: Called by `main()` with parsed `Commands` variant
- Responsibilities: Match arm per command; creates Tokio runtime for async commands (`Landing`, `Preview`, `Deploy`); calls into command module `execute()` fns

## Error Handling

**Strategy:** `anyhow::Result<()>` propagated through all function return types; errors bubble up to `main()` which prints the chain and exits nonzero.

**Patterns:**
- `anyhow::bail!("message")` for early-exit with a formatted error
- `.context("description")` / `.with_context(|| ...)` to annotate errors with human-readable context
- `SecretError` is a custom `thiserror`-derived enum in `src/secrets/error.rs` aliased as `SecretResult<T>`
- No panic-driven error handling in command paths; `unwrap()` appears only in test helpers

## Cross-Cutting Concerns

**Logging:** `colored` crate for colored terminal output; `indicatif` for progress bars in longer operations; `console` crate in schema commands. No structured logging framework — all output is `println!` with manual formatting.

**Validation:** `DeployConfig::load()` validates TOML parse; `commands::schema::validate()` checks required fields in `McpSchema`. No centralized validation framework.

**Authentication:** Three auth mechanisms managed by `DeployCommand`:
1. pmcp.run — OAuth2 PKCE flow via `src/deployment/targets/pmcp_run/auth.rs`; tokens stored locally
2. AWS Lambda — standard AWS credential chain (env vars / profile)
3. Cloudflare — `wrangler` CLI credential management

**Verbose mode:** `--verbose` global flag sets `PMCP_VERBOSE=1` environment variable; commands check this env var for extra output.

---

*Architecture analysis: 2026-02-26*
