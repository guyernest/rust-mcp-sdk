//! Comprehensive benchmarks for PMCP SDK
//!
//! Performance regression testing following Toyota Way principles.
//! ALWAYS Requirement: Performance benchmarks for new features

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pmcp::error::*;
use pmcp::shared::transport::{MessagePriority, TransportMessage};
use pmcp::shared::uri_template::UriTemplate;
use pmcp::types::*;
use pmcp::utils::batching::*;
use pmcp::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;

/// Benchmark JSON-RPC serialization/deserialization
fn bench_jsonrpc_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonrpc_serialization");

    // Test different message sizes
    let sizes = vec![10, 100, 1000, 10000];

    for size in sizes {
        let params = json!({
            "data": "x".repeat(size),
            "numbers": (0..size).collect::<Vec<_>>(),
            "nested": {
                "level1": {
                    "level2": (0..size/10).map(|i| format!("item_{}", i)).collect::<Vec<_>>()
                }
            }
        });

        let request = types::jsonrpc::JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            id: types::jsonrpc::RequestId::Number(1),
            method: "test/benchmark".to_string(),
            params: Some(params),
        };

        group.bench_with_input(BenchmarkId::new("serialize", size), &request, |b, req| {
            b.iter(|| {
                black_box(serde_json::to_string(req).unwrap());
            });
        });

        let serialized = serde_json::to_string(&request).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            &serialized,
            |b, data| {
                b.iter(|| {
                    black_box(
                        serde_json::from_str::<types::jsonrpc::JSONRPCRequest>(data).unwrap(),
                    );
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("roundtrip", size), &request, |b, req| {
            b.iter(|| {
                let serialized = serde_json::to_string(req).unwrap();
                black_box(
                    serde_json::from_str::<types::jsonrpc::JSONRPCRequest>(&serialized).unwrap(),
                );
            });
        });
    }

    group.finish();
}

/// Benchmark error creation and handling
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    group.bench_function("create_parse_error", |b| {
        b.iter(|| {
            black_box(Error::parse("JSON parsing failed"));
        });
    });

    group.bench_function("create_internal_error", |b| {
        b.iter(|| {
            black_box(Error::internal("Internal server error"));
        });
    });

    group.bench_function("error_code_conversion", |b| {
        let error = Error::parse("Test error");
        b.iter(|| {
            let code = error.error_code();
            if let Some(code) = code {
                let as_i32 = code.as_i32();
                black_box(as_i32);
            }
        });
    });

    group.bench_function("error_with_data", |b| {
        let data = json!({
            "details": "Additional information",
            "trace_id": "abc-123-def",
            "timestamp": 1234567890
        });
        b.iter(|| {
            black_box(Error::Protocol {
                code: ErrorCode::INTERNAL_ERROR,
                message: "Custom error".to_string(),
                data: Some(data.clone()),
            });
        });
    });

    group.finish();
}

/// Benchmark URI template operations
fn bench_uri_templates(c: &mut Criterion) {
    let mut group = c.benchmark_group("uri_templates");

    // Test different template complexities
    let templates = vec![
        ("/simple", "simple_static"),
        ("/users/{id}", "single_param"),
        ("/api/{version}/users/{id}", "two_params"),
        (
            "/api/{version}/users/{id}/posts/{postId}/comments/{commentId}",
            "complex",
        ),
    ];

    for (template_str, name) in templates {
        let template = UriTemplate::new(template_str).unwrap();

        group.bench_function(format!("create_{}", name), |b| {
            b.iter(|| {
                black_box(UriTemplate::new(template_str).unwrap());
            });
        });

        let params: Vec<(&str, &str)> = vec![
            ("version", "v1"),
            ("id", "123"),
            ("postId", "456"),
            ("commentId", "789"),
        ];

        group.bench_function(format!("expand_{}", name), |b| {
            b.iter(|| {
                black_box(template.expand(&params).unwrap());
            });
        });

        // Create a matching URI for testing
        let expanded = template.expand(&params).unwrap();
        let uri_str = format!("https://example.com{}", expanded);
        group.bench_function(format!("match_{}", name), |b| {
            b.iter(|| {
                black_box(template.match_uri(&uri_str));
            });
        });
    }

    group.finish();
}

/// Benchmark capability operations
fn bench_capabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("capabilities");

    let client_caps = types::capabilities::ClientCapabilities::full();
    let server_caps = types::capabilities::ServerCapabilities::tools_only();

    group.bench_function("client_capability_checks", |b| {
        b.iter(|| {
            black_box(client_caps.supports_tools());
            black_box(client_caps.supports_resources());
            black_box(client_caps.supports_prompts());
            black_box(client_caps.supports_sampling());
        });
    });

    group.bench_function("server_capability_checks", |b| {
        b.iter(|| {
            black_box(server_caps.provides_tools());
            black_box(server_caps.provides_resources());
            black_box(server_caps.provides_prompts());
        });
    });

    group.bench_function("capability_serialization", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&client_caps).unwrap();
            black_box(
                serde_json::from_str::<types::capabilities::ClientCapabilities>(&serialized)
                    .unwrap(),
            );
        });
    });

    group.finish();
}

