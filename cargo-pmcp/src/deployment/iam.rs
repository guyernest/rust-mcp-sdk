//! IAM translation rules: IamConfig → TypeScript `addToRolePolicy` strings.
//!
//! Phase 76 Wave 3 (RED phase stub).
//!
//! This file is introduced as a RED-phase scaffold — the public
//! [`render_iam_block`] entry point returns an empty string unconditionally so
//! the 13 in-module unit tests below fail loudly. The Wave 3 Task-1 GREEN
//! commit replaces the stub body with the three-renderer implementation.

use crate::deployment::config::IamConfig;

/// Render an IAM block for the given [`IamConfig`] — RED-phase stub.
///
/// Always returns an empty string in this stub. GREEN will implement the
/// D-02 translation rules (DynamoDB 4-action read / 4-action write lists,
/// S3 `GetObject`/`PutObject`/`DeleteObject`, passthrough statements) plus
/// the D-05 empty-config invariant (already satisfied here trivially).
#[must_use]
pub fn render_iam_block(_iam: &IamConfig) -> String {
    String::new()
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
        assert!(
            out.contains("table/my-table/index/*`"),
            "index ARN missing"
        );
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
        assert!(
            table_idx < bucket_idx,
            "tables must render before buckets"
        );
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
