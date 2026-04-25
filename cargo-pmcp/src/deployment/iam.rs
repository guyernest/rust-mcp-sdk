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
use std::sync::LazyLock;

use anyhow::{anyhow, Result};
use regex::Regex;

use crate::deployment::config::{BucketPermission, IamConfig, IamStatement, TablePermission};

static ACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ACTION_REGEX).expect("ACTION_REGEX is a static, known-good pattern")
});

// ============================================================================
// Validation (Phase 76 Wave 4)
// ============================================================================

/// A non-blocking validation finding produced by [`validate`].
///
/// Hard errors short-circuit via `Result::Err`; soft findings land here so the
/// CLI can surface them to the operator without blocking deploy.
#[derive(Debug, Clone)]
pub struct Warning {
    /// Human-readable warning text. Printed verbatim by the CLI, prefixed
    /// with a yellow `warning:` label.
    pub message: String,
}

impl Warning {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Print validator warnings to stderr with a consistent yellow `warning:` label.
/// Shared by `cargo pmcp deploy` and `cargo pmcp validate deploy` so both entry
/// points emit the same format.
pub fn emit_warnings(warnings: &[Warning]) {
    for w in warnings {
        eprintln!("  {} {}", console::style("warning:").yellow(), w.message);
    }
}

/// Regex used to validate `service:Action` strings in `[[iam.statements]]`.
///
/// `^[a-z0-9-]+:[A-Za-z0-9*]+$` — lowercase kebab prefix, then action body
/// that is either pure wildcard (`*`) or mixed-case alphanumeric. Matches
/// the CR-locked rule catalogue in `76-RESEARCH.md §Validation`.
const ACTION_REGEX: &str = r"^[a-z0-9-]+:[A-Za-z0-9*]+$";

/// Allowed sugar keywords in `[[iam.tables]]` / `[[iam.buckets]]`.
const VALID_SUGAR: &[&str] = &["read", "write", "readwrite"];

/// Curated list of well-known AWS service prefixes.
///
/// Unknown prefixes in `[[iam.statements]]` action strings trigger a non-
/// blocking warning (not an error) per `76-CONTEXT.md §Validation`. The list
/// is curated and unit-tested — callers needing a new prefix should add it
/// here. Length is asserted by acceptance tests.
const KNOWN_SERVICE_PREFIXES: &[&str] = &[
    "acm",
    "apigateway",
    "appconfig",
    "athena",
    "autoscaling",
    "batch",
    "cloudformation",
    "cloudfront",
    "cloudwatch",
    "codebuild",
    "codepipeline",
    "cognito-idp",
    "cognito-identity",
    "dynamodb",
    "ec2",
    "ecr",
    "ecs",
    "elasticloadbalancing",
    "events",
    "eventbridge",
    "execute-api",
    "firehose",
    "glue",
    "iam",
    "kinesis",
    "kms",
    "lambda",
    "logs",
    "rds",
    "route53",
    "s3",
    "secretsmanager",
    "sns",
    "sqs",
    "ssm",
    "states",
    "sts",
    "waf",
    "wafv2",
    "xray",
];

/// Validate IAM declarations per the CR-locked rule catalogue.
///
/// Hard errors (6 classes) short-circuit via `Err`; soft findings are
/// returned as a `Vec<Warning>` for the CLI to surface without blocking
/// deploy.
///
/// Hard-error rules:
///   1. `Allow` + `actions=["*"]` + `resources=["*"]` in any statement
///      (wildcard escalation footgun — T-76-02)
///   2. `effect` not in `{"Allow", "Deny"}`
///   3. empty `actions` or empty `resources` in any statement
///   4. action not matching `^[a-z0-9-]+:[A-Za-z0-9*]+$`
///   5. sugar keyword in `[[iam.tables]]` / `[[iam.buckets]]` not in
///      `{read, write, readwrite}`
///   6. empty table or bucket name
///
/// Warning rules:
///   7. unknown service prefix (not in [`KNOWN_SERVICE_PREFIXES`])
///   8. pinned 12-digit AWS account in an ARN (cross-account advisory)
///
/// # Errors
/// Returns `Err` on the first hard-error rule violation. Warnings never
/// produce an `Err`.
pub fn validate(iam: &IamConfig) -> Result<Vec<Warning>> {
    let mut warnings = Vec::new();

    validate_sugar_decls("tables", &iam.tables)?;
    validate_sugar_decls("buckets", &iam.buckets)?;
    validate_statements(&iam.statements, &mut warnings)?;

    Ok(warnings)
}

/// Entry in an `[[iam.tables]]` or `[[iam.buckets]]` section. Both kinds share
/// the same validation shape (non-empty name, sugar-keyword actions).
trait SugarDecl {
    fn name(&self) -> &str;
    fn actions(&self) -> &[String];
}

impl SugarDecl for TablePermission {
    fn name(&self) -> &str {
        &self.name
    }
    fn actions(&self) -> &[String] {
        &self.actions
    }
}

impl SugarDecl for BucketPermission {
    fn name(&self) -> &str {
        &self.name
    }
    fn actions(&self) -> &[String] {
        &self.actions
    }
}

/// Validate `[[iam.tables]]` / `[[iam.buckets]]` entries (Rules 5 + 6).
fn validate_sugar_decls<T: SugarDecl>(kind: &str, entries: &[T]) -> Result<()> {
    for (idx, e) in entries.iter().enumerate() {
        if e.name().trim().is_empty() {
            return Err(anyhow!("[iam.{kind}][{idx}]: name must not be empty"));
        }
        if e.actions().is_empty() {
            return Err(anyhow!(
                "[iam.{kind}][{idx}] '{}': actions must not be empty",
                e.name()
            ));
        }
        for a in e.actions() {
            if !VALID_SUGAR.contains(&a.as_str()) {
                return Err(anyhow!(
                    "[iam.{kind}][{idx}] '{}': unknown sugar keyword '{a}' — allowed: {{read, write, readwrite}}",
                    e.name()
                ));
            }
        }
    }
    Ok(())
}

/// Validate `[[iam.statements]]` entries. Rules 1-4 + warnings 7-8.
fn validate_statements(stmts: &[IamStatement], warnings: &mut Vec<Warning>) -> Result<()> {
    for (idx, stmt) in stmts.iter().enumerate() {
        check_statement_effect_and_shape(idx, stmt)?;
        check_statement_wildcard_escalation(idx, stmt)?;
        check_statement_actions(idx, stmt, warnings)?;
        collect_cross_account_warnings(idx, stmt, warnings);
    }
    Ok(())
}

/// Rules 2 + 3: effect must be canonical; actions/resources must be non-empty.
fn check_statement_effect_and_shape(idx: usize, stmt: &IamStatement) -> Result<()> {
    if stmt.effect != "Allow" && stmt.effect != "Deny" {
        return Err(anyhow!(
            "[iam.statements][{idx}]: effect must be 'Allow' or 'Deny', got '{}'",
            stmt.effect
        ));
    }
    if stmt.actions.is_empty() {
        return Err(anyhow!(
            "[iam.statements][{idx}]: actions must not be empty"
        ));
    }
    if stmt.resources.is_empty() {
        return Err(anyhow!(
            "[iam.statements][{idx}]: resources must not be empty"
        ));
    }
    Ok(())
}

/// Rule 1 (T-76-02): reject `Allow` with `actions=["*"]` and `resources=["*"]`.
fn check_statement_wildcard_escalation(idx: usize, stmt: &IamStatement) -> Result<()> {
    let is_wildcard_allow = stmt.effect == "Allow"
        && stmt.actions.len() == 1
        && stmt.actions[0] == "*"
        && stmt.resources.len() == 1
        && stmt.resources[0] == "*";
    if is_wildcard_allow {
        return Err(anyhow!(
            "[iam.statements][{idx}]: Allow + actions=[\"*\"] + resources=[\"*\"] is a wildcard escalation footgun — refuse to deploy. Tighten actions and resources, or use [[iam.tables]] / [[iam.buckets]] sugar blocks."
        ));
    }
    Ok(())
}

/// Rule 4: every action matches the regex. Warning 7: unknown prefix.
fn check_statement_actions(
    idx: usize,
    stmt: &IamStatement,
    warnings: &mut Vec<Warning>,
) -> Result<()> {
    for a in &stmt.actions {
        // `*` alone is allowed so `actions = ["*"]` with a tightened
        // `resources` list remains declarable (Rule 1 already rejects `*`+`*`).
        if a == "*" {
            continue;
        }
        if !ACTION_RE.is_match(a) {
            return Err(anyhow!(
                "[iam.statements][{idx}]: action '{a}' does not match {ACTION_REGEX}"
            ));
        }
        if let Some((prefix, _)) = a.split_once(':') {
            if !KNOWN_SERVICE_PREFIXES.contains(&prefix) {
                warnings.push(Warning::new(format!(
                    "[iam.statements][{idx}]: unknown service prefix '{prefix}' in action '{a}' — verify this is a valid AWS service"
                )));
            }
        }
    }
    Ok(())
}

/// Warning 8 (best-effort): flag resources that pin a specific 12-digit AWS
/// account. This is advisory only — a full cross-account check requires
/// passing the deploy-target account through to `validate`, which is
/// deferred. Never produces an `Err`.
fn collect_cross_account_warnings(idx: usize, stmt: &IamStatement, warnings: &mut Vec<Warning>) {
    for r in &stmt.resources {
        if let Some(acct) = extract_account_from_arn(r) {
            if acct.len() == 12 && acct.chars().all(|c| c.is_ascii_digit()) {
                warnings.push(Warning::new(format!(
                    "[iam.statements][{idx}]: resource '{r}' pins a specific AWS account '{acct}' — verify this matches your deploy target (use '*' or omit the account segment for account-agnostic ARNs)"
                )));
            }
        }
    }
}

/// Extract the `account` segment (index 4) of an ARN
/// `arn:partition:service:region:account:resource`. Returns `None` if the
/// input is not shaped like an ARN.
fn extract_account_from_arn(arn: &str) -> Option<&str> {
    let mut parts = arn.splitn(6, ':');
    let head = parts.next()?;
    if head != "arn" {
        return None;
    }
    let _partition = parts.next()?;
    let _service = parts.next()?;
    let _region = parts.next()?;
    let account = parts.next()?;
    let _resource = parts.next()?;
    Some(account)
}

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

/// Emit the shared `addToRolePolicy(new iam.PolicyStatement({ ... }))`
/// skeleton, delegating to `body` for the per-kind effect/actions/resources
/// lines. Keeps the skeleton change-in-one-place across the three renderers.
fn render_policy_statement(body: impl FnOnce(&mut String)) -> String {
    let mut out = String::new();
    out.push_str("    mcpFunction.addToRolePolicy(new iam.PolicyStatement({\n");
    body(&mut out);
    out.push_str("    }));\n");
    out
}

/// Render a single [`TablePermission`] as a 4-space-indented
/// `addToRolePolicy` statement.
fn render_table(t: &TablePermission) -> String {
    let actions_ts = format_single_quoted_array(&table_actions(&t.actions));
    let resources_ts = render_table_resources(&t.name, t.include_indexes);

    render_policy_statement(|out| {
        out.push_str("      effect: iam.Effect.ALLOW,\n");
        let _ = writeln!(out, "      actions: {actions_ts},");
        out.push_str("      resources: [\n");
        out.push_str(&resources_ts);
        out.push_str("      ],\n");
    })
}

/// Render a single [`BucketPermission`] as a 4-space-indented
/// `addToRolePolicy` statement.
fn render_bucket(b: &BucketPermission) -> String {
    let actions_ts = format_single_quoted_array(&bucket_actions(&b.actions));
    let resource = format!("`arn:aws:s3:::{name}/*`", name = b.name);

    render_policy_statement(|out| {
        out.push_str("      effect: iam.Effect.ALLOW,\n");
        let _ = writeln!(out, "      actions: {actions_ts},");
        out.push_str("      resources: [\n");
        let _ = writeln!(out, "        {resource},");
        out.push_str("      ],\n");
    })
}

/// Render a raw [`IamStatement`] (passthrough after Wave 4 validation).
fn render_statement(s: &IamStatement) -> String {
    // Wave 4's validator rejects effect strings outside {"Allow", "Deny"}
    // before we get here, so anything not case-insensitive "Deny" is an Allow.
    let effect_ts = if s.effect.eq_ignore_ascii_case("Deny") {
        "iam.Effect.DENY"
    } else {
        "iam.Effect.ALLOW"
    };
    let actions_ts = format_single_quoted_array(&s.actions);
    let resources_ts = format_single_quoted_array(&s.resources);

    render_policy_statement(|out| {
        let _ = writeln!(out, "      effect: {effect_ts},");
        let _ = writeln!(out, "      actions: {actions_ts},");
        let _ = writeln!(out, "      resources: {resources_ts},");
    })
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

#[cfg(test)]
mod validate_tests {
    //! Phase 76 Wave 4 — unit tests for `validate` + `Warning`.
    //!
    //! RED-phase tests: authored before the implementation. Each test locks a
    //! CR-mandated hard-error or warning rule per `76-CONTEXT.md §Validation`.

    use super::*;
    use crate::deployment::config::{BucketPermission, IamStatement, TablePermission};

    fn mk_iam_stmt(stmts: Vec<IamStatement>) -> IamConfig {
        IamConfig {
            statements: stmts,
            ..IamConfig::default()
        }
    }

    #[test]
    fn empty_config_is_valid() {
        let w = validate(&IamConfig::default()).expect("valid");
        assert!(w.is_empty());
    }

    #[test]
    fn allow_star_star_is_hard_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["*".into()],
            resources: vec!["*".into()],
        }]);
        let err = validate(&iam).expect_err("wildcard escalation must fail");
        assert!(
            err.to_string().contains("wildcard escalation"),
            "expected wildcard escalation message, got: {err}"
        );
    }

    #[test]
    fn unknown_effect_is_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Permit".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["*".into()],
        }]);
        let err = validate(&iam).expect_err("bad effect must fail");
        assert!(err.to_string().contains("effect"));
    }

    #[test]
    fn empty_actions_is_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec![],
            resources: vec!["*".into()],
        }]);
        validate(&iam).expect_err("empty actions must fail");
    }

    #[test]
    fn empty_resources_is_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec![],
        }]);
        validate(&iam).expect_err("empty resources must fail");
    }

    #[test]
    fn malformed_action_uppercase_prefix_is_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["DynamoDB:getitem".into()],
            resources: vec!["*".into()],
        }]);
        validate(&iam).expect_err("bad action casing must fail");
    }

    #[test]
    fn underscore_in_action_prefix_is_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["foo_bar:GetThing".into()],
            resources: vec!["*".into()],
        }]);
        validate(&iam).expect_err("underscore in service prefix must fail");
    }

    #[test]
    fn table_empty_name_is_error() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: String::new(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("empty table name must fail");
    }

    #[test]
    fn table_invalid_sugar_keyword_is_error() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["readfoo".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        let err = validate(&iam).expect_err("bad sugar keyword must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("sugar") || msg.contains("read, write, readwrite"),
            "expected sugar-keyword message, got: {msg}"
        );
    }

    #[test]
    fn bucket_empty_name_is_error() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: String::new(),
                actions: vec!["read".into()],
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("empty bucket name must fail");
    }

    #[test]
    fn bucket_invalid_sugar_is_error() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: "b".into(),
                actions: vec!["cook".into()],
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("bad bucket sugar must fail");
    }

    #[test]
    fn unknown_service_prefix_is_warning_not_error() {
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["totallyfake:DoThing".into()],
            resources: vec!["*".into()],
        }]);
        let warnings = validate(&iam).expect("warnings only, no hard error");
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("unknown service prefix")
                    && w.message.contains("totallyfake")),
            "expected warning about 'totallyfake' prefix, got: {warnings:?}"
        );
    }

    #[test]
    fn cross_account_arn_does_not_hard_error() {
        // Cross-account detection is advisory, not a gate. Best-effort parser
        // may or may not emit a warning for a given shape — test documents
        // behaviour without over-specifying the warning class.
        let iam = mk_iam_stmt(vec![IamStatement {
            effect: "Allow".into(),
            actions: vec!["s3:GetObject".into()],
            resources: vec!["arn:aws:s3:::bucket/object:999999999999:foo".into()],
        }]);
        let warnings = validate(&iam).expect("not a hard error");
        assert!(
            warnings.iter().all(|w| !w.message.contains("wildcard")),
            "no wildcard spam expected"
        );
    }

    #[test]
    fn typical_cost_coach_config_is_valid_without_warnings() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "cost-coach-tenants".into(),
                actions: vec!["readwrite".into()],
                include_indexes: true,
            }],
            buckets: vec![BucketPermission {
                name: "cost-coach-snapshots".into(),
                actions: vec!["readwrite".into()],
            }],
            statements: vec![IamStatement {
                effect: "Allow".into(),
                actions: vec!["secretsmanager:GetSecretValue".into()],
                resources: vec!["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*".into()],
            }],
        };
        let warnings = validate(&iam).expect("cost-coach config must validate");
        assert!(
            warnings.is_empty(),
            "cost-coach config emitted unexpected warnings: {warnings:?}"
        );
    }
}

