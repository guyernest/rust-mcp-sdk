//! Trybuild compile-pass and compile-fail tests for `#[derive(CodeMode)]`.
//!
//! These tests verify that the macro produces correct error messages for
//! missing fields and non-Send types, and compiles successfully for valid
//! struct definitions.

#[test]
fn code_mode_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/missing_config.rs");
    t.compile_fail("tests/ui/missing_policy_evaluator.rs");
    t.compile_fail("tests/ui/non_send_field.rs");
    t.compile_fail("tests/ui/wrong_token_type.rs");
    // Compile-pass test (a valid struct with all required fields)
    t.pass("tests/ui/valid_code_mode.rs");
}
