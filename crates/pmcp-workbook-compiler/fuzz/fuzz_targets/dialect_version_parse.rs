//! Fuzz target over the untrusted dialect-version string parser (WBDL-02, T-96-01,
//! ALWAYS: FUZZ).
//!
//! The `pmcp_dialect_version` cell value is attacker-controlled-input: a BA (or a
//! malicious workbook) authors an arbitrary string that the compiler parses during
//! ingest. This target feeds arbitrary UTF-8 strings into
//! [`pmcp_workbook_compiler::dialect_version::parse_dialect_version`].
//!
//! # Invariant
//!
//! Any input yields `Ok(DialectVersion)` OR a typed `CompileError` — NEVER a panic,
//! hang, or unbounded allocation. In particular a `u64`-overflowing component must
//! map to a typed error, not an integer-parse panic. This proves the parser stays
//! total over hostile bytes (T-96-01 tampering mitigation).

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_workbook_compiler::dialect_version::parse_dialect_version;

fuzz_target!(|data: &[u8]| {
    // Interpret the fuzz bytes as a version string (only valid UTF-8 reaches the
    // parser, matching how an ingested cell value is a Rust `String`). The parser
    // must return Ok or a typed CompileError for ANY input — never panic.
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_dialect_version(text);
    }
});
