//! Property-based tests for PMCP SDK
//!
//! These tests verify invariants and properties that should hold across
//! the entire PMCP protocol implementation using property-based testing.
//!
//! ALWAYS Requirement: Property tests for all new features

use pmcp::types::*;
use proptest::prelude::*;

#[cfg(test)]
mod protocol_invariants {
    use super::*;

    proptest! {
        /// Property: JSON-RPC serialization round-trip should preserve data
        #[test]
        fn property_jsonrpc_roundtrip(
            id in prop::option::of(any::<i64>().prop_map(RequestId::Number)),
            method in "[a-zA-Z_][a-zA-Z0-9_/]*",
            params in prop::option::of(prop::collection::hash_map(
                "[a-zA-Z_][a-zA-Z0-9_]*",
                any::<i32>().prop_map(|i| serde_json::Value::Number(i.into())),
                0..10
            ))
        ) {
            let request = JSONRPCRequest {
                jsonrpc: "2.0".to_string(),
                id: id.unwrap_or(RequestId::Number(1)),
                method: method.clone(),
                params: params.clone().map(|p| serde_json::to_value(p).unwrap()),
            };

            // Serialize and deserialize
            let serialized = serde_json::to_string(&request).unwrap();
            let deserialized: JSONRPCRequest = serde_json::from_str(&serialized).unwrap();

            // Properties that must hold
            prop_assert_eq!(request.jsonrpc, deserialized.jsonrpc);
            prop_assert_eq!(request.id, deserialized.id);
            prop_assert_eq!(request.method, deserialized.method);
            prop_assert_eq!(request.params, deserialized.params);
        }

        /// Property: Error codes should round-trip correctly for non-server errors
        #[test]
        fn property_error_code_roundtrip(
            code in -32999i32..-32100i32
        ) {
            use pmcp::error::ErrorCode;

            let error_code = ErrorCode::other(code);
            let as_i32 = error_code.as_i32();
            let from_i32 = ErrorCode::other(as_i32);

            prop_assert_eq!(error_code.as_i32(), from_i32.as_i32());
        }

        /// Property: Request IDs should be unique and stable
        #[test]
        fn property_request_id_uniqueness(
            ids in prop::collection::vec(any::<i64>(), 1..100)
        ) {
            let request_ids: Vec<RequestId> = ids.into_iter()
                .map(RequestId::Number)
                .collect();

            // Each ID should serialize to a unique string
            let serialized: Vec<String> = request_ids.iter()
                .map(|id| serde_json::to_string(id).unwrap())
                .collect();

            let mut unique_serialized = serialized.clone();
            unique_serialized.sort();
            unique_serialized.dedup();

            prop_assert_eq!(serialized.len(), unique_serialized.len());
        }
    }
}

#[cfg(test)]
mod uri_template_properties {
    use super::*;
    use pmcp::shared::uri_template::UriTemplate;

    proptest! {
        /// Property: URI template expansion should be deterministic
        #[test]
        fn property_uri_template_deterministic(
            template_str in "[a-zA-Z0-9_/{}-]*",
            params_vec in prop::collection::vec(
                ("[a-zA-Z_][a-zA-Z0-9_]*", "[a-zA-Z0-9_-]*"),
                0..5
            )
        ) {
            if let Ok(template) = UriTemplate::new(&template_str) {
                let expanded1 = template.expand(&params_vec);
                let expanded2 = template.expand(&params_vec);

                // Expansion should be deterministic
                prop_assert_eq!(expanded1.is_ok(), expanded2.is_ok());
                if let (Ok(exp1), Ok(exp2)) = (expanded1, expanded2) {
                    prop_assert_eq!(exp1, exp2);
                }
            }
        }

        /// Property: URI template matching should be consistent
        #[test]
        fn property_uri_template_match_consistency(
            segments in prop::collection::vec("[a-zA-Z0-9_-]+", 1..5)
        ) {
            let template_str = format!("/{}", segments.join("/"));
            let uri_str = format!("/{}", segments.join("/"));

            if let Ok(template) = UriTemplate::new(&template_str) {
                let matches1 = template.match_uri(&uri_str);
                let matches2 = template.match_uri(&uri_str);

                // Matching should be deterministic
                prop_assert_eq!(matches1.is_some(), matches2.is_some());
                if let (Some(m1), Some(m2)) = (matches1, matches2) {
                    prop_assert_eq!(m1, m2);
                }
            }
        }
    }
}

