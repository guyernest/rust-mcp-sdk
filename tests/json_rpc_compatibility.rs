//! Tests to verify JSON-RPC 2.0 compatibility with TypeScript SDK
//!
//! This test suite addresses issue #38 by demonstrating that the Rust SDK
//! correctly serializes and deserializes standard JSON-RPC 2.0 format messages
//! that are fully compatible with Claude Code and other MCP clients.

use pmcp::shared::{StdioTransport, TransportMessage};
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::{
    ClientCapabilities, ClientRequest, Implementation, InitializeRequest, Request, RequestId,
};

#[test]
fn test_serialize_to_json_rpc_2_0_format() {
    // Create an initialize request using internal types
    let init_params = InitializeRequest::new(
        Implementation::new("test-client", "1.0.0"),
        ClientCapabilities::default(),
    );

    let request = Request::Client(Box::new(ClientRequest::Initialize(init_params)));
    let transport_msg = TransportMessage::Request {
        id: RequestId::Number(1),
        request,
    };

    // Serialize to JSON-RPC format
    let serialized = StdioTransport::serialize_message(&transport_msg).unwrap();
    let json_str = String::from_utf8(serialized).unwrap();

    // Verify it matches standard JSON-RPC 2.0 format
    assert!(json_str.contains(r#""jsonrpc":"2.0""#));
    assert!(json_str.contains(r#""id":1"#));
    assert!(json_str.contains(r#""method":"initialize""#));
    assert!(json_str.contains(r#""params":{"#));
    assert!(json_str.contains(r#""protocolVersion":"2025-06-18""#));

    // Full format check
    let expected = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    assert_eq!(json_str, expected);
}

#[test]
fn test_deserialize_typescript_sdk_format() {
    // This is the exact format sent by TypeScript SDK / Claude Code
    let typescript_format = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"1.0.0"}}}"#;

    // Parse the message
    let result = StdioTransport::parse_message(typescript_format.as_bytes());
    assert!(result.is_ok(), "Failed to parse TypeScript SDK format");

    // Verify the parsed message structure
    if let Ok(TransportMessage::Request { id, request }) = result {
        assert_eq!(id, RequestId::Number(1));
        if let Request::Client(client_req) = request {
            if let ClientRequest::Initialize(init) = *client_req {
                assert_eq!(init.protocol_version, "2025-06-18");
                assert_eq!(init.client_info.name, "claude-code");
                assert_eq!(init.client_info.version, "1.0.0");
            } else {
                panic!("Expected Initialize request");
            }
        } else {
            panic!("Expected Client request");
        }
    } else {
        panic!("Expected Request message");
    }
}

#[test]
fn test_roundtrip_compatibility() {
    // Create message, serialize, then deserialize
    let init_params = InitializeRequest::new(
        Implementation::new("test-client", "1.0.0"),
        ClientCapabilities::default(),
    );

    let request = Request::Client(Box::new(ClientRequest::Initialize(init_params.clone())));
    let original = TransportMessage::Request {
        id: RequestId::Number(42),
        request,
    };

    // Serialize
    let serialized = StdioTransport::serialize_message(&original).unwrap();

    // Deserialize
    let deserialized = StdioTransport::parse_message(&serialized).unwrap();

    // Verify roundtrip preservation
    if let TransportMessage::Request { id, request } = deserialized {
        assert_eq!(id, RequestId::Number(42));
        if let Request::Client(client_req) = request {
            if let ClientRequest::Initialize(init) = *client_req {
                assert_eq!(init.protocol_version, init_params.protocol_version);
                assert_eq!(init.client_info.name, init_params.client_info.name);
                assert_eq!(init.client_info.version, init_params.client_info.version);
            } else {
                panic!("Expected Initialize request");
            }
        } else {
            panic!("Expected Client request");
        }
    } else {
        panic!("Expected Request message");
    }
}

#[test]
fn test_parse_json_rpc_response() {
    // Test parsing a standard JSON-RPC 2.0 response
    let response_json = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{},"serverInfo":{"name":"test-server","version":"1.0.0"}}}"#;

    let result = StdioTransport::parse_message(response_json.as_bytes());
    assert!(result.is_ok(), "Failed to parse JSON-RPC response");

    if let Ok(TransportMessage::Response(response)) = result {
        assert_eq!(response.id, RequestId::Number(1));
        assert!(matches!(response.payload, ResponsePayload::Result(_)));
    } else {
        panic!("Expected Response message");
    }
}

#[test]
fn test_parse_json_rpc_error_response() {
    // Test parsing a standard JSON-RPC 2.0 error response
    let error_json =
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;

    let result = StdioTransport::parse_message(error_json.as_bytes());
    assert!(result.is_ok(), "Failed to parse JSON-RPC error response");

    if let Ok(TransportMessage::Response(response)) = result {
        assert_eq!(response.id, RequestId::Number(1));
        assert!(matches!(response.payload, ResponsePayload::Error(_)));
    } else {
        panic!("Expected Response message");
    }
}

#[test]
fn test_parse_json_rpc_notification() {
    // Test parsing a standard JSON-RPC 2.0 notification (no id field)
    let notification_json = r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"token-1","progress":50,"total":100}}"#;

    let result = StdioTransport::parse_message(notification_json.as_bytes());
    assert!(result.is_ok(), "Failed to parse JSON-RPC notification");

    assert!(
        matches!(result, Ok(TransportMessage::Notification(_))),
        "Expected Notification message"
    );
}

#[test]
fn test_various_client_requests() {
    // Test that various client request types serialize to correct JSON-RPC format
    let test_cases = vec![
        (
            "tools/list",
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
        ),
        (
            "resources/list",
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}}"#,
        ),
        (
            "prompts/list",
            r#"{"jsonrpc":"2.0","id":1,"method":"prompts/list","params":{}}"#,
        ),
    ];

    for (method, expected_prefix) in test_cases {
        // Create appropriate request based on method
        let request = match method {
            "tools/list" => Request::Client(Box::new(ClientRequest::ListTools(Default::default()))),
            "resources/list" => {
                Request::Client(Box::new(ClientRequest::ListResources(Default::default())))
            },
            "prompts/list" => {
                Request::Client(Box::new(ClientRequest::ListPrompts(Default::default())))
            },
            _ => panic!("Unknown method"),
        };

        let transport_msg = TransportMessage::Request {
            id: RequestId::Number(1),
            request,
        };

        let serialized = StdioTransport::serialize_message(&transport_msg).unwrap();
        let json_str = String::from_utf8(serialized).unwrap();

        assert!(
            json_str.starts_with(expected_prefix),
            "Method {} did not serialize correctly. Got: {}",
            method,
            json_str
        );
    }
}

/// This test demonstrates that the Rust SDK is fully compatible with
/// the TypeScript SDK's JSON-RPC 2.0 message format, addressing issue #38
#[test]
fn test_issue_38_json_rpc_compatibility() {
    println!("Testing issue #38: JSON-RPC 2.0 compatibility");

    // The issue claimed the SDK uses a non-standard format like:
    // {"id": 1, "request": {"Client": {"Initialize": {...}}}}
    // But this test proves it actually uses standard JSON-RPC 2.0:
    // {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {...}}

    let init_params = InitializeRequest::new(
        Implementation::new("claude-code", "1.0.0"),
        ClientCapabilities::default(),
    );

    let request = Request::Client(Box::new(ClientRequest::Initialize(init_params)));
    let transport_msg = TransportMessage::Request {
        id: RequestId::Number(1),
        request,
    };

    // Serialize and verify format
    let serialized = StdioTransport::serialize_message(&transport_msg).unwrap();
    let json_str = String::from_utf8(serialized.clone()).unwrap();

    // This is the standard JSON-RPC 2.0 format expected by Claude Code
    let expected = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"1.0.0"}}}"#;
    assert_eq!(json_str, expected, "Output format is not JSON-RPC 2.0");

    // Verify we can also parse TypeScript SDK messages
    let result = StdioTransport::parse_message(expected.as_bytes());
    assert!(
        result.is_ok(),
        "Cannot parse TypeScript SDK JSON-RPC format"
    );

    println!("✓ Rust SDK is fully compatible with JSON-RPC 2.0");
    println!("✓ Compatible with Claude Code and TypeScript SDK");
}
