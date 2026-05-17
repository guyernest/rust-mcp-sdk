//! Spike 004: schema-server-thin-slice-sql
//!
//! Risk-second spike following the 003 verdict that the shared abstraction
//! already exists at `pmcp-run/built-in/shared/mcp-server-common` and the
//! actionable SDK lift is *promoting it* (not re-designing it). This spike
//! validates the SMALLEST viable slice of that toolkit shape — inline, so
//! the entire surface fits in one file the user can read — against a real
//! SQLite backend.
//!
//! What this proves (or fails to prove):
//!   1. A `*Config::from_toml(&str)` loader cleanly produces a fully-
//!      populated config from a tiny TOML the user writes.
//!   2. A `DatabaseConnector` trait (the one piece of structure that does
//!      need to live in the toolkit) plus a SQLite reference impl wires
//!      up end-to-end against a real in-memory DB.
//!   3. The toolkit can synthesize MCP `ToolInfo`s from `[[tools]]` config
//!      entries (no per-tool Rust handler needed) and the resulting
//!      `pmcp::Server` reports them via `Server::has_tool(name)`.
//!   4. A per-tool handler binds `:param` placeholders, executes SQL
//!      against the connector, and returns rows as JSON.
//!   5. A code-mode bootstrap prompt is registered with the canonical
//!      `"start_code_mode"` name, body includes the schema, and the server
//!      reports it via `Server::has_prompt(name)`.
//!   6. The total user-facing surface (`run_user_facing_main` below)
//!      really is ~15 lines, with no per-tool Rust code.
//!
//! Layout:
//!   - `mod toolkit`   — the inline slice. In a real SDK lift this
//!                       becomes `crates/pmcp-server-toolkit/`.
//!   - `mod sqlite_backend` — the reference connector. Real version
//!                       would be `crates/pmcp-toolkit-sqlite/` or
//!                       feature-gated inside the toolkit.
//!   - `run_user_facing_main` — the ~15-line surface a developer writes.
//!   - `step_*` assertions — drive everything and assert end-to-end.

#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

// =============================================================================
//                                  TOOLKIT
// =============================================================================
//
// This module is the *inline* version of what `crates/pmcp-server-toolkit/`
// would expose. Everything below mod toolkit{...} is "library code"; the
// user-facing demo never reaches into these internals.

mod toolkit {
    use super::*;
    use pmcp::server::ServerBuilder;
    use pmcp::types::{
        CallToolResult, Content, GetPromptResult, PromptInfo, PromptMessage,
        Role, ToolInfo,
    };
    use pmcp::Server;
    use std::collections::HashMap;

    // -- Config types ------------------------------------------------------

