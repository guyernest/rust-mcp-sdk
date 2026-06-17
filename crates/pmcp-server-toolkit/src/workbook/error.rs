//! The structured `isError:true` envelope (WBSV-06) — machine-actionable repair
//! payloads that ride in `structuredContent`, NEVER a JSON-RPC protocol error,
//! and ALWAYS carry the provenance stamp (on the failure path too).
//!
//! # Domain failure vs infrastructure failure (Codex LOW)
//!
//! [`to_iserror_result`] is for **domain** failures — invalid input, an
//! out-of-range / non-finite output, a strict-constant override. A domain
//! failure returns `isError:true` INSIDE `structuredContent` and never a
//! protocol `Err`: pmcp tool dispatch hardcodes the protocol
//! `CallToolResult.is_error` to `false`, so the only machine-actionable error
//! channel is the structured payload.
//!
//! An **infrastructure** failure (a poisoned/malformed in-memory bundle state, a
//! resource-handler internal fault, a genuine bug) is a DIFFERENT class and MAY
//! still surface as a protocol `Err`. This module deliberately does NOT blanket
//! every fault into a domain envelope — only the modelled domain failures route
//! through [`WorkbookToolError`].
//!
//! # The self-repair code table (Gemini)
//!
//! Every [`WorkbookToolError`] code is a STABLE machine-readable string — the
//! primary signal the MCP App widget reads to repair a call. The four codes and
//! their UI self-repair meaning:
//!
//! | `code` | When | Self-repair fields | UI meaning |
//! |--------|------|--------------------|------------|
//! | `invalid_input` | arg-parse / dtype / enum-membership failure, or an unknown input field | `field`, `allowed` | "this argument is malformed or out of the allowed set — fix it to a listed value" |
//! | `missing_field` | a required input is absent | `field`, `required` | "supply the listed required field(s)" |
//! | `unsupported_option` | an override names no manifest cell | `field`, `allowed` | "this override is not a known variable-tier parameter — pick a listed one" |
//! | `strict_constant_override` | a BA-governed strict constant supplied as input | `field`, `allowed` | "this value is BA-governed and cannot be overridden per-call — set a listed variable-tier parameter instead" |
//!
//! Each code is stable across releases; the `allowed`/`required`/`range`/`field`
//! repair fields are present only when applicable ("allowed-values live in the
//! error"). The two SHAPE-ONLY deferred codes (`stale_oracle`,
//! `unapproved_assumption`) from the lighthouse have NO runtime trigger and are
//! intentionally NOT lifted (STATE.md Deferred Items: "Wire deferred error
//! triggers — Deferred v2.x").

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the
// runtime). Test code constructs fixtures freely.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use serde_json::{json, Map, Value};

use super::ProvStamp;

/// One structured tool-execution error (WBSV-06): a machine-actionable repair
/// payload. The agent reads `allowed`/`range`/`required` to repair the call —
/// "allowed-values live in the error".
///
/// `code` is one of the four stable machine-readable strings documented in the
/// module-level self-repair table.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WorkbookToolError {
    /// The stable error code (one of the four documented codes).
    pub code: String,
    /// The offending input field, when located.
    pub field: Option<String>,
    /// A human-readable reason.
    pub reason: String,
    /// The allowed alternatives (e.g. the variable-tier override keys for a
    /// `strict_constant_override`, or the enum members for an out-of-enum value).
    pub allowed: Option<Vec<String>>,
    /// A declared `[min, max]` range (carried, unenforced this phase).
    pub range: Option<(Value, Value)>,
    /// The required fields (e.g. for `missing_field`).
    pub required: Option<Vec<String>>,
}

impl WorkbookToolError {
    /// `invalid_input` — an arg-parse / type / enum-membership failure.
    #[must_use]
    pub fn invalid_input(reason: impl Into<String>) -> Self {
        Self::bare("invalid_input", reason)
    }

