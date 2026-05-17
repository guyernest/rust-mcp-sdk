# Spike 003 — Structural Diff: pmcp-run Built-in Core Crates

Source roots (all under `~/Development/mcp/sdk/pmcp-run/built-in/`):

- `sql-api/crates/mcp-sql-server-core/` — ~3.8k LoC
- `graphql-api/crates/mcp-graphql-server-core/` — ~4.0k LoC
- `openapi-api/crates/mcp-openapi-server-core/` — ~3.8k LoC
- `shared/mcp-server-common/` — ~2.2k LoC (proto-SDK, **already extracted**)

The spike binary (`src/main.rs`) re-derives every claim below by scanning the
source tree, with in-binary `assert!`s. If pmcp-run drifts, the spike fails
loudly on rerun.

## Headline Finding

**The shared abstraction already exists.** `mcp-server-common` is a 2.2k-LoC
crate (`AuthProvider`, `SecretsProvider`, `StaticResourceHandler`,
`StaticPromptHandler`, shared `ResourceConfig`/`PromptConfig`) that all three
backend cores already depend on. `pmcp-code-mode` (an SDK crate) already owns
the HMAC token machinery, `TokenSecret`, `JsCodeExecutor`, and the
`#[derive(CodeMode)]` macro that generates `validate_code` / `execute_code`.

The original question reframes: **"should the already-extracted shared
abstraction live in the PMCP SDK workspace, or stay locked behind pmcp-run?"**

## A. Config Surface — Overlap Matrix

| Section            | SQL              | GraphQL          | OpenAPI          | Shared shape? |
|--------------------|------------------|------------------|------------------|---------------|
| `Config::from_toml`| ✓ (`config.rs:50`)| ✓ (`config.rs:381`)| ✓ (`config.rs:90`)| **YES** — same signature, returns `Result<Self>` |
| `[server]`         | `ServerConfig` (`config.rs:63`) | `ServerConfig` | `ServerConfig` (`config.rs:121`) | **YES** — id/name/version/description/icon_url/website_url |
| `[[tools]]`        | `Vec<SqlToolConfig>` | `Vec<GraphQLToolConfig>` | `Vec<ToolConfig>` | **PARTIAL** — same array shape, divergent inner fields |
| `[[resources]]`    | re-export from `mcp-server-common` | re-export | re-export | **YES** — shared via toolkit |
| `[[prompts]]`      | re-export from `mcp-server-common` | re-export | re-export | **YES** — shared via toolkit |
| `[code_mode]`      | `CodeModeConfig` (`config.rs:472`) | `CodeModeServerConfig` | `CodeModeServerConfig` (`config.rs:944`) | **PARTIAL** — same shell (`enabled`, `token_secret`), divergent policy fields |
| `[secrets]`        | `SecretsConfig` (shared) | shared | shared | **YES** — shared |
| `[observability]`  | shared | shared | shared | **YES** — shared |

**Divergent tool-config inner shape:**

| Field on `[[tools]]` | SQL                       | GraphQL                | OpenAPI                |
|----------------------|---------------------------|------------------------|------------------------|
| What to execute      | `sql = "..."` (string)    | `query = "..."` (string) | `path = "..."` + `method = "..."` |
| Param binding        | `:name` placeholders      | `$name` GraphQL variables | `{name}` path templates + query/body split by verb |
| Input shape          | `[[tools.parameters]]`    | `inputs: InputConfig`  | `inputs: InputConfig`  |
| Output shape         | direct rows               | `outputs: OutputConfig` (JSONPath) | `outputs: OutputConfig` (JSONPath) |

## B. Connector / Backend Trait

