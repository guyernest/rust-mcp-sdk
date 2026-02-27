//! Rolling-window breaking point detector for load test degradation analysis.
//!
//! [`BreakingPointDetector`] monitors a sliding window of metrics snapshots and
//! auto-detects when an MCP server's performance degrades under load. Detection
//! fires exactly once per test run with report-and-continue semantics.
//!
//! The detector uses a self-calibrating rolling window: it splits the window in
//! half and compares recent metrics (newer half) against a baseline (older half).
//! This avoids hardcoded absolute thresholds and adapts to each server's normal
//! performance profile.
//!
//! # Detection Conditions
//!
//! Detection triggers when **either** condition is met:
//!
//! 1. **Error rate spike**: recent error rate > 10% (absolute) AND > 2x baseline
//! 2. **Latency degradation**: recent P99 > 3x baseline P99
//!
//! # Window Requirements
//!
//! - Minimum `window_size` samples must be collected before first evaluation
//! - Default window size is 10 samples (at 2-second snapshot interval = 20s of history)
//! - Detection fires only once; subsequent degradation does not re-trigger

use std::collections::VecDeque;
use std::time::Instant;

use crate::loadtest::metrics::MetricsSnapshot;

/// Default rolling window size (number of snapshot samples).
const DEFAULT_WINDOW_SIZE: usize = 10;

/// Minimum error rate threshold before detection triggers.
const ERROR_RATE_ABSOLUTE_THRESHOLD: f64 = 0.10; // 10%

/// Error rate must be this multiple of baseline to trigger.
const ERROR_RATE_RELATIVE_MULTIPLIER: f64 = 2.0;

/// P99 latency must be this multiple of baseline to trigger.
const P99_RELATIVE_MULTIPLIER: f64 = 3.0;

/// A single snapshot sample stored in the rolling window.
#[derive(Debug, Clone)]
struct WindowSample {
    /// Error rate as a fraction (0.0..=1.0).
    error_rate: f64,
    /// P99 latency in milliseconds.
    p99_ms: u64,
    /// Number of active virtual users at this sample point.
    active_vus: u32,
}

/// A detected breaking point event.
///
/// Contains the VU count, reason, and timing of when the server began
/// degrading under load. This is included in the JSON report and shown
/// as a live terminal warning.
#[derive(Debug, Clone)]
pub struct BreakingPoint {
    /// VU count at which the breaking point was detected.
    pub vus: u32,
    /// Reason category: `"error_rate_spike"` or `"latency_degradation"`.
    pub reason: String,
    /// Human-readable detail string explaining the detection.
    pub detail: String,
    /// When the breaking point was detected.
    pub detected_at: Instant,
}

/// Rolling-window breaking point detector.
///
/// Monitors a sliding window of [`MetricsSnapshot`] samples and fires a
/// one-time detection event when performance degrades beyond thresholds.
///
/// # Usage
///
/// ```rust,no_run
/// use cargo_pmcp::loadtest::breaking::BreakingPointDetector;
/// use cargo_pmcp::loadtest::metrics::MetricsSnapshot;
///
/// let mut detector = BreakingPointDetector::with_default_window();
/// // Feed snapshots from the metrics aggregator:
/// // if let Some(bp) = detector.observe(&snapshot, active_vus) {
/// //     eprintln!("Breaking point at {} VUs: {}", bp.vus, bp.detail);
/// // }
/// ```
pub struct BreakingPointDetector {
    /// Rolling window of recent snapshot samples.
    window: VecDeque<WindowSample>,
    /// Maximum number of samples in the window.
    window_size: usize,
    /// Whether a breaking point has already been detected (fire-once).
    detected: bool,
    /// The detected breaking point event, if any.
    breaking_point: Option<BreakingPoint>,
}

