#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_code_mode::GraphQLValidator;

fuzz_target!(|data: &[u8]| {
    // GraphQL validator must never panic on arbitrary input.
    // It may return Ok (valid GraphQL) or Err (parse/validation error),
    // but must not crash regardless of the input bytes.
    if let Ok(input) = std::str::from_utf8(data) {
        let validator = GraphQLValidator::default();
        let _ = validator.validate(input);
    }
});
