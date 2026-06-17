//! Fuzz target over the untrusted formula parser (T-93-03-DOS, ALWAYS: FUZZ).
//!
//! The formula parser is an attacker-controlled-input surface: a BA (or a
//! malicious workbook) authors arbitrary formula text that crosses into the IR
//! builder. This target feeds arbitrary UTF-8 strings as formula text into
//! [`pmcp_workbook_compiler::formula::parse`].
//!
//! # Invariant
//!
//! Any input yields `Ok(Expr)` OR a typed `ParseError` — NEVER a panic, hang,
//! or unbounded recursion/allocation. The `MAX_PARSE_DEPTH` recursion guard
//! (enforced in `parser.rs`, NOT discovered here) bounds recursion; the
//! `MAX_FORMULA_LEN` lexer guard bounds input length. This target asserts the
//! parser stays total over hostile bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_workbook_compiler::formula::parse;

fuzz_target!(|data: &[u8]| {
    // Interpret the fuzz bytes as formula text (lossy UTF-8 so every byte
    // sequence is exercised). The parser must return Ok or a typed ParseError
    // for ANY input — never panic, never hang.
    if let Ok(text) = std::str::from_utf8(data) {
        // The location args (sheet/addr) are inert in the parser; any value is fine.
        let _ = parse(text, "fuzz", "A1");
    }
});
