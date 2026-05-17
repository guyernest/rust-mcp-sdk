# Schema-Server Toolkit — Architecture & Thin Slice

This blueprint describes how to lift the `pmcp-run` built-in server
machinery (currently at `~/Development/mcp/sdk/pmcp-run/built-in/`) into
the PMCP SDK as a public toolkit. The structural diff in spike 003 and
the end-to-end thin slice in spike 004 together pin down the right shape
of that lift.

## Requirements

These are non-negotiable design contracts. Every implementation decision
must honor them.

- **The shared abstraction already exists.** `mcp-server-common`
  (~2.2k LoC at `pmcp-run/built-in/shared/mcp-server-common/`) plus
  `pmcp-code-mode` (SDK crate) already provide `AuthProvider`,
  `SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`,
  HMAC token machinery, and the `#[derive(CodeMode)]` macro. All three
  backend cores (sql/graphql/openapi) already depend on both. The
  toolkit lift PROMOTES `mcp-server-common`; it does NOT rewrite.
- **No single `SchemaServer<S, C>` trait that all backends implement.**
  Per-backend executors, parameter binding, and policy surface diverge
  semantically. SQL has multi-impl `DatabaseConnector`; GraphQL and
  OpenAPI have concrete `reqwest`-wrapped clients. `code_mode.rs` LoC
  spread is 545 / 767 / 1560 — OpenAPI's 3× weight is real (AVP/Cedar,
  two-tier field blocklist, scope binding), not implementation slop.
- **Per-backend connector traits MUST expose `schema_text()`.** The
  code-mode bootstrap prompt body needs a schema description to seed
  the LLM with the long-tail surface in one fetch.
- **The user-facing surface MUST be ~12 lines of Rust** (or zero, via
  a `pmcp-{kind}-server` binary that takes config + schema paths). The
  toolkit synthesizes `ToolInfo` from `[[tools]]` config; the user
  writes NO per-tool Rust handlers.
- **`Config::from_toml(&str) -> Result<Self>`** is the single
  entrypoint to load the entire server surface. Matches the shape used
  by all three pmcp-run cores (verified structurally in spike 003).
- **`into_pmcp_server()` and `into_pmcp_builder()` are both needed.**
  The former for the pure-config case; the latter for users adding
  custom Rust handlers alongside the config-driven ones (pattern from
  `mcp-sql-server-core/src/pmcp_server.rs:254`).

## How to Build It

### Workspace layout

```
crates/
  pmcp-server-toolkit/          # the proto-SDK extract (~2.2k → ~2.5k LoC)
    src/
      auth.rs                   # from mcp-server-common
      secrets.rs                # from mcp-server-common
      resources.rs              # StaticResourceHandler, ResourceConfig
      prompts.rs                # StaticPromptHandler, PromptConfig
      config.rs                 # Shared SchemaServerConfig pieces
      lib.rs
```

The per-backend crates stay separate (spike 003 finding — they
genuinely diverge). Phase 1 ships SQL only:

```
crates/
  pmcp-toolkit-postgres/        # tokio-postgres, pure-Rust, Lambda-suitable
  pmcp-toolkit-athena/          # aws-sdk-athena, pure-Rust
  pmcp-toolkit-mysql/           # sqlx, pure-Rust
  # SQLite ships as a feature flag on the toolkit, NOT a published crate
```

### Toolkit's top-level types (modeled on spike 004's `mod toolkit`)

```rust
// crates/pmcp-server-toolkit/src/config.rs
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaServerConfig {
    pub server: ServerSection,
    #[serde(default)] pub tools: Vec<ToolDecl>,
    #[serde(default)] pub code_mode: Option<CodeModeSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolDecl {
    pub name: String,
    pub description: String,
    pub sql: String,                  // (or `query` / `path`+`method`)
    #[serde(default)] pub parameters: Vec<ParamDecl>,
}

impl SchemaServerConfig {
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).context("parse config")
    }
}
```

### Toolkit's per-tool handler synthesis

```rust
// One handler per [[tools]] entry. Synthesizes ToolInfo (with JSON
// Schema for inputs) from the declared parameters. Calls the connector
// at execute time.
pub struct SqlToolHandler<C: SqlConnector> {
    decl: ToolDecl,
    connector: Arc<C>,
}

impl<C: SqlConnector> SqlToolHandler<C> {
    fn input_schema(&self) -> Value {
        let mut props = serde_json::Map::new();
        let mut required = Vec::new();
        for p in &self.decl.parameters {
            props.insert(p.name.clone(), json!({
                "type": p.r#type.json_schema_type(),
                "description": p.description,
            }));
            if p.required { required.push(Value::String(p.name.clone())); }
        }
        json!({
            "type": "object",
            "properties": props,
            "required": required,
            "additionalProperties": false,
        })
    }
}

#[async_trait]
impl<C: SqlConnector> pmcp::server::ToolHandler for SqlToolHandler<C> {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra)
        -> pmcp::Result<Value>
    {
        // 1. Bind named params from args in declared order
        // 2. Reject missing required params with pmcp::Error::validation
        // 3. Call connector.execute(self.decl.sql, &bound)
        // 4. Serialize rows as CallToolResult::new(vec![Content::Text { ... }])
    }
    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(self.decl.name.clone(),
                          Some(self.decl.description.clone()),
                          self.input_schema()))
    }
}
```

### The three user-facing shapes

The toolkit ships ALL THREE. The pure-config binary is the headline.

**Shape A — Pure config, zero Rust** (the headline):

```bash
pmcp-sql-server --config config.toml --schema schema.sql
```

