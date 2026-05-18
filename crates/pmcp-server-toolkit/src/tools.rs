// Net-new code for Phase 83 TKIT-07.
// Lands the `[[tools]]`-config-driven synthesizer that turns config rows into
// `ToolInfo` + `Arc<dyn ToolHandler>` pairs.

//! `[[tools]]` → `ToolInfo` + `Arc<dyn ToolHandler>` synthesizer.
//!
//! Net-new code for Phase 83 TKIT-07. Turns curated `[[tools]]` config entries
//! into complete pmcp [`ToolInfo`] + [`Arc<dyn ToolHandler>`] pairs with zero
//! per-tool Rust handlers.
//!
//! # Invariants enforced
//!
//! - **JSON Schema object envelope.** Every synthesized [`ToolInfo`] carries an
//!   `input_schema` with `"type": "object"`, an explicit `properties` map, a
//!   `required` array, and `"additionalProperties": false`. Unknown argument
//!   keys are rejected by pmcp's request-validation path at `tools/call` time —
//!   defence-in-depth against arg-injection (threat T-83-05-02).
//! - **`handler.metadata()` returns `Some(ToolInfo)`.** Phase 82's `tool_arc`
//!   consumes `handler.metadata()` at registration; returning `None` would
//!   silently degrade the schema enforcement to "anything goes" (RESEARCH
//!   §Risks #2 — threat T-83-05-01).
//! - **Constructors, never struct-literals.** Both [`ToolInfo`] and
//!   [`ToolAnnotations`] are `#[non_exhaustive]` (PATTERNS §Pattern C). The
//!   synthesizer uses [`ToolInfo::with_annotations`] / [`ToolInfo::new`] and
//!   the [`ToolAnnotations::new()`]-then-`.with_*` fluent builder.
//! - **Cognitive complexity ≤25 per function.** Decomposed into
//!   [`build_input_schema`], [`build_param_property`], and [`build_annotations`]
//!   per Phase 75 D-03 + PATTERNS §Pattern G. No `#[allow]` annotations.

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::ToolHandler;
use pmcp::types::{ToolAnnotations, ToolInfo};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Map, Value};

use crate::config::{AnnotationsDecl, ParamDecl, ServerConfig, ToolDecl};
use crate::error::Result;

/// Type alias for one synthesized tool tuple: `(name, ToolInfo, Arc<dyn ToolHandler>)`.
///
/// Exists so [`synthesize_from_config`]'s return type does not trip
/// `clippy::type_complexity` while preserving the exact `(name, ToolInfo, Arc)`
/// shape consumers register with `pmcp::ServerBuilder::tool_arc` (PATTERNS §9).
pub type SynthesizedTool = (String, ToolInfo, Arc<dyn ToolHandler>);

/// Synthesize one `ToolInfo` + handler per `[[tools]]` config entry.
///
/// Each returned tuple is `(name, ToolInfo, Arc<dyn ToolHandler>)` and is
/// ready to feed into `pmcp::ServerBuilder::tool_arc(name, handler)`. The
/// `ToolInfo` carries the full input schema (synthesized from
/// `[[tools.parameters]]`) and `ToolAnnotations` (from `[tools.annotations]`)
/// so the builder's metadata cache will never fall back to the empty schema.
///
/// # Errors
///
/// Returns [`crate::ToolkitError::Synth`] if a tool declaration is internally
/// inconsistent. The Plan 05 GREEN body never produces this error path —
/// synthesis is total over the parsed [`ServerConfig`] surface — but the
/// `Result` return is kept for forward compatibility with Plan 06 (code-mode
/// wiring) and Phase 84 (SQL backend resolution).
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::tools::synthesize_from_config;
///
/// let cfg = ServerConfig::default();
/// let synthesized = synthesize_from_config(&cfg).unwrap();
/// assert_eq!(synthesized.len(), 0);
/// ```
pub fn synthesize_from_config(config: &ServerConfig) -> Result<Vec<SynthesizedTool>> {
    let mut out = Vec::with_capacity(config.tools.len());
    for decl in &config.tools {
        let schema = build_input_schema(&decl.parameters);
        let annotations = build_annotations(decl.annotations.as_ref());
        let info = match annotations {
            Some(ann) => {
                ToolInfo::with_annotations(decl.name.clone(), decl.description.clone(), schema, ann)
            },
            None => ToolInfo::new(decl.name.clone(), decl.description.clone(), schema),
        };
        let handler: Arc<dyn ToolHandler> = Arc::new(SynthesizedToolHandler {
            info: info.clone(),
            decl: decl.clone(),
        });
        out.push((decl.name.clone(), info, handler));
    }
    Ok(out)
}

