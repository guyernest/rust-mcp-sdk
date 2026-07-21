//! The exact runtime GraphQL operations sent by the package-capture client
//! (`graphql.rs`'s `submit_package_capture` / `get_package_capture_status`),
//! factored into their own dependency-light leaf. Also hosts the PURE,
//! IO-free SDL-extraction helpers shared with the `capture_contract` dev
//! binary (`src/bin/capture_contract.rs`) — see the "Shared pure SDL
//! extraction" section below.
//!
//! This file exists ONLY so the offline blocking contract test
//! (`tests/package_capture_contract.rs`) can validate the real runtime
//! queries against the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`)
//! without pulling `pmcp_run`'s auth/deploy/reqwest tree into the `cargo-pmcp`
//! lib target. It is mounted into the lib target via `#[path]` in `lib.rs`
//! (mirrors the `templates_workbook_server` / `agent_run` / `package_kind`
//! narrow-leaf convention already used there) and re-exported as
//! `crate::pmcp_run_graphql` — `#[doc(hidden)]`, not a stable public API.
//!
//! `graphql.rs` re-exports these same consts via `super::graphql_contract::*`
//! so there is exactly one source of truth for both operations.

use anyhow::{Context, Result};
use serde_json::Value;

/// The exact `submitPackageCapture` operation the CLI sends. Shared with the
/// offline contract test (`tests/package_capture_contract.rs`) so the test
/// validates the real runtime query against the vendored SDL.
pub const SUBMIT_PACKAGE_CAPTURE_QUERY: &str = r#"
        mutation SubmitPackageCapture(
            $rootComponentType: String!,
            $rootComponentId: String!,
            $version: String!,
            $bump: String
        ) {
            submitPackageCapture(
                rootComponentType: $rootComponentType,
                rootComponentId: $rootComponentId,
                version: $version,
                bump: $bump
            ) {
                captureId
                status
                createdAt
            }
        }
    "#;

/// The exact `getPackageCaptureStatus` operation the CLI sends. Shared with the
/// offline contract test.
pub const GET_PACKAGE_CAPTURE_STATUS_QUERY: &str = r#"
        query GetPackageCaptureStatus($id: ID!) {
            getPackageCaptureStatus(id: $id) {
                id
                status
                message
                errorCode
                divergentComponents
                manifestDigest
                updatedAt
            }
        }
    "#;

// ============================================================================
// Shared pure SDL extraction (moved from `src/bin/capture_contract.rs`)
// ============================================================================
//
// These functions are PURE and IO-free (no network, no auth, no `main`/CLI
// parsing — that stays in the `capture_contract` bin). They are mounted here,
// in the already-dual-mounted `graphql_contract` leaf, so their unit tests
// run under the default `cargo test -p cargo-pmcp` (the lib target) even
// though the `capture_contract` binary itself is now gated behind the
// non-default `capture-contract-tool` feature and is NOT built by a default
// `cargo install cargo-pmcp`. The `capture_contract` bin imports these via
// `cargo_pmcp::pmcp_run_graphql::{...}` (it links only the `cargo_pmcp` lib
// crate, not the bin-only `deployment` module tree — see that file's module
// docs for why).
//
// `#[allow(dead_code)]` below: this same file is ALSO privately mounted (via
// `#[path]`) into the `cargo-pmcp` BIN target's own module tree
// (`deployment/targets/pmcp_run/mod.rs`), where `graphql.rs` only reaches the
// two query consts above — none of `cargo-pmcp`'s own `main()` call graph
// invokes these SDL-extraction helpers, so `cargo build -p cargo-pmcp`
// (which does NOT build the feature-gated `capture_contract` bin) would
// otherwise flag them dead in that target. They ARE live: used by this
// file's own tests (both mount points) and, in the LIB target, by the
// `capture_contract` bin via `cargo_pmcp::pmcp_run_graphql::*`.

/// From an introspected schema (`schema_json["__schema"]`), select ONLY the
/// `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus`
/// fields, their argument types, and the two OBJECT types they return, and
/// render the result as SDL.
///
/// Pure and offline-testable — no network, no auth. `status` fields render
/// as whatever SCALAR the schema says (always `String` in practice) — this
/// function never invents an enum.
#[allow(dead_code)]
pub fn extract_capture_sdl(schema_json: &Value) -> Result<String> {
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

/// Guard against introspecting the wrong (merged) endpoint: assert the
/// introspected schema actually contains both capture ops.
#[allow(dead_code)]
pub fn assert_capture_ops_present(schema_data: &Value, source_url: &str) -> Result<()> {
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
#[allow(dead_code)]
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

/// Find a type by name in `__schema.types[]`.
#[allow(dead_code)]
fn find_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Value> {
    types
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
}

/// Find a field by name on a type's `fields[]`.
#[allow(dead_code)]
fn find_field<'a>(type_obj: &'a Value, field_name: &str) -> Option<&'a Value> {
    type_obj
        .get("fields")?
        .as_array()?
        .iter()
        .find(|f| f.get("name").and_then(Value::as_str) == Some(field_name))
}

/// Render a GraphQL introspection type-ref as SDL, unwrapping `NON_NULL`/
/// `LIST` nesting: `T`, `T!`, `[T]`, `[T!]!`, etc.
#[allow(dead_code)]
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
#[allow(dead_code)]
fn unwrap_named_type(t: &Value) -> Option<&str> {
    match t.get("kind").and_then(Value::as_str) {
        Some("NON_NULL") | Some("LIST") => unwrap_named_type(t.get("ofType")?),
        _ => t.get("name").and_then(Value::as_str),
    }
}

/// Render a field's `args[]` as a comma-separated SDL argument list.
#[allow(dead_code)]
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
#[allow(dead_code)]
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

/// Strip a vendored contract file's leading provenance header: lines
/// starting with `#` and blank lines before the first SDL token.
#[allow(dead_code)]
pub fn strip_provenance_header(content: &str) -> String {
    content
        .lines()
        .skip_while(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Tests (moved from `src/bin/capture_contract.rs`)
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
