//! SIMD Parsing Performance Example
//!
//! PMCP-4006: Demonstrates SIMD optimization for parsing including:
//! - High-performance JSON-RPC parsing with vectorized operations
//! - Accelerated SSE parsing for real-time event streams
//! - Optimized Base64 encoding/decoding with SIMD
//! - Parallel HTTP header parsing with performance metrics
//! - CPU feature detection and runtime optimization
//! - Comprehensive performance benchmarks and comparisons
//!
//! Run with: cargo run --example 32_simd_parsing_performance --features full

use base64::{engine::general_purpose, Engine as _};
use pmcp::shared::simd_parsing::*;
use pmcp::shared::sse_parser::SseParser;
use pmcp::types::jsonrpc::JSONRPCRequest;
use std::time::Instant;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting SIMD Parsing Performance Example");

    // Display CPU features
    info!("🔍 Detecting CPU features...");
    let simd_parser = SimdJsonParser::new();
    let features = simd_parser.get_cpu_features();
    info!("  Detected features: {:?}", features);

    if features.avx2 {
        info!("  ✅ AVX2 support detected - using 256-bit vectorized operations");
    } else if features.sse42 {
        info!("  ✅ SSE4.2 support detected - using 128-bit vectorized operations");
    } else {
        info!("  ⚠️  No SIMD support detected - falling back to optimized scalar operations");
    }

    // Demonstrate 1: JSON-RPC Parsing Performance
    info!("\n📊 JSON-RPC Parsing Performance Comparison");

    // Generate test data
    let test_requests: Vec<String> = (0..10000)
        .map(|i| {
            format!(
                r#"{{"jsonrpc":"2.0","id":{},"method":"performance_test","params":{{"iteration":{},"data":"{}","timestamp":{}}}}}"#,
                i,
                i,
                "x".repeat(50), // Add some bulk to make parsing meaningful
                chrono::Utc::now().timestamp_millis()
            )
        })
        .collect();

    // SIMD JSON parsing benchmark
    info!("  🔄 Running SIMD JSON parsing benchmark...");
    let start = Instant::now();
    let mut simd_results = Vec::new();
    for json in &test_requests {
        let result = simd_parser.parse_request(json.as_bytes())?;
        simd_results.push(result);
    }
    let simd_duration = start.elapsed();
    let simd_throughput = test_requests.len() as f64 / simd_duration.as_secs_f64();

    // Standard JSON parsing benchmark
    info!("  🔄 Running standard JSON parsing benchmark...");
    let start = Instant::now();
    let mut standard_results = Vec::new();
    for json in &test_requests {
        let result: JSONRPCRequest = serde_json::from_str(json)?;
        standard_results.push(result);
    }
    let standard_duration = start.elapsed();
    let standard_throughput = test_requests.len() as f64 / standard_duration.as_secs_f64();

    info!("  📈 JSON-RPC Parsing Results:");
    info!(
        "    SIMD parsing:     {:?} ({:.0} docs/sec)",
        simd_duration, simd_throughput
    );
    info!(
        "    Standard parsing: {:?} ({:.0} docs/sec)",
        standard_duration, standard_throughput
    );
    info!(
        "    Speedup:          {:.2}x",
        standard_duration.as_nanos() as f64 / simd_duration.as_nanos() as f64
    );
    info!(
        "    Throughput gain:  {:.1}%",
        (simd_throughput / standard_throughput - 1.0) * 100.0
    );

    // Display SIMD parsing metrics
    let metrics = simd_parser.get_metrics();
    info!("  📊 SIMD Parser Metrics:");
    info!(
        "    Total bytes processed:    {}",
        metrics.total_bytes_processed
    );
    info!(
        "    Documents parsed:         {}",
        metrics.total_documents_parsed
    );
    info!(
        "    Average parse time:       {}ns",
        metrics.average_parse_time_ns
    );
    info!(
        "    Documents per second:     {:.0}",
        metrics.documents_per_second
    );
    info!(
        "    SIMD operations used:     {}",
        metrics.simd_operations_used
    );
    info!(
        "    Fallback operations:      {}",
        metrics.fallback_operations
    );

    // Demonstrate 2: Batch Processing Performance
    info!("\n📦 Batch Processing Performance");

    let batch_json = format!("[{}]", test_requests[0..1000].join(","));

    // SIMD batch parsing
    let start = Instant::now();
    let batch_results = simd_parser.parse_batch_requests(batch_json.as_bytes())?;
    let batch_duration = start.elapsed();

    info!("  📈 Batch Processing Results:");
    info!(
        "    Batch size:           {} documents",
        batch_results.len()
    );
    info!("    Processing time:      {:?}", batch_duration);
    info!(
        "    Batch throughput:     {:.0} docs/sec",
        batch_results.len() as f64 / batch_duration.as_secs_f64()
    );
    info!(
        "    Parallel efficiency:  {:.1}%",
        (batch_results.len() as f64 / batch_duration.as_secs_f64()) / simd_throughput * 100.0
    );

    // Demonstrate 3: SSE Parsing Performance
    info!("\n🌊 Server-Sent Events Parsing Performance");

    let mut simd_sse_parser = SimdSseParser::new();
    let mut standard_sse_parser = SseParser::new();

    // Generate SSE test data
    let sse_events: Vec<String> = (0..5000)
        .map(|i| {
            format!(
                "event: message\nid: {}\ndata: {{\"type\":\"update\",\"id\":{},\"payload\":\"{}\"}}\n\n",
                i,
                i,
                "y".repeat(100)
            )
        })
        .collect();
    let sse_stream = sse_events.join("");

    // SIMD SSE parsing benchmark
    info!("  🔄 Running SIMD SSE parsing benchmark...");
    let start = Instant::now();
    let simd_sse_results = simd_sse_parser.parse_chunk(sse_stream.as_bytes())?;
    let simd_sse_duration = start.elapsed();

    // Standard SSE parsing benchmark
    info!("  🔄 Running standard SSE parsing benchmark...");
    let start = Instant::now();
    let standard_sse_results = standard_sse_parser.feed(&sse_stream);
    let standard_sse_duration = start.elapsed();

    info!("  📈 SSE Parsing Results:");
    info!(
        "    Events parsed:        {} events",
        simd_sse_results.len()
    );
    info!(
        "    SIMD SSE parsing:     {:?} ({:.0} events/sec)",
        simd_sse_duration,
        simd_sse_results.len() as f64 / simd_sse_duration.as_secs_f64()
    );
    info!(
        "    Standard SSE parsing: {:?} ({:.0} events/sec)",
        standard_sse_duration,
        standard_sse_results.len() as f64 / standard_sse_duration.as_secs_f64()
    );
    info!(
        "    SSE speedup:          {:.2}x",
        standard_sse_duration.as_nanos() as f64 / simd_sse_duration.as_nanos() as f64
    );

    // Demonstrate 4: Base64 Encoding/Decoding Performance
    info!("\n🔐 Base64 Encoding/Decoding Performance");

    let simd_base64 = SimdBase64::new();

    // Generate test data of various sizes
    let test_sizes = [1024, 10240, 102400, 1024000]; // 1KB, 10KB, 100KB, 1MB

    for size in test_sizes.iter() {
        let test_data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();

        // SIMD Base64 encoding
        let start = Instant::now();
        let simd_encoded = simd_base64.encode(&test_data);
        let simd_encode_duration = start.elapsed();

        // SIMD Base64 decoding
        let start = Instant::now();
        let simd_decoded = simd_base64.decode(&simd_encoded)?;
        let simd_decode_duration = start.elapsed();

        // Standard Base64 encoding
        let start = Instant::now();
        let standard_encoded = general_purpose::STANDARD.encode(&test_data);
        let standard_encode_duration = start.elapsed();

        // Standard Base64 decoding
        let start = Instant::now();
        let standard_decoded = general_purpose::STANDARD.decode(&standard_encoded)?;
        let standard_decode_duration = start.elapsed();

        assert_eq!(simd_decoded, test_data);
        assert_eq!(standard_decoded, test_data);

        let encode_speedup =
            standard_encode_duration.as_nanos() as f64 / simd_encode_duration.as_nanos() as f64;
        let decode_speedup =
            standard_decode_duration.as_nanos() as f64 / simd_decode_duration.as_nanos() as f64;

        info!("  📈 Base64 Performance ({} bytes):", size);
        info!(
            "    SIMD encode:      {:?} ({:.2}x speedup)",
            simd_encode_duration, encode_speedup
        );
        info!(
            "    SIMD decode:      {:?} ({:.2}x speedup)",
            simd_decode_duration, decode_speedup
        );
        info!(
            "    Throughput:       {:.1} MB/s encode, {:.1} MB/s decode",
            *size as f64 / simd_encode_duration.as_secs_f64() / 1_000_000.0,
            *size as f64 / simd_decode_duration.as_secs_f64() / 1_000_000.0
        );
    }

    // Demonstrate 5: HTTP Header Parsing Performance
    info!("\n🌐 HTTP Header Parsing Performance");

    let http_parser = SimdHttpHeaderParser::new();

    // Generate test headers
    let test_headers: Vec<String> = (0..1000)
        .map(|i| {
            format!(
                "Content-Type: application/json\r\n\
                Content-Length: {}\r\n\
                Authorization: Bearer token{}\r\n\
                X-Request-ID: req-{}\r\n\
                X-Custom-Header-{}: custom-value-{}\r\n\
                Cache-Control: no-cache, no-store\r\n\
                Accept: application/json, text/plain\r\n\
                User-Agent: PMCP-Client/1.0\r\n\
                Connection: keep-alive\r\n\r\n",
                1000 + i,
                i,
                i,
                i,
                i
            )
        })
        .collect();

    // SIMD HTTP header parsing
    info!("  🔄 Running SIMD HTTP header parsing benchmark...");
    let start = Instant::now();
    let mut simd_header_results = Vec::new();
    for headers in &test_headers {
        let parsed = http_parser.parse_headers(headers.as_bytes())?;
        simd_header_results.push(parsed);
    }
    let simd_header_duration = start.elapsed();

    // Standard HTTP header parsing (simplified)
    info!("  🔄 Running standard HTTP header parsing benchmark...");
    let start = Instant::now();
    let mut standard_header_results = Vec::new();
    for headers in &test_headers {
        let mut parsed = std::collections::HashMap::new();
        for line in headers.lines() {
            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_lowercase();
                let value = line[colon_pos + 1..].trim().to_string();
                parsed.insert(name, value);
            }
        }
        standard_header_results.push(parsed);
    }
    let standard_header_duration = start.elapsed();

    info!("  📈 HTTP Header Parsing Results:");
    info!("    Header sets parsed:   {}", simd_header_results.len());
    info!(
        "    SIMD parsing:         {:?} ({:.0} sets/sec)",
        simd_header_duration,
        simd_header_results.len() as f64 / simd_header_duration.as_secs_f64()
    );
    info!(
        "    Standard parsing:     {:?} ({:.0} sets/sec)",
        standard_header_duration,
        standard_header_results.len() as f64 / standard_header_duration.as_secs_f64()
    );
    info!(
        "    Header speedup:       {:.2}x",
        standard_header_duration.as_nanos() as f64 / simd_header_duration.as_nanos() as f64
    );

    // Demonstrate 6: Memory Usage and Cache Efficiency
    info!("\n💾 Memory Usage and Cache Efficiency");

    let initial_memory = get_memory_usage();

    // Process large amount of data to test memory efficiency
    let large_requests: Vec<String> = (0..50000)
        .map(|i| {
            format!(
                r#"{{"jsonrpc":"2.0","id":{},"method":"memory_test","params":{{"data":"{}"}}}}"#,
                i,
                "z".repeat(200)
            )
        })
        .collect();

    for chunk in large_requests.chunks(1000) {
        for json in chunk {
            simd_parser.parse_request(json.as_bytes())?;
        }
    }

    let final_memory = get_memory_usage();
    let memory_increase = final_memory.saturating_sub(initial_memory);

    info!("  📊 Memory Usage Analysis:");
    info!("    Initial memory:       {} KB", initial_memory);
    info!("    Final memory:         {} KB", final_memory);
    info!("    Memory increase:      {} KB", memory_increase);
    info!(
        "    Memory per document:  {} bytes",
        memory_increase * 1024 / large_requests.len()
    );

    // Final metrics summary
    let final_metrics = simd_parser.get_metrics();
    info!("  📊 Final SIMD Parser Metrics:");
    info!(
        "    Total operations:     {} documents",
        final_metrics.total_documents_parsed
    );
    info!(
        "    Total data processed: {} MB",
        final_metrics.total_bytes_processed / 1_000_000
    );
    info!(
        "    Average throughput:   {:.0} docs/sec",
        final_metrics.documents_per_second
    );
    info!(
        "    SIMD utilization:     {:.1}%",
        final_metrics.simd_operations_used as f64
            / (final_metrics.simd_operations_used + final_metrics.fallback_operations) as f64
            * 100.0
    );

    info!("\n🔄 SIMD parsing optimizations demonstrated:");
    info!("  • JSON-RPC parsing with AVX2/SSE4.2 vectorization");
    info!("  • Server-Sent Events parsing with SIMD acceleration");
    info!("  • Base64 encoding/decoding with vectorized operations");
    info!("  • HTTP header parsing with parallel processing");
    info!("  • Runtime CPU feature detection and optimization");
    info!("  • Memory-efficient processing with cache optimization");
    info!("  • Comprehensive performance metrics and monitoring");
    info!("  • Batch processing with parallel execution");

    info!("👋 SIMD parsing performance demonstration complete");

    Ok(())
}

/// Simple memory usage tracking (Linux-specific)
fn get_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;

        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        return kb_str.parse().unwrap_or(0);
                    }
                }
            }
        }
    }

    // Fallback for non-Linux systems
    0
}
