#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_code_mode::{ApprovalToken, HmacTokenGenerator, TokenGenerator, TokenSecret};

fuzz_target!(|data: &[u8]| {
    // Fuzz token verification with random data.
    // A fixed secret + random "token" string -- must never panic,
    // must always return Err for invalid/random tokens.

    // Strategy 1: Treat entire input as an encoded token string
    if let Ok(token_str) = std::str::from_utf8(data) {
        // Try to decode and verify random token strings
        if let Ok(token) = ApprovalToken::decode(token_str) {
            let secret =
                TokenSecret::new(b"fuzz-secret-key-32-bytes-long!!!".to_vec());
            let generator = HmacTokenGenerator::new(secret);
            // verify() should return Err for random tokens, never panic
            let _ = generator.verify(&token);
            // verify_code() should return Err for random tokens, never panic
            let _ = generator.verify_code("SELECT 1", &token);
        }
    }

    // Strategy 2: Split input into token + code pair for verify_code fuzzing
    if data.len() > 4 {
        let split = (data[0] as usize) % data.len().saturating_sub(1) + 1;
        if let (Ok(token_str), Ok(code)) = (
            std::str::from_utf8(&data[1..split]),
            std::str::from_utf8(&data[split..]),
        ) {
            if let Ok(token) = ApprovalToken::decode(token_str) {
                let secret =
                    TokenSecret::new(b"fuzz-secret-key-32-bytes-long!!!".to_vec());
                let generator = HmacTokenGenerator::new(secret);
                let _ = generator.verify(&token);
                let _ = generator.verify_code(code, &token);
            }
        }
    }
});
