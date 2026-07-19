//! Conformance harness: replay the `contracts/team-servers/fixtures/**` cases
//! against live reference servers over an in-process [`crate::DuplexTransport`].
//!
//! Empty documented seam — implemented in 109-07 (TEAM-06).

pub mod runner;
