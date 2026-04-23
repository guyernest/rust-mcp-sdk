//! IAM translation rules: `IamConfig` → TypeScript `addToRolePolicy` strings.
//!
//! Produces 4-space-indented
//! `mcpFunction.addToRolePolicy(new iam.PolicyStatement({ ... }))` calls that
//! match the existing template emission style (`init.rs:621, 634`).
//!
//! # Translation rules (locked per `76-CONTEXT.md` D-02)
//!
//! - `read` → `dynamodb:GetItem`, `Query`, `Scan`, `BatchGetItem` (4 actions)
//! - `write` → `dynamodb:PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem` (4 actions)
//! - `readwrite` → union of the two (8 actions)
//! - S3 `read` → `s3:GetObject`; `write` → `s3:PutObject`, `s3:DeleteObject`
//! - Raw statements (effect/actions/resources) are emitted verbatim after
//!   normalising `effect` to `iam.Effect.ALLOW` / `iam.Effect.DENY`.
//!
//! Ordering is locked: tables → buckets → statements (`76-RESEARCH.md` Q4).
//!
//! Resource ARNs use CDK placeholders (`${this.region}` / `${this.account}`)
//! so the emitted template inherits the deploy-target's region/account at
//! `cdk synth` time.
//!
//! # D-05 byte-identity invariant
//!
//! For the default (empty) [`IamConfig`], [`render_iam_block`] returns `""`.
//! Callers that interpolate the result via a `{iam_block}` named placeholder
//! directly abutting the preceding `}}));` closer (no surrounding whitespace
//! in the template) therefore emit byte-identical output when no `[iam]`
//! section is configured.
//!
//! Validation of sugar keywords / effect strings / passthrough action shape
//! lives in a separate Wave 4 validator that MUST run before
//! [`render_iam_block`] in CLI entry points. This renderer therefore emits
//! statements verbatim without silently dropping unknown sugar keywords —
//! keeping render and validate cleanly separable.

use std::fmt::Write as _;

use crate::deployment::config::{BucketPermission, IamConfig, IamStatement, TablePermission};

/// Render the full IAM block for an [`IamConfig`].
///
/// Returns `""` when `iam.is_empty()` — preserves the D-05 byte-identity
/// backward-compat invariant for configs without an `[iam]` section.
///
/// When non-empty, the output starts with a leading `\n` + three-line banner
/// comment, then zero-or-more 4-space-indented `addToRolePolicy` statements
/// in tables → buckets → statements order. Each statement emits a trailing
/// `\n`. Callers splice this string directly after the preceding `}}));`
/// closer and before the `\n    // Outputs` comment so the empty-config path
/// collapses to byte-identity.
#[must_use]
pub fn render_iam_block(iam: &IamConfig) -> String {
    if iam.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    // Leading newline + banner so the operator-declared block is visually
    // distinct from the two platform-composition `addToRolePolicy` calls
    // emitted earlier in the template (init.rs:621, 634).
    out.push_str(
        "\n    // ========================================================================\n",
    );
    out.push_str("    // Operator-declared IAM (from .pmcp/deploy.toml [iam])\n");
    out.push_str(
        "    // ========================================================================\n",
    );

    for table in &iam.tables {
        out.push_str(&render_table(table));
    }
    for bucket in &iam.buckets {
        out.push_str(&render_bucket(bucket));
    }
    for stmt in &iam.statements {
        out.push_str(&render_statement(stmt));
    }

    out
}

/// Render a single [`TablePermission`] as a 4-space-indented
/// `addToRolePolicy` statement.
fn render_table(t: &TablePermission) -> String {
    let actions = table_actions(&t.actions);
    let actions_ts = format_single_quoted_array(&actions);
    let resources_ts = render_table_resources(&t.name, t.include_indexes);

    let mut out = String::new();
    out.push_str("    mcpFunction.addToRolePolicy(new iam.PolicyStatement({\n");
    out.push_str("      effect: iam.Effect.ALLOW,\n");
    let _ = writeln!(out, "      actions: {actions_ts},");
    out.push_str("      resources: [\n");
    out.push_str(&resources_ts);
    out.push_str("      ],\n");
    out.push_str("    }));\n");
    out
}

/// Render a single [`BucketPermission`] as a 4-space-indented
/// `addToRolePolicy` statement.
fn render_bucket(b: &BucketPermission) -> String {
    let actions = bucket_actions(&b.actions);
    let actions_ts = format_single_quoted_array(&actions);
    let resource = format!("`arn:aws:s3:::{name}/*`", name = b.name);

    let mut out = String::new();
    out.push_str("    mcpFunction.addToRolePolicy(new iam.PolicyStatement({\n");
    out.push_str("      effect: iam.Effect.ALLOW,\n");
    let _ = writeln!(out, "      actions: {actions_ts},");
    out.push_str("      resources: [\n");
    let _ = writeln!(out, "        {resource},");
    out.push_str("      ],\n");
    out.push_str("    }));\n");
    out
}

