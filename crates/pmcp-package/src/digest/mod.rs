//! Canonical digest computation for AI-Package manifests (I-1/I-2).
//!
//! - [`canonical`] — the [`ManifestDigest`] construct-only newtype plus the
//!   [`canonicalize`]/[`manifest_digest`] pure functions (byte-stable
//!   canonicalization via `olpc-cjson`).
//! - [`verify`] — the [`verify`] tamper-detection primitive: recomputes a
//!   digest over actual bytes and compares against a declared digest.

pub mod canonical;
pub mod verify;

pub use canonical::{canonicalize, manifest_digest, ManifestDigest};
pub use verify::verify;