impl BreakingPointDetector {
    /// Creates a detector with the given window size and empty state.
    pub fn new(window_size: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            detected: false,
            breaking_point: None,
        }
    }

    /// Creates a detector with [`DEFAULT_WINDOW_SIZE`] (10 samples).
    pub fn with_default_window() -> Self {
        Self::new(DEFAULT_WINDOW_SIZE)
    }

    /// Observe a new metrics snapshot and check for degradation.
    ///
    /// Returns `Some(BreakingPoint)` on first detection, `None` otherwise.
    /// After the first detection, all subsequent calls return `None`
    /// (fire-once semantics).
    ///
    /// The detector requires the window to be completely filled before
    /// evaluating, to prevent false positives during cold start.
    pub fn observe(
        &mut self,
        snapshot: &MetricsSnapshot,
        active_vus: u32,
    ) -> Option<BreakingPoint> {
        // Record the sample
        let sample = WindowSample {
            error_rate: snapshot.error_rate,
            p99_ms: snapshot.p99,
            active_vus,
        };

        self.window.push_back(sample);

        // Maintain window size
        if self.window.len() > self.window_size {
            self.window.pop_front();
        }

        // Fire once only
        if self.detected {
            return None;
        }

        // Need minimum fill before first evaluation
        if self.window.len() < self.window_size {
            return None;
        }

        // Split window in half: baseline (older) vs recent (newer)
        let half = self.window_size / 2;
        let baseline = &self.window.as_slices();
        // VecDeque may split across two slices; collect into a flat view
        let all_samples: Vec<&WindowSample> = self.window.iter().collect();
        let baseline_slice = &all_samples[..half];
        let recent_slice = &all_samples[half..];

        // Compute baseline averages
        let baseline_error_rate = mean_f64(baseline_slice.iter().map(|s| s.error_rate));
        let baseline_p99 = mean_u64(baseline_slice.iter().map(|s| s.p99_ms));

        // Compute recent averages
        let recent_error_rate = mean_f64(recent_slice.iter().map(|s| s.error_rate));
        let recent_p99 = mean_u64(recent_slice.iter().map(|s| s.p99_ms));

        // Get the most recent sample's VU count for the detection event
        let current_vus = self.window.back().map(|s| s.active_vus).unwrap_or(0);

        // Error rate check: recent > 10% absolute AND > 2x baseline
        if recent_error_rate > ERROR_RATE_ABSOLUTE_THRESHOLD
            && recent_error_rate > baseline_error_rate * ERROR_RATE_RELATIVE_MULTIPLIER
        {
            let bp = BreakingPoint {
                vus: current_vus,
                reason: "error_rate_spike".to_string(),
                detail: format!(
                    "Error rate {:.1}% exceeds threshold (>10% and >{}x baseline {:.1}%)",
                    recent_error_rate * 100.0,
                    ERROR_RATE_RELATIVE_MULTIPLIER,
                    baseline_error_rate * 100.0,
                ),
                detected_at: Instant::now(),
            };
            self.detected = true;
            self.breaking_point = Some(bp.clone());
            return Some(bp);
        }

        // Latency check: recent P99 > 3x baseline P99 (avoid div-by-zero)
        if baseline_p99 > 0 && recent_p99 > baseline_p99 * P99_RELATIVE_MULTIPLIER as u64 {
            let bp = BreakingPoint {
                vus: current_vus,
                reason: "latency_degradation".to_string(),
                detail: format!(
                    "P99 {}ms exceeds {}x baseline {}ms",
                    recent_p99, P99_RELATIVE_MULTIPLIER, baseline_p99,
                ),
                detected_at: Instant::now(),
            };
            self.detected = true;
            self.breaking_point = Some(bp.clone());
            return Some(bp);
        }

        // Suppress the unused variable warning from the baseline slices binding
        let _ = baseline;

        None
    }

    /// Returns whether a breaking point has been detected.
    pub fn detected(&self) -> bool {
        self.detected
    }

    /// Returns the detected breaking point, if any.
    pub fn breaking_point(&self) -> Option<&BreakingPoint> {
        self.breaking_point.as_ref()
    }
}