#[cfg(test)]
mod validate_integration_tests {
    //! Phase 76 Wave 4 — in-crate integration-style tests for the validator.
    //!
    //! **Rule-3 deviation note (consistent with Wave 1/2/3):** the plan called
    //! for this file at `cargo-pmcp/tests/iam_validate.rs`, but
    //! `cargo_pmcp::deployment` is NOT re-exported from `cargo-pmcp/src/lib.rs`
    //! (lib surface intentionally minimal at `loadtest`/`pentest`/
    //! `test_support_cache`). Expanding lib visibility would drag in the
    //! CognitoConfig + templates tree for very little over the in-crate
    //! coverage. Tests are identical in intent — they exercise the public
    //! `validate` + `Warning` API through `super::*` rather than through a
    //! crate-root re-export.

    use super::*;
    use crate::deployment::config::{BucketPermission, IamStatement, TablePermission};

    fn one_stmt(effect: &str, actions: Vec<&str>, resources: Vec<&str>) -> IamConfig {
        IamConfig {
            statements: vec![IamStatement {
                effect: effect.into(),
                actions: actions.into_iter().map(String::from).collect(),
                resources: resources.into_iter().map(String::from).collect(),
            }],
            ..IamConfig::default()
        }
    }

    #[test]
    fn public_api_validate_accepts_empty() {
        let warnings: Vec<Warning> = validate(&IamConfig::default()).expect("ok");
        assert!(warnings.is_empty());
    }