/// Build the JSON Schema `properties` + `required` envelope from a
/// `[[tools.parameters]]` list.
///
/// Decomposed from [`synthesize_from_config`] to keep cognitive complexity ≤25
/// (Phase 75 D-03 + PATTERNS §Pattern G).
fn build_input_schema(params: &[ParamDecl]) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for p in params {
        props.insert(p.name.clone(), build_param_property(p));
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

/// Build a single JSON Schema property object from a [`ParamDecl`].
///
/// Per-parameter constraints (`minimum`, `maximum`, `maxLength`, `default`,
/// `enum`) are folded in only when present; the param's `param_type` defaults
/// to `"string"` when omitted in TOML to match JSON Schema's permissive
/// default.
fn build_param_property(p: &ParamDecl) -> Value {
    let ty = p.param_type.as_deref().unwrap_or("string");
    let mut prop = json!({ "type": ty });
    if let Some(desc) = &p.description {
        prop["description"] = Value::String(desc.clone());
    }
    if let Some(min) = p.minimum {
        prop["minimum"] = json!(min);
    }
    if let Some(max) = p.maximum {
        prop["maximum"] = json!(max);
    }
    if let Some(max_len) = p.max_length {
        prop["maxLength"] = json!(max_len);
    }
    if let Some(default) = &p.default {
        // toml::Value serializes losslessly into serde_json::Value via serde.
        if let Ok(v) = serde_json::to_value(default) {
            prop["default"] = v;
        }
    }
    if let Some(enum_vals) = &p.enum_values {
        if let Ok(v) = serde_json::to_value(enum_vals) {
            prop["enum"] = v;
        }
    }
    prop
}

/// Build [`ToolAnnotations`] from an optional `[tools.annotations]` block.
///
/// Per PATTERNS §Pattern C, the constructor + fluent builder is used (never a
/// struct literal — [`ToolAnnotations`] is `#[non_exhaustive]`). The `cost_hint`
/// field has no `ToolAnnotations` accessor and is therefore not propagated at
/// this layer; it lives on the toolkit's [`AnnotationsDecl`] and is consumed
/// by future plans that surface cost into rate-limiting policy.
fn build_annotations(decl: Option<&AnnotationsDecl>) -> Option<ToolAnnotations> {
    let d = decl?;
    let a = ToolAnnotations::new()
        .with_read_only(d.read_only_hint)
        .with_destructive(d.destructive_hint)
        .with_idempotent(d.idempotent_hint)
        .with_open_world(d.open_world_hint);
    Some(a)
}

// -----------------------------------------------------------------------------
// SynthesizedToolHandler — crate-private
// -----------------------------------------------------------------------------

/// Crate-private handler wrapping a synthesized [`ToolInfo`].
///
/// [`ToolHandler::metadata`] MUST return `Some(self.info.clone())` — Phase 82's
/// `tool_arc` consumes `handler.metadata()` at registration time; returning
/// `None` would cause the builder to fall back to an empty schema (RESEARCH
/// §Risks #2 — threat T-83-05-01). The unit + property tests in
/// [`crate::tools`] and `tests/tool_synthesis_props.rs` lock this in.
///
/// `handle()` returns an `Err` placeholder until Plan 06 (code-mode wiring) or
/// Phase 84 (SQL connector dispatch) lands. An `Err` is preferable to a `Value`
/// pretending success — MCP clients see a tool-call error rather than silent
/// "ok with empty body" (Gemini review note). The `decl` is held for later
/// plans to inspect `sql` / `ui_resource_uri` without re-walking the config.
struct SynthesizedToolHandler {
    info: ToolInfo,
    // Held for Plan 06 (code-mode validate/execute) and Phase 84 (SQL backends).
    #[allow(dead_code)]
    decl: ToolDecl,
}

#[async_trait]
impl ToolHandler for SynthesizedToolHandler {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Err(pmcp::Error::Internal(format!(
            "tool '{}' is not yet wired — Plan 06 (code-mode) or Phase 84 (SQL connector) required",
            self.info.name
        )))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(self.info.clone())
    }
}

