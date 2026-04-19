# Changelog

All notable changes to `pmcp-code-mode` will be documented in this file.

## [0.5.0] - 2026-04-16

### Added — Developer Experience

- **Auto-resolve `server_id` from environment.** `CodeModeConfig::resolve_server_id()`
  auto-fills `server_id` from `PMCP_SERVER_ID` or `AWS_LAMBDA_FUNCTION_NAME` env vars
  (in that order) when not set in TOML. All `ValidationPipeline` constructors call
  this automatically, so wrappers no longer need to implement their own resolution
  chain. When a policy evaluator is configured but `server_id` is still unresolved,
  `with_policy_evaluator` emits a tracing warning — this used to produce silent
  default-deny failures.
- `CodeModeConfig::require_server_id()` — fail-fast accessor returning
  `Result<&str, ValidationError::ConfigError>` for code paths that need AVP authorization.
- `resolve_server_id_from_env()` — exposed as a free function for tests and
  non-pipeline callers.

- **SQL TOML DX.** `sql_*` config fields now accept both prefixed (`sql_allow_writes`)
  and unprefixed (`allow_writes`) names via `#[serde(alias = ...)]`. Downstream SQL
  servers can use the natural vocabulary in their `[code_mode]` block without
  manual conversion:

  ```toml
  [code_mode]
  enabled = true
  allow_writes = false     # same as sql_allow_writes
  blocked_tables = ["secrets"]
  max_rows = 5000
  ```

  Aliased fields: `allow_writes`, `allow_deletes`, `allow_ddl`, `reads_enabled`,
  `allowed_statements`, `blocked_statements`, `allowed_tables`, `blocked_tables`,
  `blocked_columns`, `max_rows`, `max_joins`, `require_where_on_writes`.

- **Debuggable default-deny.** When a policy evaluator returns `allowed: false`
  with empty `determining_policies` (the canonical "no Permit matched" case), the
  SDK previously returned `violations: []` — impossible to debug from the client.
  The new `build_policy_violations` helper (applied to all three language paths):
  1. Maps each `determining_policies` entry to a `policy` violation (existing)
  2. Maps each `decision.errors` entry to an `evaluation_error` violation (new —
     surfaces Cedar schema errors, missing attributes, etc.)
  3. If both are empty, injects a synthetic `default_deny` violation naming the
     `server_id` and `action` so the client has context to debug

### Added — SQL Code Mode

- **SQL Code Mode** — new `sql-code-mode` feature flag with `sqlparser 0.61` backend.
  Enables `#[derive(CodeMode)] #[code_mode(language = "sql")]` for SQL-based MCP servers.
- `ValidationPipeline::validate_sql_query` (sync, config-only checks) and
  `validate_sql_query_async` (with `PolicyEvaluator::evaluate_statement` policy enforcement).
  The derive macro now routes `language = "sql"` to `validate_sql_query_async` for
  consistency with GraphQL/JS async patterns.
- `sql` module: `SqlValidator`, `SqlStatementInfo`, `SqlStatementType` — classifies
  SELECT/INSERT/UPDATE/DELETE/DDL and extracts tables, columns, JOINs, subqueries,
  WHERE/LIMIT/ORDER BY flags, estimated rows.
- `StatementEntity` + `SqlServerEntity` Cedar entities (behind `sql-code-mode`).
- `PolicyEvaluator::evaluate_statement` default-deny trait method.
- `AvpPolicyEvaluator::evaluate_statement` + `is_statement_authorized` + entity builders
  (behind `avp + sql-code-mode`).
- `NoopPolicyEvaluator::evaluate_statement` (allow-all, test use).
- `get_sql_code_mode_schema_json()` and `get_sql_baseline_policies()` schema/policy exports.
- `CodeModeConfig.to_sql_server_entity()` converts config to Cedar entity.
- Schema sync test: `test_sql_schema_sources_in_sync` enforces that `SQL_CEDAR_SCHEMA`
  and `get_sql_code_mode_schema_json()` stay aligned.

### SQL Config Fields

New `sql_*` prefixed fields in `CodeModeConfig` (feel natural to DB admins):

