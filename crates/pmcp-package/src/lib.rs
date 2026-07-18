//! # pmcp-package
//!
//! The AI-Package format crate (Phase 168).
//!
//! ## Dual-consumer contract (D-10)
//!
//! This crate is consumed by **two** independent call sites:
//! - `cargo-pmcp` (the CLI, published from the sibling `rust-mcp-sdk` repo) —
//!   packs local server/agent/team/workflow artifacts for publish.
//! - The `pmcp.run` platform (this repo) — unpacks and validates packages at
//!   import/pre-flight time.
//!
//! Both call sites MUST resolve identical validation/digest behavior from the
//! same crate code — that is the entire point of this crate existing as a
//! shared, standalone, publishable library rather than being duplicated in
//! each consumer.
//!
//! ## Scope fence (I-4 / §11)
//!
//! This crate is **format only**:
//! - Typed manifest schemas for the four package kinds (mcp-server, agent,
//!   team, workflow).
//! - Config-slot type system: classification, aggregation, deviation
//!   detection (I-5).
//! - Local OCI artifact pack/unpack (construct manifests + content-addressed
//!   blobs on disk — no registry calls).
//! - Canonical-digest computation for approval-record keying (I-2).
//!
//! It explicitly does **NOT** contain:
//! - Agent runtime semantics (no execution, no LLM calls, no tool dispatch).
//! - Network or AWS SDK dependencies (no `reqwest`, no `tokio`, no
//!   `oci-client`, no `aws-sdk-*` crate). Registry push/pull is a Phase 169+
//!   concern at the *caller's* call site, not this crate's.
//! - Secret **values** — config slots may declare that a secret is required
//!   by *name*, but the crate's types are structurally incapable of holding
//!   a resolved secret value (see `slot` module docs).
//!
//! Downstream plans (digest, slots, packages, OCI, tests) fill in the stub
//! modules declared below. This module tree and the `error` contract are
//! established first (Wave 0) so later, parallel Wave-2 plans never need to
//! edit `lib.rs` and never collide with each other over module wiring.

pub mod digest;
pub mod error;
pub mod oci;
pub mod package;
pub mod reference;
pub mod slot;
pub mod validation;

// ---------------------------------------------------------------------
// Crate-root re-exports (D-10 — dual-consumer ergonomics)
// ---------------------------------------------------------------------
//
// `cargo-pmcp` and the `pmcp.run` platform both consume this crate's primary
// types and functions without needing deep module paths (`pmcp_package::ServerPackage`
// rather than `pmcp_package::package::ServerPackage`). Deep paths still work —
// this block is additive, not a replacement for the module tree.

pub use digest::{canonicalize, manifest_digest, verify, ManifestDigest};
pub use error::{PackageError, Result};
pub use oci::{
    pack_agent, pack_server, pack_team, pack_workflow, unpack_agent, unpack_server, unpack_team,
    unpack_workflow, OciLayout,
};
pub use package::{
    AgentPackage, CedarPolicySet, DeployDescriptor, ServerPackage, TeamPackage, WorkflowManifest,
};
pub use reference::{ComponentRef, PinnedRef};
pub use slot::{aggregate, classify, detect_deviation, ConfigSlot, Deviation, SlotClass, SlotType};
pub use validation::validate as validate_deploy_descriptor;
