// Net-new code for Phase 90 OAPI-04 / OAPI-02a (D-03 — spec OPTIONAL at runtime).
// BODY lifted from the pmcp-run OpenAPI reference
// (`mcp-openapi-server-core::schema::parser`): the openapiv3 parse + serde_yaml
// fallback + the per-location parameter extraction. SHAPE adapted to the
// toolkit-owned `Operation` model (the authoritative request type, re-exported
// from `http::mod`).

//! OpenAPI schema parser — the AUTHORITATIVE home of [`Operation`] (OAPI-04).
//!
//! Parses an OpenAPI 3.0/3.1 document (JSON **or** YAML) into an indexed
//! [`OpenApiSchema`] whose [`Operation`] values the single-call synthesizer
//! (Plan 03) and the code-mode executor (Plan 04/05) consume. The parser is the
//! producer of [`Operation`], so the canonical struct lives HERE and is
//! re-exported from [`crate::http`] (mod.rs) — the type path
//! `crate::http::Operation` stays stable across every plan (Codex MEDIUM: one
//! home from day one).
//!
//! # Runtime-optional (D-03)
//!
//! A spec is OPTIONAL at runtime. [`OpenApiSchema::parse`] is never called unless
//! the operator supplies a `--spec` document; the binary threads the result as an
//! `Option<OpenApiSchema>`, and a curated-only server (single-call `[[tools]]`
//! with explicit `path`/`method`) boots with `None`. Contrast the SQL `--schema`
//! input which is effectively required. The spec, when present, surfaces two
//! ways: (a) verbatim spec text for the code-mode `api_schema` resource, and
//! (b) parsed [`Operation`] values for richer tool synthesis.

// Why: HTTP method names ("GET", "POST") and product nouns ("OpenAPI") are
// proper nouns / acronyms clippy::doc_markdown otherwise flags for back-ticks.
#![allow(clippy::doc_markdown)]

use std::collections::HashMap;
use std::path::Path;

use openapiv3::{OpenAPI, ReferenceOr};
use serde::{Deserialize, Serialize};

use super::HttpConnectorError;

/// An extracted REST operation backed by an OpenAPI definition.
///
/// The AUTHORITATIVE request model the [`crate::http::HttpConnector::execute`]
/// signature names (re-exported from [`crate::http`]). Plan 01 defined a minimal
/// shape; Plan 03 makes this the canonical home and populates these values from
/// an `openapiv3` parse. The shape mirrors the pmcp-run reference
/// `mcp-openapi-server-core::schema::Operation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// HTTP method (`GET`, `POST`, ...).
    pub method: String,

    /// Path template, e.g. `"/users/{id}"`.
    pub path: String,

    /// Input parameters (path / query / header).
    #[serde(default)]
    pub parameters: Vec<Parameter>,

    /// Whether this operation expects a request body.
    #[serde(default)]
    pub has_request_body: bool,

    /// Per-tool base-URL override (D-06 / Codex MEDIUM). When `Some`, this
    /// operation targets the given host instead of the configured `[backend]`
    /// `base_url`. Carried so the synthesizer NEVER silently drops a per-tool
    /// `base_url`; `None` means inherit the connector's configured base.
    #[serde(default)]
    pub base_url: Option<String>,
}

impl Operation {
    /// Path parameters (the `{...}` segments of [`Operation::path`]).
    #[must_use]
    pub fn path_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Path)
            .collect()
    }

    /// Query parameters.
    #[must_use]
    pub fn query_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Query)
            .collect()
    }

    /// Header parameters.
    #[must_use]
    pub fn header_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Header)
            .collect()
    }
}

/// A single OpenAPI operation parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name (matches the `{name}` placeholder for path params).
    pub name: String,

    /// Where the parameter is carried in the request.
    pub location: ParameterLocation,

    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
}

impl Parameter {
    /// Construct a parameter (test/parser convenience).
    #[must_use]
    pub fn new(name: impl Into<String>, location: ParameterLocation, required: bool) -> Self {
        Self {
            name: name.into(),
            location,
            required,
        }
    }
}

/// Where an [`Operation`] parameter is carried in the outgoing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    /// Substituted into the path template (`/users/{id}`).
    Path,
    /// Appended to the query string.
    Query,
    /// Sent as a request header.
    Header,
}

