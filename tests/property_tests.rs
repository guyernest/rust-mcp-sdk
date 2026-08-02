//! Property-based tests for PMCP SDK
//!
//! These tests verify invariants and properties that should hold across
//! the entire PMCP protocol implementation using property-based testing.
//!
//! ALWAYS Requirement: Property tests for all new features

// Phase 73 list_all_* property tests share MockTransport + builders with
// tests/list_all_pagination.rs via this single `#[path]` declaration —
// `mod mock_paginated` MUST NOT be redeclared inside any nested module.
#[path = "common/mock_paginated.rs"]
mod mock_paginated;

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

// === Typed-helper delegation equivalence ===
//
// Property: `call_tool_typed(name, &args)` sends the same wire bytes as
// `call_tool(name, serde_json::to_value(&args).unwrap())`. Validated by
// capturing the outgoing JSON-RPC `tools/call` request on a pair of mock
// transports and asserting the recovered `params.arguments` field equals
// `serde_json::to_value(&args)`.
#[cfg(test)]
mod typed_helper_properties {
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

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
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
                let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
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

// === list_all_* pagination properties ===
//
// The `#[path = "common/mock_paginated.rs"] mod mock_paginated;` declaration
// lives ONCE at the top of this file — do NOT redeclare it here.
#[cfg(test)]
mod list_all_pagination_properties {
    use super::mock_paginated::{
        build_paginated_responses, init_response, MockTransport, PaginationCapability,
    };
    use pmcp::{types::ClientCapabilities, Client, ClientOptions, Error};
    use proptest::prelude::*;
    use serde_json::{json, Value};

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

        /// Flat-concatenation invariant: for any N-page sequence (N in 1..=7),
        /// `list_all_tools` returns the in-order concatenation of tool names
        /// across all pages.
        #[test]
        fn prop_list_all_tools_flat_concatenation(
            pages in prop::collection::vec(
                prop::collection::vec("[a-z]{1,6}", 0..4),
                1..8,
            ),
        ) {
            let page_payloads: Vec<Vec<Value>> = pages
                .iter()
                .map(|names| {
                    names
                        .iter()
                        .map(|n| json!({"name": n, "description": "", "inputSchema": {}}))
                        .collect()
                })
                .collect();
            let responses = build_paginated_responses(
                init_response(),
                page_payloads,
                PaginationCapability::Tools,
            );
            let expected: Vec<String> = pages.into_iter().flatten().collect();

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let observed = rt.block_on(async move {
                let mut client = Client::new(MockTransport::with_responses(responses));
                client
                    .initialize(ClientCapabilities::minimal())
                    .await
                    .unwrap();
                client.list_all_tools().await.unwrap()
            });
            let observed_names: Vec<String> = observed.into_iter().map(|t| t.name).collect();
            prop_assert_eq!(observed_names, expected);
        }

        /// Cap-enforcement invariant: `max_iterations = cap` + `cap + 2` scripted
        /// pages forces the cap-exceeded branch to fire with `Error::Validation`.
        ///
        /// `build_paginated_responses` assigns `next_cursor: None` to the final
        /// scripted page. With `cap + 1` pages, the `cap`-th iteration would see
        /// that terminal `None` and exit with `Ok(_)`, so the cap branch would be
        /// unreachable and the property would pass vacuously. `cap + 2` pages
        /// guarantees every page inside the budget carries `Some(_)`, so the
        /// `cap`-th iteration observes a non-terminal cursor and the for-loop's
        /// cap branch fires. `Ok(_)` is a counter-example under this property.
        #[test]
        fn prop_list_all_tools_cap_enforced(cap in 1usize..20) {
            let page_count = cap + 2;
            let page_payloads: Vec<Vec<Value>> = (0..page_count)
                .map(|i| {
                    vec![json!({
                        "name": format!("t{i}"),
                        "description": "",
                        "inputSchema": {}
                    })]
                })
                .collect();
            let responses = build_paginated_responses(
                init_response(),
                page_payloads,
                PaginationCapability::Tools,
            );

            let opts = ClientOptions::default().with_max_iterations(cap);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let result = rt.block_on(async move {
                let mut client = Client::with_client_options(
                    MockTransport::with_responses(responses),
                    opts,
                );
                client
                    .initialize(ClientCapabilities::minimal())
                    .await
                    .unwrap();
                client.list_all_tools().await
            });

            prop_assert!(
                result.is_err(),
                "cap-enforced property violated: helper returned Ok(_) when it should have errored with Error::Validation after {cap} iterations"
            );
            let e = result.unwrap_err();
            prop_assert!(
                matches!(e, Error::Validation(_)),
                "expected Error::Validation, got a different error variant: {e}"
            );
            let msg = format!("{e}");
            prop_assert!(
                msg.contains("list_all_tools"),
                "method name missing from validation error: {msg}"
            );
        }
    }
}