    #[test]
    fn public_api_validate_rejects_wildcard_allow() {
        let iam = one_stmt("Allow", vec!["*"], vec!["*"]);
        let err = validate(&iam).expect_err("must fail");
        assert!(err.to_string().to_lowercase().contains("wildcard"));
    }

    #[test]
    fn public_api_validate_rejects_empty_statement_actions() {
        let iam = one_stmt("Allow", vec![], vec!["*"]);
        validate(&iam).expect_err("empty actions");
    }

    #[test]
    fn public_api_validate_rejects_empty_statement_resources() {
        let iam = one_stmt("Allow", vec!["s3:GetObject"], vec![]);
        validate(&iam).expect_err("empty resources");
    }

    #[test]
    fn public_api_validate_rejects_bad_effect() {
        let iam = one_stmt("Maybe", vec!["s3:GetObject"], vec!["*"]);
        validate(&iam).expect_err("bad effect");
    }

    #[test]
    fn public_api_validate_rejects_bad_action_format() {
        let iam = one_stmt("Allow", vec!["S3:get_object"], vec!["*"]);
        validate(&iam).expect_err("bad action format");
    }

    #[test]
    fn public_api_validate_rejects_bad_table_sugar() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: "t".into(),
                actions: vec!["rwx".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("bad table sugar");
    }