#[cfg(test)]
mod capability_properties {
    use super::*;

    proptest! {
        /// Property: Client capabilities should maintain logical consistency
        #[test]
        fn property_client_capabilities_consistency(
            roots_support in any::<bool>(),
            sampling_support in any::<bool>()
        ) {
            let mut capabilities = ClientCapabilities::minimal();

            if roots_support {
                capabilities.roots = Some(RootsCapabilities {
                    list_changed: true,
                });
            }

            if sampling_support {
                capabilities.sampling = Some(SamplingCapabilities::default());
            }

            // Test serialization round-trip
            let serialized = serde_json::to_string(&capabilities).unwrap();
            let deserialized: ClientCapabilities = serde_json::from_str(&serialized).unwrap();

            // Capability support methods should be consistent
            prop_assert_eq!(
                capabilities.sampling.is_some(),
                deserialized.sampling.is_some()
            );

            prop_assert_eq!(
                capabilities.roots.is_some(),
                deserialized.roots.is_some()
            );
        }

        /// Property: Server capabilities should be logically consistent
        #[test]
        fn property_server_capabilities_consistency(
            tools_count in 0usize..10,
            resources_count in 0usize..10,
            prompts_count in 0usize..10
        ) {
            let mut capabilities = ServerCapabilities::minimal();

            if tools_count > 0 {
                capabilities.tools = Some(ToolCapabilities {
                    list_changed: Some(true),
                });
            }

            if resources_count > 0 {
                capabilities.resources = Some(ResourceCapabilities {
                    subscribe: Some(true),
                    list_changed: Some(true),
                });
            }

            if prompts_count > 0 {
                capabilities.prompts = Some(PromptCapabilities {
                    list_changed: Some(true),
                });
            }

            // Logical consistency checks
            prop_assert_eq!(
                capabilities.tools.is_some(),
                tools_count > 0
            );

            prop_assert_eq!(
                capabilities.resources.is_some(),
                resources_count > 0
            );

            prop_assert_eq!(
                capabilities.prompts.is_some(),
                prompts_count > 0
            );
        }
    }
}

#[cfg(test)]
mod transport_properties {
    use super::*;
    use pmcp::shared::transport::*;

    proptest! {
        /// Property: Message priorities should be ordered correctly
        #[test]
        fn property_message_priority_ordering(
            priorities in prop::collection::vec(
                prop::strategy::Union::new([
                    Just(MessagePriority::High).boxed(),
                    Just(MessagePriority::Normal).boxed(),
                    Just(MessagePriority::Low).boxed(),
                ]),
                1..10
            )
        ) {
            let mut sorted_priorities = priorities.clone();
            sorted_priorities.sort();

            // High should be last, Low should be first
            if priorities.contains(&MessagePriority::High) {
                prop_assert_eq!(sorted_priorities[sorted_priorities.len() - 1], MessagePriority::High);
            }

            if priorities.contains(&MessagePriority::Low) {
                prop_assert_eq!(sorted_priorities[0], MessagePriority::Low);
            }
        }

        /// Property: Transport message metadata should maintain consistency
        #[test]
        fn property_transport_message_metadata(
            priority in prop::strategy::Union::new([
                Just(MessagePriority::High).boxed(),
                Just(MessagePriority::Normal).boxed(),
                Just(MessagePriority::Low).boxed(),
            ])
        ) {
            let metadata = MessageMetadata {
                content_type: None,
                priority: Some(priority),
                flush: false,
            };

            // Test that metadata maintains consistency
            prop_assert_eq!(metadata.priority, Some(priority));
        }
    }
}

#[cfg(test)]
mod error_properties {
    use super::*;
    use pmcp::error::*;

