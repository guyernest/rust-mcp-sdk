//! HdrHistogram-based metrics pipeline with coordinated omission correction.
//!
//! Provides [`MetricsRecorder`] for recording MCP request latency samples into
//! separate success/error HdrHistogram buckets, with per-operation-type tracking.
//! Coordinated omission correction is applied at recording time via
//! [`hdrhistogram::Histogram::record_correct`].
//!
//! # Design
//!
//! - **Single-owner**: No `Arc<Mutex>` -- designed for Phase 2's mpsc channel
//!   aggregation pattern where one thread owns the recorder.
//! - **Separate buckets**: Success and error latencies are tracked in independent
//!   histograms so error spikes don't pollute success percentiles.
//! - **Coordinated omission correction**: Applied at recording time via
//!   `record_correct()`, not post-hoc. This fills in synthetic samples for
//!   intervals missed when the system was stalled.
//! - **Millisecond resolution**: Matches how users think about latency.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use hdrhistogram::Histogram;

use crate::loadtest::error::McpError;

/// Type of MCP operation being measured.
///
/// Each variant maps to an MCP protocol method. The [`fmt::Display`] impl
/// produces the wire-format string (e.g., `"tools/call"`).
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
        let s = match self {
            Self::Initialize => "initialize",
            Self::ToolsCall => "tools/call",
            Self::ResourcesRead => "resources/read",
            Self::PromptsGet => "prompts/get",
            Self::ToolsList => "tools/list",
            Self::ResourcesList => "resources/list",
            Self::PromptsList => "prompts/list",
        };
        f.write_str(s)
    }
}

/// A single request measurement sample.
///
/// Created via [`RequestSample::success`] or [`RequestSample::error`] convenience
/// constructors. Passed to [`MetricsRecorder::record`] for ingestion.
pub struct RequestSample {
    /// The MCP operation type that was measured.
    pub operation: OperationType,
    /// Wall-clock duration of the request.
    pub duration: Duration,
    /// `Ok(())` for success, `Err(McpError)` for failure.
    pub result: Result<(), McpError>,
    /// When the sample was taken.
    pub timestamp: Instant,
}

impl RequestSample {
    /// Create a success sample with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `operation` - The MCP operation type that was measured.
    /// * `duration` - Wall-clock duration of the request.
    pub fn success(operation: OperationType, duration: Duration) -> Self {
        Self {
            operation,
            duration,
            result: Ok(()),
            timestamp: Instant::now(),
        }
    }

    /// Create an error sample with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `operation` - The MCP operation type that was measured.
    /// * `duration` - Wall-clock duration of the request.
    /// * `err` - The MCP error that occurred.
    pub fn error(operation: OperationType, duration: Duration, err: McpError) -> Self {
        Self {
            operation,
            duration,
            result: Err(err),
            timestamp: Instant::now(),
        }
    }
}

/// Point-in-time snapshot of all metrics state.
///
/// Captured via [`MetricsRecorder::snapshot`]. All percentile values are in
/// milliseconds.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Success latency P50 (milliseconds).
    pub p50: u64,
    /// Success latency P95 (milliseconds).
    pub p95: u64,
    /// Success latency P99 (milliseconds).
    pub p99: u64,
    /// Error latency P50 (milliseconds).
    pub error_p50: u64,
    /// Error latency P95 (milliseconds).
    pub error_p95: u64,
    /// Error latency P99 (milliseconds).
    pub error_p99: u64,
    /// Total successful requests recorded.
    pub success_count: u64,
    /// Total failed requests recorded.
    pub error_count: u64,
    /// Total requests (success + error).
    pub total_requests: u64,
    /// Fraction of requests that were errors (0.0..=1.0).
    pub error_rate: f64,
    /// Per-operation total request counts.
    pub operation_counts: HashMap<OperationType, u64>,
    /// Per-operation error counts.
    pub per_operation_errors: HashMap<OperationType, u64>,
}

/// HdrHistogram-backed metrics recorder with coordinated omission correction.
///
/// Records MCP request latency samples into separate success/error histograms.
/// Designed for single-owner usage -- no internal locking. Phase 2 will use
/// mpsc channels to feed samples from worker tasks to a single recorder.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use cargo_pmcp::loadtest::metrics::{MetricsRecorder, RequestSample, OperationType};
///
/// let mut recorder = MetricsRecorder::new(100);
/// let sample = RequestSample::success(OperationType::ToolsCall, Duration::from_millis(42));
/// recorder.record(&sample);
///
/// assert_eq!(recorder.success_count(), 1);
/// assert_eq!(recorder.p50(), 42);
/// ```
pub struct MetricsRecorder {
    /// Histogram for successful request latencies.
    success_histogram: Histogram<u64>,
    /// Histogram for failed request latencies.
    error_histogram: Histogram<u64>,
    /// Expected interval between requests in milliseconds.
    /// Used by `record_correct()` for coordinated omission correction.
    expected_interval_ms: u64,
    /// Per-operation total request counts (success + error).
    operation_counts: HashMap<OperationType, u64>,
    /// Per-operation error counts.
    error_counts: HashMap<OperationType, u64>,
    /// Running total of successful requests (logical, not histogram entries).
    total_success: u64,
    /// Running total of failed requests (logical, not histogram entries).
    total_errors: u64,
}