/// Render a raw [`IamStatement`] (passthrough after Wave 4 validation).
fn render_statement(s: &IamStatement) -> String {
    let effect_ts = if s.effect.eq_ignore_ascii_case("Deny") {
        "iam.Effect.DENY"
    } else {
        // Default to ALLOW for anything that isn't a case-insensitive "Deny".
        // Wave 4's validator rejects effect strings outside {"Allow", "Deny"}
        // before calling into this renderer, so in the supported path this
        // branch always corresponds to a canonical "Allow".
        "iam.Effect.ALLOW"
    };
    let actions_ts = format_single_quoted_array(&s.actions);
    let resources_ts = format_single_quoted_array(&s.resources);

    let mut out = String::new();
    out.push_str("    mcpFunction.addToRolePolicy(new iam.PolicyStatement({\n");
    let _ = writeln!(out, "      effect: {effect_ts},");
    let _ = writeln!(out, "      actions: {actions_ts},");
    let _ = writeln!(out, "      resources: {resources_ts},");
    out.push_str("    }));\n");
    out
}

/// Expand sugar keywords in `actions` to the D-02 DynamoDB action list.
///
/// Returns a `Vec<&'static str>` covering all four read actions if any sugar
/// keyword in the slice is `"read"` or `"readwrite"`, and similarly for
/// write. Unknown sugar keywords are silently ignored here — Wave 4's
/// validator rejects them upstream.
fn table_actions(actions: &[String]) -> Vec<&'static str> {
    let has_read = actions.iter().any(|a| a == "read" || a == "readwrite");
    let has_write = actions.iter().any(|a| a == "write" || a == "readwrite");

    let mut out: Vec<&'static str> = Vec::with_capacity(8);
    if has_read {
        out.extend_from_slice(&[
            "dynamodb:GetItem",
            "dynamodb:Query",
            "dynamodb:Scan",
            "dynamodb:BatchGetItem",
        ]);
    }
    if has_write {
        out.extend_from_slice(&[
            "dynamodb:PutItem",
            "dynamodb:UpdateItem",
            "dynamodb:DeleteItem",
            "dynamodb:BatchWriteItem",
        ]);
    }
    out
}

/// Expand sugar keywords in `actions` to the D-02 S3 action list.
fn bucket_actions(actions: &[String]) -> Vec<&'static str> {
    let has_read = actions.iter().any(|a| a == "read" || a == "readwrite");
    let has_write = actions.iter().any(|a| a == "write" || a == "readwrite");

    let mut out: Vec<&'static str> = Vec::with_capacity(3);
    if has_read {
        out.push("s3:GetObject");
    }
    if has_write {
        out.push("s3:PutObject");
        out.push("s3:DeleteObject");
    }
    out
}

/// Render the `resources: [...]` body for a table permission.
///
/// Emits the base `table/NAME` ARN and, when `include_indexes` is set, an
/// additional `table/NAME/index/*` ARN on its own 8-space-indented line so
/// GSI/LSI access is granted as documented on [`TablePermission`].
fn render_table_resources(name: &str, include_indexes: bool) -> String {
    let mut out =
        format!("        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/{name}`,\n");
    if include_indexes {
        let _ = writeln!(
            out,
            "        `arn:aws:dynamodb:${{this.region}}:${{this.account}}:table/{name}/index/*`,"
        );
    }
    out
}

