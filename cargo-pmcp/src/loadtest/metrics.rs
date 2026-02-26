//! HdrHistogram-based metrics pipeline with coordinated omission correction.
//!
//! Provides [`MetricsRecorder`] for recording MCP request latency samples into
//! separate success/error HdrHistogram buckets, with per-operation-type tracking.
//! Coordinated omission correction is applied at recording time via
//! [`hdrhistogram::Histogram::record_correct`].

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use hdrhistogram::Histogram;

use crate::loadtest::error::McpError;

/// Type of MCP operation being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// MCP initialize handshake.
    Initialize,
    /// tools/call request.
    ToolsCall,
    /// resources/read request.
    ResourcesRead,
    /// prompts/get request.
    PromptsGet,
    /// tools/list discovery request.
    ToolsList,
    /// resources/list discovery request.
    ResourcesList,
    /// prompts/list discovery request.
    PromptsList,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initialize => write!(f, "initialize"),
            Self::ToolsCall => write!(f, "tools/call"),
            Self::ResourcesRead => write!(f, "resources/read"),
            Self::PromptsGet => write!(f, "prompts/get"),
            Self::ToolsList => write!(f, "tools/list"),
            Self::ResourcesList => write!(f, "resources/list"),
            Self::PromptsList => write!(f, "prompts/list"),
        }
    }
}

/// A single request measurement sample.
pub struct RequestSample {
    /// The MCP operation type that was measured.
    pub operation: OperationType,
    /// Wall-clock duration of the request.
    pub duration: Duration,
    /// Ok(()) for success, Err(McpError) for failure.
    pub result: Result<(), McpError>,
    /// When the sample was recorded.
    pub timestamp: Instant,
}

impl RequestSample {
    /// Creates a success sample.
    pub fn success(operation: OperationType, duration: Duration) -> Self {
        Self {
            operation,
            duration,
            result: Ok(()),
            timestamp: Instant::now(),
        }
    }

    /// Creates an error sample.
    pub fn error(operation: OperationType, duration: Duration, err: McpError) -> Self {
        Self {
            operation,
            duration,
            result: Err(err),
            timestamp: Instant::now(),
        }
    }
}

/// Point-in-time snapshot of metrics.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Total successful requests.
    pub success_count: u64,
    /// Total failed requests.
    pub error_count: u64,
    /// Total requests (success + error).
    pub total_requests: u64,
    /// 50th percentile latency in milliseconds (success histogram).
    pub p50: u64,
    /// 95th percentile latency in milliseconds (success histogram).
    pub p95: u64,
    /// 99th percentile latency in milliseconds (success histogram).
    pub p99: u64,
    /// 99th percentile latency in milliseconds (error histogram).
    pub error_p99: u64,
}

/// Metrics recorder with HdrHistogram.
///
/// Full implementation in Plan 01-03.
pub struct MetricsRecorder {
    success_histogram: Histogram<u64>,
    error_histogram: Histogram<u64>,
    operation_counts: HashMap<OperationType, u64>,
    expected_interval_ms: u64,
}

impl MetricsRecorder {
    /// Creates a new recorder with the given expected interval (ms) for
    /// coordinated omission correction.
    pub fn new(expected_interval_ms: u64) -> Self {
        Self {
            success_histogram: Histogram::<u64>::new_with_bounds(1, 60_000, 3)
                .expect("valid histogram params"),
            error_histogram: Histogram::<u64>::new_with_bounds(1, 60_000, 3)
                .expect("valid histogram params"),
            operation_counts: HashMap::new(),
            expected_interval_ms,
        }
    }

    /// Records a request sample into the appropriate histogram.
    pub fn record(&mut self, sample: &RequestSample) {
        let ms = sample.duration.as_millis().max(1) as u64;
        *self
            .operation_counts
            .entry(sample.operation)
            .or_insert(0) += 1;

        match &sample.result {
            Ok(()) => {
                let _ = self
                    .success_histogram
                    .record_correct(ms, self.expected_interval_ms);
            }
            Err(_) => {
                let _ = self
                    .error_histogram
                    .record_correct(ms, self.expected_interval_ms);
            }
        }
    }

    /// Total successful request count (from histogram).
    pub fn success_count(&self) -> u64 {
        self.success_histogram.len()
    }

    /// Total error request count (from histogram).
    pub fn error_count(&self) -> u64 {
        self.error_histogram.len()
    }

    /// Total request count (success + error).
    pub fn total_requests(&self) -> u64 {
        self.success_count() + self.error_count()
    }

