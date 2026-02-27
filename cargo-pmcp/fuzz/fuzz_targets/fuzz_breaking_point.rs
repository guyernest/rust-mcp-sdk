//! Fuzz target for the breaking point detector.
//!
//! Feeds arbitrary byte sequences as pseudo-random metrics snapshots
//! into [`BreakingPointDetector::observe()`] and verifies it never panics.
//! Checks the fire-once invariant after each fuzz run.
//!
//! Run with: `cargo +nightly fuzz run fuzz_breaking_point`

#![no_main]
use libfuzzer_sys::fuzz_target;

use cargo_pmcp::loadtest::breaking::BreakingPointDetector;
use cargo_pmcp::loadtest::metrics::MetricsSnapshot;
use std::collections::HashMap;

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }

    let mut detector = BreakingPointDetector::new(10);
    let mut fire_count = 0u32;

    // Decode arbitrary bytes into a sequence of (error_rate, p99, active_vus) tuples.
    // Each tuple consumes 10 bytes: 4 for error_rate (f32 -> f64), 4 for p99 (u32 -> u64), 2 for vus (u16 -> u32).
    for chunk in data.chunks(10) {
        if chunk.len() < 10 {
            break;
        }

        // Decode error_rate from 4 bytes as f32, then convert to f64 and clamp to [0.0, 1.0]
        let error_rate_raw = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let error_rate = if error_rate_raw.is_nan() || error_rate_raw.is_infinite() {
            0.0
        } else {
            (error_rate_raw.abs() % 1.0) as f64
        };

        // Decode p99 from 4 bytes as u32
        let p99 = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;

        // Decode active_vus from 2 bytes as u16
        let active_vus = u16::from_le_bytes([chunk[8], chunk[9]]) as u32;

        let snapshot = MetricsSnapshot {
            p50: 0,
            p95: 0,
            p99,
            error_p50: 0,
            error_p95: 0,
            error_p99: 0,
            success_count: 0,
            error_count: 0,
            total_requests: 0,
            error_rate,
            operation_counts: HashMap::new(),
            per_operation_errors: HashMap::new(),
            error_category_counts: HashMap::new(),
            per_tool: Vec::new(),
        };

        if detector.observe(&snapshot, active_vus).is_some() {
            fire_count += 1;
        }
    }

    // Fire-once invariant: at most one detection
    assert!(fire_count <= 1, "Detector fired {} times", fire_count);
});