- `sql_reads_enabled: bool` (default `true`) — SELECT statements enabled.
- `sql_allow_writes: bool` — INSERT/UPDATE allowed globally.
- `sql_allow_deletes: bool` — DELETE/TRUNCATE allowed globally.
- `sql_allow_ddl: bool` — CREATE/ALTER/DROP/GRANT/REVOKE allowed (default `false`).
- `sql_allowed_statements: HashSet<String>` — statement-type allowlist.
- `sql_blocked_statements: HashSet<String>` — statement-type blocklist.
- `sql_allowed_tables: HashSet<String>` — table allowlist (e.g., `["users", "orders"]`).
- `sql_blocked_tables: HashSet<String>` — table blocklist (e.g., `["secrets"]`).
- `sql_blocked_columns: HashSet<String>` — column blocklist (e.g., `["password", "ssn"]`).
- `sql_max_rows: u64` (default `10_000`) — row-count estimate limit.
- `sql_max_joins: u32` (default `5`) — JOIN count limit.
- `sql_require_where_on_writes: bool` (default `true`) — UPDATE/DELETE require WHERE.

## [0.3.0] - 2026-04-14

### Breaking Changes

- `validate_javascript_code_async` added — the derive macro now calls this instead of the sync
  `validate_javascript_code`. This means **policy evaluation (Cedar/AVP) is now enforced** for
  JavaScript/OpenAPI servers using `#[derive(CodeMode)]`.

  The default `PolicyEvaluator::evaluate_script` implementation **denies all scripts**. If you
  have a custom evaluator that only implements `evaluate_operation`, override `evaluate_script`
  to allow scripts. `NoopPolicyEvaluator` already allows all scripts.

- Policy evaluation errors are **fail-closed** (matching the GraphQL path). A policy backend
  outage blocks JavaScript validation requests instead of silently allowing them.

### Added

- `validate_javascript_code_async` — async JavaScript validation with `PolicyEvaluator::evaluate_script`
  policy enforcement. Mirrors `validate_graphql_query_async` pattern.
- `validate_js_preamble` and `check_js_config_authorization` — shared helpers extracted from
  sync/async JavaScript validation (eliminates 55 lines of duplication).
- `CodeLanguage` enum — extensible runtime representation of supported languages with `from_attr`,
  `as_str`, `required_feature` methods.
- `JsCodeExecutor<H>` — standard adapter bridging `HttpExecutor` to `CodeExecutor` (Pattern B: JS+HTTP).
- `SdkCodeExecutor<S>` — standard adapter bridging `SdkExecutor` to `CodeExecutor` (Pattern C: JS+SDK).
- `McpCodeExecutor<M>` — standard adapter bridging `McpExecutor` to `CodeExecutor` (Pattern D: JS+MCP).
- `SdkExecutor` now exported from lib.rs (was previously missing).

## [0.2.0] - 2026-04-13

### Breaking Changes

- `HmacTokenGenerator::new` and `new_from_bytes` now return `Result<Self, TokenError>` instead of
  panicking on short secrets. This catches misconfigured HMAC secrets at startup instead of
  panicking at runtime.

- `ValidationPipeline::new`, `from_token_secret`, `with_policy_evaluator`, and
  `from_token_secret_with_policy` now return `Result<Self, TokenError>` instead of `Self`.

  **Migration:** Add `?` or `.unwrap()` to your constructor calls:

  ```rust
  // Before (v0.1.0):
  let pipeline = ValidationPipeline::new(config, secret);

  // After (v0.2.0):
  let pipeline = ValidationPipeline::new(config, secret)?;
  ```

- `ValidationPipeline::policy_evaluator` field changed from `Option<Box<dyn PolicyEvaluator>>`
  to `Option<Arc<dyn PolicyEvaluator>>`. `with_policy_evaluator` and `set_policy_evaluator`
  accept `Arc<dyn PolicyEvaluator>`.

- `language` attribute now selects the validation path at compile time, not just tool metadata.

### Added

- `TokenError` enum for token generator construction errors (`TokenError::SecretTooShort`).
- `#[code_mode(context_from = "method_name")]` attribute for real `ValidationContext` binding.
- `#[code_mode(language = "...")]` attribute — selects validation method and tool metadata.
- `PolicyEvaluator` trait wiring through `Arc<dyn PolicyEvaluator>` in `ValidationPipeline`.
- `from_token_secret_with_policy` constructor for derive macro use.
- Compile-fail trybuild tests for missing `token_secret` and `code_executor` fields.
