//! Compile-fail harness — proves `SecretValue`'s negative trait invariant
//! at compile time (per Phase 83 review R5 + R6; replaces commented-out
//! assertions which carry no enforcement weight).
//!
//! Each source file under `tests/compile_fail/` MUST fail to compile. The
//! harness invokes them via `trybuild` and asserts that the failures land on
//! the expected missing-trait diagnostic.

#[test]
fn secret_value_lacks_dangerous_traits() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