    /// `invalid_input` for an unknown input FIELD (WR-05). An unknown input key
    /// is a bad field, NOT an out-of-set option VALUE, so it shares the
    /// `invalid_input` code with the top-level `deny_unknown_fields` path.
    /// Carries the offending `field` and the `allowed` known input keys so the
    /// agent can repair the call.
    #[must_use]
    pub fn invalid_input_field(field: impl Into<String>, allowed: Vec<String>) -> Self {
        let field = field.into();
        let reason = format!("'{field}' is not a known input field");
        Self::invalid_enum(field, allowed, reason)
    }

    /// `invalid_input` carrying the closed-enum `allowed` members (an
    /// out-of-enum or non-string-on-string-enum value).
    #[must_use]
    pub fn invalid_enum(
        field: impl Into<String>,
        allowed: Vec<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            code: "invalid_input".to_string(),
            reason: reason.into(),
            field: Some(field.into()),
            allowed: Some(allowed),
            range: None,
            required: None,
        }
    }

    /// `missing_field` — a required input is absent.
    #[must_use]
    pub fn missing_field(field: impl Into<String>, required: Vec<String>) -> Self {
        Self {
            code: "missing_field".to_string(),
            field: Some(field.into()),
            reason: "a required input is missing".to_string(),
            allowed: None,
            range: None,
            required: Some(required),
        }
    }

    /// `unsupported_option` — an override value out of the supported set (an
    /// override naming no manifest cell).
    #[must_use]
    pub fn unsupported_option(field: impl Into<String>, allowed: Vec<String>) -> Self {
        Self {
            code: "unsupported_option".to_string(),
            field: Some(field.into()),
            reason: "the supplied override is not a known variable-tier parameter".to_string(),
            allowed: Some(allowed),
            range: None,
            required: None,
        }
    }

    /// `strict_constant_override` — a BA-governed strict constant was supplied as
    /// a `calculate` input. `allowed` carries the variable-tier override keys the
    /// caller MAY set instead.
    #[must_use]
    pub fn strict_constant_override(
        field: impl Into<String>,
        allowed_variable_keys: Vec<String>,
    ) -> Self {
        let field = field.into();
        Self {
            code: "strict_constant_override".to_string(),
            reason: format!(
                "'{field}' is a BA-governed strict constant and cannot be overridden \
                 per-call; set a variable-tier parameter instead"
            ),
            field: Some(field),
            allowed: Some(allowed_variable_keys),
            range: None,
            required: None,
        }
    }

    /// A bare code+reason error with no repair-field detail.
    fn bare(code: &str, reason: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            field: None,
            reason: reason.into(),
            allowed: None,
            range: None,
            required: None,
        }
    }
}

