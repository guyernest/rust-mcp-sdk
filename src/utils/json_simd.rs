// High-performance JSON parsing with SIMD acceleration
// Falls back to standard serde_json when SIMD is not available

#![allow(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Error as JsonError, Value};

/// Validate UTF-8 via the SIMD helper, returning a `JsonError` on failure.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn validate_utf8_or_err(input: &[u8]) -> Result<(), JsonError> {
    let ok = unsafe { crate::simd::json::validate_utf8_simd(input) };
    if ok {
        return Ok(());
    }
    Err(serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Invalid UTF-8",
    )))
}

/// Strip whitespace bytes (outside of JSON strings) identified by SIMD
/// position scanning. Returns a new `Vec<u8>` with only significant bytes.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn strip_whitespace_simd_aware(input: &[u8], ws_positions: &[usize]) -> Vec<u8> {
    let mut cleaned = Vec::with_capacity(input.len().saturating_sub(ws_positions.len()));
    let mut state = StripState::default();

    for (i, &byte) in input.iter().enumerate() {
        if let Some(b) = state.next_output_byte(byte, i, ws_positions) {
            cleaned.push(b);
        }
    }
    cleaned
}

/// State machine for whitespace stripping inside/outside JSON string literals.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[derive(Default)]
struct StripState {
    in_string: bool,
    escape_next: bool,
}

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
impl StripState {
    /// Process one byte and decide whether it ends up in the cleaned output.
    ///
    /// Returns `Some(byte)` if the byte should be emitted, `None` if it is a
    /// whitespace byte (outside a string) at a SIMD-detected position.
    fn next_output_byte(&mut self, byte: u8, index: usize, ws_positions: &[usize]) -> Option<u8> {
        if self.escape_next {
            self.escape_next = false;
            return Some(byte);
        }
        if byte == b'\\' && self.in_string {
            self.escape_next = true;
            return Some(byte);
        }
        if byte == b'"' {
            self.in_string = !self.in_string;
            return Some(byte);
        }
        if !self.in_string && ws_positions.binary_search(&index).is_ok() {
            return None;
        }
        Some(byte)
    }
}

/// Parse JSON with SIMD acceleration when available.
///
/// Refactored in 75-01 Task 1a-B (P1): extracted
/// `validate_utf8_or_err` and `strip_whitespace_simd_aware` so this
/// function becomes a short control-flow dispatch around the SIMD hot path.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
pub fn parse_json_fast<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T, JsonError> {
    if !is_x86_feature_detected!("avx2") {
        return serde_json::from_slice(input);
    }
    validate_utf8_or_err(input)?;

    let ws_positions = unsafe { crate::simd::json::find_whitespace_simd(input) };

    // Minimal whitespace: parse directly.
    if ws_positions.len() < input.len() / 10 {
        return serde_json::from_slice(input);
    }

    // Strip unnecessary whitespace first.
    let cleaned = strip_whitespace_simd_aware(input, &ws_positions);
    serde_json::from_slice(&cleaned)
}

/// Parse JSON - fallback for non-SIMD platforms
#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
pub fn parse_json_fast<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T, JsonError> {
    serde_json::from_slice(input)
}

/// Serialize JSON with SIMD acceleration when available
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
pub fn serialize_json_fast<T: Serialize>(value: &T) -> Result<Vec<u8>, JsonError> {
    let json = serde_json::to_vec(value)?;

    // Runtime feature detection
    if is_x86_feature_detected!("avx2") {
        // Use SIMD to validate output
        unsafe {
            if !crate::simd::json::validate_utf8_simd(&json) {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Generated invalid UTF-8",
                )));
            }
        }
    }

    Ok(json)
}

/// Serialize JSON - fallback for non-SIMD platforms
#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
pub fn serialize_json_fast<T: Serialize>(value: &T) -> Result<Vec<u8>, JsonError> {
    serde_json::to_vec(value)
}

/// Batch JSON parsing - sequential version
/// For parallel processing, use the parallel_batch module with rayon feature
pub fn parse_json_batch<T: for<'de> Deserialize<'de>>(
    inputs: &[&[u8]],
) -> Vec<Result<T, JsonError>> {
    inputs.iter().map(|input| parse_json_fast(input)).collect()
}

/// Mutable indent/in-string state used while pretty-printing the SIMD path.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
struct PrettyPrintCtx {
    indent: usize,
    in_string: bool,
    escape_next: bool,
}

