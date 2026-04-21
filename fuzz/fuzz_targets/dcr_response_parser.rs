//! Fuzz target for `pmcp::client::oauth::DcrResponse` JSON parser.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run dcr_response_parser`.
//!
//! Invariant: `serde_json::from_slice::<DcrResponse>` must never panic on
//! arbitrary bytes. Error paths are acceptable; panics are not. Also validates
//! that a hostile registration_endpoint returning malformed JSON can't crash
//! the SDK's DCR parser (threat T-74-C).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must return Result, never panic.
    let _ = serde_json::from_slice::<pmcp::client::oauth::DcrResponse>(data);
});