/// Format a slice as a TypeScript single-quoted string array: `['a', 'b']`.
///
/// Accepts any iterable of items that dereference to `&str` so both the
/// `&'static str` vecs from the sugar expanders and the `Vec<String>` from
/// raw statements re-use the same code path.
fn format_single_quoted_array<S: AsRef<str>>(items: &[S]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let body = items
        .iter()
        .map(|s| format!("'{item}'", item = s.as_ref()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::config::{BucketPermission, IamStatement, TablePermission};

    #[test]
    fn empty_iam_renders_empty_string() {
        assert_eq!(render_iam_block(&IamConfig::default()), "");
    }

    #[test]
    fn table_read_emits_four_read_actions() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "my-table".into(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        for needle in &[
            "dynamodb:GetItem",
            "dynamodb:Query",
            "dynamodb:Scan",
            "dynamodb:BatchGetItem",
        ] {
            assert!(out.contains(needle), "missing {needle} in output:\n{out}");
        }
        assert!(
            !out.contains("dynamodb:PutItem"),
            "read-only must not include write actions"
        );
    }

    #[test]
    fn table_write_emits_four_write_actions() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["write".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        for needle in &[
            "dynamodb:PutItem",
            "dynamodb:UpdateItem",
            "dynamodb:DeleteItem",
            "dynamodb:BatchWriteItem",
        ] {
            assert!(out.contains(needle));
        }
        assert!(
            !out.contains("dynamodb:GetItem"),
            "write-only must not include read actions"
        );
    }

    #[test]
    fn table_readwrite_emits_eight_actions() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["readwrite".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        for needle in &[
            "dynamodb:GetItem",
            "dynamodb:Query",
            "dynamodb:Scan",
            "dynamodb:BatchGetItem",
            "dynamodb:PutItem",
            "dynamodb:UpdateItem",
            "dynamodb:DeleteItem",
            "dynamodb:BatchWriteItem",
        ] {
            assert!(out.contains(needle), "readwrite missing {needle}");
        }
    }

    #[test]
    fn table_read_write_both_entries_equivalent_to_readwrite() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["read".into(), "write".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        for needle in &[
            "dynamodb:GetItem",
            "dynamodb:BatchGetItem",
            "dynamodb:PutItem",
            "dynamodb:BatchWriteItem",
        ] {
            assert!(out.contains(needle));
        }
    }

    #[test]
    fn table_include_indexes_adds_index_resource() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "my-table".into(),
                actions: vec!["read".into()],
                include_indexes: true,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("table/my-table`"), "base ARN missing");
        assert!(out.contains("table/my-table/index/*`"), "index ARN missing");
    }

    #[test]
    fn table_include_indexes_false_omits_index_resource() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "my-table".into(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(
            !out.contains("/index/*"),
            "index ARN must NOT appear when include_indexes=false"
        );
    }

    #[test]
    fn bucket_read_emits_get_object() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: "my-bucket".into(),
                actions: vec!["read".into()],
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("s3:GetObject"));
        assert!(!out.contains("s3:PutObject"));
        assert!(out.contains("arn:aws:s3:::my-bucket/*"));
    }

    #[test]
    fn bucket_write_emits_put_and_delete() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: "b".into(),
                actions: vec!["write".into()],
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("s3:PutObject"));
        assert!(out.contains("s3:DeleteObject"));
        assert!(!out.contains("s3:GetObject"));
    }

    #[test]
    fn bucket_readwrite_emits_three_actions() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: "b".into(),
                actions: vec!["readwrite".into()],
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("s3:GetObject"));
        assert!(out.contains("s3:PutObject"));
        assert!(out.contains("s3:DeleteObject"));
    }

    #[test]
    fn statement_allow_emits_iam_effect_allow() {
        let iam = IamConfig {
            statements: vec![IamStatement {
                effect: "Allow".into(),
                actions: vec!["secretsmanager:GetSecretValue".into()],
                resources: vec!["arn:aws:secretsmanager:us-west-2:*:secret:foo/*".into()],
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("iam.Effect.ALLOW"));
        assert!(out.contains("secretsmanager:GetSecretValue"));
        assert!(out.contains("arn:aws:secretsmanager"));
    }

    #[test]
    fn statement_deny_emits_iam_effect_deny() {
        let iam = IamConfig {
            statements: vec![IamStatement {
                effect: "Deny".into(),
                actions: vec!["s3:*".into()],
                resources: vec!["arn:aws:s3:::restricted/*".into()],
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(out.contains("iam.Effect.DENY"));
    }

    #[test]
    fn ordering_is_tables_then_buckets_then_statements() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t1".into(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            buckets: vec![BucketPermission {
                name: "b1".into(),
                actions: vec!["read".into()],
            }],
            statements: vec![IamStatement {
                effect: "Allow".into(),
                actions: vec!["kms:Decrypt".into()],
                resources: vec!["*".into()],
            }],
        };
        let out = render_iam_block(&iam);
        let table_idx = out.find("t1").expect("table rendered");
        let bucket_idx = out.find("b1/*").expect("bucket rendered");
        let statement_idx = out.find("kms:Decrypt").expect("statement rendered");
        assert!(table_idx < bucket_idx, "tables must render before buckets");
        assert!(
            bucket_idx < statement_idx,
            "buckets must render before statements"
        );
    }

    #[test]
    fn output_is_four_space_indented() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let out = render_iam_block(&iam);
        assert!(
            out.contains("    mcpFunction.addToRolePolicy"),
            "expected 4-space indent on addToRolePolicy; got:\n{out}"
        );
    }
}
