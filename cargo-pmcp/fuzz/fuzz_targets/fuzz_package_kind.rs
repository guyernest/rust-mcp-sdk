//! Phase 110-06 CLI-04: fuzz target for the untrusted `.pmcp` manifest-parse
//! boundary.
//!
//! Feeds arbitrary/adversarial byte sequences drawn from an untrusted `.pmcp`
//! package straight into `cargo_pmcp::package_kind::artifact_type_from_manifest_json`
//! (the RAW manifest-JSON parser — NO utf8 pre-filter, the parser must tolerate
//! arbitrary bytes), and chains `detect_kind` on any extracted candidate string.
//! This is the REAL untrusted seam `package show` runs (bytes → artifactType →
//! kind), not just an 8-constant string match (Codex 110-06 MEDIUM).
//!
//! Invariant: neither function panics or hangs on any input.
//!
//! Threat model: T-110-05-03 / T-110-06-01 (parser DoS — adversarial manifest
//! bytes crashing or hanging the `package show` parse+dispatch path).
//!
//! Run with: `cargo +nightly fuzz run fuzz_package_kind`
//! Quick smoke: `cargo +nightly fuzz run fuzz_package_kind -- -max_total_time=60`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Parse the RAW bytes — the manifest parser is the untrusted boundary, so it
    // must tolerate arbitrary (non-UTF-8, non-JSON, adversarial) input. Must not
    // panic.
    if let Some(candidate) = cargo_pmcp::package_kind::artifact_type_from_manifest_json(data) {
        // Chain the full extraction path: an extracted candidate string feeds the
        // kind dispatch. Must not panic.
        let _ = cargo_pmcp::package_kind::detect_kind(&candidate);
    }
});