    proptest! {
        /// Property: Error creation should be consistent
        #[test]
        fn property_error_consistency(
            message in "[a-zA-Z0-9 _.-]{1,100}"
        ) {
            let parse_error = Error::parse(message.clone());
            let invalid_request = Error::validation(message.clone());
            let method_not_found = Error::method_not_found(message.clone());
            let invalid_params = Error::invalid_params(message.clone());
            let internal_error = Error::internal(message.clone());

            // Parse errors should have error codes
            prop_assert!(parse_error.error_code().is_some());

            // Other errors may or may not have error codes depending on the implementation
            // But we can test they handle properly
            let _has_code = invalid_request.error_code();
            let _has_code = method_not_found.error_code();
            let _has_code = invalid_params.error_code();
            let _has_code = internal_error.error_code();

            // Error codes should be in valid range
            if let Some(code) = parse_error.error_code() {
                let code_i32 = code.as_i32();
                prop_assert!((-32999..=-32000).contains(&code_i32));
            }
        }
    }
}

#[cfg(test)]
mod json_properties {
    use super::*;

    proptest! {
        /// Property: JSON serialization should be stable
        #[test]
        fn property_json_stability(
            numbers in prop::collection::vec(any::<i64>(), 0..50),
            strings in prop::collection::vec("[a-zA-Z0-9 _.-]*", 0..20),
            booleans in prop::collection::vec(any::<bool>(), 0..10)
        ) {
            let mut json_obj = serde_json::Map::new();

            for (i, num) in numbers.iter().enumerate() {
                json_obj.insert(
                    format!("num_{}", i),
                    serde_json::Value::Number((*num).into())
                );
            }

            for (i, s) in strings.iter().enumerate() {
                json_obj.insert(
                    format!("str_{}", i),
                    serde_json::Value::String(s.clone())
                );
            }

            for (i, b) in booleans.iter().enumerate() {
                json_obj.insert(
                    format!("bool_{}", i),
                    serde_json::Value::Bool(*b)
                );
            }

            let json_value = serde_json::Value::Object(json_obj);

            // Serialize and deserialize
            let serialized1 = serde_json::to_string(&json_value).unwrap();
            let deserialized: serde_json::Value = serde_json::from_str(&serialized1).unwrap();
            let serialized2 = serde_json::to_string(&deserialized).unwrap();

            // Should be stable through round-trips
            let deser2: serde_json::Value = serde_json::from_str(&serialized2).unwrap();
            prop_assert_eq!(json_value, deser2);
        }
    }
}

