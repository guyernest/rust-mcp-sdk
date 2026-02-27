//! Property-based tests for engine metrics invariants.
//!
//! These tests use proptest to verify that core invariants hold
//! across a wide range of random inputs for the metrics pipeline.

use proptest::prelude::*;
use std::collections::HashMap;
use std::time::Duration;

use cargo_pmcp::loadtest::breaking::BreakingPointDetector;
use cargo_pmcp::loadtest::error::McpError;
use cargo_pmcp::loadtest::metrics::{
    MetricsRecorder, MetricsSnapshot, OperationType, RequestSample,
};

proptest! {
    /// Total requests in snapshot equals number of samples recorded.
    /// Uses high expected_interval to avoid coordinated omission fill
    /// inflating the count beyond the logical sample count.
    #[test]
    fn metrics_total_matches_sample_count(
        success_count in 0u32..100,
        error_count in 0u32..50,
        latency_ms in 1u64..500,
    ) {
        // Use very high expected_interval so record_correct doesn't add synthetic fills
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..success_count {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(latency_ms),
                None,
            ));
        }
        for _ in 0..error_count {
            recorder.record(&RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(latency_ms),
                McpError::Timeout,
                None,
            ));
        }
        let snap = recorder.snapshot();
        // With high expected_interval, no synthetic fills, counts should match exactly
        prop_assert_eq!(snap.success_count, success_count as u64);
        prop_assert_eq!(snap.error_count, error_count as u64);
        prop_assert_eq!(snap.total_requests, (success_count + error_count) as u64);
    }

    /// Error rate is always between 0.0 and 1.0 inclusive.
    #[test]
    fn error_rate_bounded(
        success_count in 0u32..100,
        error_count in 0u32..100,
    ) {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..success_count {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                None,
            ));
        }
        for _ in 0..error_count {
            recorder.record(&RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                McpError::Timeout,
                None,
            ));
        }
        let snap = recorder.snapshot();
        prop_assert!(snap.error_rate >= 0.0);
        prop_assert!(snap.error_rate <= 1.0);
    }

    /// P50 <= P95 <= P99 always (monotonicity of percentiles).
    #[test]
    fn percentiles_monotonic(
        latencies in prop::collection::vec(1u64..10000, 1..200),
    ) {
        let mut recorder = MetricsRecorder::new(10_000);
        for lat in &latencies {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(*lat),
                None,
            ));
        }
        let snap = recorder.snapshot();
        prop_assert!(snap.p50 <= snap.p95, "p50 ({}) > p95 ({})", snap.p50, snap.p95);
        prop_assert!(snap.p95 <= snap.p99, "p95 ({}) > p99 ({})", snap.p95, snap.p99);
    }

    /// Per-operation counts sum to total (when no CO correction).
    #[test]
    fn operation_counts_sum_to_total(
        tool_calls in 0u32..50,
        resource_reads in 0u32..50,
        prompt_gets in 0u32..50,
    ) {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..tool_calls {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
                None,
            ));
        }
        for _ in 0..resource_reads {
            recorder.record(&RequestSample::success(
                OperationType::ResourcesRead,
                Duration::from_millis(10),
                None,
            ));
        }
        for _ in 0..prompt_gets {
            recorder.record(&RequestSample::success(
                OperationType::PromptsGet,
                Duration::from_millis(10),
                None,
            ));
        }
        let snap = recorder.snapshot();
        let op_total: u64 = snap.operation_counts.values().sum();
        let expected = (tool_calls + resource_reads + prompt_gets) as u64;
        prop_assert_eq!(op_total, expected);
    }

    /// Any valid config roundtrips through from_toml without loss.
    #[test]
    fn valid_config_roundtrips(
        vus in 1u32..100,
        duration in 1u64..3600,
        timeout in 100u64..30000,
        weight in 1u32..1000,
    ) {
        let toml_str = format!(
            r#"
[settings]
virtual_users = {}
duration_secs = {}
timeout_ms = {}

[[scenario]]
type = "tools/call"
weight = {}
tool = "test-tool"
"#,
            vus, duration, timeout, weight
        );
        let config = cargo_pmcp::loadtest::config::LoadTestConfig::from_toml(&toml_str);
        prop_assert!(config.is_ok(), "Failed to parse: {:?}", config.err());
        let config = config.unwrap();
        prop_assert_eq!(config.settings.virtual_users, vus);
        prop_assert_eq!(config.settings.duration_secs, duration);
        prop_assert_eq!(config.settings.timeout_ms, timeout);
        prop_assert_eq!(config.scenario[0].weight(), weight);
    }

    /// BreakingPointDetector fires at most once across arbitrary input sequences.
    #[test]
    fn proptest_breaking_point_fires_at_most_once(
        error_rates in prop::collection::vec(0.0f64..1.0, 5..50),
        p99_values in prop::collection::vec(0u64..10000, 5..50),
        vus_values in prop::collection::vec(1u32..200, 5..50),
    ) {
        let len = error_rates.len().min(p99_values.len()).min(vus_values.len());
        let mut detector = BreakingPointDetector::new(10);
        let mut fire_count = 0u32;

        for i in 0..len {
            let snapshot = MetricsSnapshot {
                p50: 0, p95: 0, p99: p99_values[i],
                error_p50: 0, error_p95: 0, error_p99: 0,
                success_count: 0, error_count: 0, total_requests: 0,
                error_rate: error_rates[i],
                operation_counts: HashMap::new(),
                per_operation_errors: HashMap::new(),
                error_category_counts: HashMap::new(),
                per_tool: Vec::new(),
            };
            if detector.observe(&snapshot, vus_values[i]).is_some() {
                fire_count += 1;
            }
        }
        prop_assert!(fire_count <= 1, "Detector fired {} times, expected at most 1", fire_count);
    }

    /// BreakingPointDetector never fires when the window is not full.
    #[test]
    fn proptest_breaking_point_requires_full_window(
        error_rates in prop::collection::vec(0.0f64..1.0, 1..10),
        p99_values in prop::collection::vec(0u64..10000, 1..10),
    ) {
        // Window size 10 but we feed < 10 samples
        let len = error_rates.len().min(p99_values.len()).min(9);
        let mut detector = BreakingPointDetector::new(10);

        for i in 0..len {
            let snapshot = MetricsSnapshot {
                p50: 0, p95: 0, p99: p99_values[i],
                error_p50: 0, error_p95: 0, error_p99: 0,
                success_count: 0, error_count: 0, total_requests: 0,
                error_rate: error_rates[i],
                operation_counts: HashMap::new(),
                per_operation_errors: HashMap::new(),
                error_category_counts: HashMap::new(),
                per_tool: Vec::new(),
            };
            let result = detector.observe(&snapshot, 10);
            prop_assert!(result.is_none(), "Should not fire with only {} samples (need 10)", i + 1);
        }
    }

    /// BreakingPointDetector never triggers on error_rate_spike when all error rates
    /// are below the absolute threshold (10%).
    #[test]
    fn proptest_breaking_point_low_error_rate_never_triggers_error_spike(
        error_rates in prop::collection::vec(0.0f64..0.10, 10..30),
    ) {
        let mut detector = BreakingPointDetector::new(10);

        for (i, &error_rate) in error_rates.iter().enumerate() {
            let snapshot = MetricsSnapshot {
                p50: 0, p95: 0, p99: 50, // Stable P99 -- no latency trigger
                error_p50: 0, error_p95: 0, error_p99: 0,
                success_count: 0, error_count: 0, total_requests: 0,
                error_rate,
                operation_counts: HashMap::new(),
                per_operation_errors: HashMap::new(),
                error_category_counts: HashMap::new(),
                per_tool: Vec::new(),
            };
            if let Some(bp) = detector.observe(&snapshot, (i as u32) + 1) {
                // Only latency degradation is allowed, not error_rate_spike
                prop_assert_ne!(
                    bp.reason, "error_rate_spike",
                    "Should not trigger error_rate_spike with error_rate < 10%"
                );
            }
        }
    }
}
