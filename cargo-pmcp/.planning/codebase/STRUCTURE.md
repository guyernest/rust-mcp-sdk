# Codebase Structure

**Analysis Date:** 2026-02-26

## Directory Layout

```
cargo-pmcp/
├── src/
│   ├── main.rs                         # CLI entry point, Commands enum, execute_command()
│   ├── commands/                       # One module per CLI subcommand
│   │   ├── mod.rs                      # pub mod declarations
│   │   ├── new.rs                      # `cargo pmcp new`
│   │   ├── add.rs                      # `cargo pmcp add server/tool/workflow`
│   │   ├── dev.rs                      # `cargo pmcp dev`
│   │   ├── connect.rs                  # `cargo pmcp connect`
│   │   ├── schema.rs                   # `cargo pmcp schema export/validate/diff`
│   │   ├── validate.rs                 # `cargo pmcp validate`
│   │   ├── preview.rs                  # `cargo pmcp preview`
│   │   ├── app.rs                      # `cargo pmcp app new/manifest/landing/build`
│   │   ├── deploy/
│   │   │   ├── mod.rs                  # DeployCommand, DeployAction, OAuthAction enums
│   │   │   ├── deploy.rs               # Deploy execution logic
│   │   │   └── init.rs                 # InitCommand (AWS Lambda init path)
│   │   ├── landing/
│   │   │   ├── mod.rs                  # LandingCommand enum
│   │   │   ├── init.rs                 # `cargo pmcp landing init`
│   │   │   ├── dev.rs                  # `cargo pmcp landing dev`
│   │   │   └── deploy.rs               # `cargo pmcp landing deploy`
│   │   ├── secret/
│   │   │   └── mod.rs                  # SecretCommand enum and execution
│   │   └── test/
│   │       ├── mod.rs                  # TestCommand enum
│   │       ├── run.rs                  # Local test runner
│   │       ├── check.rs                # Scenario check
│   │       ├── generate.rs             # Scenario generation
│   │       ├── list.rs                 # List scenarios
│   │       ├── upload.rs               # Upload to pmcp.run
│   │       └── download.rs             # Download from pmcp.run
│   ├── deployment/                     # Multi-target deployment subsystem
│   │   ├── mod.rs                      # Re-exports
│   │   ├── trait.rs                    # DeploymentTarget trait, BuildArtifact, DeploymentOutputs
│   │   ├── config.rs                   # DeployConfig + all nested config structs
│   │   ├── registry.rs                 # TargetRegistry (HashMap<String, Arc<dyn DeploymentTarget>>)
│   │   ├── builder.rs                  # BinaryBuilder (cargo-lambda cross-compilation + zip)
│   │   ├── metadata.rs                 # Deployment metadata helpers
│   │   ├── naming.rs                   # Binary naming conventions, workspace binary detection
│   │   ├── operations.rs               # OperationStatus enum, AsyncOperation, DestroyResult
│   │   └── outputs.rs                  # load_cdk_outputs() for reading CDK output JSON
│   │   └── targets/
│   │       ├── mod.rs                  # Re-exports for all four targets
│   │       ├── aws_lambda/
│   │       │   ├── mod.rs              # AwsLambdaTarget impl
│   │       │   ├── deploy.rs           # CDK-based deployment logic
│   │       │   └── init.rs             # AWS init (CDK project scaffolding)
│   │       ├── cloudflare/
│   │       │   ├── mod.rs              # CloudflareTarget impl
│   │       │   ├── deploy.rs           # wrangler-based deployment
│   │       │   └── init.rs             # Cloudflare init
│   │       ├── google_cloud_run/
│   │       │   ├── mod.rs              # GoogleCloudRunTarget impl
│   │       │   ├── deploy.rs           # gcloud-based deployment
│   │       │   ├── auth.rs             # GCR authentication
│   │       │   └── dockerfile.rs       # Dockerfile generation
│   │       └── pmcp_run/
│   │           ├── mod.rs              # PmcpRunTarget impl
│   │           ├── deploy.rs           # pmcp.run HTTP API deployment
│   │           ├── auth.rs             # OAuth2 PKCE flow, token storage
│   │           └── graphql.rs          # pmcp.run GraphQL API calls
│   ├── secrets/                        # Multi-provider secret management
│   │   ├── mod.rs                      # Re-exports
│   │   ├── provider.rs                 # SecretProvider trait, parse_secret_name()
│   │   ├── registry.rs                 # ProviderRegistry (HashMap<String, Arc<dyn SecretProvider>>)
│   │   ├── config.rs                   # SecretsConfig, SecretTarget enum
│   │   ├── error.rs                    # SecretError (thiserror), SecretResult<T>
│   │   ├── value.rs                    # SecretValue (secrecy-wrapped), SecretEntry, SecretMetadata
│   │   └── providers/
│   │       ├── mod.rs                  # Re-exports
│   │       ├── local.rs                # LocalSecretProvider (file at .pmcp/secrets/)
│   │       ├── pmcp_run.rs             # PmcpRunSecretProvider (REST API)
│   │       └── aws.rs                  # AwsSecretProvider (AWS Secrets Manager, feature-gated)
│   ├── templates/                      # In-binary code generation strings
│   │   ├── mod.rs                      # pub mod declarations
│   │   ├── workspace.rs                # Cargo workspace Cargo.toml template
│   │   ├── server.rs                   # MCP server Cargo.toml + main.rs template
│   │   ├── server_common.rs            # server-common shared crate template
│   │   ├── calculator.rs               # Calculator server template (minimal)
│   │   ├── complete_calculator.rs      # Calculator server template (complete)
│   │   ├── sqlite_explorer.rs          # SQLite explorer server template
│   │   ├── mcp_app.rs                  # MCP Apps project template
│   │   └── oauth/
│   │       ├── mod.rs                  # OAuth template re-exports
│   │       ├── proxy.rs                # OAuth proxy Lambda template
│   │       └── authorizer.rs           # OAuth authorizer Lambda template
│   ├── landing/                        # Landing page lifecycle
│   │   ├── mod.rs                      # pub mod declarations
│   │   ├── config.rs                   # LandingConfig (pmcp-landing.toml)
│   │   └── template.rs                 # Template rendering helpers
│   ├── publishing/                     # MCP Apps artifact generation
│   │   ├── mod.rs                      # pub mod declarations
│   │   ├── detect.rs                   # detect_project() — scans cwd for MCP Apps project
│   │   ├── manifest.rs                 # generate_manifest(), write_manifest()
│   │   └── landing.rs                  # generate_landing(), load_mock_data(), write_landing()
│   └── utils/
│       ├── mod.rs                      # pub mod declarations
│       └── config.rs                   # WorkspaceConfig (.pmcp-config.toml)
├── templates/                          # File-system templates copied verbatim
│   └── landing/
│       └── nextjs/                     # Next.js landing page project skeleton
│           ├── app/                    # Next.js App Router pages and components
│           ├── lib/                    # Shared Next.js utilities
│           ├── public/                 # Static assets
│           ├── package.json            # Next.js dependencies
│           ├── tailwind.config.js
│           ├── next.config.js
│           └── pmcp-landing.toml       # Landing page config skeleton
├── examples/                           # Runnable examples for secrets subsystem
│   ├── secrets_local_workflow.rs
│   └── secrets_provider_demo.rs
├── docs/                               # Design documentation
│   ├── BINARY_NAMING_CONVENTIONS.md
│   ├── DEPLOYMENT_METADATA_DESIGN.md
│   ├── TEMPLATE_REGISTRY_DESIGN.md
│   ├── oauth-design.md
│   ├── oauth-sdk-design.md
│   ├── pmcp-run-oauth-design.md
│   └── PMCP_RUN_INTEGRATION_UPDATE.md
├── Cargo.toml                          # cargo-pmcp package manifest
└── .pmcp/                              # Generated at runtime (not in repo)
    ├── deploy.toml                     # Deployment config (generated by `deploy init`)
    ├── deployment.toml                 # Last deploy outputs (server_id, endpoint)
    └── secrets/                        # Local secret storage (0600 permissions)
```

