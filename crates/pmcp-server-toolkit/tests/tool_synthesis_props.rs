//! TEST-02 property test for the [[tools]] → ToolInfo synthesizer.
//!
//! Invariant: for any ServerConfig with N [[tools]] entries,
//! [`synthesize_from_config`] returns Ok(Vec) with exactly N tuples, each with
//! a non-empty input_schema (object type, per RESEARCH §Risks #2).
//!
//! Additional invariants:
//! - Every synthesized handler's `metadata()` returns `Some(ToolInfo)` whose
//!   `name` equals the tuple's name (Phase 82 `tool_arc` fallback invariant).

use pmcp_server_toolkit::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
use pmcp_server_toolkit::tools::synthesize_from_config;
use proptest::prelude::*;

fn arb_param() -> impl Strategy<Value = ParamDecl> {
    (
        "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        prop_oneof!["string", "integer", "boolean", "number"],
        any::<bool>(),
    )
        .prop_map(|(name, ty, required)| ParamDecl {
            name,
            param_type: Some(ty.to_string()),
            description: Some(String::new()),
            required,
            ..Default::default()
        })
}

fn arb_tool() -> impl Strategy<Value = ToolDecl> {
    (
        "[a-zA-Z_][a-zA-Z0-9_]{0,31}",
        ".{0,100}",
        proptest::collection::vec(arb_param(), 0..5),
    )
        .prop_map(|(name, description, parameters)| ToolDecl {
            name,
            description: Some(description),
            parameters,
            annotations: None,
            ..Default::default()
        })
}

fn cfg(tools: Vec<ToolDecl>) -> ServerConfig {
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

proptest! {
    /// TKIT-07 invariant: tools.len() == synthesize_from_config(cfg).unwrap().len()
    #[test]
    fn tools_count_preserved(tools in proptest::collection::vec(arb_tool(), 0..10)) {
        let config = cfg(tools.clone());
        let out = synthesize_from_config(&config).expect("synthesize");
        prop_assert_eq!(out.len(), tools.len());
    }

    /// TKIT-07 invariant: every synthesized ToolInfo has a non-empty input_schema
    /// (object type — RESEARCH §Risks #2).
    #[test]
    fn every_tool_has_object_schema(tools in proptest::collection::vec(arb_tool(), 1..10)) {
        let config = cfg(tools);
        let out = synthesize_from_config(&config).expect("synthesize");
        for (_, info, _) in &out {
            prop_assert_eq!(
                info.input_schema["type"].as_str(),
                Some("object"),
                "input_schema must be a JSON Schema object — empty schema would be {{}} fallback per RESEARCH §Risks #2"
            );
        }
    }

    /// TKIT-07 invariant: handler.metadata() returns Some(ToolInfo) (RESEARCH §Risks #2).
    #[test]
    fn handler_metadata_always_some(tools in proptest::collection::vec(arb_tool(), 1..5)) {
        let config = cfg(tools);
        let out = synthesize_from_config(&config).expect("synthesize");
        for (name, _, handler) in &out {
            let md = handler.metadata();
            prop_assert!(md.is_some(), "RESEARCH §Risks #2: handler.metadata() must NEVER return None");
            prop_assert_eq!(md.unwrap().name, name.clone());
        }
    }
}