/// Compute the arithmetic mean of an iterator of f64 values.
/// Returns 0.0 for empty iterators.
fn mean_f64(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut count = 0u64;
    for v in values {
        sum += v;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Compute the arithmetic mean of an iterator of u64 values.
/// Returns 0 for empty iterators.
fn mean_u64(values: impl Iterator<Item = u64>) -> u64 {
    let mut sum = 0u64;
    let mut count = 0u64;
    for v in values {
        sum = sum.saturating_add(v);
        count += 1;
    }
    if count == 0 {
        0
    } else {
        sum / count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Create a MetricsSnapshot with the specified error rate and P99.
    fn make_snapshot(error_rate: f64, p99: u64) -> MetricsSnapshot {
        MetricsSnapshot {
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
        }
    }

    #[test]
    fn test_no_detection_below_threshold() {
        let mut detector = BreakingPointDetector::new(10);
        // Feed 10 samples with low error rate (2%) and stable P99 (50ms)
        for _ in 0..10 {
            let snap = make_snapshot(0.02, 50);
            let result = detector.observe(&snap, 10);
            assert!(result.is_none(), "Should not detect with low error rate");
        }
        assert!(!detector.detected());
    }

    #[test]
    fn test_detection_on_error_rate_spike() {
        let mut detector = BreakingPointDetector::new(10);

        // 5 baseline samples with 2% error rate
        for _ in 0..5 {
            let snap = make_snapshot(0.02, 50);
            assert!(detector.observe(&snap, 5).is_none());
        }

        // 5 recent samples with 15% error rate (>10% absolute, >2x baseline 2%)
        let mut detected = false;
        for _ in 0..5 {
            let snap = make_snapshot(0.15, 50);
            if let Some(bp) = detector.observe(&snap, 20) {
                detected = true;
                assert_eq!(bp.reason, "error_rate_spike");
                assert_eq!(bp.vus, 20);
                assert!(bp.detail.contains("15.0%"));
            }
        }
        assert!(detected, "Should have detected error rate spike");
    }

    #[test]
    fn test_detection_on_latency_spike() {
        let mut detector = BreakingPointDetector::new(10);

        // 5 baseline samples with P99=50ms
        for _ in 0..5 {
            let snap = make_snapshot(0.01, 50);
            assert!(detector.observe(&snap, 5).is_none());
        }

        // 5 recent samples with P99=200ms (>3x baseline 50ms = 150ms threshold)
        let mut detected = false;
        for _ in 0..5 {
            let snap = make_snapshot(0.01, 200);
            if let Some(bp) = detector.observe(&snap, 15) {
                detected = true;
                assert_eq!(bp.reason, "latency_degradation");
                assert_eq!(bp.vus, 15);
                assert!(bp.detail.contains("200ms"));
            }
        }
        assert!(detected, "Should have detected latency degradation");
    }

    #[test]
    fn test_no_detection_before_window_full() {
        let mut detector = BreakingPointDetector::new(10);

        // Feed only 9 samples (window not full) with very bad metrics
        for _ in 0..9 {
            let snap = make_snapshot(0.90, 5000);
            let result = detector.observe(&snap, 50);
            assert!(result.is_none(), "Should not detect before window is full");
        }
        assert!(!detector.detected());
    }

    #[test]
    fn test_fires_only_once() {
        let mut detector = BreakingPointDetector::new(10);

        // 5 baseline with low error rate
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.02, 50), 5);
        }

        // 5 recent with high error rate -- should trigger
        let mut fire_count = 0;
        for _ in 0..5 {
            if detector.observe(&make_snapshot(0.30, 50), 20).is_some() {
                fire_count += 1;
            }
        }
        assert_eq!(fire_count, 1, "Should fire exactly once");

        // Feed more bad data -- should not fire again
        for _ in 0..10 {
            let result = detector.observe(&make_snapshot(0.50, 50), 30);
            assert!(result.is_none(), "Should not fire after first detection");
        }
    }

    #[test]
    fn test_no_detection_when_error_rate_below_absolute() {
        let mut detector = BreakingPointDetector::new(10);

        // Baseline: 1% error rate
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.01, 50), 5);
        }

        // Recent: 5% error rate (>2x baseline 1%, but below 10% absolute)
        for _ in 0..5 {
            let result = detector.observe(&make_snapshot(0.05, 50), 10);
            assert!(
                result.is_none(),
                "Should not trigger when below 10% absolute threshold"
            );
        }
    }

    #[test]
    fn test_no_detection_when_error_rate_below_relative() {
        let mut detector = BreakingPointDetector::new(10);

        // Baseline: 8% error rate
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.08, 50), 5);
        }

        // Recent: 11% error rate (>10% absolute, but NOT >2x baseline 8% = 16%)
        for _ in 0..5 {
            let result = detector.observe(&make_snapshot(0.11, 50), 10);
            assert!(
                result.is_none(),
                "Should not trigger when below 2x baseline threshold"
            );
        }
    }

    #[test]
    fn test_detection_returns_correct_vus() {
        let mut detector = BreakingPointDetector::new(10);

        // 5 baseline samples
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.02, 50), 5);
        }

        // 5 recent samples with escalating VUs
        for vu_count in 10..15 {
            if let Some(bp) = detector.observe(&make_snapshot(0.25, 50), vu_count) {
                // The VU count should be from the most recent sample
                assert_eq!(bp.vus, vu_count);
                return;
            }
        }
        panic!("Expected detection to fire");
    }

    #[test]
    fn test_detected_and_breaking_point_accessors() {
        let mut detector = BreakingPointDetector::new(10);

        // Initially, no detection
        assert!(!detector.detected());
        assert!(detector.breaking_point().is_none());

        // 5 baseline + 5 degraded to trigger
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.02, 50), 5);
        }
        for _ in 0..5 {
            detector.observe(&make_snapshot(0.25, 50), 20);
        }

        assert!(detector.detected());
        let bp = detector
            .breaking_point()
            .expect("should have breaking point");
        assert_eq!(bp.reason, "error_rate_spike");
        assert_eq!(bp.vus, 20);
    }
}