/// Benchmark transport operations
fn bench_transport(c: &mut Criterion) {
    let mut group = c.benchmark_group("transport");

    let message_sizes = vec![100, 1000, 10000, 100000];

    for size in message_sizes {
        group.bench_with_input(
            BenchmarkId::new("message_creation", size),
            &size,
            |b, _size| {
                b.iter(|| {
                    // TransportMessage is an enum, create a mock request
                    let request = types::Request::Client(Box::new(types::ClientRequest::Ping));
                    black_box(TransportMessage::Request {
                        id: types::RequestId::from(1i64),
                        request,
                    });
                });
            },
        );
    }

    // Benchmark priority ordering
    let priorities = vec![
        MessagePriority::Low,
        MessagePriority::Normal,
        MessagePriority::High,
    ];

    group.bench_function("priority_sorting", |b| {
        b.iter(|| {
            let mut p = priorities.clone();
            p.sort();
            black_box(p);
        });
    });

    group.finish();
}

/// Benchmark batching operations
fn bench_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("batching");

    group.bench_function("batcher_creation", |b| {
        b.iter(|| {
            let config = BatchingConfig {
                max_batch_size: 100,
                max_wait_time: Duration::from_millis(10),
                batched_methods: vec![],
            };
            black_box(MessageBatcher::new(config));
        });
    });

    group.bench_function("debouncer_creation", |b| {
        b.iter(|| {
            let config = DebouncingConfig {
                wait_time: Duration::from_millis(10),
                debounced_methods: HashMap::new(),
            };
            black_box(MessageDebouncer::new(config));
        });
    });

    group.finish();
}

/// Benchmark authentication operations
fn bench_auth(c: &mut Criterion) {
    let mut group = c.benchmark_group("auth");

    group.bench_function("auth_info_none", |b| {
        b.iter(|| {
            black_box(types::auth::AuthInfo::none());
        });
    });

    group.bench_function("auth_info_bearer", |b| {
        b.iter(|| {
            black_box(types::auth::AuthInfo::bearer("test-token-123".to_string()));
        });
    });

    group.bench_function("auth_info_oauth2", |b| {
        b.iter(|| {
            let oauth_info = types::auth::OAuthInfo {
                auth_url: "https://example.com/auth".to_string(),
                token_url: "https://example.com/token".to_string(),
                client_id: "client-123".to_string(),
                scopes: Some(vec!["read".to_string(), "write".to_string()]),
                redirect_uri: Some("https://example.com/callback".to_string()),
                pkce_method: None,
            };
            black_box(types::auth::AuthInfo::oauth2(oauth_info));
        });
    });

    let auth_bearer = types::auth::AuthInfo::bearer("test-token".to_string());

    group.bench_function("auth_header_generation", |b| {
        b.iter(|| {
            black_box(auth_bearer.authorization_header());
        });
    });

    group.bench_function("auth_required_check", |b| {
        b.iter(|| {
            black_box(auth_bearer.is_required());
        });
    });

    group.finish();
}

/// Benchmark JSON operations with different data types
fn bench_json_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_operations");

    // Test different JSON structures
    let json_data = vec![
        (json!(null), "null"),
        (json!(true), "boolean"),
        (json!(42), "number"),
        (json!("simple string"), "string"),
        (json!([1, 2, 3, 4, 5]), "array"),
        (json!({"key": "value", "num": 42}), "object"),
        (
            json!({
                "complex": {
                    "nested": {
                        "array": [1, 2, 3],
                        "string": "value",
                        "null_field": null
                    }
                }
            }),
            "complex",
        ),
    ];

    for (data, name) in json_data {
        group.bench_function(format!("serialize_{}", name), |b| {
            b.iter(|| {
                black_box(serde_json::to_string(&data).unwrap());
            });
        });

        let serialized = serde_json::to_string(&data).unwrap();
        group.bench_function(format!("deserialize_{}", name), |b| {
            b.iter(|| {
                black_box(serde_json::from_str::<Value>(&serialized).unwrap());
            });
        });
    }

    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");

    let sizes = vec![100, 1000, 10000];

    for size in sizes {
        group.bench_with_input(
            BenchmarkId::new("vec_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut vec = Vec::with_capacity(size);
                    for i in 0..size {
                        vec.push(i);
                    }
                    black_box(vec);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("string_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    black_box("x".repeat(size));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("hashmap_operations", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut map = HashMap::with_capacity(size);
                    for i in 0..size {
                        map.insert(format!("key_{}", i), i);
                    }
                    black_box(map);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark protocol constants and utilities
fn bench_protocol_utilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_utilities");

    group.bench_function("version_constants", |b| {
        b.iter(|| {
            black_box(DEFAULT_PROTOCOL_VERSION);
            black_box(LATEST_PROTOCOL_VERSION);
            black_box(SUPPORTED_PROTOCOL_VERSIONS);
        });
    });

    group.bench_function("timeout_constants", |b| {
        b.iter(|| {
            black_box(DEFAULT_REQUEST_TIMEOUT_MS);
        });
    });

    group.bench_function("request_id_operations", |b| {
        b.iter(|| {
            let number_id = types::jsonrpc::RequestId::Number(42);
            let string_id = types::jsonrpc::RequestId::String("test-id".to_string());

            black_box(serde_json::to_string(&number_id).unwrap());
            black_box(serde_json::to_string(&string_id).unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_jsonrpc_serialization,
    bench_error_handling,
    bench_uri_templates,
    bench_capabilities,
    bench_transport,
    bench_batching,
    bench_auth,
    bench_json_operations,
    bench_memory_operations,
    bench_protocol_utilities
);

criterion_main!(benches);
