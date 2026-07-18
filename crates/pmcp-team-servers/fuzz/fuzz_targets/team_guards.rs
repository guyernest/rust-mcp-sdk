//! Fuzz target for team-mcp depth / self-call / ancestor-cycle guard parsing.
//! Panic-free no-op stub — the guard-fuzzing body is implemented in 109-05.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data;
});