    /// Top-level config. Matches the shape of `SqlConfig::from_toml` in
    /// `mcp-sql-server-core` (validated as a shared shape by spike 003).
    #[derive(Debug, Clone, Deserialize)]
    pub struct SchemaServerConfig {
        pub server: ServerSection,
        #[serde(default)]
        pub tools: Vec<ToolDecl>,
        #[serde(default)]
        pub code_mode: Option<CodeModeSection>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct ServerSection {
        pub name: String,
        pub version: String,
        #[serde(default)]
        pub description: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct ToolDecl {
        pub name: String,
        pub description: String,
        pub sql: String,
        #[serde(default)]
        pub parameters: Vec<ParamDecl>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct ParamDecl {
        pub name: String,
        #[serde(rename = "type")]
        pub r#type: ParamType,
        pub description: String,
        #[serde(default)]
        pub required: bool,
    }

    #[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum ParamType {
        Integer,
        String,
        Number,
        Boolean,
    }

    impl ParamType {
        fn json_schema_type(self) -> &'static str {
            match self {
                ParamType::Integer => "integer",
                ParamType::String => "string",
                ParamType::Number => "number",
                ParamType::Boolean => "boolean",
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct CodeModeSection {
        pub enabled: bool,
    }

    impl SchemaServerConfig {
        /// Single entrypoint to load the entire server surface from a
        /// TOML string. Matches the shape `SqlConfig::from_toml`
        /// (config.rs:50 in mcp-sql-server-core) confirmed as shared by
        /// spike 003.
        pub fn from_toml(s: &str) -> Result<Self> {
            toml::from_str(s).map_err(|e| anyhow!("config parse error: {e}"))
        }
    }

    // -- Backend trait ----------------------------------------------------

    /// Backend-pluggable executor. SQL flavor sits here. In a real toolkit
    /// this is the analog of `DatabaseConnector` from
    /// `mcp-sql-server-core/src/connectors/mod.rs:165`.
    ///
    /// Spike 003 validated that this trait genuinely DOES need to be
    /// per-backend (SQLite ≠ Athena ≠ Postgres). The toolkit ships the
    /// trait + reference impls; non-SQL backends (GraphQL, OpenAPI) would
    /// have their OWN executor trait in a separate per-backend crate.
    #[async_trait]
    pub trait SqlConnector: Send + Sync + 'static {
        /// Run `sql` with named `params` (`:name` placeholders). Return
        /// rows as a JSON array of objects (column_name -> value).
        async fn execute(&self, sql: &str, params: &[(String, Value)])
            -> Result<Vec<Value>>;

        /// Best-effort schema rendering for the code-mode bootstrap
        /// prompt. The toolkit's prompt builder uses this to seed the
        /// LLM with the long-tail surface.
        async fn schema_text(&self) -> Result<String>;
    }

    // -- The per-tool handler. Builds ToolInfo from ToolDecl and
    //    executes the declared SQL on call.

    pub struct SqlToolHandler {
        pub decl: ToolDecl,
        pub connector: Arc<dyn SqlConnector>,
    }

    impl SqlToolHandler {
        fn input_schema(&self) -> Value {
            let mut props = serde_json::Map::new();
            let mut required = Vec::new();
            for p in &self.decl.parameters {
                props.insert(
                    p.name.clone(),
                    json!({
                        "type": p.r#type.json_schema_type(),
                        "description": p.description,
                    }),
                );
                if p.required {
                    required.push(Value::String(p.name.clone()));
                }
            }
            json!({
                "type": "object",
                "properties": props,
                "required": required,
                "additionalProperties": false,
            })
        }

        fn make_tool_info(&self) -> ToolInfo {
            ToolInfo::new(
                self.decl.name.clone(),
                Some(self.decl.description.clone()),
                self.input_schema(),
            )
        }
    }

    #[async_trait]
    impl pmcp::server::ToolHandler for SqlToolHandler {
        async fn handle(
            &self,
            args: Value,
            _extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<Value> {
            // Bind declared parameters from `args` in the order declared.
            // Missing required params are an error; non-required missing
            // params are simply omitted (the connector substitutes NULL
            // semantics or the SQL relies on defaults).
            let mut bound = Vec::with_capacity(self.decl.parameters.len());
            let args_obj = args
                .as_object()
                .ok_or_else(|| pmcp::Error::validation("expected JSON object for arguments"))?;
            for p in &self.decl.parameters {
                match args_obj.get(&p.name) {
                    Some(v) => bound.push((p.name.clone(), v.clone())),
                    None if p.required => {
                        return Err(pmcp::Error::validation(format!(
                            "missing required parameter `{}`",
                            p.name
                        )));
                    }
                    None => bound.push((p.name.clone(), Value::Null)),
                }
            }

            let rows = self
                .connector
                .execute(&self.decl.sql, &bound)
                .await
                .map_err(|e| pmcp::Error::internal(format!("backend error: {e}")))?;

            // Return rows as a single text content node (JSON-serialized).
            // A production toolkit would also support output_schema +
            // structuredContent; out of scope for this spike.
            let result = CallToolResult::new(vec![Content::Text {
                text: serde_json::to_string_pretty(&rows).unwrap(),
            }]);
            serde_json::to_value(&result)
                .map_err(|e| pmcp::Error::internal(format!("serialize CallToolResult: {e}")))
        }

        fn metadata(&self) -> Option<ToolInfo> {
            Some(self.make_tool_info())
        }
    }

    // -- Code-mode bootstrap prompt --------------------------------------

    /// Canonical name from `mcp-server-common::prompts::CODE_MODE_PROMPT_NAME`
    /// (`prompts.rs:33`). Hardcoded here to match the shared convention
    /// without depending on the proto-SDK crate.
    pub const CODE_MODE_PROMPT_NAME: &str = "start_code_mode";

    pub struct CodeModePrompt {
        pub schema_text: String,
    }

    #[async_trait]
    impl pmcp::server::PromptHandler for CodeModePrompt {
        async fn handle(
            &self,
            _args: HashMap<String, String>,
            _extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<GetPromptResult> {
            let _ = Role::User; // suppress unused-import warning when Role isn't otherwise referenced
            let body = format!(
                "You can author ad-hoc SQL queries against the following \
                 schema. Prefer the curated tools for known operations; use \
                 ad-hoc SQL only when no curated tool fits.\n\n\
                 ---\nSCHEMA:\n{}\n---\n",
                self.schema_text
            );
            Ok(GetPromptResult::new(
                vec![PromptMessage::user(Content::Text { text: body })],
                Some("Bootstrap context for ad-hoc SQL.".to_string()),
            ))
        }

        fn metadata(&self) -> Option<PromptInfo> {
            Some(
                PromptInfo::new(CODE_MODE_PROMPT_NAME)
                    .with_description("Long-tail SQL bootstrap."),
            )
        }
    }

    // -- The toolkit's top-level type. -----------------------------------

    /// Delegating wrapper so we can share one `Arc<SqlToolHandler>` between
    /// the builder (which takes `impl ToolHandler + 'static` by value) and
    /// the spike's own handler map (which keeps an `Arc` for in-process
    /// invocation). Public ServerBuilder lacks `tool_arc` today; a future
    /// toolkit lift should either add it upstream or ship this shim.
    pub struct ToolHandlerArc(pub Arc<SqlToolHandler>);

    #[async_trait]
    impl pmcp::server::ToolHandler for ToolHandlerArc {
        async fn handle(
            &self,
            args: Value,
            extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<Value> {
            pmcp::server::ToolHandler::handle(&*self.0, args, extra).await
        }
        fn metadata(&self) -> Option<ToolInfo> {
            pmcp::server::ToolHandler::metadata(&*self.0)
        }
    }

    /// Same shim shape for prompts.
    pub struct PromptHandlerArc(pub Arc<CodeModePrompt>);

    #[async_trait]
    impl pmcp::server::PromptHandler for PromptHandlerArc {
        async fn handle(
            &self,
            args: HashMap<String, String>,
            extra: pmcp::RequestHandlerExtra,
        ) -> pmcp::Result<GetPromptResult> {
            pmcp::server::PromptHandler::handle(&*self.0, args, extra).await
        }
        fn metadata(&self) -> Option<PromptInfo> {
            pmcp::server::PromptHandler::metadata(&*self.0)
        }
    }

    pub struct SchemaServer {
        config: SchemaServerConfig,
        connector: Arc<dyn SqlConnector>,
    }

    impl SchemaServer {
        pub fn new(config: SchemaServerConfig, connector: Arc<dyn SqlConnector>) -> Self {
            Self { config, connector }
        }

        /// Build a `pmcp::Server` plus a sidecar map of `tool_name ->
        /// Arc<SqlToolHandler>` so the spike (and future tests) can drive
        /// handlers in-process without a transport. In a real toolkit this
        /// would be split into `into_pmcp_server()` (production) and
        /// `into_pmcp_server_with_handlers()` (testing) per the
        /// `into_pmcp_builder()` pattern at
        /// `mcp-sql-server-core/src/pmcp_server.rs:254`.
        pub async fn into_pmcp_server_with_handlers(
            self,
        ) -> Result<(Server, HashMap<String, Arc<SqlToolHandler>>, Arc<CodeModePrompt>)>
        {
            let mut builder: ServerBuilder = Server::builder()
                .name(self.config.server.name.clone())
                .version(self.config.server.version.clone());

            // Register each [[tools]] entry as a per-tool handler.
            let mut handlers: HashMap<String, Arc<SqlToolHandler>> = HashMap::new();
            for decl in &self.config.tools {
                let handler = Arc::new(SqlToolHandler {
                    decl: decl.clone(),
                    connector: self.connector.clone(),
                });
                handlers.insert(decl.name.clone(), handler.clone());
                builder = builder.tool(decl.name.clone(), ToolHandlerArc(handler));
            }

            // Wire the code-mode bootstrap prompt (if enabled). The body
            // includes the schema text the connector reports, so the LLM
            // has the long-tail surface in one prompt fetch.
            let cm_prompt = if self
                .config
                .code_mode
                .as_ref()
                .map(|c| c.enabled)
                .unwrap_or(false)
            {
                let schema = self.connector.schema_text().await?;
                let prompt = Arc::new(CodeModePrompt {
                    schema_text: schema,
                });
                builder = builder.prompt(
                    CODE_MODE_PROMPT_NAME.to_string(),
                    PromptHandlerArc(prompt.clone()),
                );
                prompt
            } else {
                Arc::new(CodeModePrompt {
                    schema_text: String::new(),
                })
            };

            let server = builder
                .build()
                .map_err(|e| anyhow!("server build failed: {e}"))?;
            Ok((server, handlers, cm_prompt))
        }
    }
}

// =============================================================================
//                              SQLITE BACKEND
// =============================================================================
//
// Reference impl of `toolkit::SqlConnector` over `rusqlite`. In a real
// crate-organization this would live in `crates/pmcp-toolkit-sqlite/` (or
// behind a `sqlite` feature flag on the toolkit crate).

mod sqlite_backend {
    use super::*;
    use rusqlite::types::{Value as SqlValue, ValueRef};
    use rusqlite::Connection;

    pub struct SqliteConnector {
        conn: Mutex<Connection>,
        schema_sql: String,
    }

    impl SqliteConnector {
        /// Open an in-memory SQLite DB and seed it from a schema SQL blob.
        /// Real impl would also accept a file path; in-memory keeps the
        /// spike self-contained and side-effect-free.
        pub fn in_memory_with_schema(schema_sql: &str) -> Result<Self> {
            let conn = Connection::open_in_memory()
                .context("opening in-memory SQLite")?;
            conn.execute_batch(schema_sql).context("loading schema")?;
            Ok(Self {
                conn: Mutex::new(conn),
                schema_sql: schema_sql.to_string(),
            })
        }
    }

    /// Convert a `serde_json::Value` to a rusqlite-bindable value.
    fn json_to_sql_value(v: &Value) -> SqlValue {
        match v {
            Value::Null => SqlValue::Null,
            Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    SqlValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    SqlValue::Real(f)
                } else {
                    SqlValue::Null
                }
            }
            Value::String(s) => SqlValue::Text(s.clone()),
            Value::Array(_) | Value::Object(_) => SqlValue::Text(v.to_string()),
        }
    }

    fn sql_value_to_json(v: ValueRef<'_>) -> Value {
        match v {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(i) => Value::Number(i.into()),
            ValueRef::Real(f) => serde_json::Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(_) => Value::String("<blob>".to_string()),
        }
    }

    #[async_trait]
    impl toolkit::SqlConnector for SqliteConnector {
        async fn execute(
            &self,
            sql: &str,
            params: &[(String, Value)],
        ) -> Result<Vec<Value>> {
            // Convert :name → ? positionally. rusqlite supports `:name`
            // directly via named_params! macro, but we need to bind
            // dynamically. Use prepared statement with `parameter_count`
            // + iteration to keep this generic.
            let conn = self.conn.lock().expect("sqlite mutex");
            let mut stmt = conn.prepare(sql).context("preparing statement")?;

            // rusqlite numbers named params from 1. We feed them via
            // raw_bind_parameter by name (rusqlite has `parameter_index`
            // for that).
            for (name, val) in params {
                let bind_name = format!(":{name}");
                if let Some(idx) = stmt.parameter_index(&bind_name)? {
                    stmt.raw_bind_parameter(idx, json_to_sql_value(val))
                        .context("binding param")?;
                }
                // Params not present in the SQL are silently skipped —
                // matches the toolkit's "missing optional params bind
                // to null" semantics.
            }

            // Read column names BEFORE iterating rows (lifetime constraint).
            let col_names: Vec<String> =
                stmt.column_names().iter().map(|c| c.to_string()).collect();
            let col_count = col_names.len();

            let mut rows = stmt.raw_query();
            let mut out = Vec::new();
            while let Some(row) = rows.next().context("iterating rows")? {
                let mut obj = serde_json::Map::new();
                for i in 0..col_count {
                    let v = row.get_ref(i).context("reading column value")?;
                    obj.insert(col_names[i].clone(), sql_value_to_json(v));
                }
                out.push(Value::Object(obj));
            }
            Ok(out)
        }

        async fn schema_text(&self) -> Result<String> {
            // For the spike we hand back the schema blob we were seeded
            // with. A real connector would introspect `sqlite_master`
            // and emit CREATE TABLE statements + sample column types.
            Ok(self.schema_sql.clone())
        }
    }
}

// =============================================================================
//                       USER-FACING SURFACE  (~15 lines)
// =============================================================================
//
// This is the "what the developer writes" surface. The toolkit + backend
// modules above are the SDK lift; below is everything a person needs.
//
// Count: from `let config = ...` through `Ok((server, ...))` is 12 lines
// of meaningful code (comments + blanks excluded). The user does NOT write
// per-tool handlers, parameter parsers, or JSON Schema generators.

const CONFIG_TOML: &str = include_str!("../config.toml");
const SCHEMA_SQL: &str = include_str!("../schema.sql");

async fn run_user_facing_main() -> Result<(
    pmcp::Server,
    std::collections::HashMap<String, Arc<toolkit::SqlToolHandler>>,
    Arc<toolkit::CodeModePrompt>,
)> {
    let config = toolkit::SchemaServerConfig::from_toml(CONFIG_TOML)?;
    let connector = Arc::new(sqlite_backend::SqliteConnector::in_memory_with_schema(
        SCHEMA_SQL,
    )?) as Arc<dyn toolkit::SqlConnector>;
    let schema_server = toolkit::SchemaServer::new(config, connector);
    schema_server.into_pmcp_server_with_handlers().await
}

// =============================================================================
//                              SPIKE ASSERTIONS
// =============================================================================

fn print_banner() {
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Spike 004: schema-server-thin-slice-sql");
    println!("  Risk-second: validate the smallest viable SDK toolkit lift end-to-end");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
}

fn rule() {
    println!("{}", "─".repeat(78));
}

fn header(title: &str) {
    println!();
    rule();
    println!("▶ {title}");
    rule();
}

fn ok(msg: &str) {
    println!("  ✓ {msg}");
}

#[tokio::main]
async fn main() -> Result<()> {
    print_banner();

    header("Step A · Parse config + spin up server (the user-facing surface)");
    let (server, handlers, cm_prompt) = run_user_facing_main().await?;
    ok("config.toml parsed");
    ok("schema.sql seeded into in-memory SQLite");
    ok("toolkit::SchemaServer::new(config, connector).into_pmcp_server_with_handlers() OK");

    header("Step B · ToolInfo synthesis from [[tools]] config");
    assert_eq!(
        handlers.len(),
        2,
        "expected 2 handlers from 2 [[tools]] entries"
    );
    ok(&format!("handler count = {} (matches 2 [[tools]] entries)", handlers.len()));

    let get_emp = handlers
        .get("get_employee_by_id")
        .expect("get_employee_by_id handler not registered");
    let meta = pmcp::server::ToolHandler::metadata(&**get_emp)
        .expect("handler must expose metadata");
    assert_eq!(meta.name, "get_employee_by_id");
    assert_eq!(
        meta.description.as_deref(),
        Some("Look up a single employee by their unique id.")
    );
    let schema = &meta.input_schema;
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["id"]["type"], "integer");
    assert_eq!(
        schema["properties"]["id"]["description"],
        "Employee id (1-based)."
    );
    assert_eq!(
        schema["required"],
        json!(["id"]),
        "`id` is declared required:true and must appear in the JSON Schema `required` array"
    );
    ok("ToolInfo.input_schema synthesized correctly (type/description/required)");

    header("Step C · pmcp::Server reports both tools via has_tool()");
    assert!(
        server.has_tool("get_employee_by_id"),
        "server.has_tool should report the registered tool"
    );
    assert!(
        server.has_tool("list_employees_by_department"),
        "server.has_tool should report the second registered tool"
    );
    assert!(
        !server.has_tool("nonexistent_tool"),
        "server.has_tool should NOT report unregistered tools"
    );
    ok("server.has_tool('get_employee_by_id')        = true");
    ok("server.has_tool('list_employees_by_department') = true");
    ok("server.has_tool('nonexistent_tool')          = false");

    header("Step D · tools/call against SQLite — parameter binding + row return");
    let extra = pmcp::RequestHandlerExtra::default();
    let result = pmcp::server::ToolHandler::handle(
        &**get_emp,
        json!({ "id": 3 }),
        extra.clone(),
    )
    .await?;
    // result is a CallToolResult serialized as JSON. Pull the text content.
    let content = &result["content"][0]["text"];
    let rows: Value = serde_json::from_str(content.as_str().expect("text content"))?;
    println!("\n  Wire-format response payload:");
    for line in serde_json::to_string_pretty(&rows)?.lines() {
        println!("    {line}");
    }
    let arr = rows.as_array().expect("rows is array");
    assert_eq!(arr.len(), 1, "id=3 selects exactly one row");
    assert_eq!(arr[0]["name"], "Alan Turing");
    assert_eq!(arr[0]["department"], "Research");
    assert_eq!(arr[0]["salary"], 210_000);
    ok("get_employee_by_id(id=3) → 1 row, name='Alan Turing'");

    // Second tool: list_employees_by_department(department=Research)
    let list_dept = handlers
        .get("list_employees_by_department")
        .expect("list_employees_by_department handler not registered");
    let result = pmcp::server::ToolHandler::handle(
        &**list_dept,
        json!({ "department": "Research" }),
        extra.clone(),
    )
    .await?;
    let rows: Value = serde_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .expect("text content"),
    )?;
    let arr = rows.as_array().expect("rows is array");
    assert_eq!(
        arr.len(),
        4,
        "Research dept has 4 employees (Ada, Grace, Alan, Knuth)"
    );
    // ORDER BY salary DESC — Knuth (220k) first.
    assert_eq!(arr[0]["name"], "Donald Knuth");
    assert_eq!(arr[0]["salary"], 220_000);
    ok("list_employees_by_department(department='Research') → 4 rows, ORDER BY salary DESC honored");

    header("Step E · Missing-required-parameter is a clean validation error");
    let bad = pmcp::server::ToolHandler::handle(
        &**get_emp,
        json!({}),
        extra.clone(),
    )
    .await;
    assert!(
        bad.is_err(),
        "missing required `id` parameter must produce an error, not silent NULL"
    );
    let msg = format!("{}", bad.unwrap_err());
    assert!(
        msg.contains("id") || msg.to_lowercase().contains("required"),
        "error message should mention the missing parameter `id`, got: {msg}"
    );
    ok(&format!("missing required param produced error: {msg}"));

    header("Step F · Code-mode bootstrap prompt registered with canonical name");
    assert_eq!(toolkit::CODE_MODE_PROMPT_NAME, "start_code_mode");
    assert!(
        server.has_prompt("start_code_mode"),
        "server.has_prompt should report the bootstrap prompt"
    );
    let cm_meta = pmcp::server::PromptHandler::metadata(&*cm_prompt)
        .expect("prompt must expose metadata");
    assert_eq!(cm_meta.name, "start_code_mode");
    ok("server.has_prompt('start_code_mode') = true (matches mcp-server-common canonical name)");

    // Verify the prompt body includes the schema text (so the LLM has
    // the long-tail surface in one fetch).
    let prompt_result = pmcp::server::PromptHandler::handle(
        &*cm_prompt,
        std::collections::HashMap::new(),
        extra.clone(),
    )
    .await?;
    let body = match &prompt_result.messages[0].content {
        pmcp::types::Content::Text { text } => text.clone(),
        _ => panic!("expected text content in prompt"),
    };
    assert!(
        body.contains("CREATE TABLE employees"),
        "prompt body must include the schema (it does not — long-tail bootstrap will fail to inform the LLM)"
    );
    ok("prompt body includes `CREATE TABLE employees` (LLM gets long-tail surface in one fetch)");

    header("Step G · User-facing surface size");
    // Count meaningful (non-comment, non-blank) LoC in `run_user_facing_main`.
    let src = include_str!("./main.rs");
    let in_fn = src
        .lines()
        .skip_while(|l| !l.contains("async fn run_user_facing_main"))
        .take_while(|l| !l.starts_with("// =====")); // until next section banner
    let meaningful: usize = in_fn
        .clone()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .count();
    println!("  run_user_facing_main meaningful LoC = {meaningful}");
    assert!(
        meaningful <= 15,
        "user-facing surface grew to {meaningful} lines — toolkit DX is failing the '~15 lines' bar"
    );
    ok(&format!(
        "user-facing surface is {meaningful} meaningful lines (target: ≤15)"
    ));

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  VERDICT: ✓ VALIDATED");
    println!();
    println!("  The smallest viable toolkit slice ({} LoC in `mod toolkit`,", count_lines_in_module(src, "mod toolkit"));
    println!("  {} LoC in `mod sqlite_backend`) is sufficient to drive a real", count_lines_in_module(src, "mod sqlite_backend"));
    println!("  PMCP server end-to-end from a tiny config.toml + schema.sql. The");
    println!("  user-facing surface is ~12 lines with NO per-tool Rust code.");
    println!();
    println!("  Confirms spike 003's recommendation: lift `mcp-server-common`-shape");
    println!("  to `crates/pmcp-server-toolkit/` plus a `crates/pmcp-toolkit-sqlite/`");
    println!("  (or feature-gated reference connector). cargo-pmcp scaffolding sits");
    println!("  on top as the developer-facing entry point.");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

/// Approximate LoC counter for a `mod X { ... }` block. Used only for the
/// verdict report; not load-bearing.
fn count_lines_in_module(src: &str, module_decl: &str) -> usize {
    let lines: Vec<&str> = src.lines().collect();
    let mut start = None;
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with(module_decl) {
            start = Some(i);
            break;
        }
    }
    let Some(start) = start else {
        return 0;
    };
    let mut depth = 0_i32;
    let mut count = 0;
    for l in lines.iter().skip(start) {
        count += 1;
        depth += l.matches('{').count() as i32;
        depth -= l.matches('}').count() as i32;
        if depth == 0 && count > 1 {
            return count;
        }
    }
    count
}
