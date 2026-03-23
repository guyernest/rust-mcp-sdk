//! Tests to verify JSON-RPC 2.0 compatibility for HTTP transport
//!
//! This test verifies that the HTTP transport correctly handles standard
//! JSON-RPC 2.0 messages from Claude Code and other MCP clients.

#[cfg(feature = "streamable-http")]
mod http_tests {
    use pmcp::shared::StdioTransport;

    #[test]
    fn test_parse_message_compatibility() {
        // Test that StdioTransport::parse_message correctly handles JSON-RPC 2.0
        let json_rpc = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"claude-code","version":"1.0.0"}}}"#;

        let result = StdioTransport::parse_message(json_rpc.as_bytes());
        assert!(
            result.is_ok(),
            "Failed to parse standard JSON-RPC 2.0 message: {:?}",
            result.err()
        );

        // Verify it parses to the correct internal format
        if let Ok(pmcp::shared::TransportMessage::Request { id, request }) = result {
            assert_eq!(id, pmcp::types::RequestId::Number(1));
            assert!(matches!(request, pmcp::types::Request::Client(_)));
        } else {
            panic!("Expected Request message");
        }
    }

    #[test]
    fn test_parse_tools_list_with_null_params() {
        // Test that we can parse tools/list with null params (common from Claude Code)
        let json_rpc = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":null}"#;

        let result = StdioTransport::parse_message(json_rpc.as_bytes());
        assert!(
            result.is_ok(),
            "Failed to parse tools/list with null params: {:?}",
            result.err()
        );

        // Verify it parses to the correct request type
        if let Ok(pmcp::shared::TransportMessage::Request { id, request }) = result {
            assert_eq!(id, pmcp::types::RequestId::Number(1));
            if let pmcp::types::Request::Client(boxed) = request {
                assert!(
                    matches!(*boxed, pmcp::types::ClientRequest::ListTools(_)),
                    "Expected ListTools request"
                );
            } else {
                panic!("Expected Client request");
            }
        } else {
            panic!("Expected Request message");
        }
    }

    #[test]
    fn test_serialize_message_compatibility() {
        // Test that StdioTransport::serialize_message produces JSON-RPC 2.0
        use pmcp::shared::TransportMessage;
        use pmcp::types::{
            ClientCapabilities, ClientRequest, Implementation, InitializeRequest, Request,
            RequestId,
        };

        let init_params = InitializeRequest::new(
            Implementation::new("test-client", "1.0.0"),
            ClientCapabilities::default(),
        );

        let request = Request::Client(Box::new(ClientRequest::Initialize(init_params)));
        let transport_msg = TransportMessage::Request {
            id: RequestId::Number(1),
            request,
        };

        let serialized = StdioTransport::serialize_message(&transport_msg).unwrap();
        let json_str = String::from_utf8(serialized).unwrap();

        // Verify it's standard JSON-RPC 2.0 format
        assert!(json_str.contains(r#""jsonrpc":"2.0""#));
        assert!(json_str.contains(r#""method":"initialize""#));
        assert!(json_str.contains(r#""params":{"#));
        assert!(!json_str.contains(r#""request":{"#)); // Should NOT contain internal format
    }
}