## Directory Purposes

**`src/commands/`:**
- Purpose: One module per CLI subcommand; thin orchestration layer
- Contains: Command struct definitions (clap), `execute()` / `execute_async()` functions
- Key files: `src/commands/deploy/mod.rs` (largest command, ~1037 lines), `src/commands/app.rs`, `src/commands/schema.rs`

**`src/deployment/`:**
- Purpose: Full lifecycle management for all cloud deployment targets
- Contains: Trait definitions, configuration, build tooling, per-target implementations
- Key files: `src/deployment/trait.rs` (core trait), `src/deployment/config.rs` (data structures), `src/deployment/registry.rs`, `src/deployment/builder.rs`

**`src/deployment/targets/`:**
- Purpose: One subdirectory per supported cloud target; each implements `DeploymentTarget`
- Contains: `aws_lambda/`, `cloudflare/`, `google_cloud_run/`, `pmcp_run/`
- Key files: `src/deployment/targets/pmcp_run/auth.rs` (OAuth PKCE), `src/deployment/targets/pmcp_run/graphql.rs`

**`src/secrets/`:**
- Purpose: Secure secret storage and retrieval with multiple backends
- Contains: Trait, registry, three provider implementations, error types, zeroizing value wrapper
- Key files: `src/secrets/provider.rs` (trait + `parse_secret_name()`), `src/secrets/value.rs`, `src/secrets/registry.rs`

