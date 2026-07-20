//! `capture_contract` — shared introspect + extract-subset tool for the
//! pmcp.run package-capture GraphQL contract (Task 2 of the package-capture
//! contract seam).
//!
//! Two modes:
//! - `capture_contract emit` — introspects the live **source data API**
//!   (the amplifyData API behind `api_url`, NOT the merged
//!   `PMCP_RUN_GRAPHQL_URL`/`DEFAULT_GRAPHQL_URL`), extracts just the
//!   capture-subset (`submitPackageCapture` + `getPackageCaptureStatus` and
//!   their return types), and prints the resulting SDL to stdout.
//! - `capture_contract check <path>` — does the same live introspection,
//!   then diffs the extracted SDL against the vendored contract file at
//!   `<path>` (after stripping its provenance header comment), exiting
//!   non-zero on drift.
//!
//! Deliberately kept as a `cargo-pmcp`-subcommand-free internal binary
//! (`src/bin/capture_contract.rs`, auto-discovered by Cargo — no `[[bin]]`
//! entry needed) so CI can invoke it directly without adding a public
//! subcommand to the main CLI.
//!
//! ## Why this does NOT call `auth::get_credentials()` directly
//!
//! The brief for this tool describes reusing
//! `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs`'s `get_credentials()`
//! M2M (`client_credentials`) path. In practice this isn't reachable from a
//! `src/bin/*.rs` binary: those link only the `cargo_pmcp` **library** crate
//! (see `src/lib.rs`), and `auth.rs` lives in the bin-only module tree
//! declared in `src/main.rs`. Worse, `get_credentials()`'s config-discovery
//! fallback (`configured_api_base_url()`) transitively needs
//! `commands::configure::resolver`, which the Phase 77 HIGH-1 fix
//! (`.planning/phases/77-cargo-pmcp-configure-commands/77-03-PLAN.md`,
//! `commands/configure/name_validation.rs` doc comment) deliberately keeps
//! bin-only — `commands::*` is never mounted into the lib surface, and
//! integration tests that need real CLI behavior invoke the built binary as
//! a subprocess instead. Pulling that whole tree into the lib crate for this
//! one M2M call would violate that documented boundary.
//!
//! So this file reimplements just the M2M branch directly against
//! `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` (mirroring
//! `auth::get_credentials_via_client_credentials`'s discovery + token
//! exchange), self-contained, with no dependency on the `cargo_pmcp` lib
//! surface at all.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

/// `capture_contract <command>`
#[derive(Debug, Parser)]
#[command(name = "capture_contract")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Introspect the live source data API and print the capture-subset SDL.
    Emit,
    /// Introspect live, extract the capture-subset SDL, and diff it against
    /// the SDL file at `<path>`. Exits non-zero on drift.
    Check {
        /// Path to the vendored contract SDL (e.g. `contracts/pmcp-run/capture-v1.graphql`).
        path: PathBuf,
    },
}

/// The standard full `IntrospectionQuery` document (introspects `__schema`:
/// query/mutation root type names, all `types[]` with fields/args/enum
/// values, and directives). Sent as-is to the source data API.
const INTROSPECTION_QUERY: &str = r#"
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      ...FullType
    }
    directives {
      name
      description
      locations
      args {
        ...InputValue
      }
    }
  }
}

fragment FullType on __Type {
  kind
  name
  description
  fields(includeDeprecated: true) {
    name
    description
    args {
      ...InputValue
    }
    type {
      ...TypeRef
    }
    isDeprecated
    deprecationReason
  }
  inputFields {
    ...InputValue
  }
  interfaces {
    ...TypeRef
  }
  enumValues(includeDeprecated: true) {
    name
    description
    isDeprecated
    deprecationReason
  }
  possibleTypes {
    ...TypeRef
  }
}

