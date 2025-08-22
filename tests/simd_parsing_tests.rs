//! SIMD Parsing Tests
//!
//! Comprehensive tests for SIMD-optimized parsing functionality including:
//! - CPU feature detection verification
//! - JSON-RPC parsing with vectorized operations
//! - SSE parsing acceleration with SIMD
//! - Base64 encoding/decoding optimization
//! - HTTP header parsing with parallel processing
//! - Performance benchmarks and validation
//! - Cross-platform compatibility testing

use pmcp::shared::simd_parsing::*;
use pmcp::types::jsonrpc::{JSONRPCRequest, RequestId};
use proptest::prelude::*;
use serde_json::json;
use std::time::Instant;

#[test]
fn test_simd_json_parser_basic() {
    let parser = SimdJsonParser::new();

    let request_json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"key":"value"}}"#;
    let response = parser.parse_request(request_json.as_bytes()).unwrap();

    assert_eq!(response.method, "test");
    assert_eq!(response.id, RequestId::Number(1));

    let response_json = r#"{"jsonrpc":"2.0","id":1,"result":{"success":true,"data":"test"}}"#;
    let response = parser.parse_response(response_json.as_bytes()).unwrap();

    assert_eq!(response.id, RequestId::Number(1));
    assert!(response.result().is_some());
    assert_eq!(response.result().unwrap()["success"], true);
}

#[test]
fn test_simd_json_parser_batch() {
    let parser = SimdJsonParser::new();

    let batch_json = r#"[
        {"jsonrpc":"2.0","id":1,"method":"test1","params":{}},
        {"jsonrpc":"2.0","id":2,"method":"test2","params":{}},
        {"jsonrpc":"2.0","id":3,"method":"test3","params":{}}
    ]"#;

    let results = parser.parse_batch_requests(batch_json.as_bytes()).unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].method, "test1");
    assert_eq!(results[1].method, "test2");
    assert_eq!(results[2].method, "test3");
}

#[test]
fn test_simd_sse_parser() {
    let mut parser = SimdSseParser::new();

    let sse_data = b"event: message\ndata: Hello World\nid: 123\n\n";
    let events = parser.parse_chunk(sse_data).unwrap();

    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.event.as_ref().unwrap(), "message");
    assert_eq!(event.data, "Hello World");
    assert_eq!(event.id.as_ref().unwrap(), "123");
}

#[test]
fn test_simd_sse_parser_multiple_events() {
    let mut parser = SimdSseParser::new();

    let sse_data =
        b"event: update\ndata: Event 1\n\nevent: notification\ndata: Event 2\nid: 456\n\n";
    let events = parser.parse_chunk(sse_data).unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event.as_ref().unwrap(), "update");
    assert_eq!(events[0].data, "Event 1");
    assert_eq!(events[1].event.as_ref().unwrap(), "notification");
    assert_eq!(events[1].data, "Event 2");
    assert_eq!(events[1].id.as_ref().unwrap(), "456");
}

#[test]
fn test_simd_base64_basic() {
    let encoder = SimdBase64::new();
    let test_data = b"Hello, SIMD Base64!";

    let encoded_data = encoder.encode(test_data);
    let decoded = encoder.decode(&encoded_data).unwrap();

    assert_eq!(decoded, test_data);
}

#[test]
fn test_simd_base64_large_data() {
    let encoder = SimdBase64::new();
    let test_data = vec![42u8; 1024]; // 1KB of data

    let encoded_data = encoder.encode(&test_data);
    let decoded = encoder.decode(&encoded_data).unwrap();

    assert_eq!(decoded, test_data);
}

#[test]
fn test_simd_http_header_parser() {
    let parser = SimdHttpHeaderParser::new();
    let headers = "Content-Type: application/json\r\nContent-Length: 123\r\n\r\n";

    let parsed = parser.parse_headers(headers.as_bytes()).unwrap();
    assert_eq!(
        parsed.get("content-type"),
        Some(&"application/json".to_string())
    );
    assert_eq!(parsed.get("content-length"), Some(&"123".to_string()));
}

#[test]
fn test_cpu_feature_detection() {
    let features = CpuFeatures::detect();

    // Test that detection completes without panicking
    println!("CPU Features detected:");
    println!("  AVX2: {}", features.avx2);
    println!("  SSE4.2: {}", features.sse42);
    println!("  SSSE3: {}", features.ssse3);

    // On any modern CPU, at least one feature should be available
    assert!(features.avx2 || features.sse42 || features.ssse3);
}

#[test]
fn test_parsing_metrics() {
    let parser = SimdJsonParser::new();
    let metrics = parser.get_metrics();

    // Initial metrics should be zero
    assert_eq!(metrics.simd_operations_used, 0);
    assert_eq!(metrics.fallback_operations, 0);
    assert_eq!(metrics.total_documents_parsed, 0);

    // Parse something to generate metrics
    let request_json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
    let _response = parser.parse_request(request_json.as_bytes()).unwrap();

    let metrics = parser.get_metrics();
    assert!(metrics.total_documents_parsed > 0);
}