#[cfg(test)]
mod structured_output_invariants {
    use super::*;

    /// Arbitrary JSON value (no floats — NaN/precision break equality
    /// round-trips and the invariant under test is structural, not numeric).
    ///
    /// Visible to siblings since 115-09: the
    /// `schema_dialect_normalization_properties` module below builds its schema
    /// documents from this same strategy rather than growing a second,
    /// subtly-different arbitrary-JSON generator. `pub` rather than
    /// `pub(super)` because clippy's `redundant_pub_crate` rejects the latter
    /// here; the enclosing module is private, so nothing escapes this test
    /// binary either way.
    pub fn arb_json() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::Bool),
            any::<i64>().prop_map(|n| serde_json::json!(n)),
            "[a-zA-Z0-9 _-]{0,12}".prop_map(serde_json::Value::String),
        ];
        leaf.prop_recursive(3, 32, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
                prop::collection::hash_map("[a-zA-Z_][a-zA-Z0-9_]{0,8}", inner, 0..4)
                    .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        /// Property: `CallToolResult::structured(v)` dual-emits ONE value in
        /// both voices — `structuredContent` carries `v` verbatim, the text
        /// voice parses back to `v`, and the wire (serde) shape exposes the
        /// camelCase `structuredContent` field with the same value.
        #[test]
        fn property_structured_dual_emit_roundtrip(value in arb_json()) {
            let result = CallToolResult::structured(value.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(result.structured_content.as_ref(), Some(&value));

            let Content::Text { text } = &result.content[0] else {
                return Err(TestCaseError::fail("structured() must emit a text voice"));
            };
            let parsed: serde_json::Value = serde_json::from_str(text)
                .map_err(|e| TestCaseError::fail(format!("text voice must be valid JSON: {e}")))?;
            prop_assert_eq!(&parsed, &value);

            let wire = serde_json::to_value(&result)
                .map_err(|e| TestCaseError::fail(format!("result must serialize: {e}")))?;
            prop_assert_eq!(wire.get("structuredContent"), Some(&value));
        }

        /// Property: `structured_with_text` keeps the human voice verbatim and
        /// never leaks it into `structuredContent`.
        #[test]
        fn property_structured_with_text_two_voices(
            value in arb_json(),
            human in "[a-zA-Z0-9 .,!?-]{1,40}"
        ) {
            let result = CallToolResult::structured_with_text(value.clone(), human.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(result.structured_content.as_ref(), Some(&value));
            let Content::Text { text } = &result.content[0] else {
                return Err(TestCaseError::fail("structured_with_text must emit a text voice"));
            };
            prop_assert_eq!(text, &human);
        }

        /// Property (115-09, SCHM-02): `CallToolResult::structured_value(v)`
        /// preserves `v`'s SHAPE — object, array, string, number, boolean or
        /// null — both in memory and across a full serde round trip.
        ///
        /// The strategy is the module's existing `arb_json()`, reused rather
        /// than duplicated: a second arbitrary-JSON generator would drift from
        /// this one and the two properties would then be held over different
        /// input spaces.
        ///
        /// The `Value::Null` case is asserted EXPLICITLY on every iteration
        /// rather than left to the generator, because it is the one shape whose
        /// wire behaviour is easy to break by accident:
        /// `skip_serializing_if = "Option::is_none"` must NOT elide it, since
        /// the field is `Some(Value::Null)` and not `None`. That distinction is
        /// exactly what SCHM-02 buys — v2 permits an explicit
        /// `"structuredContent": null`, and an omitted key means something else.
        ///
        /// # A MEASURED asymmetry, recorded rather than fixed
        ///
        /// The EMIT half of that claim holds: `Some(Value::Null)` serializes to
        /// an explicit `"structuredContent":null`. The PARSE half does not.
        /// `Option<Value>`'s stock `Deserialize` maps a JSON `null` to `None`,
        /// so reading that same wire back yields `None`, not `Some(Null)` —
        /// a pmcp CLIENT cannot currently distinguish "explicitly null" from
        /// "absent". Measured on 2026-08-01 while writing this property; the
        /// minimal failing input was `value = Null`, wire
        /// `{"content":[…],"isError":false,"structuredContent":null}`.
        ///
        /// It is NOT fixed here, and it is NOT news: 115-04 already measured
        /// it, fenced it with the tripwire
        /// `present_null_structured_content_does_not_survive_a_typed_reread`
        /// in `tests/structured_tool_output.rs`, and booked it as **D-115-04-A**
        /// in the phase's `deferred-items.md`. The fix is a `#[serde(default,
        /// deserialize_with = …)]` double-`Option` on
        /// `CallToolResult::structured_content` in `src/types/tools.rs` — a
        /// shipped public type this plan does not touch, and a change to how
        /// every existing client parses every tool result on BOTH eras. This
        /// property therefore holds the round trip over the non-null shapes and
        /// asserts what is MEASURED for null, so a future fix turns this
        /// assertion red and gets read instead of landing silently.
        #[test]
        fn property_structured_content_preserves_shape_through_a_call_tool_result(
            value in arb_json()
        ) {
            let result = CallToolResult::structured_value(value.clone());

            prop_assert!(!result.is_error);
            prop_assert_eq!(
                result.structured_content.as_ref(),
                Some(&value),
                "structured_value must carry the payload verbatim, whatever its shape"
            );

            let raw = serde_json::to_string(&result)
                .map_err(|e| TestCaseError::fail(format!("result must serialize: {e}")))?;
            let wire: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| TestCaseError::fail(format!("the wire must be JSON: {e}")))?;

            // The WIRE always carries the value verbatim, every shape included:
            // object, array, string, number, boolean AND null.
            prop_assert_eq!(
                wire.get("structuredContent"),
                Some(&value),
                "the serialized wire must carry the payload verbatim; wire was {}",
                &raw
            );

            let back: CallToolResult = serde_json::from_str(&raw)
                .map_err(|e| TestCaseError::fail(format!("result must deserialize: {e}")))?;
            let expected_after_round_trip = if value.is_null() {
                // The measured asymmetry — see this test's docs (D-115-04-A).
                None
            } else {
                Some(&value)
            };
            prop_assert_eq!(
                back.structured_content.as_ref(),
                expected_after_round_trip,
                "a serialize -> deserialize round trip must preserve the shape for every \
                 non-null value, and is MEASURED to collapse Some(Null) to None; if the null \
                 case now round-trips, the D-115-04-A fix has landed and this branch should be \
                 deleted. wire was {}",
                &raw
            );

            // The explicit-null EMIT case is NOT asserted here — it is a
            // constant, so see `structured_value_null_emits_an_explicit_null`
            // below. Running it inside the property re-serialized the same
            // fixed value on all 256 generated cases for coverage identical to
            // asserting it once.
        }
    }

    /// `Some(Value::Null)` must reach the wire as an EXPLICIT `null`.
    ///
    /// Constant input, so this is a plain `#[test]` rather than a property: the
    /// assertion does not depend on anything the generator produces. It guards
    /// the one shape `skip_serializing_if` could silently elide, which is why it
    /// is stated separately rather than folded into the round-trip property.
    #[test]
    fn structured_value_null_emits_an_explicit_null() {
        let null_result = CallToolResult::structured_value(serde_json::Value::Null);
        assert_eq!(
            null_result.structured_content.as_ref(),
            Some(&serde_json::Value::Null)
        );
        let null_raw = serde_json::to_string(&null_result).expect("null result must serialize");
        assert!(
            null_raw.contains(r#""structuredContent":null"#),
            "Some(Value::Null) must emit an EXPLICIT null, not be elided by \
             skip_serializing_if; wire was {null_raw}"
        );
    }
}

/// `$schema` normalization held over arbitrary generated schemas (115-09,
/// SCHM-01; widened by 115-13).
///
/// `src/server/output_validation.rs` fences normalization with five FIXED
/// documents (`normalize_schema_dialect_changes_only_dollar_schema_keys` and
/// `..._is_idempotent`). This is the correct generalization of those two:
/// idempotence, surgical scope and post-normalization dialect PURITY over
/// arbitrary input.
///
/// # The normalizer's scope is the whole document, not the root
///
/// `normalize_schema_dialect` rewrites EVERY string-valued `$schema` at ANY
/// depth (115-12). Until then it rewrote only the root key, and this module
/// could not have noticed: `arb_schema_document()` stripped every non-root
/// `$schema` before generating, so the generated space structurally excluded
/// the `$id`-bearing EMBEDDED SCHEMA RESOURCE — the one shape 2020-12 sanctions
/// a nested declaration on, and the one `115-VERIFICATION.md` reproduced the
/// vacuous-validator bypass with. The generator now EMITS that shape and the
/// property asserts over it.
///
/// # Why this module is `fuzzing`-gated, and why that is deliberate
///
/// The normalizer is a private function. Rather than widen a `pub(crate)` item
/// for test convenience — which would put a shipped-API item on the surface for
/// the sake of a test — this reaches it through the SAME `feature = "fuzzing"`
/// seam `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` uses. `fuzzing` is in
/// neither `default` nor `full`, so this block does NOT run under a plain
/// `cargo test --features full`; its verification command is
/// `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'`,
/// which is an acceptance criterion of `115-09-PLAN.md`, and the same property
/// is exercised continuously by the fuzz target itself. Its absence from the
/// default run is a consequence of not widening the API, not an oversight.
///
/// # BOTH features, not just `fuzzing`
///
/// `output_validation::fuzz_support` is gated `#[cfg(all(feature = "fuzzing",
/// feature = "validation"))]` — `fuzzing` widens the MODULE, `validation`
/// supplies its CONTENTS (`fuzz/Cargo.toml` enables both for the same reason).
/// `fuzzing = []` implies nothing, so a `cargo test --features fuzzing` without
/// `validation` would fail to compile this ENTIRE integration crate, not just
/// this module.
#[cfg(all(test, feature = "fuzzing", feature = "validation"))]
mod schema_dialect_normalization_properties {
    use super::structured_output_invariants::arb_json;
    use super::*;
    use pmcp::server::output_validation::fuzz_support::normalize_bytes;

    /// The Draft 2020-12 meta-schema URI the v2 pin rewrites to.
    const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

    /// Keywords whose VALUE is instance data rather than a subschema.
    ///
    /// Mirrors `DATA_ONLY_KEYWORDS` in `src/server/output_validation.rs`. The
    /// shipped walk never descends into these — a `$schema` string inside a
    /// `const`/`enum`/`default`/`examples` payload is DATA, and rewriting it
    /// would change which instances conform — so neither do the strip and the
    /// scan below. Restating the rule here rather than guessing at it is what
    /// keeps this property an assertion about the SHIPPED normalizer.
    const DATA_ONLY_KEYWORDS: &[&str] = &["const", "enum", "default", "examples"];

    /// The `$id` of the generated embedded schema resource.
    ///
    /// `example.test` is a reserved, NON-RESOLVABLE host. An `$id` establishes a
    /// base URI without any fetch, and SEP-2106 forbids I/O anywhere on this
    /// path — so this value can never become an outbound request even if a
    /// retriever were somehow compiled in. Every `$ref` this module generates is
    /// a LOCAL JSON pointer (`#/$defs/Inner`) for the same reason.
    const EMBEDDED_RESOURCE_ID: &str = "https://example.test/inner";

    /// The seven-way spread of dialect declarations: absent, the four legacy
    /// drafts, 2020-12 itself, and an invented URI.
    ///
    /// Drawn INDEPENDENTLY for the root and for the embedded resource, so every
    /// combination of the two is reachable — including the pair
    /// `115-VERIFICATION.md` measured as `(Violates, Conforms)` before 115-12
    /// (root draft-07 + an embedded draft-07 resource).
    fn arb_dialect() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            Just(None),
            Just(Some("http://json-schema.org/draft-04/schema#".to_string())),
            Just(Some("http://json-schema.org/draft-06/schema#".to_string())),
            Just(Some("http://json-schema.org/draft-07/schema#".to_string())),
            Just(Some(
                "https://json-schema.org/draft/2019-09/schema".to_string()
            )),
            Just(Some(DRAFT_2020_12.to_string())),
            "[a-z]{2,6}://[a-z.]{2,10}/[a-z]{2,8}".prop_map(Some),
        ]
    }

    /// An arbitrary JSON OBJECT usable as a schema document, sometimes carrying
    /// a root `$schema` drawn from a spread of real and invented draft URIs, and
    /// sometimes carrying an `$id`-bearing EMBEDDED SCHEMA RESOURCE with its own
    /// independently-drawn declaration.
    ///
    /// The body comes from the crate's existing `arb_json()` strategy; the
    /// dialect declarations and the embedded resource are generated here,
    /// because those are the only keys the normalizer is allowed to touch.
    fn arb_schema_document() -> impl Strategy<Value = serde_json::Value> {
        (arb_json(), arb_dialect(), arb_dialect(), any::<bool>()).prop_map(
            |(body, dialect, nested_dialect, embed)| {
                let mut object = match body {
                    serde_json::Value::Object(map) => map,
                    // A non-object body still makes a usable document once
                    // wrapped: `const` takes an arbitrary value in every draft.
                    other => {
                        let mut map = serde_json::Map::new();
                        map.insert("const".to_string(), other);
                        map
                    },
                };
                // `arb_json` never generates a `$schema` key, but removing it
                // first keeps the INJECTED declarations the only ones, whatever
                // that strategy grows into later. The nested declaration below
                // is injected deliberately rather than removed accidentally —
                // that accidental removal is what made this generated space
                // unable to contain the 115-12 defect.
                object.remove("$schema");

                if embed {
                    let mut inner = serde_json::Map::new();
                    // The `$id` is what makes this an EMBEDDED SCHEMA RESOURCE
                    // rather than an inert subschema: 2020-12 sanctions a
                    // `$schema` at the root of one, and `jsonschema` 0.49.2
                    // honours it there.
                    inner.insert(
                        "$id".to_string(),
                        serde_json::Value::String(EMBEDDED_RESOURCE_ID.to_string()),
                    );
                    if let Some(uri) = nested_dialect {
                        inner.insert("$schema".to_string(), serde_json::Value::String(uri));
                    }
                    inner.insert(
                        "type".to_string(),
                        serde_json::Value::String("integer".to_string()),
                    );
                    let mut defs = serde_json::Map::new();
                    defs.insert("Inner".to_string(), serde_json::Value::Object(inner));
                    object.insert("$defs".to_string(), serde_json::Value::Object(defs));
                    // A LOCAL JSON pointer, never a scheme'd URI (SEP-2106).
                    let mut properties = serde_json::Map::new();
                    properties.insert(
                        "n".to_string(),
                        serde_json::json!({ "$ref": "#/$defs/Inner" }),
                    );
                    object.insert(
                        "properties".to_string(),
                        serde_json::Value::Object(properties),
                    );
                }

                if let Some(uri) = dialect {
                    object.insert("$schema".to_string(), serde_json::Value::String(uri));
                }
                serde_json::Value::Object(object)
            },
        )
    }

    /// Remove every string-valued `$schema` at EVERY depth, skipping the values
    /// of [`DATA_ONLY_KEYWORDS`].
    ///
    /// This is the surgical-scope comparison's stripper. It must mirror the
    /// shipped traversal rule exactly: a root-only strip would read a legitimate
    /// NESTED rewrite as collateral damage and fail the property on correct
    /// behaviour.
    fn strip_dialect_declarations(node: &mut serde_json::Value) {
        match node {
            serde_json::Value::Object(map) => {
                if map.get("$schema").is_some_and(serde_json::Value::is_string) {
                    map.remove("$schema");
                }
                for (key, value) in map.iter_mut() {
                    if !DATA_ONLY_KEYWORDS.contains(&key.as_str()) {
                        strip_dialect_declarations(value);
                    }
                }
            },
            serde_json::Value::Array(items) => {
                items.iter_mut().for_each(strip_dialect_declarations);
            },
            _ => {},
        }
    }

    /// Every string-valued `$schema` at every depth, under the same skip rule.
    fn collect_dialect_declarations<'a>(node: &'a serde_json::Value, out: &mut Vec<&'a str>) {
        match node {
            serde_json::Value::Object(map) => {
                if let Some(declared) = map.get("$schema").and_then(serde_json::Value::as_str) {
                    out.push(declared);
                }
                for (key, value) in map {
                    if !DATA_ONLY_KEYWORDS.contains(&key.as_str()) {
                        collect_dialect_declarations(value, out);
                    }
                }
            },
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_dialect_declarations(item, out);
                }
            },
            _ => {},
        }
    }

    proptest! {
        /// Property: normalizing twice equals normalizing once; the normalized
        /// document differs from the input ONLY at string-valued `$schema` keys
        /// at ANY depth; and NO legacy declaration survives anywhere.
        ///
        /// All three halves matter. A non-idempotent rewrite would make the
        /// same declaration compile to two different validators, because the
        /// validator cache is keyed by schema TEXT. A rewrite that touched any
        /// other key would silently weaken every v2 validator while every
        /// behavioural test kept passing. And a surviving legacy declaration —
        /// the 115-12 defect — resolves an EMPTY vocabulary set on that
        /// resource and yields an accept-everything sub-validator, which the
        /// first two halves are both blind to.
        #[test]
        fn property_schema_normalization_is_idempotent_and_surgical(
            schema in arb_schema_document()
        ) {
            let bytes = serde_json::to_vec(&schema)
                .map_err(|e| TestCaseError::fail(format!("schema must serialize: {e}")))?;
            let Some((input, once, twice)) = normalize_bytes(&bytes) else {
                return Err(TestCaseError::fail(
                    "a document produced by serde_json must parse back as JSON",
                ));
            };

            prop_assert_eq!(
                &once,
                &twice,
                "normalization must be idempotent, but a second pass changed {}",
                &input
            );

            // Surgical scope, RECURSIVELY: strip every string-valued `$schema`
            // at every depth from both sides. A root-only strip would report a
            // legitimate nested rewrite as collateral damage.
            let mut stripped_input = input.clone();
            let mut stripped_once = once.clone();
            strip_dialect_declarations(&mut stripped_input);
            strip_dialect_declarations(&mut stripped_once);
            prop_assert_eq!(
                stripped_input,
                stripped_once,
                "normalization touched a key other than a string-valued $schema: {} became {}",
                &input,
                &once
            );

            // And the root key itself lands in exactly one of two states.
            let declared = input.get("$schema").and_then(serde_json::Value::as_str);
            let normalized = once.get("$schema").and_then(serde_json::Value::as_str);
            match declared {
                // Undeclared stays undeclared: `Draft::default()` is already
                // 2020-12, so there is nothing to announce.
                None => prop_assert_eq!(
                    normalized,
                    None,
                    "an undeclared document must not GAIN a $schema key: {}",
                    &once
                ),
                // Anything declared is OVERWRITTEN with the pinned URI, never
                // deleted — the compiled document states the dialect it was
                // evaluated under.
                Some(_) => prop_assert_eq!(
                    normalized,
                    Some(DRAFT_2020_12),
                    "a declared dialect must be rewritten to the 2020-12 URI: {}",
                    &once
                ),
            }

            // DIALECT PURITY. Total over the normalized document: no
            // string-valued `$schema` anywhere may be anything but the pinned
            // URI. This is the assertion the two above cannot make — both are
            // satisfied by a root-only normalizer.
            let mut surviving = Vec::new();
            collect_dialect_declarations(&once, &mut surviving);
            let legacy: Vec<&&str> = surviving
                .iter()
                .filter(|declared| **declared != DRAFT_2020_12)
                .collect();
            prop_assert!(
                legacy.is_empty(),
                "a LEGACY $schema survived normalization: {:?} in {}. A declaration that \
                 survives on an $id-bearing embedded schema resource resolves an EMPTY \
                 vocabulary set there and produces a sub-validator that accepts everything — \
                 the vacuous-validator bypass 115-VERIFICATION.md reproduced as the row \
                 `root-draft07 + embedded (v1,v2) = (Violates, Conforms)`, v2 measurably \
                 WEAKER than v1. normalize_schema_dialect must rewrite EVERY declaration at \
                 EVERY depth, not just the root one.",
                legacy,
                &once
            );

            // The embedded resource specifically, addressed by POINTER so the
            // failure message names the path.
            let nested_declared = input.pointer("/$defs/Inner/$schema");
            let nested_normalized = once.pointer("/$defs/Inner/$schema");
            match nested_declared {
                None => prop_assert!(
                    nested_normalized.is_none(),
                    "an embedded resource that declared no dialect must not GAIN one at \
                     /$defs/Inner/$schema: {}",
                    &once
                ),
                Some(_) => prop_assert_eq!(
                    nested_normalized.and_then(serde_json::Value::as_str),
                    Some(DRAFT_2020_12),
                    "an embedded schema resource's dialect declaration must be rewritten to \
                     the 2020-12 URI at /$defs/Inner/$schema: {}",
                    &once
                ),
            }
        }
    }
}