Binary the toolkit ships. Installable via `cargo install pmcp-sql-server`
or attached to GitHub Releases as static Lambda binaries.

**Shape B — Scaffolded crate** (`cargo pmcp new --kind sql-server`):

Generates a starter `Cargo.toml` + `main.rs` (12 lines, from spike 004)
+ `config.toml` stub + `schema` placeholder. For users who want to add
a custom Rust handler alongside the config-driven tools.

**Shape C — Library use** (what spike 004 validated):

```rust
let config = SchemaServerConfig::from_toml(CONFIG_TOML)?;
let connector = Arc::new(PostgresConnector::connect(&config.database).await?);
let schema_server = SchemaServer::new(config, connector);
let pmcp_server = schema_server.into_pmcp_server().await?;
pmcp_server.run_lambda().await
```

12 meaningful lines. Asserted in-binary in spike 004 step G.

### Upstream `pmcp` SDK additions required

These are the DX gaps surfaced by spike 004 that the toolkit lift
should close upstream:

1. **`pmcp::ServerBuilder::tool_arc(name, Arc<dyn ToolHandler>)`** —
   public-builder version of the arc-registration method that
   `ServerCoreBuilder` already has (`src/server/builder.rs:203`).
   Without it, every toolkit author writes a 20-line delegating
   wrapper shim.
2. **`pmcp::ServerBuilder::prompt_arc(name, Arc<dyn PromptHandler>)`**
   — same pattern for prompts.
3. **Public in-process driver** OR documented "use `ToolHandler::handle`
   directly for testing" pattern. `Server::handle_request` is private
   today; external toolkit tests can only drive handlers, not the full
   JSON-RPC dispatch.

## What to Avoid

- **Don't design a `SchemaServer<S, C>` trait.** It looks elegant; it
  doesn't work. The three backends' executors / parameter binders /
  policy surfaces diverge semantically (proven by structural diff in
  spike 003 — `code_mode.rs` LoC spread 545 / 767 / 1560). Any
  trait-fitting-all-three either omits real concerns (useless for
  OpenAPI) or includes them (overweight for SQL).
- **Don't try to lift OpenAPI in Phase 1.** Its `code_mode.rs` is 1560
  LoC — that's AVP/Cedar policy, JS-sandbox executor, two-tier field
  blocklist, multi-tenant OAuth passthrough. Defer until Phase 3 and
  run a separate spike that resolves the auth/policy/JS-sandbox
  questions first.
- **Don't lift the per-backend crates ALL together.** Phase 1 is
  toolkit core + SQL. Phase 2 is GraphQL. Phase 3 is OpenAPI. Each
  phase derisks the next.
- **Don't bake AVP/Cedar specifically into the toolkit.** Different
  orgs use OPA, Cedar, bespoke RBAC. Ship a `PolicyEvaluator` trait
  with a `NoopPolicyEvaluator` default; AVP-specific lives in an
  optional `pmcp-toolkit-avp` crate.
- **Don't use `Server::handle_request` for in-process testing** — it's
  private. Drive `ToolHandler::handle(args, extra)` directly via the
  trait (spike 002 and spike 004 both used this pattern).
- **Don't use struct-literal syntax for `#[non_exhaustive]` types**:
  - `CallToolResult::new(content)` — sets is_error=false
  - `CallToolResult::error(content)` — sets is_error=true
  - `GetPromptResult::new(messages, description)`
  - `PromptMessage::user(content)` / `::assistant(...)` / `::new(role, content)`
  - `PromptInfo::new(name).with_description(...)`
- **`RequestHandlerExtra` path is `pmcp::RequestHandlerExtra`** (top-level re-export at `src/lib.rs:57`),
  NOT `pmcp::shared::cancellation::*`.

## Constraints

- **AWS Lambda is the primary deployment target.** Pure-Rust binaries
  only. No Docker, no testcontainers, no system libs beyond what
  Lambda's Amazon Linux 2 ships. All connector crates use pure-Rust
  drivers (`tokio-postgres`, `sqlx`, `aws-sdk-athena`, `rusqlite`
  bundled).
- **`mcp-server-common` already depends on `pmcp`** — promoting it
  into a workspace crate of rust-mcp-sdk needs careful version handling
  during the lift. Plan: bump together, never partially.
- **`pmcp-code-mode` is a separate SDK crate** (already published).
  The toolkit re-exports its `CODE_MODE_PROMPT_NAME` ("start_code_mode")
  and `validate_code` / `execute_code` tool names — it does NOT redefine
  them.
- **`#[derive(CodeMode)]` is the macro that names the long-tail tools.**
  Per-backend cores at pmcp-run use it; the toolkit lift should keep
  using it (SQL + GraphQL via macro; OpenAPI uses manual registration
  because AVP interposes between validation and token issuance).

## Origin

Synthesized from spikes:
- **003 schema-server-surface-diff** — structural diff across the three
  pmcp-run backend cores. Verdict: PARTIAL → reframed VALIDATED. Found
  the proto-SDK already extracted at `mcp-server-common`. Source files:
  `sources/003-schema-server-surface-diff/`.
- **004 schema-server-thin-slice-sql** — end-to-end inline toolkit
  slice + SQLite reference. 12-line user surface validated.
  Two upstream DX gaps surfaced (`tool_arc`/`prompt_arc`, in-process
  driver). Source files: `sources/004-schema-server-thin-slice-sql/`.

Cross-spike: see also `schema-server-sql-dialects.md` (spike 005, the
multi-dialect connector shape on top of this architecture) and
`schema-server-authoring-skills.md` (spike 006, the Type 2 SEP-2640
authoring-skills MCP server that ships alongside the toolkit).