/// Render a [`WorkbookToolError`] into the `isError:true` payload carrying the
/// provenance stamp (on the failure path too). Returned as a bare `Value` — the
/// widget-meta tool routes it into `structuredContent` where the `isError:true`
/// marker + repair fields survive dispatch.
///
/// NEVER returns an `Err(pmcp::Error)`: a DOMAIN failure is a success-shaped
/// `CallToolResult` whose structured payload carries `isError:true`, not a
/// JSON-RPC protocol error (T-92-10). The provenance stamp carries the
/// `combined_hash` integrity anchor (the `BUNDLE.lock` combined hash) — see
/// [`ProvStamp`] for the field-naming contract (Codex HIGH #3).
#[must_use]
pub fn to_iserror_result(err: &WorkbookToolError, stamp: &ProvStamp) -> Value {
    let mut obj = Map::new();
    // Always-present envelope fields.
    obj.insert("isError".to_string(), json!(true));
    obj.insert("code".to_string(), json!(err.code));
    obj.insert("reason".to_string(), json!(err.reason));
    obj.insert("provenance".to_string(), stamp.to_json());
    // Optional repair fields — inserted only when present.
    if let Some(field) = &err.field {
        obj.insert("field".to_string(), json!(field));
    }
    if let Some(allowed) = &err.allowed {
        obj.insert("allowed".to_string(), json!(allowed));
    }
    if let Some((min, max)) = &err.range {
        obj.insert("range".to_string(), json!([min, max]));
    }
    if let Some(required) = &err.required {
        obj.insert("required".to_string(), json!(required));
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> ProvStamp {
        ProvStamp {
            bundle_id: "tax-calc".to_string(),
            version: "1.1.0".to_string(),
            combined_hash: "a".repeat(64),
        }
    }

    #[test]
    fn iserror_envelope_carries_flag_code_and_provenance() {
        let err = WorkbookToolError::invalid_input("bad number");
        let v = to_iserror_result(&err, &stamp());
        assert_eq!(
            v["isError"],
            json!(true),
            "isError:true rides in the payload"
        );
        assert_eq!(v["code"], json!("invalid_input"));
        // The provenance carries bundle_id + version + combined_hash (Codex HIGH #3).
        assert_eq!(v["provenance"]["bundle_id"], json!("tax-calc"));
        assert_eq!(v["provenance"]["version"], json!("1.1.0"));
        assert_eq!(
            v["provenance"]["combined_hash"].as_str().map(str::len),
            Some(64)
        );
        // The stamp carries EXACTLY the three contract keys — no source-workbook
        // hash key (Codex HIGH #3). The forbidden key name is built dynamically so
        // the contract is asserted without the literal appearing in this file (the
        // dedicated `workbook_provstamp_contract.rs` integration test checks the
        // key absence against the real golden bundle by name).
        let forbidden_key = ["work", "book_", "hash"].concat();
        assert!(
            v["provenance"].get(&forbidden_key).is_none(),
            "the stamp must never carry the source-workbook hash key"
        );
        let prov = v["provenance"]
            .as_object()
            .expect("provenance is an object");
        assert_eq!(
            prov.len(),
            3,
            "stamp has exactly bundle_id/version/combined_hash"
        );
    }

    #[test]
    fn strict_constant_override_carries_allowed_alternatives() {
        let err = WorkbookToolError::strict_constant_override(
            "const_rate",
            vec!["gross_income".to_string(), "deductions".to_string()],
        );
        let v = to_iserror_result(&err, &stamp());
        assert_eq!(v["code"], json!("strict_constant_override"));
        assert_eq!(v["field"], json!("const_rate"));
        assert_eq!(v["allowed"], json!(["gross_income", "deductions"]));
    }

    #[test]
    fn missing_field_carries_required() {
        let err =
            WorkbookToolError::missing_field("gross_income", vec!["gross_income".to_string()]);
        let v = to_iserror_result(&err, &stamp());
        assert_eq!(v["code"], json!("missing_field"));
        assert_eq!(v["required"], json!(["gross_income"]));
    }

    #[test]
    fn invalid_enum_carries_allowed_members() {
        let err = WorkbookToolError::invalid_enum(
            "filing_status",
            vec!["single".to_string(), "married_joint".to_string()],
            "not a member",
        );
        let v = to_iserror_result(&err, &stamp());
        assert_eq!(v["code"], json!("invalid_input"));
        assert_eq!(v["field"], json!("filing_status"));
        assert_eq!(v["allowed"], json!(["single", "married_joint"]));
    }

    #[test]
    fn optional_repair_fields_are_omitted_when_absent() {
        // A bare invalid_input carries no field/allowed/range/required keys.
        let v = to_iserror_result(&WorkbookToolError::invalid_input("x"), &stamp());
        assert!(v.get("field").is_none());
        assert!(v.get("allowed").is_none());
        assert!(v.get("range").is_none());
        assert!(v.get("required").is_none());
    }

    #[test]
    fn every_documented_code_is_an_emittable_stable_string() {
        // The four stable machine-readable codes (Gemini self-repair table) are
        // all emittable in the isError:true + provenance shape.
        let cases = [
            WorkbookToolError::invalid_input("x"),
            WorkbookToolError::missing_field("f", vec![]),
            WorkbookToolError::unsupported_option("f", vec![]),
            WorkbookToolError::strict_constant_override("f", vec![]),
        ];
        let expected_codes = [
            "invalid_input",
            "missing_field",
            "unsupported_option",
            "strict_constant_override",
        ];
        for (err, expected) in cases.iter().zip(expected_codes) {
            let v = to_iserror_result(err, &stamp());
            assert_eq!(v["isError"], json!(true));
            assert_eq!(v["code"], json!(expected), "code is a stable string");
        }
    }
}