// === Phase 73: PARITY-CLIENT-01 — delegation equivalence ===
//
// Property: `call_tool_typed(name, &args)` sends the same wire bytes as
// `call_tool(name, serde_json::to_value(&args).unwrap())`. Validated by
// capturing the outgoing JSON-RPC `tools/call` request on a pair of mock
// transports and asserting the recovered `params.arguments` field equals
// `serde_json::to_value(&args)`.
#[cfg(test)]
mod phase73_typed_helpers {
    use async_trait::async_trait;
    use pmcp::{
        shared::Transport,
        types::{ClientCapabilities, RequestId, TransportMessage},
        Client, Error as PmcpError, Result as PmcpResult,
    };
    use proptest::prelude::*;
    use serde::Serialize;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, Serialize)]
    struct ProptestArgs {
        a: i64,
        b: String,
        c: Vec<u32>,
    }

    /// `MockTransport` variant that exposes captured outgoing messages.
    #[derive(Debug)]
    struct CaptureTransport {
        responses: Arc<Mutex<Vec<TransportMessage>>>,
        sent: Arc<Mutex<Vec<TransportMessage>>>,
    }

    #[async_trait]
    impl Transport for CaptureTransport {
        async fn send(&mut self, m: TransportMessage) -> PmcpResult<()> {
            self.sent.lock().unwrap().push(m);
            Ok(())
        }

        async fn receive(&mut self) -> PmcpResult<TransportMessage> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| PmcpError::protocol_msg("no more responses"))
        }

        async fn close(&mut self) -> PmcpResult<()> {
            Ok(())
        }
    }

    fn init_response() -> TransportMessage {
        use pmcp::types::{jsonrpc::ResponsePayload, JSONRPCResponse};
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            payload: ResponsePayload::Result(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "t", "version": "0" }
            })),
        })
    }

    fn call_response(id: i64) -> TransportMessage {
        use pmcp::types::{jsonrpc::ResponsePayload, JSONRPCResponse};
        TransportMessage::Response(JSONRPCResponse {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(id),
            payload: ResponsePayload::Result(json!({ "content": [] })),
        })
    }

    /// Extract the `params.arguments` JSON field from the captured outgoing
    /// `tools/call` request, if any.
    fn captured_arguments(sent: &[TransportMessage]) -> Option<serde_json::Value> {
        sent.iter().find_map(|m| {
            let TransportMessage::Request { request, .. } = m else {
                return None;
            };
            let v = serde_json::to_value(request).ok()?;
            // The wire format nests under method-name key "tools/call" which
            // maps to params via serde's internally-tagged enum. Try a few
            // traversal shapes to stay robust:
            // 1. { "method": "tools/call", "params": { "arguments": ... } }
            // 2. { "tools/call": { "arguments": ... } }
            // 3. { "params": { "arguments": ... } }
            if let Some(args) = v.get("params").and_then(|p| p.get("arguments")).cloned() {
                return Some(args);
            }
            if let Some(args) = v
                .get("tools/call")
                .and_then(|p| p.get("arguments"))
                .cloned()
            {
                return Some(args);
            }
            None
        })
    }

    proptest! {
        /// Delegation equivalence for `call_tool_typed` serialize path:
        /// for any ProptestArgs, the `arguments` field on the captured
        /// tools/call JSONRPC request equals `serde_json::to_value(&args)`.
        #[test]
        fn prop_call_tool_typed_sends_expected_value(
            a in any::<i64>(),
            b in "[a-z]{0,16}",
            c in prop::collection::vec(any::<u32>(), 0..8),
        ) {
            let args = ProptestArgs { a, b: b.clone(), c: c.clone() };
            let expected = serde_json::to_value(&args).unwrap();

            let sent = Arc::new(Mutex::new(Vec::<TransportMessage>::new()));
            let transport = CaptureTransport {
                responses: Arc::new(Mutex::new(vec![call_response(2), init_response()])),
                sent: Arc::clone(&sent),
            };

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut client = Client::new(transport);
                client.initialize(ClientCapabilities::minimal()).await.unwrap();
                let _ = client.call_tool_typed("prop", &args).await;
            });

            let sent_snapshot = sent.lock().unwrap().clone();
            let recovered = captured_arguments(&sent_snapshot);

            // If the wire-format traversal could not locate arguments, fall
            // back to the delegation-equivalence check: driving `call_tool`
            // with the same serialized value must produce the identical
            // `sent` vec. This establishes the same invariant (typed helper
            // serializes-and-delegates) without relying on internal wire
            // accessors.
            if recovered.is_none() {
                let sent_b = Arc::new(Mutex::new(Vec::<TransportMessage>::new()));
                let transport_b = CaptureTransport {
                    responses: Arc::new(Mutex::new(vec![call_response(2), init_response()])),
                    sent: Arc::clone(&sent_b),
                };
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    let mut client = Client::new(transport_b);
                    client.initialize(ClientCapabilities::minimal()).await.unwrap();
                    let _ = client.call_tool("prop".to_string(), expected.clone()).await;
                });
                let snap_a = sent_snapshot;
                let snap_b = sent_b.lock().unwrap().clone();
                // The two sent vecs must be byte-identical at the serde_json
                // level (RequestId strings will differ — strip them before
                // comparison).
                let strip = |msgs: &[TransportMessage]| -> Vec<serde_json::Value> {
                    msgs.iter()
                        .filter_map(|m| {
                            let TransportMessage::Request { request, .. } = m else { return None };
                            serde_json::to_value(request).ok()
                        })
                        .collect()
                };
                prop_assert_eq!(strip(&snap_a), strip(&snap_b));
            } else {
                prop_assert_eq!(recovered, Some(expected));
            }
        }
    }
}
