// Net-new code for Phase 83 TKIT-07.
// Lands the `[[tools]]`-config-driven synthesizer that turns config rows into
// `ToolInfo` + `Arc<dyn ToolHandler>` pairs.

//! `synthesize_from_config` — `[[tools]]` → `ToolInfo` + `Arc<dyn ToolHandler>`. Net-new for TKIT-07.
//!
//! The body of [`synthesize_from_config`] lands in Plan 83-05 Task 2 (GREEN).
//! Task 1 (RED) commits only the test scaffold against an unimplemented
//! signature so the suite goes red before any implementation is added.

use std::sync::Arc;

use pmcp::server::ToolHandler;
use pmcp::types::ToolInfo;

use crate::config::ServerConfig;
use crate::error::Result;

/// Synthesize one `ToolInfo` + handler per `[[tools]]` config entry.
///
/// Plan 05 RED: signature exists, body is `todo!()` so the unit + property
/// tests below compile but FAIL at runtime — the canonical TDD red step.
/// Plan 05 Task 2 (GREEN) replaces the body with the real implementation.
///
/// # Errors
///
/// Will return `ToolkitError::Synth(...)` for malformed tool declarations once
/// the body lands in Task 2; in the RED phase the function unconditionally
/// panics via `todo!()`.
pub fn synthesize_from_config(
    _config: &ServerConfig,
) -> Result<Vec<(String, ToolInfo, Arc<dyn ToolHandler>)>> {
    todo!("Plan 83-05 Task 2 (GREEN) implements this")
}

// -----------------------------------------------------------------------------
// Tests — Plan 05 Task 1 (RED)
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