fragment InputValue on __InputValue {
  name
  description
  type { ...TypeRef }
  defaultValue
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
              }
            }
          }
        }
      }
    }
  }
}
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Emit => {
            let sdl = introspect_and_extract().await?;
            print!("{sdl}");
        },
        Command::Check { path } => {
            let live_sdl = introspect_and_extract().await?;
            let vendored_raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let vendored_body = strip_provenance_header(&vendored_raw);

            if normalize_body(&vendored_body) == normalize_body(&live_sdl) {
                println!("OK: {} matches the live source schema", path.display());
            } else {
                eprintln!(
                    "DRIFT DETECTED: {} no longer matches the live source schema",
                    path.display()
                );
                print_line_diff(&vendored_body, &live_sdl);
                std::process::exit(1);
            }
        },
    }
    Ok(())
}

// ============================================================================
// Live path: auth + introspection + endpoint guard + extraction
// ============================================================================

/// Full live flow: resolve the source endpoint, get an M2M token, introspect,
/// assert it's the right endpoint, and extract the capture-subset SDL.
async fn introspect_and_extract() -> Result<String> {
    let source_url = resolve_source_url()?;
    let token = get_m2m_token().await?;
    let schema_data = introspect_source_schema(&source_url, &token).await?;
    assert_capture_ops_present(&schema_data, &source_url)?;
    extract_capture_sdl(&schema_data)
}

/// Read `PMCP_SOURCE_GRAPHQL_URL` — the source data API (amplifyData) behind
/// `api_url`, distinct from the merged `PMCP_RUN_GRAPHQL_URL`/`DEFAULT_GRAPHQL_URL`.
fn resolve_source_url() -> Result<String> {
    std::env::var("PMCP_SOURCE_GRAPHQL_URL").context(
        "PMCP_SOURCE_GRAPHQL_URL is not set — required: the source data API (amplifyData) \
         endpoint behind api_url, distinct from the merged DEFAULT_GRAPHQL_URL",
    )
}

/// Fetch an M2M access token via the OAuth2 `client_credentials` grant.
/// Mirrors `auth::get_credentials_via_client_credentials` (see module docs
/// above for why this isn't a direct call).
async fn get_m2m_token() -> Result<String> {
    let client_id = std::env::var("PMCP_CLIENT_ID")
        .context("PMCP_CLIENT_ID is not set — required for M2M auth (client_credentials)")?;
    let client_secret = std::env::var("PMCP_CLIENT_SECRET")
        .context("PMCP_CLIENT_SECRET is not set — required for M2M auth (client_credentials)")?;

    let cognito_domain = resolve_cognito_domain().await?;
    let token_url = format!("https://{cognito_domain}/oauth2/token");

    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .basic_auth(&client_id, Some(&client_secret))
        .form(&[("grant_type", "client_credentials")])
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("failed to request access token via client_credentials")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("client_credentials token request failed: {status}\n{body}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let token: TokenResponse = response
        .json()
        .await
        .context("failed to parse client_credentials token response")?;
    Ok(token.access_token)
}

/// Resolve the Cognito auth domain: `PMCP_RUN_COGNITO_DOMAIN` override, else
/// discovery via `.well-known/pmcp-config` at `PMCP_API_URL`/`PMCP_RUN_API_URL`
/// (default `https://api.pmcp.run`).
async fn resolve_cognito_domain() -> Result<String> {
    if let Ok(domain) = std::env::var("PMCP_RUN_COGNITO_DOMAIN") {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            return Ok(strip_scheme(trimmed));
        }
    }

    let api_url = std::env::var("PMCP_API_URL")
        .or_else(|_| std::env::var("PMCP_RUN_API_URL"))
        .unwrap_or_else(|_| "https://api.pmcp.run".to_string());
    let discovery_url = format!("{}/.well-known/pmcp-config", api_url.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let response = client
        .get(&discovery_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("failed to reach discovery endpoint {discovery_url}"))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "discovery endpoint {discovery_url} returned status {}",
            response.status()
        );
    }

    #[derive(serde::Deserialize)]
    struct Discovery {
        cognito_domain: String,
    }

    let discovered: Discovery = response
        .json()
        .await
        .context("failed to parse pmcp-config discovery response")?;
    Ok(strip_scheme(&discovered.cognito_domain))
}

