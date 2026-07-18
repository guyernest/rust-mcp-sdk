//! Fuzz target for team-fs path resolution / jail-escape rejection.
//! Panic-free no-op stub — the path-resolution body is implemented in 109-02.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = data;
});