// -----------------------------------------------------------------------------
// Tests — Plan 05 Task 1 (RED) → GREEN in Task 2
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnnotationsDecl, ParamDecl, ServerConfig, ServerSection, ToolDecl};
    use serde_json::Value;

    /// Construct a minimal `ServerConfig` that satisfies `validate()` (non-empty
    /// `name` + `version`) so the synthesizer path is the system under test —
    /// not the parser/validator from Plan 04.
    fn cfg_with_tools(tools: Vec<ToolDecl>) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "demo".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            tools,
            ..Default::default()
        }
    }

    #[test]
    fn empty_tools_returns_empty_vec() {
        let cfg = cfg_with_tools(vec![]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn one_tool_no_params_yields_object_schema() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "ping".to_string(),
            description: Some("Ping the server".to_string()),
            parameters: vec![],
            annotations: None,
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        assert_eq!(out.len(), 1);
        let (name, info, _handler) = &out[0];
        assert_eq!(name, "ping");
        assert_eq!(info.name, "ping");
        assert_eq!(info.description.as_deref(), Some("Ping the server"));
        let schema = &info.input_schema;
        assert_eq!(schema["type"], Value::String("object".to_string()));
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
    }

    #[test]
    fn required_and_optional_params_partitioned() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "search".to_string(),
            description: Some("Search".to_string()),
            parameters: vec![
                ParamDecl {
                    name: "query".to_string(),
                    param_type: Some("string".to_string()),
                    description: Some("the search query".to_string()),
                    required: true,
                    ..Default::default()
                },
                ParamDecl {
                    name: "max_results".to_string(),
                    param_type: Some("integer".to_string()),
                    description: Some("maximum result count".to_string()),
                    required: false,
                    default: Some(toml::Value::Integer(100)),
                    minimum: Some(1.0),
                    maximum: Some(1000.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        let schema = &info.input_schema;
        assert_eq!(schema["required"], serde_json::json!(["query"]));
        let props = schema["properties"].as_object().expect("object");
        assert_eq!(props["query"]["type"], "string");
        assert_eq!(props["max_results"]["type"], "integer");
        assert_eq!(props["max_results"]["minimum"], serde_json::json!(1.0));
        assert_eq!(props["max_results"]["maximum"], serde_json::json!(1000.0));
        assert_eq!(props["max_results"]["default"], serde_json::json!(100));
    }

    #[test]
    fn param_max_length_propagates() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "echo".to_string(),
            description: Some("Echo".to_string()),
            parameters: vec![ParamDecl {
                name: "text".to_string(),
                param_type: Some("string".to_string()),
                description: Some("input text".to_string()),
                required: true,
                max_length: Some(256),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        assert_eq!(
            info.input_schema["properties"]["text"]["maxLength"],
            serde_json::json!(256)
        );
    }

    #[test]
    fn annotations_round_trip_via_fluent_builder() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "destroy_all".to_string(),
            description: Some("Destroy all data (test)".to_string()),
            parameters: vec![],
            annotations: Some(AnnotationsDecl {
                read_only_hint: false,
                destructive_hint: true,
                idempotent_hint: false,
                open_world_hint: false,
                cost_hint: Some("high".to_string()),
            }),
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, info, _) = &out[0];
        let ann = info.annotations.as_ref().expect("annotations");
        assert_eq!(ann.read_only_hint, Some(false));
        assert_eq!(ann.destructive_hint, Some(true));
        assert_eq!(ann.idempotent_hint, Some(false));
        assert_eq!(ann.open_world_hint, Some(false));
    }

    #[tokio::test]
    async fn synthesized_handler_metadata_returns_some() {
        let cfg = cfg_with_tools(vec![ToolDecl {
            name: "ping".to_string(),
            description: Some("ping".to_string()),
            parameters: vec![],
            annotations: None,
            ..Default::default()
        }]);
        let out = synthesize_from_config(&cfg).expect("synthesize");
        let (_, expected_info, handler) = &out[0];
        let actual = handler.metadata();
        assert!(
            actual.is_some(),
            "RESEARCH §Risks #2 invariant: SynthesizedToolHandler::metadata() MUST return Some(ToolInfo)"
        );
        assert_eq!(actual.unwrap().name, expected_info.name);
    }
}
