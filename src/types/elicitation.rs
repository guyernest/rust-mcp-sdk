//! Spec-compliant MCP elicitation types (2025-11-25).
//!
//! Replaces the PMCP-proprietary elicitation format with the official
//! MCP specification. Two modes: form (JSON Schema-based) and URL.
//!
//! # Breaking Changes (v2.0)
//!
//! The entire elicitation API has changed:
//! - `ElicitInputRequest` -> `ElicitRequestParams`
//! - `ElicitInputResponse` -> `ElicitResult`
//! - `InputType` enum (16 variants) -> JSON Schema `requestedSchema`
//! - `ElicitInputBuilder` -> removed (construct `ElicitRequestParams` directly)
//! - Method name: `elicitation/elicitInput` -> `elicitation/create`

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Elicitation request parameters (MCP 2025-11-25).
///
/// Supports two modes:
/// - `form`: Server provides a JSON Schema subset; client renders a form
/// - `url`: Server provides a URL for out-of-band user interaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum ElicitRequestParams {
    /// Form-based elicitation with JSON Schema
    #[serde(rename = "form", rename_all = "camelCase")]
    Form {
        /// Human-readable message explaining what input is needed
        message: String,
        /// JSON Schema subset defining the requested input fields.
        /// Supports primitive types: boolean, string, number/integer, enum.
        requested_schema: Value,
    },
    /// URL-based elicitation for out-of-band interaction
    #[serde(rename = "url", rename_all = "camelCase")]
    Url {
        /// Human-readable message explaining what action is needed
        message: String,
        /// Elicitation identifier for correlation
        elicitation_id: String,
        /// URL the user should visit
        url: String,
    },
}

/// Elicitation result returned by the client (MCP 2025-11-25).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitResult {
    /// User's action on the elicitation
    pub action: ElicitAction,
    /// Form content (present when action is Accept, absent otherwise)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HashMap<String, Value>>,
}

/// Action taken by the user on an elicitation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElicitAction {
    /// User accepted and provided input
    Accept,
    /// User declined the request
    Decline,
    /// User cancelled the interaction
    Cancel,
}

/// Notification that an out-of-band elicitation has completed.
///
/// Sent when a URL-mode elicitation completes outside the normal
/// request/response cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationCompleteNotification {
    /// The elicitation ID that completed
    pub elicitation_id: String,
    /// Result of the elicitation
    pub result: ElicitResult,
}

// ========================================================================
// Backward-compatible aliases (deprecated)
// ========================================================================

/// Deprecated: Use `ElicitRequestParams` instead.
///
/// Provided for backward compatibility during the v2.0 transition.
/// This type will be removed in a future release.
#[deprecated(since = "2.0.0", note = "Use ElicitRequestParams instead")]
pub type ElicitInputRequest = ElicitRequestParams;

