//! Trybuild compile-pass and compile-fail tests for `#[derive(CodeMode)]`.
//!
//! These tests verify that the macro produces correct error messages for
//! missing fields and non-Send types, and compiles successfully for valid
//! struct definitions.

/// Ensures the derive macro's `gen_validation_call` string matching stays in sync
/// with the runtime `CodeLanguage` enum. If a variant is added to one side but not
/// the other, this test fails.
#[test]
fn language_dispatch_covers_all_code_language_variants() {
    use pmcp_code_mode::CodeLanguage;

    // All strings the derive macro's gen_validation_call() accepts.
    // When adding a new language, add its string(s) here too.
    let derive_accepted = ["graphql", "javascript", "js", "sql", "mcp"];

    // Forward: every string the derive macro accepts must be recognized by CodeLanguage
    for lang in &derive_accepted {
        assert!(
            CodeLanguage::from_attr(lang).is_some(),
            "derive macro accepts \"{lang}\" but CodeLanguage::from_attr does not"
        );
    }

    // Reverse: every CodeLanguage variant's canonical string must be in the derive list
    let all_variants = [
        CodeLanguage::GraphQL,
        CodeLanguage::JavaScript,
        CodeLanguage::Sql,
        CodeLanguage::Mcp,
    ];
    for variant in &all_variants {
        assert!(
            derive_accepted.contains(&variant.as_str()),
            "CodeLanguage::{variant:?} has as_str()=\"{}\" but derive macro does not list it",
            variant.as_str()
        );
    }
}

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