/// Strip a leading `https://`/`http://` scheme and trailing `/`.
fn strip_scheme(s: &str) -> String {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s)
        .trim_end_matches('/')
        .to_string()
}

/// POST the standard introspection query to the source data API with the M2M
/// bearer token. Returns the `data` object of the GraphQL response (i.e. the
/// value carrying the `__schema` key), NOT `data.__schema` directly — matches
/// what [`extract_capture_sdl`] expects.
async fn introspect_source_schema(source_url: &str, token: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "query": INTROSPECTION_QUERY,
        "variables": {},
    });

    let response = client
        .post(source_url)
        .bearer_auth(token)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .with_context(|| format!("failed to POST introspection query to {source_url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("introspection request to {source_url} failed: {status}\n{text}");
    }

    let raw: Value = response
        .json()
        .await
        .context("failed to parse introspection response as JSON")?;

    if let Some(errors) = raw.get("errors") {
        anyhow::bail!("GraphQL errors introspecting {source_url}: {errors}");
    }

    raw.get("data")
        .cloned()
        .with_context(|| format!("introspection response from {source_url} has no `data` field"))
}

// ============================================================================
// Step 3: the right-endpoint assertion (source vs merged API)
// ============================================================================

/// Guard against introspecting the wrong (merged) endpoint: assert the
/// introspected schema actually contains both capture ops.
fn assert_capture_ops_present(schema_data: &Value, source_url: &str) -> Result<()> {
    if schema_has_capture_ops(schema_data) {
        Ok(())
    } else {
        anyhow::bail!(
            "error: introspected schema has no capture ops — wrong endpoint? expected the \
             source data API (amplifyData) behind api_url, got {source_url}"
        )
    }
}

/// Pure check: does `schema_data["__schema"]` contain both
/// `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus`?
fn schema_has_capture_ops(schema_data: &Value) -> bool {
    (|| -> Option<bool> {
        let schema = schema_data.get("__schema")?;
        let types = schema.get("types")?.as_array()?;
        let mutation_name = schema.get("mutationType")?.get("name")?.as_str()?;
        let query_name = schema.get("queryType")?.get("name")?.as_str()?;
        let mutation_type = find_type(types, mutation_name)?;
        let query_type = find_type(types, query_name)?;
        let has_submit = find_field(mutation_type, "submitPackageCapture").is_some();
        let has_status = find_field(query_type, "getPackageCaptureStatus").is_some();
        Some(has_submit && has_status)
    })()
    .unwrap_or(false)
}

// ============================================================================
// Step 2: the pure subset extractor
// ============================================================================