    /// 50th percentile latency in ms (success histogram).
    pub fn p50(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.50)
    }

    /// 95th percentile latency in ms (success histogram).
    pub fn p95(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.95)
    }

    /// 99th percentile latency in ms (success histogram).
    pub fn p99(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.99)
    }

    /// 99th percentile latency in ms (error histogram).
    pub fn error_p99(&self) -> u64 {
        self.error_histogram.value_at_quantile(0.99)
    }

    /// Error rate as a fraction (0.0 to 1.0).
    pub fn error_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.error_count() as f64 / total as f64
    }

    /// Returns the count for a specific operation type.
    pub fn operation_count(&self, op: OperationType) -> u64 {
        self.operation_counts.get(&op).copied().unwrap_or(0)
    }

    /// Takes a point-in-time snapshot of all metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            success_count: self.success_count(),
            error_count: self.error_count(),
            total_requests: self.total_requests(),
            p50: self.p50(),
            p95: self.p95(),
            p99: self.p99(),
            error_p99: self.error_p99(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::error::McpError;

    #[test]
    fn test_new_recorder_has_zero_counts() {
        let recorder = MetricsRecorder::new(100);
        assert_eq!(recorder.total_requests(), 0);
        assert_eq!(recorder.success_count(), 0);
        assert_eq!(recorder.error_count(), 0);
    }

    #[test]
    fn test_record_success_increments_count() {
        let mut recorder = MetricsRecorder::new(100);
        for _ in 0..5 {
            let sample = RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(50),
            );
            recorder.record(&sample);
        }
        assert_eq!(recorder.success_count(), 5);
        assert_eq!(recorder.error_count(), 0);
        assert_eq!(recorder.total_requests(), 5);
    }

    #[test]
    fn test_record_error_increments_count() {
        let mut recorder = MetricsRecorder::new(100);
        for _ in 0..3 {
            let sample = RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(50),
                McpError::Timeout,
            );
            recorder.record(&sample);
        }
        assert_eq!(recorder.error_count(), 3);
        assert_eq!(recorder.success_count(), 0);
    }

    #[test]
    fn test_percentiles_single_value() {
        let mut recorder = MetricsRecorder::new(10_000);
        let sample = RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(50),
        );
        recorder.record(&sample);
        assert_eq!(recorder.p50(), 50);
        assert_eq!(recorder.p95(), 50);
        assert_eq!(recorder.p99(), 50);
    }

    #[test]
    fn test_percentiles_known_distribution() {
        let mut recorder = MetricsRecorder::new(10_000);
        for i in 1..=100 {
            let sample = RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(i),
            );
            recorder.record(&sample);
        }
        let p50 = recorder.p50() as i64;
        let p95 = recorder.p95() as i64;
        let p99 = recorder.p99() as i64;
        assert!((p50 - 50).abs() <= 1, "p50 was {p50}, expected ~50");
        assert!((p95 - 95).abs() <= 1, "p95 was {p95}, expected ~95");
        assert!((p99 - 99).abs() <= 1, "p99 was {p99}, expected ~99");
    }

    #[test]
    fn test_coordinated_omission_correction() {
        let mut recorder = MetricsRecorder::new(10);
        let sample = RequestSample::success(
            OperationType::ToolsCall,
            Duration::from_millis(100),
        );
        recorder.record(&sample);
        // With correction, HdrHistogram fills in synthetic samples at
        // 90ms, 80ms, 70ms, ..., 10ms. So total count > 1.
        assert!(
            recorder.success_count() > 1,
            "Expected synthetic fills, got count={}",
            recorder.success_count()
        );
        // The corrected median should be well below 100ms.
        assert!(
            recorder.p50() < 100,
            "p50 was {}, expected < 100 with correction",
            recorder.p50()
        );
    }

    #[test]
    fn test_success_and_error_separate_buckets() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..10 {
            let sample = RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
            );
            recorder.record(&sample);
        }
        for _ in 0..10 {
            let sample = RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(500),
                McpError::Timeout,
            );
            recorder.record(&sample);
        }
        assert_eq!(recorder.p99(), 10, "success p99 should be ~10ms");
        assert_eq!(recorder.error_p99(), 500, "error p99 should be ~500ms");
    }

    #[test]
    fn test_per_operation_counts() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..3 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
            ));
        }
        for _ in 0..2 {
            recorder.record(&RequestSample::success(
                OperationType::ResourcesRead,
                Duration::from_millis(10),
            ));
        }
        recorder.record(&RequestSample::error(
            OperationType::PromptsGet,
            Duration::from_millis(10),
            McpError::Timeout,
        ));
        assert_eq!(recorder.operation_count(OperationType::ToolsCall), 3);
        assert_eq!(recorder.operation_count(OperationType::ResourcesRead), 2);
        assert_eq!(recorder.operation_count(OperationType::PromptsGet), 1);
    }

    #[test]
    fn test_snapshot_captures_current_state() {
        let mut recorder = MetricsRecorder::new(10_000);
        for i in 1..=100 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(i),
            ));
        }
        recorder.record(&RequestSample::error(
            OperationType::PromptsGet,
            Duration::from_millis(200),
            McpError::Timeout,
        ));
        let snap = recorder.snapshot();
        assert_eq!(snap.success_count, 100);
        assert_eq!(snap.error_count, 1);
        assert_eq!(snap.total_requests, 101);
        assert!((snap.p50 as i64 - 50).abs() <= 1);
        assert_eq!(snap.error_p99, 200);
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(OperationType::ToolsCall.to_string(), "tools/call");
        assert_eq!(OperationType::ResourcesRead.to_string(), "resources/read");
        assert_eq!(OperationType::PromptsGet.to_string(), "prompts/get");
        assert_eq!(OperationType::Initialize.to_string(), "initialize");
        assert_eq!(OperationType::ToolsList.to_string(), "tools/list");
        assert_eq!(OperationType::ResourcesList.to_string(), "resources/list");
        assert_eq!(OperationType::PromptsList.to_string(), "prompts/list");
    }

    #[test]
    fn test_error_rate_calculation() {
        let mut recorder = MetricsRecorder::new(10_000);
        for _ in 0..7 {
            recorder.record(&RequestSample::success(
                OperationType::ToolsCall,
                Duration::from_millis(10),
            ));
        }
        for _ in 0..3 {
            recorder.record(&RequestSample::error(
                OperationType::ToolsCall,
                Duration::from_millis(10),
                McpError::Timeout,
            ));
        }
        let rate = recorder.error_rate();
        assert!(
            (rate - 0.3).abs() < 0.001,
            "error_rate was {rate}, expected 0.3"
        );
    }

    #[test]
    fn test_empty_percentiles_return_zero() {
        let recorder = MetricsRecorder::new(100);
        assert_eq!(recorder.p50(), 0);
        assert_eq!(recorder.p95(), 0);
        assert_eq!(recorder.p99(), 0);
    }
}