**`src/templates/`:**
- Purpose: Inline Rust source code strings for scaffolding new projects
- Contains: One module per template type returning `String` or writing files
- Key files: `src/templates/workspace.rs`, `src/templates/server_common.rs`, `src/templates/mcp_app.rs`

**`src/publishing/`:**
- Purpose: MCP Apps publishing pipeline — detect project, generate manifest JSON and landing HTML
- Contains: `detect.rs` scans `widgets/` dir; `manifest.rs` builds ChatGPT-compatible JSON; `landing.rs` builds standalone HTML
- Key files: `src/publishing/detect.rs`, `src/publishing/landing.rs`

**`templates/landing/nextjs/`:**
- Purpose: Next.js project skeleton copied to user project on `cargo pmcp landing init`
- Contains: Full Next.js App Router project with Tailwind, pmcp-landing.toml config
- Generated: No (source), Committed: Yes

## Key File Locations

**Entry Points:**
- `src/main.rs`: CLI entry point, `Commands` enum, `execute_command()` dispatch
- `src/commands/mod.rs`: Declares all command submodules

**Configuration:**
- `src/deployment/config.rs`: `DeployConfig` and all nested structs; `load()` from `.pmcp/deploy.toml`
- `src/utils/config.rs`: `WorkspaceConfig`; `load()` from `.pmcp-config.toml`
- `Cargo.toml`: Package manifest; `aws-secrets` feature flag for optional AWS provider

**Core Traits:**
- `src/deployment/trait.rs`: `DeploymentTarget` trait — implement this to add a new cloud target
- `src/secrets/provider.rs`: `SecretProvider` trait — implement this to add a new secrets backend

**Registries:**
- `src/deployment/registry.rs`: `TargetRegistry::new()` — add new targets here
- `src/secrets/registry.rs`: `ProviderRegistry::new()` — add new providers here

**Build Tooling:**
- `src/deployment/builder.rs`: `BinaryBuilder` — `cargo-lambda` orchestration and asset zip packaging

