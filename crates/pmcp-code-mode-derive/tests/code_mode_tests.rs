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
    t.compile_fail("tests/ui/missing_token_secret.rs");
    t.compile_fail("tests/ui/missing_code_executor.rs");
    t.compile_fail("tests/ui/non_send_field.rs");
    t.compile_fail("tests/ui/wrong_token_type.rs");
    // Compile-pass tests
    t.pass("tests/ui/valid_code_mode.rs");
    t.pass("tests/ui/valid_code_mode_context.rs");
    t.pass("tests/ui/valid_code_mode_language.rs");
}
