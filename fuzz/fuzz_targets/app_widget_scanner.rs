#![no_main]

use libfuzzer_sys::fuzz_target;
use mcp_tester::AppValidationMode;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let validator = mcp_tester::AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let _ = validator.validate_widgets(&[(
        "fuzz-tool".to_string(),
        "ui://fuzz".to_string(),
        s.to_string(),
    )]);
});