**MCP Apps Pipeline:**
- `src/publishing/detect.rs`: `detect_project()` — discovers widgets from `widgets/*.html`
- `src/publishing/manifest.rs`: `generate_manifest()` — produces ChatGPT manifest JSON
- `src/publishing/landing.rs`: `generate_landing()` — produces standalone demo HTML

## Naming Conventions

**Files:**
- Snake_case module names matching their content: `server_common.rs`, `sqlite_explorer.rs`
- Subcommand modules named after the command: `deploy/`, `landing/`, `secret/`, `test/`
- Trait files named after the trait concept: `trait.rs` (deployment), `provider.rs` (secrets)

**Directories:**
- Plural for collections of related files: `commands/`, `targets/`, `providers/`, `templates/`
- Singular for a specific domain: `deployment/`, `landing/`, `publishing/`, `secrets/`

**Types:**
- Structs: PascalCase (`DeployConfig`, `BinaryBuilder`, `SecretValue`)
- Traits: PascalCase ending in role noun (`DeploymentTarget`, `SecretProvider`)
- Enums: PascalCase (`Commands`, `BuildArtifact`, `OperationStatus`, `SecretTarget`)
- Functions: snake_case (`execute_command`, `parse_secret_name`, `detect_project`)

**Config Files:**
- `.pmcp/deploy.toml` — deployment config for a specific server
- `.pmcp-config.toml` — workspace-level server registry (ports, templates)
- `.pmcp/secrets/` — local secret storage directory
- `.pmcp/deployment.toml` — last deploy outputs (auto-generated)

## Where to Add New Code

**New CLI subcommand:**
1. Add variant to `Commands` enum in `src/main.rs`
2. Add match arm in `execute_command()` in `src/main.rs`
3. Create `src/commands/<name>.rs` with `execute()` fn
4. Add `pub mod <name>;` to `src/commands/mod.rs`

**New deployment target:**
1. Create `src/deployment/targets/<platform>/` with `mod.rs`, `deploy.rs`, `init.rs`
2. Implement `DeploymentTarget` trait for a new struct
3. Add `pub mod <platform>;` and `pub use <platform>::<Target>;` to `src/deployment/targets/mod.rs`
4. Call `registry.register(Arc::new(<Target>::new()))` in `TargetRegistry::new()` in `src/deployment/registry.rs`

**New secrets provider:**
1. Create `src/secrets/providers/<name>.rs` implementing `SecretProvider`
2. Add `pub mod <name>;` and re-export in `src/secrets/providers/mod.rs`
3. Register in `ProviderRegistry::new()` in `src/secrets/registry.rs`
4. Add variant to `SecretTarget` in `src/secrets/config.rs` and match arm in `get_for_target()`

**New scaffold template:**
1. Create `src/templates/<name>.rs` with a `generate(dir: &Path, name: &str) -> Result<()>` function
2. Add `pub mod <name>;` to `src/templates/mod.rs`
3. Call from `src/commands/new.rs` or `src/commands/add.rs`

**Shared utilities:**
1. Add to `src/utils/config.rs` (workspace-level) or create new file in `src/utils/`
2. Add `pub mod <name>;` to `src/utils/mod.rs`

## Special Directories

**`.pmcp/` (runtime-generated):**
- Purpose: Per-project deployment state
- Generated: Yes (at runtime by `cargo pmcp deploy init` and `cargo pmcp deploy`)
- Committed: Typically no for `deploy.toml`; yes or no depending on team preference

**`templates/landing/nextjs/`:**
- Purpose: Next.js skeleton copied to user projects on `cargo pmcp landing init`
- Generated: No (curated source)
- Committed: Yes

**`docs/`:**
- Purpose: Design documents for internal reference; not user-facing docs
- Generated: No
- Committed: Yes

**`examples/`:**
- Purpose: Runnable Rust examples demonstrating secrets subsystem usage; referenced by `Cargo.toml` as `[[example]]` entries
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-02-26*
