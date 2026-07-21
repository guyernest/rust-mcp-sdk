//! Stable, hash-free logical-ID scheme for rendered CloudFormation resources.
//!
//! CDK derives logical IDs from construct-tree content hashes (e.g.
//! `McpFunction1A2B3C4D`), so renaming or reordering constructs can silently
//! rename the logical ID of an unrelated resource. This renderer instead
//! derives logical IDs directly and deterministically from descriptor
//! names via [`pascal`] — no hashes, ever.
//!
//! One function per resource family, each returning a fixed, documented
//! logical ID (or, for families with descriptor-supplied names like
//! DynamoDB tables, a function of that name). A renamed descriptor entity
//! therefore renames its logical ID — this is an accepted tradeoff, not a
//! bug: the design's migration model is fleet recreation, not in-place
//! updates of foreign stacks (see the design spec, "Determinism").
//!
//! Future resource families (`cognito`, `dynamodb`'s multi-table shape,
//! etc.) get their own `for_*` functions as their resource modules land in
//! later tasks — this module is additive.

/// Logical ID for the MCP server's Lambda function. Exactly one per stack
/// (a descriptor names exactly one server function).
#[must_use]
pub fn for_function() -> &'static str {
    "McpFunction"
}

/// Logical ID for the function's CloudWatch log group.
#[must_use]
pub fn for_log_group() -> &'static str {
    "LogGroup"
}

/// Logical ID for the function's Lambda execution IAM role.
#[must_use]
pub fn for_execution_role() -> &'static str {
    "ExecutionRole"
}

/// Logical ID for the HTTP API (API Gateway v2 `AWS::ApiGatewayV2::Api`).
#[must_use]
pub fn for_http_api() -> &'static str {
    "HttpApi"
}

/// Logical ID for a named DynamoDB table: `PascalCase(name)` + `"Table"`.
///
/// e.g. `for_table("audit-log")` -> `"AuditLogTable"`.
#[must_use]
pub fn for_table(name: &str) -> String {
    format!("{}Table", pascal(name))
}

/// Split `name` on `-`/`_`, uppercase each segment's first character, and
/// concatenate (PascalCase) — the transform every `for_*` function that
/// takes a descriptor-supplied name is built on.
///
/// Non-alphanumeric separators (`-`, `_`) are dropped; everything else in a
/// segment is left untouched (so an already-cased segment like `"API"`
/// stays `"API"` rather than becoming `"Api"`). Empty segments (from
/// leading/trailing/repeated separators) are skipped.
#[must_use]
pub fn pascal(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_function_is_stable_and_hash_free() {
        assert_eq!(for_function(), "McpFunction");
    }

    #[test]
    fn for_log_group_is_stable() {
        assert_eq!(for_log_group(), "LogGroup");
    }

    #[test]
    fn for_http_api_is_stable() {
        assert_eq!(for_http_api(), "HttpApi");
    }

    #[test]
    fn for_execution_role_is_stable() {
        assert_eq!(for_execution_role(), "ExecutionRole");
    }

    #[test]
    fn for_table_pascal_cases_and_suffixes() {
        assert_eq!(for_table("audit-log"), "AuditLogTable");
        assert_eq!(for_table("session_store"), "SessionStoreTable");
        assert_eq!(for_table("orders"), "OrdersTable");
    }

    #[test]
    fn pascal_splits_on_dash_and_underscore() {
        assert_eq!(pascal("audit-log"), "AuditLog");
        assert_eq!(pascal("session_store"), "SessionStore");
        assert_eq!(pascal("mixed-case_name"), "MixedCaseName");
    }

    #[test]
    fn pascal_preserves_existing_casing_within_a_segment() {
        assert_eq!(pascal("API-gateway"), "APIGateway");
    }

    #[test]
    fn pascal_skips_empty_segments() {
        assert_eq!(pascal("--leading"), "Leading");
        assert_eq!(pascal("trailing--"), "Trailing");
        assert_eq!(pascal("a--b"), "AB");
    }

    #[test]
    fn pascal_is_idempotent_on_a_single_word() {
        assert_eq!(pascal("orders"), "Orders");
    }

    #[test]
    fn for_table_never_produces_the_same_id_for_distinct_names() {
        // Not a full injectivity proof — a fast, cheap regression guard that
        // two distinct realistic names don't collide.
        assert_ne!(for_table("orders"), for_table("order"));
    }
}