/// Append an open-brace/bracket byte with line break + indent increase.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn push_open_bracket(result: &mut String, byte: u8, ctx: &mut PrettyPrintCtx) {
    result.push(byte as char);
    ctx.indent += 2;
    result.push('\n');
    result.push_str(&" ".repeat(ctx.indent));
}

/// Append a close-brace/bracket byte with indent decrease + preceding line break.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn push_close_bracket(result: &mut String, byte: u8, ctx: &mut PrettyPrintCtx) {
    ctx.indent = ctx.indent.saturating_sub(2);
    result.push('\n');
    result.push_str(&" ".repeat(ctx.indent));
    result.push(byte as char);
}

/// Handle a single byte in the out-of-string pretty-printer path.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn append_non_string_byte(result: &mut String, byte: u8, ctx: &mut PrettyPrintCtx) {
    match byte {
        b'{' | b'[' => push_open_bracket(result, byte, ctx),
        b'}' | b']' => push_close_bracket(result, byte, ctx),
        b',' => {
            result.push(',');
            result.push('\n');
            result.push_str(&" ".repeat(ctx.indent));
        },
        b':' => result.push_str(": "),
        b' ' | b'\t' | b'\n' | b'\r' => { /* skip whitespace */ },
        _ => result.push(byte as char),
    }
}

/// Process a single byte of the compact JSON buffer, mutating the result
/// string and context state. Collapsed from a 3-level nested match+match
/// into a flat early-return chain.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn process_pretty_byte(
    result: &mut String,
    byte: u8,
    i: usize,
    escapes: &[usize],
    ctx: &mut PrettyPrintCtx,
) {
    if ctx.escape_next {
        ctx.escape_next = false;
        result.push(byte as char);
        return;
    }
    if escapes.binary_search(&i).is_ok() {
        if byte == b'\\' && ctx.in_string {
            ctx.escape_next = true;
        } else if byte == b'"' {
            ctx.in_string = !ctx.in_string;
        }
    }
    if ctx.in_string {
        result.push(byte as char);
    } else {
        append_non_string_byte(result, byte, ctx);
    }
}

/// SIMD-accelerated pretty printer body (wraps the byte loop).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
fn pretty_print_simd(value: &Value) -> Result<String, JsonError> {
    let compact = serde_json::to_vec(value)?;
    let escapes = unsafe { crate::simd::json::find_escapes_simd(&compact) };
    let mut result = String::with_capacity(compact.len() * 2);
    let mut ctx = PrettyPrintCtx {
        indent: 0,
        in_string: false,
        escape_next: false,
    };
    for (i, &byte) in compact.iter().enumerate() {
        process_pretty_byte(&mut result, byte, i, &escapes, &mut ctx);
    }
    Ok(result)
}

/// Fast JSON pretty printing.
///
/// Refactored in 75-01 Task 1a-B (P1): extracted `PrettyPrintCtx` state
/// struct, `process_pretty_byte`, `append_non_string_byte`,
/// `push_open_bracket`, `push_close_bracket`, and
/// `pretty_print_simd` so this orchestrator is a short cfg+feature-gate.
pub fn pretty_print_fast(value: &Value) -> Result<String, JsonError> {
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            return pretty_print_simd(value);
        }
    }
    // Fallback to standard pretty printing
    serde_json::to_string_pretty(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_json_fast() {
        let input = r#"{"name": "test", "value": 42, "nested": {"array": [1, 2, 3]}}"#;
        let result: Value = parse_json_fast(input.as_bytes()).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
        assert_eq!(result["nested"]["array"][0], 1);
    }

    #[test]
    fn test_serialize_json_fast() {
        let value = json!({
            "message": "Hello, SIMD!",
            "numbers": [1, 2, 3, 4, 5],
            "nested": {
                "key": "value"
            }
        });

        let serialized = serialize_json_fast(&value).unwrap();
        let parsed: Value = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(parsed, value);
    }

    #[test]
    fn test_batch_parsing() {
        let inputs = vec![
            r#"{"id": 1}"#.as_bytes(),
            r#"{"id": 2}"#.as_bytes(),
            r#"{"id": 3}"#.as_bytes(),
        ];

        let results: Vec<Result<Value, _>> = parse_json_batch(&inputs);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap()["id"], 1);
        assert_eq!(results[1].as_ref().unwrap()["id"], 2);
        assert_eq!(results[2].as_ref().unwrap()["id"], 3);
    }

    #[test]
    fn test_pretty_print() {
        let value = json!({"compact": true, "array": [1, 2, 3]});
        let pretty = pretty_print_fast(&value).unwrap();

        assert!(pretty.contains('\n'));
        assert!(pretty.contains("  "));
    }
}