impl MetricsRecorder {
    /// Create a new recorder with the given expected request interval.
    ///
    /// The `expected_interval_ms` is used for coordinated omission correction:
    /// when a request takes much longer than expected, synthetic samples are
    /// filled in to account for requests that *would have* been sent during
    /// the stall.
    ///
    /// Histograms are created with 3 significant figures of precision and
    /// auto-resize enabled.
    pub fn new(expected_interval_ms: u64) -> Self {
        let mut success_histogram =
            Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        success_histogram.auto(true);

        let mut error_histogram =
            Histogram::<u64>::new(3).expect("3 sigfigs is always valid");
        error_histogram.auto(true);

        Self {
            success_histogram,
            error_histogram,
            expected_interval_ms,
            operation_counts: HashMap::new(),
            error_counts: HashMap::new(),
            total_success: 0,
            total_errors: 0,
        }
    }

    /// Record a request sample with coordinated omission correction.
    ///
    /// The sample is routed to the success or error histogram based on its
    /// `result` field. `record_correct()` is used instead of plain `record()`
    /// to apply coordinated omission correction.
    pub fn record(&mut self, sample: &RequestSample) {
        let ms = sample.duration.as_millis() as u64;

        // Increment per-operation total count
        *self.operation_counts.entry(sample.operation).or_insert(0) += 1;

        match &sample.result {
            Ok(()) => {
                let _ = self
                    .success_histogram
                    .record_correct(ms, self.expected_interval_ms);
                self.total_success += 1;
            }
            Err(_) => {
                let _ = self
                    .error_histogram
                    .record_correct(ms, self.expected_interval_ms);
                self.total_errors += 1;
                *self.error_counts.entry(sample.operation).or_insert(0) += 1;
            }
        }
    }

    /// Success latency P50 in milliseconds. Returns 0 if no samples recorded.
    pub fn p50(&self) -> u64 {
        if self.success_histogram.len() == 0 {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.50)
    }

    /// Success latency P95 in milliseconds. Returns 0 if no samples recorded.
    pub fn p95(&self) -> u64 {
        if self.success_histogram.len() == 0 {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.95)
    }

    /// Success latency P99 in milliseconds. Returns 0 if no samples recorded.
    pub fn p99(&self) -> u64 {
        if self.success_histogram.len() == 0 {
            return 0;
        }
        self.success_histogram.value_at_quantile(0.99)
    }

    /// Error latency P50 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p50(&self) -> u64 {
        if self.error_histogram.len() == 0 {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.50)
    }

    /// Error latency P95 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p95(&self) -> u64 {
        if self.error_histogram.len() == 0 {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.95)
    }

    /// Error latency P99 in milliseconds. Returns 0 if no samples recorded.
    pub fn error_p99(&self) -> u64 {
        if self.error_histogram.len() == 0 {
            return 0;
        }
        self.error_histogram.value_at_quantile(0.99)
    }

    /// Total number of successful requests recorded.
    ///
    /// Note: this returns histogram entry count which includes synthetic
    /// fill-ins from coordinated omission correction. Use this for accurate
    /// percentile denominators.
    pub fn success_count(&self) -> u64 {
        self.success_histogram.len()
    }

    /// Total number of failed requests recorded.
    ///
    /// Note: this returns histogram entry count which includes synthetic
    /// fill-ins from coordinated omission correction.
    pub fn error_count(&self) -> u64 {
        self.error_histogram.len()
    }

    /// Total requests recorded (success + error histogram entries).
    pub fn total_requests(&self) -> u64 {
        self.success_histogram.len() + self.error_histogram.len()
    }

    /// Error rate as a fraction (0.0..=1.0). Returns 0.0 if no requests recorded.
    pub fn error_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.error_histogram.len() as f64 / total as f64
    }

    /// Total requests for a specific operation type (success + error).
    ///
    /// This is the *logical* count (one per `record()` call), not inflated
    /// by coordinated omission correction.
    pub fn operation_count(&self, op: OperationType) -> u64 {
        self.operation_counts.get(&op).copied().unwrap_or(0)
    }

    /// Capture a point-in-time snapshot of all metrics.
    ///
    /// The snapshot is a self-contained value that can be sent across threads
    /// or serialized without holding a reference to the recorder.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            p50: self.p50(),
            p95: self.p95(),
            p99: self.p99(),
            error_p50: self.error_p50(),
            error_p95: self.error_p95(),
            error_p99: self.error_p99(),
            success_count: self.success_histogram.len(),
            error_count: self.error_histogram.len(),
            total_requests: self.total_requests(),
            error_rate: self.error_rate(),
            operation_counts: self.operation_counts.clone(),
            per_operation_errors: self.error_counts.clone(),
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
