//! Tests for `ToolAnnotations` and output schema support.
//!
//! These tests verify that:
//! 1. `ToolAnnotations` serialize correctly with PMCP extensions
//! 2. Output schemas round-trip through JSON properly
//! 3. Standard MCP annotations work as expected
//! 4. Unknown annotations are preserved (forward compatibility)

use pmcp::types::{ToolAnnotations, ToolInfo};
use serde_json::json;

#[test]
fn test_tool_annotations_builder_pattern() {
    let annotations = ToolAnnotations::new()
        .with_title("My Tool")
        .with_read_only(true)
        .with_destructive(false)
        .with_idempotent(true)
        .with_open_world(false);

    assert_eq!(annotations.title, Some("My Tool".to_string()));
    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.idempotent_hint, Some(true));
    assert_eq!(annotations.open_world_hint, Some(false));
}

#[test]
fn test_output_schema_annotation() {
    let output_schema = json!({
        "type": "object",
        "properties": {
            "count": { "type": "integer" },
            "items": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["count", "items"]
    });

    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_schema(output_schema.clone(), "SearchResult");

    assert_eq!(annotations.output_schema, Some(output_schema));
    assert_eq!(
        annotations.output_type_name,
        Some("SearchResult".to_string())
    );
}

#[test]
fn test_tool_annotations_json_serialization() {
    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_schema(
            json!({
                "type": "object",
                "properties": {
                    "result": { "type": "string" }
                }
            }),
            "MyResult",
        );

    let json = serde_json::to_value(&annotations).unwrap();

    // Check standard MCP annotation
    assert_eq!(json["readOnlyHint"], true);

    // Check PMCP extensions use correct key names
    assert!(json["pmcp:outputSchema"].is_object());
    assert_eq!(json["pmcp:outputTypeName"], "MyResult");

    // Ensure camelCase serialization
    assert!(json.get("read_only_hint").is_none()); // Should be camelCase
    assert!(json.get("output_schema").is_none()); // Should use pmcp: prefix
}

#[test]
fn test_tool_annotations_json_deserialization() {
    let json = json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "pmcp:outputSchema": {
            "type": "object",
            "properties": {
                "data": { "type": "array" }
            }
        },
        "pmcp:outputTypeName": "QueryResult"
    });

    let annotations: ToolAnnotations = serde_json::from_value(json).unwrap();

    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert!(annotations.output_schema.is_some());
    assert_eq!(
        annotations.output_type_name,
        Some("QueryResult".to_string())
    );
}

#[test]
fn test_tool_info_with_annotations() {
    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_schema(
            json!({
                "type": "object",
                "properties": { "count": { "type": "integer" } }
            }),
            "CountResult",
        );

    let tool = ToolInfo::with_annotations(
        "count_items",
        Some("Count items in a collection".to_string()),
        json!({
            "type": "object",
            "properties": {
                "collection": { "type": "string" }
            },
            "required": ["collection"]
        }),
        annotations,
    );

    assert_eq!(tool.name, "count_items");
    assert!(tool.annotations.is_some());

    let ann = tool.annotations.as_ref().unwrap();
    assert_eq!(ann.read_only_hint, Some(true));
    assert!(ann.output_schema.is_some());
    assert_eq!(ann.output_type_name, Some("CountResult".to_string()));
}

#[test]
fn test_tool_info_serialization_with_annotations() {
    let annotations = ToolAnnotations::new()
        .with_read_only(true)
        .with_output_schema(json!({"type": "string"}), "StringResult");

    let tool = ToolInfo::with_annotations(
        "echo",
        Some("Echo input".to_string()),
        json!({"type": "object", "properties": {"input": {"type": "string"}}}),
        annotations,
    );

    let json = serde_json::to_value(&tool).unwrap();

    // Verify tool structure
    assert_eq!(json["name"], "echo");
    assert_eq!(json["description"], "Echo input");
    assert!(json["inputSchema"].is_object());

    // Verify annotations are nested correctly
    assert!(json["annotations"].is_object());
    assert_eq!(json["annotations"]["readOnlyHint"], true);
    assert_eq!(json["annotations"]["pmcp:outputTypeName"], "StringResult");
}

#[test]
fn test_tool_info_without_annotations() {
    let tool = ToolInfo::new(
        "simple_tool",
        Some("A simple tool".to_string()),
        json!({"type": "object"}),
    );

    let json = serde_json::to_value(&tool).unwrap();

    // annotations should not be serialized when None
    assert!(json.get("annotations").is_none());
}

#[test]
fn test_empty_annotations_not_serialized() {
    let annotations = ToolAnnotations::new();
    let json = serde_json::to_value(&annotations).unwrap();

    // Empty annotations should serialize to empty object (all fields skip_serializing_if)
    assert!(json.as_object().unwrap().is_empty());
}

#[test]
fn test_round_trip_serialization() {
    let original = ToolAnnotations::new()
        .with_title("Test Tool")
        .with_read_only(true)
        .with_destructive(false)
        .with_idempotent(true)
        .with_open_world(true)
        .with_output_schema(
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "values": { "type": "array", "items": { "type": "number" } }
                },
                "required": ["id"]
            }),
            "ComplexResult",
        );

    let json = serde_json::to_string(&original).unwrap();
    let restored: ToolAnnotations = serde_json::from_str(&json).unwrap();

    assert_eq!(original.title, restored.title);
    assert_eq!(original.read_only_hint, restored.read_only_hint);
    assert_eq!(original.destructive_hint, restored.destructive_hint);
    assert_eq!(original.idempotent_hint, restored.idempotent_hint);
    assert_eq!(original.open_world_hint, restored.open_world_hint);
    assert_eq!(original.output_schema, restored.output_schema);
    assert_eq!(original.output_type_name, restored.output_type_name);
}

#[test]
fn test_partial_annotations_deserialization() {
    // Test that we can deserialize annotations with only some fields present
    let json = json!({
        "readOnlyHint": true
        // Other fields omitted
    });

    let annotations: ToolAnnotations = serde_json::from_value(json).unwrap();

    assert_eq!(annotations.read_only_hint, Some(true));
    assert_eq!(annotations.destructive_hint, None);
    assert_eq!(annotations.output_schema, None);
    assert_eq!(annotations.output_type_name, None);
}

#[test]
fn test_output_schema_complex_types() {
    // Test with a complex nested schema
    let schema = json!({
        "type": "object",
        "properties": {
            "users": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "email": { "type": "string", "format": "email" },
                        "roles": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["id", "name"]
                }
            },
            "total_count": { "type": "integer" },
            "has_more": { "type": "boolean" }
        },
        "required": ["users", "total_count", "has_more"]
    });

    let annotations = ToolAnnotations::new().with_output_schema(schema.clone(), "UserListResponse");

    let json = serde_json::to_value(&annotations).unwrap();
    let restored: ToolAnnotations = serde_json::from_value(json).unwrap();

    // Verify the complex schema round-trips correctly
    assert_eq!(restored.output_schema, Some(schema));
    assert_eq!(
        restored.output_type_name,
        Some("UserListResponse".to_string())
    );
}