/// A parsed OpenAPI document with its [`Operation`] values indexed by
/// `(path, METHOD)` (OAPI-04 / D-03).
///
/// Runtime-OPTIONAL: the binary holds an `Option<OpenApiSchema>` and only parses
/// when the operator supplies a spec. Retains the raw spec text so the code-mode
/// `api_schema` resource (D-03 surface (a)) can serve it verbatim.
#[derive(Debug, Clone)]
pub struct OpenApiSchema {
    /// Raw spec text, retained verbatim for the code-mode `api_schema` resource.
    spec_text: String,

    /// Extracted operations in document order.
    operations: Vec<Operation>,

    /// `(path, METHOD)` → index into [`Self::operations`].
    by_path: HashMap<(String, String), usize>,
}

impl OpenApiSchema {
    /// Parse an OpenAPI spec from JSON, falling back to YAML.
    ///
    /// Tries `serde_json` first (the common machine-emitted shape), then
    /// `serde_yaml`. The retained spec text is `text` verbatim so the
    /// `api_schema` resource serves exactly what the operator supplied.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when the text is neither valid
    /// OpenAPI JSON nor YAML. The error message carries a static reason only —
    /// it does NOT echo the (admin-authored) spec body (T-90-03-03 discipline).
    pub fn parse(text: &str) -> Result<Self, HttpConnectorError> {
        let spec: OpenAPI = serde_json::from_str(text)
            .or_else(|_| serde_yaml::from_str(text))
            .map_err(|_| {
                HttpConnectorError::Backend(
                    "OpenAPI spec is not valid JSON or YAML".to_string(),
                )
            })?;
        Self::from_spec(spec, text.to_string())
    }

    /// Read and parse an OpenAPI spec from a file path.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when the file cannot be read or
    /// the contents do not parse. The error message carries a static reason and
    /// never echoes the file path or spec body (T-90-03-03 discipline).
    pub fn parse_path(path: &Path) -> Result<Self, HttpConnectorError> {
        let text = std::fs::read_to_string(path)
            .map_err(|_| HttpConnectorError::Backend("could not read OpenAPI spec file".to_string()))?;
        Self::parse(&text)
    }

    /// Build the indexed schema from an already-parsed `openapiv3` document.
    fn from_spec(spec: OpenAPI, spec_text: String) -> Result<Self, HttpConnectorError> {
        let mut operations = Vec::new();
        let mut by_path = HashMap::new();

        for (path, path_item) in &spec.paths.paths {
            let item = match path_item {
                ReferenceOr::Item(item) => item,
                // $ref path items are skipped (reference resolution not required
                // for the single-call surface — admin-authored specs inline).
                ReferenceOr::Reference { .. } => continue,
            };

            let path_level: Vec<Parameter> = item
                .parameters
                .iter()
                .filter_map(convert_parameter)
                .collect();

            let methods = [
                ("GET", &item.get),
                ("POST", &item.post),
                ("PUT", &item.put),
                ("PATCH", &item.patch),
                ("DELETE", &item.delete),
                ("HEAD", &item.head),
                ("OPTIONS", &item.options),
            ];

            for (method, op_opt) in methods {
                if let Some(op) = op_opt {
                    let operation = extract_operation(path, method, op, &path_level);
                    let idx = operations.len();
                    by_path.insert((path.clone(), method.to_string()), idx);
                    operations.push(operation);
                }
            }
        }

        Ok(Self {
            spec_text,
            operations,
            by_path,
        })
    }