/// Deprecated: Use `ElicitResult` instead.
///
/// Provided for backward compatibility during the v2.0 transition.
/// This type will be removed in a future release.
#[deprecated(since = "2.0.0", note = "Use ElicitResult instead")]
pub type ElicitInputResponse = ElicitResult;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn elicit_request_form_mode_serialization() {
        let params = ElicitRequestParams::Form {
            message: "Enter your name".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["mode"], "form");
        assert_eq!(json["message"], "Enter your name");
        assert!(json["requestedSchema"]["properties"]["name"].is_object());

        let roundtrip: ElicitRequestParams = serde_json::from_value(json).unwrap();
        match roundtrip {
            ElicitRequestParams::Form { message, .. } => {
                assert_eq!(message, "Enter your name");
            },
            ElicitRequestParams::Url { .. } => panic!("Expected Form variant"),
        }
    }

    #[test]
    fn elicit_request_url_mode_serialization() {
        let params = ElicitRequestParams::Url {
            message: "Please authenticate".to_string(),
            elicitation_id: "auth-123".to_string(),
            url: "https://example.com/auth".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["mode"], "url");
        assert_eq!(json["message"], "Please authenticate");
        assert_eq!(json["elicitationId"], "auth-123");
        assert_eq!(json["url"], "https://example.com/auth");

        let roundtrip: ElicitRequestParams = serde_json::from_value(json).unwrap();
        match roundtrip {
            ElicitRequestParams::Url { elicitation_id, .. } => {
                assert_eq!(elicitation_id, "auth-123");
            },
            ElicitRequestParams::Form { .. } => panic!("Expected Url variant"),
        }
    }

    #[test]
    fn elicit_request_form_mode_is_optional() {
        // The 2026-07-28 schema makes `ElicitRequestFormParams.mode` OPTIONAL
        // (implicit form). A conformant server omits it, and the derived
        // internally-tagged Deserialize used to reject that with
        // "missing field `mode`" — breaking the client half of CLNT-02.
        let params: ElicitRequestParams = serde_json::from_value(json!({
            "message": "What is your name?",
            "requestedSchema": { "type": "object" }
        }))
        .expect("a mode-less form elicitation must deserialize");
        match params {
            ElicitRequestParams::Form {
                message,
                requested_schema,
            } => {
                assert_eq!(message, "What is your name?");
                assert_eq!(requested_schema["type"], "object");
            },
            ElicitRequestParams::Url { .. } => panic!("Expected Form variant"),
        }
    }

    #[test]
    fn elicit_request_explicit_form_mode_still_deserializes() {
        let params: ElicitRequestParams = serde_json::from_value(json!({
            "mode": "form",
            "message": "hi",
            "requestedSchema": {}
        }))
        .expect("an explicit form mode must still deserialize");
        assert!(matches!(params, ElicitRequestParams::Form { .. }));
    }

    #[test]
    fn elicit_request_url_mode_still_requires_its_fields() {
        let params: ElicitRequestParams = serde_json::from_value(json!({
            "mode": "url",
            "message": "auth",
            "elicitationId": "auth-1",
            "url": "https://example.com"
        }))
        .expect("a complete url elicitation must deserialize");
        assert!(matches!(params, ElicitRequestParams::Url { .. }));

        // `mode: "url"` stays REQUIRED and so do its fields — a url elicitation
        // missing `elicitationId` or `url` must NOT silently fall back to form.
        assert!(serde_json::from_value::<ElicitRequestParams>(
            json!({ "mode": "url", "message": "auth", "url": "https://example.com" })
        )
        .is_err());
        assert!(serde_json::from_value::<ElicitRequestParams>(
            json!({ "mode": "url", "message": "auth", "elicitationId": "auth-1" })
        )
        .is_err());
    }

    #[test]
    fn elicit_request_rejects_an_unknown_mode() {
        assert!(serde_json::from_value::<ElicitRequestParams>(json!({
            "mode": "bogus",
            "message": "hi",
            "requestedSchema": {}
        }))
        .is_err());
    }

    #[test]
    fn elicit_request_rejects_a_non_string_mode() {
        assert!(serde_json::from_value::<ElicitRequestParams>(json!({
            "mode": 7,
            "message": "hi",
            "requestedSchema": {}
        }))
        .is_err());
    }

    #[test]
    fn elicit_request_form_still_serializes_the_mode_tag() {
        // v1 byte compatibility (D-10): serialization is UNCHANGED — the enum must
        // keep emitting `"mode":"form"` even though deserialization now tolerates
        // its absence.
        let params = ElicitRequestParams::Form {
            message: "hi".to_string(),
            requested_schema: json!({}),
        };
        let value = serde_json::to_value(&params).unwrap();
        assert_eq!(value["mode"], "form");
        assert_eq!(value["message"], "hi");
        assert!(value["requestedSchema"].is_object());
        assert_eq!(
            serde_json::to_string(&params).unwrap(),
            r#"{"mode":"form","message":"hi","requestedSchema":{}}"#
        );
    }

    #[test]
    fn elicit_request_form_missing_required_fields_is_an_error() {
        // A mode-less object that is not a valid form must still fail — the
        // implicit-form default must not swallow malformed input.
        assert!(serde_json::from_value::<ElicitRequestParams>(json!({ "message": "hi" })).is_err());
        assert!(serde_json::from_value::<ElicitRequestParams>(json!({})).is_err());
    }

    #[test]
    fn elicit_result_accept() {
        let mut content = HashMap::new();
        content.insert("name".to_string(), json!("Alice"));

        let result = ElicitResult {
            action: ElicitAction::Accept,
            content: Some(content),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["action"], "accept");
        assert_eq!(json["content"]["name"], "Alice");

        let roundtrip: ElicitResult = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.action, ElicitAction::Accept);
        assert!(roundtrip.content.is_some());
    }

    #[test]
    fn elicit_result_decline() {
        let result = ElicitResult {
            action: ElicitAction::Decline,
            content: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["action"], "decline");
        assert!(json.get("content").is_none());
    }

    #[test]
    fn elicit_action_values() {
        assert_eq!(
            serde_json::to_value(ElicitAction::Accept).unwrap(),
            "accept"
        );
        assert_eq!(
            serde_json::to_value(ElicitAction::Decline).unwrap(),
            "decline"
        );
        assert_eq!(
            serde_json::to_value(ElicitAction::Cancel).unwrap(),
            "cancel"
        );
    }
}
