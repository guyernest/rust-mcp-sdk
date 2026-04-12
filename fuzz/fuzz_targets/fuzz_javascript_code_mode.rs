#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_code_mode::JavaScriptValidator;

fuzz_target!(|data: &[u8]| {
    // JavaScript validator must never panic on arbitrary input.
    // It may return Ok (valid JS subset) or Err (parse/safety error),
    // but must not crash regardless of the input bytes.
    if let Ok(input) = std::str::from_utf8(data) {
        let validator = JavaScriptValidator::default();
        let _ = validator.validate(input);
    }
});