| Aspect | SQL | GraphQL | OpenAPI |
|--------|-----|---------|---------|
| Trait or concrete | `pub trait DatabaseConnector` (`connectors/mod.rs:165`) | concrete `GraphQLClient` (`client/mod.rs:18`) | concrete `HttpClient` (`http/mod.rs:19`) |
| Multi-impl?       | YES — `SqliteConnector`, `AthenaConnector` | NO — one client, one transport (reqwest) | NO — one client, one transport (reqwest) |
| Key method        | `execute_query(sql, params) -> QueryResult` | `execute(query, variables) -> Value` | `execute(operation, params) -> ApiResponse` |
| Pool / state      | connection pool (`max_connections`) | reqwest client | reqwest client |

**Insight:** SQL *needs* a trait because SQLite ≠ Athena ≠ Postgres at the
storage layer. GraphQL and OpenAPI don't need one because both are over HTTP.
A single shared "backend executor" trait would have to either (a) be so
abstract it's useless, or (b) leak HTTP details that don't fit SQL. **Not
viable.**

## C. Tool Execution Path

| Aspect | SQL | GraphQL | OpenAPI |
|--------|-----|---------|---------|
| Entry type | `ToolExecutor<'a>` (`tools.rs:14`) | `ToolGenerator::generate` (`tools/mod.rs:169`) | `create_tool_from_config` (`tools/mod.rs:229`) |
| Binding | `:name` → `?`, positional via `PreparedStatementRegistry` | regex-extract `$var`, build JSON variables object | path template parser → required path params; remainder split into query (GET) or body (POST/PUT/PATCH) |
| Validation pre-exec | `SqlValidator` parses query (`validation/parser.rs`) | `GraphQLValidator::validate(query)` (`graphql_validator.rs:68`) | `Operation` extracted at config-load; per-call validates inputs against JSON Schema |
| Verb routing | `QueryType::Select` → `execute_query`; DML → `execute_statement` | always `client.execute_with_operation_name(...)` | HTTP verb dispatch (`code_mode.rs:207-218`) |

## D. Code Mode

| Aspect | SQL | GraphQL | OpenAPI |
|--------|-----|---------|---------|
| LoC `code_mode.rs` | 545 | 767 | **1560** (3× SQL) |
| Handler type | `SqlCodeModeHandler` (`code_mode.rs:75`) | `GraphqlCodeModeServer` (`code_mode.rs:517`) | `OpenApiCodeModeServer` (`code_mode.rs:272`) |
| Trait impl | `#[async_trait] CodeExecutor` | `#[derive(CodeMode)]` macro | manual `register_code_mode_tools` (not the macro) |
| Bootstrap prompt name | `CODE_MODE_PROMPT_NAME` (shared) | shared | shared |
| Token machinery owner | `pmcp_code_mode` (SDK) | `pmcp_code_mode` (SDK) | `pmcp_code_mode` (SDK) |
| `validate_code` / `execute_code` | from SDK macro | from SDK macro | manually registered (so AVP can intercept) |
| Policy hash | `policy_hash` over `allow_writes`, `blocked_tables`, etc. (`code_mode.rs:354-400`) | hash over mutation allowlist | hash + AVP/Cedar entity scoring |
| Long-tail security | LIMIT enforcement, blocklist | mutation root-field allowlist, hardcoded sensitive fields | HTTP verb policy, `/admin/*` → Critical, two-tier field blocklist (`internal_blocked_fields` + `output_blocked_fields`), AVP/Cedar `PolicyEvaluator` |

**Why OpenAPI's code_mode is 3× the size:**
- AVP/Cedar `PolicyEvaluator` integration (`code_mode.rs:382-417`)
- Manual `register_code_mode_tools` (the derive macro doesn't fit because AVP
  evaluation has to interpose between sync validation and token issuance)
- `HttpClientExecutor` trait impl owning HTTP verb dispatch + path templating
  + query/body splitting (`code_mode.rs:89-259`)
- Two-tier field blocklist enforcement (internal vs output)
- Schema exposure conversion in `src/schema/exposure.rs`

This is **substance, not duplication**. The OpenAPI code-mode genuinely has
more to do because its policy surface is richer.