    /// All extracted operations, in document order.
    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Look up an operation by path template and HTTP method (case-insensitive
    /// on the method).
    #[must_use]
    pub fn operation_for(&self, path: &str, method: &str) -> Option<&Operation> {
        self.by_path
            .get(&(path.to_string(), method.to_uppercase()))
            .and_then(|&idx| self.operations.get(idx))
    }

    /// The raw spec text, for the code-mode `api_schema` resource (D-03 (a)).
    #[must_use]
    pub fn spec_text(&self) -> &str {
        &self.spec_text
    }
}

/// Merge path-level and operation-level parameters (operation-level wins on a
/// name collision) into the toolkit [`Operation`] model.
fn extract_operation(
    path: &str,
    method: &str,
    op: &openapiv3::Operation,
    path_level: &[Parameter],
) -> Operation {
    let mut parameters: Vec<Parameter> = path_level.to_vec();
    for param_ref in &op.parameters {
        if let Some(p) = convert_parameter(param_ref) {
            if let Some(idx) = parameters.iter().position(|x| x.name == p.name) {
                parameters[idx] = p;
            } else {
                parameters.push(p);
            }
        }
    }

    Operation {
        method: method.to_string(),
        path: path.to_string(),
        parameters,
        has_request_body: op.request_body.is_some(),
        base_url: None,
    }
}

/// Convert an `openapiv3` parameter into the toolkit [`Parameter`] model.
///
/// Cookie parameters and unresolved `$ref` parameters are dropped (the
/// single-call surface carries path / query / header only); path parameters are
/// always required.
fn convert_parameter(param_ref: &ReferenceOr<openapiv3::Parameter>) -> Option<Parameter> {
    let param = match param_ref {
        ReferenceOr::Item(p) => p,
        ReferenceOr::Reference { .. } => return None,
    };
    match param {
        openapiv3::Parameter::Query { parameter_data, .. } => Some(Parameter::new(
            parameter_data.name.clone(),
            ParameterLocation::Query,
            parameter_data.required,
        )),
        openapiv3::Parameter::Path { parameter_data, .. } => Some(Parameter::new(
            parameter_data.name.clone(),
            ParameterLocation::Path,
            true,
        )),
        openapiv3::Parameter::Header { parameter_data, .. } => Some(Parameter::new(
            parameter_data.name.clone(),
            ParameterLocation::Header,
            parameter_data.required,
        )),
        openapiv3::Parameter::Cookie { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"
    {
        "openapi": "3.0.0",
        "info": { "title": "Test API", "version": "1.0.0" },
        "paths": {
            "/users/{id}": {
                "get": {
                    "operationId": "getUser",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true,
                          "schema": { "type": "string" } },
                        { "name": "verbose", "in": "query", "required": false,
                          "schema": { "type": "boolean" } }
                    ],
                    "responses": { "200": { "description": "OK" } }
                }
            }
        }
    }
    "#;

    const SAMPLE_YAML: &str = r#"
openapi: 3.0.0
info:
  title: Test API
  version: 1.0.0
paths:
  /users/{id}:
    get:
      operationId: getUser
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: verbose
          in: query
          required: false
          schema:
            type: boolean
      responses:
        '200':
          description: OK
"#;

    fn assert_get_user(schema: &OpenApiSchema) {
        let op = schema
            .operation_for("/users/{id}", "GET")
            .expect("getUser operation present");
        assert_eq!(op.method, "GET");
        assert_eq!(op.path, "/users/{id}");
        let path_params: Vec<&str> =
            op.path_parameters().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(path_params, vec!["id"]);
        let query_params: Vec<&str> = op
            .query_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(query_params, vec!["verbose"]);
    }

    #[test]
    fn schema_parse_json_extracts_operation_and_path_params() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        assert_get_user(&schema);
        assert_eq!(schema.operations().len(), 1);
    }

    #[test]
    fn schema_parse_yaml_matches_json() {
        let schema = OpenApiSchema::parse(SAMPLE_YAML).expect("parse YAML");
        assert_get_user(&schema);
    }

    #[test]
    fn schema_parse_retains_spec_text_for_resource() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        // D-03 surface (a): the raw text is served verbatim by api_schema.
        assert_eq!(schema.spec_text(), SAMPLE_JSON);
    }

    #[test]
    fn schema_parse_method_case_insensitive_lookup() {
        let schema = OpenApiSchema::parse(SAMPLE_JSON).expect("parse JSON");
        assert!(schema.operation_for("/users/{id}", "get").is_some());
        assert!(schema.operation_for("/users/{id}", "GET").is_some());
        assert!(schema.operation_for("/users/{id}", "POST").is_none());
    }

    #[test]
    fn schema_parse_malformed_returns_typed_error_no_panic() {
        let err = OpenApiSchema::parse("this is neither json nor yaml: [unclosed").unwrap_err();
        // Typed error, no panic.
        assert!(matches!(err, HttpConnectorError::Backend(_)));
    }

    /// T-90-03-03: the parser error MUST NOT echo the spec body (redaction
    /// discipline kept consistent with the connector, though specs carry no
    /// creds).
    #[test]
    fn test_schema_parse_error_display_no_secret() {
        let secret_marker = "SUPER_SECRET_TOKEN_abc123";
        let bad_spec = format!("not-a-spec {secret_marker} [");
        let err = OpenApiSchema::parse(&bad_spec).unwrap_err();
        let rendered = format!("{err}");
        assert!(
            !rendered.contains(secret_marker),
            "parser error must not echo the spec body; got {rendered:?}"
        );
    }
}