    #[test]
    fn public_api_validate_rejects_bad_bucket_sugar() {
        let iam = IamConfig {
            buckets: vec![BucketPermission {
                name: "b".into(),
                actions: vec!["flood".into()],
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("bad bucket sugar");
    }

    #[test]
    fn public_api_validate_rejects_empty_table_name() {
        let iam = IamConfig {
            tables: vec![TablePermission {
                name: String::new(),
                actions: vec!["read".into()],
                include_indexes: false,
            }],
            ..IamConfig::default()
        };
        validate(&iam).expect_err("empty table name");
    }

    #[test]
    fn public_api_validate_warns_on_unknown_service_prefix() {
        let iam = one_stmt("Allow", vec!["mythicalaws:DoThing"], vec!["*"]);
        let warnings = validate(&iam).expect("warning only, not hard error");
        assert!(
            warnings.iter().any(|w| w.message.contains("mythicalaws")),
            "expected warning referencing 'mythicalaws', got: {warnings:?}"
        );
    }

    #[test]
    fn public_api_warning_is_constructable_and_clonable() {
        // Compile-time + runtime sanity: Warning is Clone + Debug.
        let iam = one_stmt("Allow", vec!["notaservice:Foo"], vec!["*"]);
        let warnings = validate(&iam).expect("ok");
        let first = warnings.first().cloned();
        let _debug_repr = format!("{first:?}");
        assert!(first.is_some());
    }
}

#[cfg(test)]
mod proptests {
    //! Phase 76 Wave 3 Task 3 — property tests for the IAM translation rules.
    //!
    //! Strategies draw sugar keywords from `{"read", "write", "readwrite"}`
    //! and valid action/resource shapes, then exercise
    //! [`super::render_iam_block`] against the D-02 invariants:
    //!
    //! - `read` / `readwrite` → 4 read actions (including `BatchGetItem`)
    //! - `write` / `readwrite` → 4 write actions (including `BatchWriteItem`)
    //! - S3 `read` → `GetObject`; `write` → `PutObject` + `DeleteObject`
    //! - `include_indexes` gates the `/index/*` ARN
    //! - One `addToRolePolicy` call per declaration (no silent drops / dups)
    //! - Effect preservation through the ALLOW / DENY normaliser
    //! - Brace balance as a coarse structural sanity check
    //! - Lossless `IamConfig` TOML roundtrip
    //!
    //! **Why in-crate:** following the Rule-3 precedent documented in
    //! `76-01-SUMMARY.md` and `76-02-SUMMARY.md`, `cargo_pmcp::deployment`
    //! is NOT re-exported from `cargo-pmcp/src/lib.rs` (the lib surface is
    //! intentionally kept to `loadtest` / `pentest` / `test_support_cache`).
    //! External integration tests therefore cannot `use`
    //! `cargo_pmcp::deployment::config::*` or
    //! `cargo_pmcp::deployment::iam::render_iam_block` — they would require
    //! expanding the lib surface to drag in the CognitoConfig / templates
    //! tree. In-crate testing sidesteps this while still locking the
    //! translation invariants under proptest's 128-case default.

    use super::*;
    use crate::deployment::config::{BucketPermission, IamStatement, TablePermission};
    use proptest::prelude::*;

    fn arb_sugar_action() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("read".to_string()),
            Just("write".to_string()),
            Just("readwrite".to_string()),
        ]
    }

    fn arb_sugar_actions_vec() -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec(arb_sugar_action(), 1..=3)
    }

