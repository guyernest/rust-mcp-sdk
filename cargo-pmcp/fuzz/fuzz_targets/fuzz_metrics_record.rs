//! Fuzz target for the metrics recording pipeline.
//!
//! Feeds arbitrary byte sequences as pseudo-random metrics samples
//! into [`MetricsRecorder`] and verifies it never panics. Checks
//! basic invariants (error rate bounds) after each fuzz run.
//!
//! Run with: `cargo +nightly fuzz run fuzz_metrics_record`

#![no_main]
use libfuzzer_sys::fuzz_target;

use cargo_pmcp::loadtest::error::McpError;
use cargo_pmcp::loadtest::metrics::{MetricsRecorder, OperationType, RequestSample};
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let mut recorder = MetricsRecorder::new(100);

    // Use bytes to generate pseudo-random samples
    for chunk in data.chunks(4) {
        let latency_ms = u16::from_le_bytes([
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
        ]) as u64;
        let is_error = chunk.get(2).copied().unwrap_or(0) > 128;
        let op_byte = chunk.get(3).copied().unwrap_or(0);
        let op = match op_byte % 4 {
            0 => OperationType::ToolsCall,
            1 => OperationType::ResourcesRead,
            2 => OperationType::PromptsGet,
            _ => OperationType::Initialize,
        };

        if is_error {
            recorder.record(&RequestSample::error(
                op,
                Duration::from_millis(latency_ms),
                McpError::Timeout,
                None,
            ));
        } else {
            recorder.record(&RequestSample::success(
                op,
                Duration::from_millis(latency_ms),
                None,
            ));
        }
    }

    // Must not panic when taking snapshot after arbitrary recording
    let snap = recorder.snapshot();
    // Basic invariants
    assert!(snap.error_rate >= 0.0 && snap.error_rate <= 1.0);
});