## E. Server Bootstrap

| Aspect | SQL | GraphQL | OpenAPI |
|--------|-----|---------|---------|
| Top-level type | `SqlMcpServer<C: DatabaseConnector>` (`server.rs:42`) | `GraphqlMcpServer` (`server.rs:44`) | `OpenApiPmcpBuilder` (`pmcp_server.rs:94`) |
| Convert to `pmcp::Server` | `into_pmcp_server(self) -> pmcp::Result<pmcp::Server>` | `into_pmcp_server(self)` | `build(self) -> pmcp::Result<pmcp::Server>` |
| Lambda | `run_lambda(server)` (`lambda.rs:34`) | `run_lambda(server)` (`lambda.rs:31`) | `run_openapi_lambda(...)` (`lambda.rs`) |

## F. Public API (`lib.rs` re-exports) — Cross-cutting Audit

All three crates re-export:

- `*Config` (`SqlConfig` / `GraphQLConfig` / `OpenApiConfig`)
- A tool-execution entry (`ToolExecutor` / `GeneratedGraphQLTool` / `GeneratedTool`)
- The server bootstrap type
- `CODE_MODE_PROMPT_NAME` (via `mcp-server-common` re-export)
- `run_lambda` (via `lambda` feature)

All three depend on:

- `pmcp` (the SDK)
- `mcp-server-common` (the proto-SDK)
- `pmcp-code-mode` (the HMAC + executor SDK crate)
- `serde` + `serde_json` + `tokio` + `async-trait` + `tracing`

## G. Non-Generalizable Surface Per Crate

**SQL:** parameter rewriting (`:name` → `?`), LIMIT enforcement, prepared
statement registry, query-type routing (SELECT vs DML), blocked tables /
columns / sensitive columns, complexity limits (`max_join_depth`,
`max_subquery_depth`).

**GraphQL:** introspection query, fragments + multi-operation handling
(`operation_name`), selection-set tracking, mutation root-field allowlist,
hardcoded sensitive field names (`password`, `ssn`, `apiKey`, `token`),
GraphQL type-system awareness for enum detection.

**OpenAPI:** HTTP verb dispatch, path templates with `{placeholder}`
expansion, parameter location separation (path / query / header / cookie /
body), content negotiation, response status policy, OAuth flows
(client-credentials, passthrough), AVP/Cedar policy integration, two-tier
field blocklist, schema exposure risk levels (`/admin/*` → Critical), scope
binding (user_id + session_id).

## H. Verdict

**PARTIAL** — re-framed as **VALIDATED with a specific shape**.

- ✓ Shared abstraction is real and **already extracted** into
  `mcp-server-common` + `pmcp-code-mode`.
- ✗ A single `SchemaServer<S, C>` trait covering all three backends is **not
  viable** — the per-backend executor, validator, and policy layers diverge
  semantically.
- ⇒ The actionable lift is **promoting `mcp-server-common` to a `crates/`
  workspace member of `rust-mcp-sdk`** with a public, stable home (candidate
  names: `pmcp-server-toolkit`, `pmcp-builtin-server`). Per-backend crates
  stay where they are (pmcp-run/built-in) but gain the option to publish to
  crates.io against a versioned toolkit dep instead of a path dep.
- ⇒ `cargo-pmcp new --kind {sql,graphql,openapi}-server` is the cheap, useful
  scaffolding layer on top — it just drops a starter `Cargo.toml` that pulls
  in the toolkit + a chosen backend crate. No new abstraction required.
- ⇒ `#[pmcp::sql_server]` proc-macro is **secondary** — without the toolkit
  being public on crates.io, the macro would expand to types nobody can
  depend on. Defer.

Spike 004 will validate the smallest viable lift end-to-end against this
shape: a minimal `pmcp-server-toolkit`-shaped slice (auth + resources +
prompts + config helpers) plus a SQLite reference backend that exercises the
toolkit and runs a real PMCP server.