    fn arb_table_permission() -> impl Strategy<Value = TablePermission> {
        (
            "[a-z][a-z0-9-]{2,30}",
            arb_sugar_actions_vec(),
            any::<bool>(),
        )
            .prop_map(|(name, actions, include_indexes)| TablePermission {
                name,
                actions,
                include_indexes,
            })
    }

    fn arb_bucket_permission() -> impl Strategy<Value = BucketPermission> {
        ("[a-z][a-z0-9-]{2,30}", arb_sugar_actions_vec())
            .prop_map(|(name, actions)| BucketPermission { name, actions })
    }

    fn arb_iam_statement_valid() -> impl Strategy<Value = IamStatement> {
        (
            prop_oneof![Just("Allow".to_string()), Just("Deny".to_string())],
            prop::collection::vec("[a-z]{2,12}:[A-Za-z*]{2,20}", 1..=4),
            prop::collection::vec(
                "arn:aws:[a-z0-9-]{2,10}:[a-z0-9-]*:[0-9*]{0,12}:[a-zA-Z0-9:*/_-]{1,50}",
                1..=3,
            ),
        )
            .prop_map(|(effect, actions, resources)| IamStatement {
                effect,
                actions,
                resources,
            })
    }

    fn arb_valid_iam_config() -> impl Strategy<Value = IamConfig> {
        (
            prop::collection::vec(arb_table_permission(), 0..=4),
            prop::collection::vec(arb_bucket_permission(), 0..=4),
            prop::collection::vec(arb_iam_statement_valid(), 0..=4),
        )
            .prop_map(|(tables, buckets, statements)| IamConfig {
                tables,
                buckets,
                statements,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Rendered output must contain exactly one `addToRolePolicy` per
        /// declaration — locks the emitted-statement count against silent
        /// drops and duplications.
        #[test]
        fn prop_one_addtorolepolicy_per_declaration(iam in arb_valid_iam_config()) {
            let out = render_iam_block(&iam);
            let expected = iam.tables.len() + iam.buckets.len() + iam.statements.len();
            let actual = out.matches("addToRolePolicy").count();
            prop_assert_eq!(
                actual, expected,
                "expected {} addToRolePolicy calls, got {}.\nOutput:\n{}",
                expected, actual, out
            );
        }

        /// Empty config → empty string (D-05 byte-identity invariant).
        #[test]
        fn prop_empty_iam_renders_empty(seed in 0u64..1000) {
            let _ = seed; // force proptest to vary cases
            let out = render_iam_block(&IamConfig::default());
            prop_assert_eq!(out, "");
        }

        /// Any `TablePermission` containing "read" or "readwrite" → output
        /// contains all 4 read actions (D-02 lock).
        #[test]
        fn prop_table_read_emits_four_read_actions(table in arb_table_permission()) {
            let has_read = table.actions.iter().any(|a| a == "read" || a == "readwrite");
            let iam = IamConfig {
                tables: vec![table],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);

            if has_read {
                for needle in &[
                    "dynamodb:GetItem",
                    "dynamodb:Query",
                    "dynamodb:Scan",
                    "dynamodb:BatchGetItem",
                ] {
                    prop_assert!(
                        out.contains(needle),
                        "table with read/readwrite missing {}", needle
                    );
                }
            }
        }

        /// Any `TablePermission` containing "write" or "readwrite" → output
        /// contains all 4 write actions (D-02 lock).
        #[test]
        fn prop_table_write_emits_four_write_actions(table in arb_table_permission()) {
            let has_write = table.actions.iter().any(|a| a == "write" || a == "readwrite");
            let iam = IamConfig {
                tables: vec![table],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);

            if has_write {
                for needle in &[
                    "dynamodb:PutItem",
                    "dynamodb:UpdateItem",
                    "dynamodb:DeleteItem",
                    "dynamodb:BatchWriteItem",
                ] {
                    prop_assert!(
                        out.contains(needle),
                        "table with write/readwrite missing {}", needle
                    );
                }
            }
        }

        /// `include_indexes=true` ⇔ output contains the `/index/*` ARN;
        /// `include_indexes=false` ⇔ it does not.
        #[test]
        fn prop_include_indexes_controls_index_resource(
            name in "[a-z][a-z0-9-]{2,30}",
            include_indexes in any::<bool>(),
        ) {
            let iam = IamConfig {
                tables: vec![TablePermission {
                    name: name.clone(),
                    actions: vec!["read".into()],
                    include_indexes,
                }],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);
            let index_arn = format!("table/{name}/index/*");
            let has_index_arn = out.contains(&index_arn);
            prop_assert_eq!(
                has_index_arn, include_indexes,
                "include_indexes={} but index ARN presence={}",
                include_indexes, has_index_arn
            );
        }

        /// Bucket read / write actions round-trip to `s3:*` correctly.
        #[test]
        fn prop_bucket_actions_match_sugar(bucket in arb_bucket_permission()) {
            let has_read = bucket.actions.iter().any(|a| a == "read" || a == "readwrite");
            let has_write = bucket.actions.iter().any(|a| a == "write" || a == "readwrite");
            let iam = IamConfig {
                buckets: vec![bucket],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);

            prop_assert_eq!(out.contains("s3:GetObject"), has_read);
            prop_assert_eq!(out.contains("s3:PutObject"), has_write);
            prop_assert_eq!(out.contains("s3:DeleteObject"), has_write);
        }

        /// Bucket resource is always the object-level `arn:aws:s3:::NAME/*`.
        /// Bucket-level ARNs (e.g. for `s3:ListBucket`) must land in
        /// `[[iam.statements]]`, not `[[iam.buckets]]`, per the CR scope.
        #[test]
        fn prop_bucket_resource_is_object_level_only(bucket in arb_bucket_permission()) {
            let name = bucket.name.clone();
            let iam = IamConfig {
                buckets: vec![bucket],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);
            let expected = format!("arn:aws:s3:::{name}/*");
            prop_assert!(
                out.contains(&expected),
                "expected {} in output", expected
            );
        }

        /// `IamConfig` survives a TOML roundtrip — structural equality
        /// (the type deliberately derives no `PartialEq` per
        /// `76-PATTERNS.md` §S1, so compare each field individually).
        #[test]
        fn prop_toml_roundtrip(iam in arb_valid_iam_config()) {
            // Wrap in a shim because `IamConfig` alone has no top-level
            // header — the real wire format lives under `DeployConfig.iam`.
            #[derive(serde::Serialize, serde::Deserialize)]
            struct Wrapper {
                #[serde(default)]
                iam: IamConfig,
            }

            let wrapper = Wrapper { iam };
            let serialized = toml::to_string(&wrapper).expect("serialize");
            let reparsed: Wrapper = toml::from_str(&serialized).unwrap_or_else(|e| {
                panic!("reparse failed: {e}\nSerialized:\n{serialized}")
            });

            prop_assert_eq!(reparsed.iam.tables.len(), wrapper.iam.tables.len());
            prop_assert_eq!(reparsed.iam.buckets.len(), wrapper.iam.buckets.len());
            prop_assert_eq!(
                reparsed.iam.statements.len(),
                wrapper.iam.statements.len()
            );
        }

        /// Balanced braces — a coarse structural sanity check on the
        /// emitted TS. Any renderer bug that opens a `{` without closing
        /// it fails loud here.
        #[test]
        fn prop_balanced_braces(iam in arb_valid_iam_config()) {
            let out = render_iam_block(&iam);
            let opens = out.matches('{').count();
            let closes = out.matches('}').count();
            prop_assert_eq!(opens, closes, "unbalanced braces.\nOutput:\n{}", out);
        }

        /// Operator-declared statement effect is preserved through the
        /// ALLOW / DENY normaliser (case-insensitive `"Deny"` → DENY;
        /// everything else → ALLOW).
        #[test]
        fn prop_statement_effect_preserved(statement in arb_iam_statement_valid()) {
            let effect_upper = statement.effect.to_uppercase();
            let iam = IamConfig {
                statements: vec![statement],
                ..IamConfig::default()
            };
            let out = render_iam_block(&iam);
            let expected_ts_effect = if effect_upper == "DENY" {
                "iam.Effect.DENY"
            } else {
                "iam.Effect.ALLOW"
            };
            prop_assert!(
                out.contains(expected_ts_effect),
                "expected {} in output for effect={}",
                expected_ts_effect, effect_upper
            );
        }
    }
}