/// From an introspected schema (`schema_json["__schema"]`), select ONLY the
/// `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus`
/// fields, their argument types, and the two OBJECT types they return, and
/// render the result as SDL.
///
/// Pure and offline-testable — no network, no auth. `status` fields render
/// as whatever SCALAR the schema says (always `String` in practice) — this
/// function never invents an enum.
fn extract_capture_sdl(schema_json: &Value) -> Result<String> {
    let schema = schema_json
        .get("__schema")
        .context("introspection response missing __schema key")?;
    let types = schema
        .get("types")
        .and_then(Value::as_array)
        .context("__schema missing types[]")?;

    let mutation_type_name = schema
        .get("mutationType")
        .and_then(|m| m.get("name"))
        .and_then(Value::as_str)
        .context("__schema missing mutationType.name")?;
    let query_type_name = schema
        .get("queryType")
        .and_then(|q| q.get("name"))
        .and_then(Value::as_str)
        .context("__schema missing queryType.name")?;

    let mutation_type = find_type(types, mutation_type_name)
        .with_context(|| format!("Mutation type '{mutation_type_name}' not found in types[]"))?;
    let query_type = find_type(types, query_type_name)
        .with_context(|| format!("Query type '{query_type_name}' not found in types[]"))?;

    let submit_field = find_field(mutation_type, "submitPackageCapture")
        .context("submitPackageCapture field not found on the Mutation type — wrong endpoint?")?;
    let status_field = find_field(query_type, "getPackageCaptureStatus")
        .context("getPackageCaptureStatus field not found on the Query type — wrong endpoint?")?;

    let submit_type_ref = submit_field.get("type").unwrap_or(&Value::Null);
    let status_type_ref = status_field.get("type").unwrap_or(&Value::Null);

    let submit_ret_name = unwrap_named_type(submit_type_ref)
        .context("submitPackageCapture return type could not be resolved to a named type")?;
    let status_ret_name = unwrap_named_type(status_type_ref)
        .context("getPackageCaptureStatus return type could not be resolved to a named type")?;

    let submit_ret_type = find_type(types, submit_ret_name)
        .with_context(|| format!("return type '{submit_ret_name}' not found in types[]"))?;
    let status_ret_type = find_type(types, status_ret_name)
        .with_context(|| format!("return type '{status_ret_name}' not found in types[]"))?;

    let mut sdl = String::new();
    sdl.push_str(&render_object_type(submit_ret_type));
    sdl.push('\n');
    sdl.push_str(&render_object_type(status_ret_type));
    sdl.push('\n');
    sdl.push_str(&format!(
        "type {mutation_type_name} {{\n  submitPackageCapture({}): {}\n}}\n",
        render_args(submit_field),
        render_type_ref(submit_type_ref)
    ));
    sdl.push('\n');
    sdl.push_str(&format!(
        "type {query_type_name} {{\n  getPackageCaptureStatus({}): {}\n}}\n",
        render_args(status_field),
        render_type_ref(status_type_ref)
    ));

    Ok(sdl)
}

// ============================================================================
// Introspection-JSON helpers (shared by extraction + the endpoint guard)
// ============================================================================

/// Find a type by name in `__schema.types[]`.
fn find_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Value> {
    types
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
}

/// Find a field by name on a type's `fields[]`.
fn find_field<'a>(type_obj: &'a Value, field_name: &str) -> Option<&'a Value> {
    type_obj
        .get("fields")?
        .as_array()?
        .iter()
        .find(|f| f.get("name").and_then(Value::as_str) == Some(field_name))
}

/// Render a GraphQL introspection type-ref as SDL, unwrapping `NON_NULL`/
/// `LIST` nesting: `T`, `T!`, `[T]`, `[T!]!`, etc.
fn render_type_ref(t: &Value) -> String {
    match t.get("kind").and_then(Value::as_str) {
        Some("NON_NULL") => format!(
            "{}!",
            render_type_ref(t.get("ofType").unwrap_or(&Value::Null))
        ),
        Some("LIST") => format!(
            "[{}]",
            render_type_ref(t.get("ofType").unwrap_or(&Value::Null))
        ),
        _ => t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_string(),
    }
}

/// Unwrap `NON_NULL`/`LIST` nesting down to the innermost named type (used to
/// resolve a field's return OBJECT type for lookup in `types[]`).
fn unwrap_named_type(t: &Value) -> Option<&str> {
    match t.get("kind").and_then(Value::as_str) {
        Some("NON_NULL") | Some("LIST") => unwrap_named_type(t.get("ofType")?),
        _ => t.get("name").and_then(Value::as_str),
    }
}