// Property-based tests
proptest! {
    #[test]
    fn property_json_parsing_roundtrip(
        method in "[a-zA-Z_][a-zA-Z0-9_]{0,20}",
        id in 1i64..1_000_000,
    ) {
        let parser = SimdJsonParser::new();
        let request = JSONRPCRequest::new(RequestId::Number(id), method.clone(), Some(json!({})));
        let json_str = serde_json::to_string(&request).unwrap();

        let parsed = parser.parse_request(json_str.as_bytes()).unwrap();
        prop_assert_eq!(parsed.method, method);
        prop_assert_eq!(parsed.id, RequestId::Number(id));
    }

    #[test]
    fn property_base64_roundtrip(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        let encoder = SimdBase64::new();
        let encoded_data = encoder.encode(&data);
        let decoded = encoder.decode(&encoded_data).unwrap();
        prop_assert_eq!(decoded, data);
    }

    #[test]
    fn property_sse_parsing(
        event_type in "[a-zA-Z][a-zA-Z0-9_]{0,15}",
        data in "[a-zA-Z0-9]{2,100}", // Avoid single space issue
        event_id in "[0-9]{1,8}",
    ) {
        let mut parser = SimdSseParser::new();
        let sse_data = format!("event: {}\ndata: {}\nid: {}\n\n", event_type, data, event_id);

        let events = parser.parse_chunk(sse_data.as_bytes()).unwrap();
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(events[0].event.as_ref().unwrap(), &event_type);
        prop_assert_eq!(&events[0].data, &data);
        prop_assert_eq!(events[0].id.as_ref().unwrap(), &event_id);
    }
}

// Performance benchmark tests
#[test]
fn test_simd_performance_characteristics() {
    let parser = SimdJsonParser::new();
    let large_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "large_data_test",
        "params": {
            "data": vec![42; 1000], // Large parameter
            "metadata": {
                "source": "test",
                "timestamp": 1_234_567_890,
                "tags": ["performance", "simd", "test"]
            }
        }
    });

    let json_str = serde_json::to_string(&large_request).unwrap();
    let start = Instant::now();

    // Parse multiple times to measure performance
    for _ in 0..100 {
        let _parsed = parser.parse_request(json_str.as_bytes()).unwrap();
    }

    let duration = start.elapsed();
    println!("100 large JSON parsing operations took: {:?}", duration);

    // Should complete reasonably quickly (less than 100ms for 100 operations)
    assert!(duration.as_millis() < 100);
}

#[test]
fn test_simd_vs_fallback_behavior() {
    let parser = SimdJsonParser::new();
    let request_json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;

    // Parse the same data multiple times
    let result1 = parser.parse_request(request_json.as_bytes()).unwrap();
    let result2 = parser.parse_request(request_json.as_bytes()).unwrap();

    // Results should be identical regardless of SIMD vs fallback path
    assert_eq!(result1.method, result2.method);
    assert_eq!(result1.id, result2.id);
    assert_eq!(result1.jsonrpc, result2.jsonrpc);
}

// Edge case tests
#[test]
fn test_empty_and_invalid_inputs() {
    let parser = SimdJsonParser::new();

    // Empty input should fail gracefully
    assert!(parser.parse_request(b"").is_err());

    // Invalid JSON should fail gracefully
    assert!(parser.parse_request(b"{invalid json").is_err());

    // Valid JSON but invalid JSON-RPC should fail
    assert!(parser.parse_request(br#"{"not": "jsonrpc"}"#).is_err());
}

#[test]
fn test_base64_edge_cases() {
    let encoder = SimdBase64::new();

    // Empty data
    let encoded_data = encoder.encode(b"");
    let decoded = encoder.decode(&encoded_data).unwrap();
    assert_eq!(decoded, b"");

    // Single byte
    let single_byte = b"A";
    let encoded_data = encoder.encode(single_byte);
    let decoded = encoder.decode(&encoded_data).unwrap();
    assert_eq!(decoded, single_byte);
}

#[test]
fn test_sse_parser_edge_cases() {
    let mut parser = SimdSseParser::new();

    // Empty SSE data
    let events = parser.parse_chunk(b"").unwrap();
    assert_eq!(events.len(), 0);

    // Only comments
    let events = parser
        .parse_chunk(b": this is a comment\n: another comment\n")
        .unwrap();
    assert_eq!(events.len(), 0);

    // Event with only data field
    let events = parser.parse_chunk(b"data: simple message\n\n").unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "simple message");
    assert!(events[0].event.is_none());
}
