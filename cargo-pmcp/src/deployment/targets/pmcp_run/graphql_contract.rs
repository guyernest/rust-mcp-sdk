//! The exact runtime GraphQL operations sent by the package-capture client
//! (`graphql.rs`'s `submit_package_capture` / `get_package_capture_status`),
//! factored into their own dependency-light leaf.
//!
//! This file exists ONLY so the offline blocking contract test
//! (`tests/package_capture_contract.rs`) can validate the real runtime
//! queries against the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`)
//! without pulling `pmcp_run`'s auth/deploy/reqwest tree into the `cargo-pmcp`
//! lib target. It is mounted into the lib target via `#[path]` in `lib.rs`
//! (mirrors the `templates_workbook_server` / `agent_run` / `package_kind`
//! narrow-leaf convention already used there) and re-exported as
//! `crate::pmcp_run_graphql` — `#[doc(hidden)]`, not a stable public API.
//!
//! `graphql.rs` re-exports these same consts via `super::graphql_contract::*`
//! so there is exactly one source of truth for both operations.

/// The exact `submitPackageCapture` operation the CLI sends. Shared with the
/// offline contract test (`tests/package_capture_contract.rs`) so the test
/// validates the real runtime query against the vendored SDL.
pub const SUBMIT_PACKAGE_CAPTURE_QUERY: &str = r#"
        mutation SubmitPackageCapture(
            $rootComponentType: String!,
            $rootComponentId: String!,
            $version: String!,
            $bump: String
        ) {
            submitPackageCapture(
                rootComponentType: $rootComponentType,
                rootComponentId: $rootComponentId,
                version: $version,
                bump: $bump
            ) {
                captureId
                status
                createdAt
            }
        }
    "#;

/// The exact `getPackageCaptureStatus` operation the CLI sends. Shared with the
/// offline contract test.
pub const GET_PACKAGE_CAPTURE_STATUS_QUERY: &str = r#"
        query GetPackageCaptureStatus($id: ID!) {
            getPackageCaptureStatus(id: $id) {
                id
                status
                message
                errorCode
                divergentComponents
                manifestDigest
                updatedAt
            }
        }
    "#;