/// Render a field's `args[]` as a comma-separated SDL argument list.
fn render_args(field: &Value) -> String {
    let Some(args) = field.get("args").and_then(Value::as_array) else {
        return String::new();
    };
    args.iter()
        .map(|a| {
            let name = a.get("name").and_then(Value::as_str).unwrap_or("");
            let ty = render_type_ref(a.get("type").unwrap_or(&Value::Null));
            format!("{name}: {ty}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render an OBJECT type's `fields[]` (deterministic, introspection order) as
/// a `type Name { ... }` SDL block.
fn render_object_type(type_obj: &Value) -> String {
    let name = type_obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let mut out = format!("type {name} {{\n");
    if let Some(fields) = type_obj.get("fields").and_then(Value::as_array) {
        for f in fields {
            let fname = f.get("name").and_then(Value::as_str).unwrap_or("");
            let fty = render_type_ref(f.get("type").unwrap_or(&Value::Null));
            out.push_str(&format!("  {fname}: {fty}\n"));
        }
    }
    out.push_str("}\n");
    out
}

// ============================================================================
// `check` mode helpers
// ============================================================================

/// Strip a vendored contract file's leading provenance header: lines
/// starting with `#` and blank lines before the first SDL token.
fn strip_provenance_header(content: &str) -> String {
    content
        .lines()
        .skip_while(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalize an SDL body for comparison: trim trailing whitespace per line,
/// trim leading/trailing blank lines. Still a plain string/line comparison —
/// no GraphQL parsing.
fn normalize_body(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Print a line-level unified-ish diff between the vendored and live SDL
/// bodies (`-` vendored-only/changed, `+` live-only/changed).
fn print_line_diff(vendored: &str, live: &str) {
    let vendored_norm = normalize_body(vendored);
    let live_norm = normalize_body(live);
    let vendored_lines: Vec<&str> = vendored_norm.lines().collect();
    let live_lines: Vec<&str> = live_norm.lines().collect();
    let max = vendored_lines.len().max(live_lines.len());
    for i in 0..max {
        let v = vendored_lines.get(i).copied();
        let l = live_lines.get(i).copied();
        match (v, l) {
            (Some(a), Some(b)) if a == b => {},
            (Some(a), Some(b)) => {
                println!("- {a}");
                println!("+ {b}");
            },
            (Some(a), None) => println!("- {a}"),
            (None, Some(b)) => println!("+ {b}"),
            (None, None) => {},
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned `__schema` introspection fixture (the shape
    /// `extract_capture_sdl` expects: a `data`-object-like value carrying a
    /// top-level `__schema` key) modeled on the CLI's real
    /// `submitPackageCapture`/`getPackageCaptureStatus` operations
    /// (`cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs`).
    fn capture_schema_fixture() -> Value {
        serde_json::json!({
            "__schema": {
                "queryType": { "name": "Query" },
                "mutationType": { "name": "Mutation" },
                "types": [
                    {
                        "kind": "OBJECT",
                        "name": "Mutation",
                        "fields": [
                            {
                                "name": "submitPackageCapture",
                                "args": [
                                    { "name": "rootComponentType", "type": non_null(scalar("String")) },
                                    { "name": "rootComponentId", "type": non_null(scalar("String")) },
                                    { "name": "version", "type": non_null(scalar("String")) },
                                    { "name": "bump", "type": scalar("String") },
                                ],
                                "type": non_null(object_ref("CaptureInfo")),
                            }
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "Query",
                        "fields": [
                            {
                                "name": "getPackageCaptureStatus",
                                "args": [
                                    { "name": "id", "type": non_null(scalar("ID")) },
                                ],
                                "type": object_ref("CaptureStatus"),
                            }
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "CaptureInfo",
                        "fields": [
                            { "name": "captureId", "args": [], "type": non_null(scalar("String")) },
                            { "name": "status", "args": [], "type": non_null(scalar("String")) },
                            { "name": "createdAt", "args": [], "type": non_null(scalar("String")) },
                        ],
                    },
                    {
                        "kind": "OBJECT",
                        "name": "CaptureStatus",
                        "fields": [
                            { "name": "id", "args": [], "type": non_null(scalar("ID")) },
                            { "name": "status", "args": [], "type": scalar("String") },
                            { "name": "message", "args": [], "type": scalar("String") },
                            { "name": "errorCode", "args": [], "type": scalar("String") },
                            { "name": "divergentComponents", "args": [], "type": list(non_null(scalar("String"))) },
                            { "name": "manifestDigest", "args": [], "type": scalar("String") },
                            { "name": "updatedAt", "args": [], "type": scalar("String") },
                        ],
                    },
                ],
            }
        })
    }

    fn scalar(name: &str) -> Value {
        serde_json::json!({ "kind": "SCALAR", "name": name, "ofType": null })
    }

    fn object_ref(name: &str) -> Value {
        serde_json::json!({ "kind": "OBJECT", "name": name, "ofType": null })
    }

    fn non_null(inner: Value) -> Value {
        serde_json::json!({ "kind": "NON_NULL", "name": null, "ofType": inner })
    }

    fn list(inner: Value) -> Value {
        serde_json::json!({ "kind": "LIST", "name": null, "ofType": inner })
    }

    #[test]
    fn extract_capture_sdl_renders_expected_shape() {
        let sdl =
            extract_capture_sdl(&capture_schema_fixture()).expect("extraction should succeed");

        // The two ops, rendered on the Mutation/Query root types.
        assert!(
            sdl.contains("type Mutation {"),
            "missing Mutation block:\n{sdl}"
        );
        assert!(
            sdl.contains("submitPackageCapture("),
            "missing submitPackageCapture:\n{sdl}"
        );
        assert!(sdl.contains("type Query {"), "missing Query block:\n{sdl}");
        assert!(
            sdl.contains("getPackageCaptureStatus(id: ID!)"),
            "missing getPackageCaptureStatus(id: ID!):\n{sdl}"
        );

        // status is ALWAYS String, never an invented enum.
        assert!(
            sdl.contains("status: String") || sdl.contains("status: String!"),
            "status must render as String (never an enum):\n{sdl}"
        );
        assert!(
            !sdl.to_lowercase().contains("enum"),
            "must not invent an enum type:\n{sdl}"
        );

        // Submit return type (CaptureInfo-equivalent) carries captureId + createdAt.
        assert!(sdl.contains("captureId"), "missing captureId:\n{sdl}");
        assert!(sdl.contains("createdAt"), "missing createdAt:\n{sdl}");

        // Status return type (CaptureStatus-equivalent) carries id + updatedAt.
        assert!(
            sdl.contains("type CaptureStatus {"),
            "missing CaptureStatus type:\n{sdl}"
        );
        assert!(sdl.contains("updatedAt"), "missing updatedAt:\n{sdl}");
    }

    #[test]
    fn extract_capture_sdl_preserves_captureid_vs_id_distinction() {
        // captureId (submit return) and id (status return) must not collapse
        // into the same field name — regression guard for the extractor
        // accidentally merging the two return types.
        let sdl = extract_capture_sdl(&capture_schema_fixture()).unwrap();
        assert!(sdl.contains("captureId: String!"), "got:\n{sdl}");
        assert!(sdl.contains("id: ID!"), "got:\n{sdl}");
    }

    #[test]
    fn assert_capture_ops_present_succeeds_when_ops_present() {
        assert!(assert_capture_ops_present(
            &capture_schema_fixture(),
            "https://source.example/graphql"
        )
        .is_ok());
    }

    #[test]
    fn assert_capture_ops_present_fails_on_wrong_endpoint() {
        // A schema that simply lacks the two capture ops (e.g. introspecting
        // the merged/default API instead of the source data API).
        let wrong_endpoint_schema = serde_json::json!({
            "__schema": {
                "queryType": { "name": "Query" },
                "mutationType": { "name": "Mutation" },
                "types": [
                    { "kind": "OBJECT", "name": "Mutation", "fields": [] },
                    { "kind": "OBJECT", "name": "Query", "fields": [] },
                ],
            }
        });

        let err =
            assert_capture_ops_present(&wrong_endpoint_schema, "https://merged.example/graphql")
                .expect_err("must fail when capture ops are absent");
        let msg = err.to_string();
        assert!(msg.contains("wrong endpoint?"), "got: {msg}");
        assert!(msg.contains("https://merged.example/graphql"), "got: {msg}");
    }

    #[test]
    fn strip_provenance_header_skips_comments_and_blank_lines() {
        let content = "# header line 1\n# header line 2\n\ntype Mutation {\n  foo: String\n}\n";
        let body = strip_provenance_header(content);
        assert_eq!(body, "type Mutation {\n  foo: String\n}");
    }
}
