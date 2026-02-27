//! Load test execution engine with metrics aggregation and graceful shutdown.
//!
//! [`LoadTestEngine`] is the top-level orchestrator that:
//! - Spawns N virtual user tasks via [`tokio_util::task::TaskTracker`]
//! - Collects metrics through a bounded mpsc channel
//! - Publishes snapshots through a watch channel for live display
//! - Coordinates graceful shutdown via [`CancellationToken`]
//!
//! The engine supports duration-based and iteration-based stopping with
//! first-limit-wins semantics.

use crate::loadtest::config::LoadTestConfig;
use crate::loadtest::display::display_loop;
use crate::loadtest::error::LoadTestError;
use crate::loadtest::metrics::{MetricsRecorder, MetricsSnapshot, RequestSample};
use crate::loadtest::vu::{vu_loop, ActiveVuCounter};

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Compile-time Send bounds verification for channel-transported types.
///
/// These assertions fail at compile time if the types are not `Send`,
/// which would prevent them from being used in mpsc/watch channels.
fn _assert_send<T: Send>() {}
#[allow(dead_code)]
fn _check_send_bounds() {
    _assert_send::<RequestSample>();
    _assert_send::<MetricsSnapshot>();
}

/// Top-level load test engine configuration and entry point.
///
/// Spawns N virtual users as independent tokio tasks, collects metrics
/// through a bounded mpsc channel, and publishes snapshots through a
/// watch channel for live display consumption.
pub struct LoadTestEngine {
    config: LoadTestConfig,
    base_url: String,
    max_iterations: Option<u64>,
    ramp_up: Option<Duration>,
    no_color: bool,
}

impl LoadTestEngine {
    /// Creates a new engine with the given configuration and target URL.
    pub fn new(config: LoadTestConfig, base_url: String) -> Self {
        Self {
            config,
            base_url,
            max_iterations: None,
            ramp_up: None,
            no_color: false,
        }
    }

    /// Sets an iteration limit. The test stops after this many total iterations
    /// across all VUs (first-limit-wins with duration).
    pub fn with_iterations(mut self, n: u64) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Sets a ramp-up duration. VUs are spawned with uniform stagger over this
    /// period. Ramp-up metrics are excluded from the final report.
    pub fn with_ramp_up(mut self, duration: Duration) -> Self {
        self.ramp_up = Some(duration);
        self
    }

    /// Disables colored output.
    pub fn with_no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    /// Returns a reference to the engine's configuration.
    pub fn config(&self) -> &LoadTestConfig {
        &self.config
    }

    /// Returns the configured max iterations, if any.
    pub fn max_iterations(&self) -> Option<u64> {
        self.max_iterations
    }

    /// Returns the configured ramp-up duration, if any.
    pub fn ramp_up(&self) -> Option<Duration> {
        self.ramp_up
    }

    /// Returns whether colored output is disabled.
    pub fn no_color(&self) -> bool {
        self.no_color
    }

    /// Run the load test. Returns the final metrics snapshot.
    ///
    /// This method:
    /// 1. Validates the configuration
    /// 2. Creates the metrics channel pipeline (mpsc + watch)
    /// 3. Spawns the metrics aggregator task
    /// 4. Spawns N VU tasks (with optional ramp-up stagger)
    /// 5. Runs the controller (duration/iteration/Ctrl+C)
    /// 6. Waits for drain, collects final snapshot
    pub async fn run(&self) -> Result<LoadTestResult, LoadTestError> {
        // Validate config
        self.config.validate()?;

        let vu_count = self.config.settings.virtual_users;
        let cancel = CancellationToken::new();
        let tracker = TaskTracker::new();
        let active_vus = ActiveVuCounter::new();
        let iteration_counter = self
            .max_iterations
            .map(|_| Arc::new(AtomicU64::new(0)));
        let http_client = reqwest::Client::new();

        // Metrics channels
        let buffer_size = (vu_count as usize) * 100;
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(buffer_size);
        let expected_interval_ms = self.config.settings.expected_interval_ms;
        let initial_snapshot = MetricsRecorder::new(expected_interval_ms).snapshot();
        let (snapshot_tx, snapshot_rx) = watch::channel(initial_snapshot);

        // Spawn VU tasks with optional ramp-up
        let test_start = Instant::now();
        let config_arc = Arc::new(self.config.clone());

        let ramp_up_end: Instant;
        if let Some(ramp_duration) = self.ramp_up {
            let delay_per_vu = if vu_count > 1 {
                ramp_duration / vu_count
            } else {
                Duration::ZERO
            };
            for i in 0..vu_count {
                tracker.spawn(vu_loop(
                    i,
                    config_arc.clone(),
                    http_client.clone(),
                    self.base_url.clone(),
                    sample_tx.clone(),
                    cancel.clone(),
                    iteration_counter.clone(),
                    self.max_iterations,
                    active_vus.clone(),
                ));
                if i < vu_count - 1 {
                    tokio::time::sleep(delay_per_vu).await;
                }
            }
            ramp_up_end = Instant::now();
        } else {
            for i in 0..vu_count {
                tracker.spawn(vu_loop(
                    i,
                    config_arc.clone(),
                    http_client.clone(),
                    self.base_url.clone(),
                    sample_tx.clone(),
                    cancel.clone(),
                    iteration_counter.clone(),
                    self.max_iterations,
                    active_vus.clone(),
                ));
            }
            ramp_up_end = test_start; // No ramp-up, all metrics count
        }

        // Drop original sender -- VUs hold their own clones
        drop(sample_tx);

        // Spawn metrics aggregator (NOT on tracker -- must outlive VU tasks)
        let aggregator_cancel = cancel.clone();
        let aggregator_handle = tokio::spawn(metrics_aggregator(
            sample_rx,
            snapshot_tx,
            aggregator_cancel,
            expected_interval_ms,
            ramp_up_end,
        ));

        // Spawn live display task (reads from watch channel, NOT on tracker)
        let display_cancel = cancel.clone();
        let display_vus = active_vus.clone();
        let display_rx = snapshot_rx.clone();
        let target_vus = vu_count;
        let no_color = self.no_color;
        let display_handle = tokio::spawn(display_loop(
            display_rx,
            display_vus,
            target_vus,
            display_cancel,
            no_color,
            test_start,
        ));

        // Run controller -- first-limit-wins between duration, iteration limit, Ctrl+C
        let duration = Duration::from_secs(self.config.settings.duration_secs);
        let controller_cancel = cancel.clone();

        tokio::select! {
            _ = tokio::time::sleep(duration) => {
                controller_cancel.cancel();
            }
            _ = controller_cancel.cancelled() => {
                // Iteration limit or other cancellation triggered by VU
            }
            _ = handle_ctrl_c(cancel.clone()) => {
                // Ctrl+C handled inside
            }
        }

        // Drain: close tracker and wait for all VU tasks
        tracker.close();
        tracker.wait().await;

        // Wait for aggregator to finish processing remaining samples
        let _ = aggregator_handle.await;

        // Wait for display to render final state
        let _ = display_handle.await;

        // Collect final snapshot
        let final_snapshot = snapshot_rx.borrow().clone();

        Ok(LoadTestResult {
            snapshot: final_snapshot,
            elapsed: test_start.elapsed(),
            final_active_vus: active_vus.get(),
        })
    }
}

/// Result of a completed load test run.
#[derive(Debug)]
pub struct LoadTestResult {
    /// Final metrics snapshot (excludes ramp-up if applicable).
    pub snapshot: MetricsSnapshot,
    /// Total elapsed time of the test.
    pub elapsed: Duration,
    /// Number of VUs that were still active at test end.
    pub final_active_vus: u32,
}

/// Metrics aggregator task.
///
/// Consumes [`RequestSample`] values from the mpsc channel, records them
/// into dual recorders (live + report), and publishes snapshots via the
/// watch channel every 2 seconds.
///
/// Uses `biased;` select to ensure the tick branch is checked first,
/// preventing display starvation when the mpsc channel is busy.
///
/// The dual-recorder pattern excludes ramp-up samples from the final report:
/// - `live` records ALL samples (for live display)
/// - `report` records only post-ramp-up samples (for final result)
async fn metrics_aggregator(
    mut sample_rx: mpsc::Receiver<RequestSample>,
    snapshot_tx: watch::Sender<MetricsSnapshot>,
    cancel: CancellationToken,
    expected_interval_ms: u64,
    ramp_up_end: Instant,
) {
    let mut live = MetricsRecorder::new(expected_interval_ms);
    let mut report = MetricsRecorder::new(expected_interval_ms);
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;

            _ = tick.tick() => {
                // Drain all available samples before publishing snapshot
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let _ = snapshot_tx.send(live.snapshot());
            }
            result = sample_rx.recv() => {
                match result {
                    Some(sample) => {
                        if sample.timestamp >= ramp_up_end {
                            report.record(&sample);
                        }
                        live.record(&sample);
                    }
                    None => {
                        // All senders dropped -- VUs are done
                        let _ = snapshot_tx.send(report.snapshot());
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                // Drain remaining samples
                while let Ok(sample) = sample_rx.try_recv() {
                    if sample.timestamp >= ramp_up_end {
                        report.record(&sample);
                    }
                    live.record(&sample);
                }
                let _ = snapshot_tx.send(report.snapshot());
                break;
            }
        }
    }
}

/// Ctrl+C handler with two-phase shutdown.
///
/// First Ctrl+C triggers graceful drain via the cancellation token.
/// Second Ctrl+C performs a hard abort via `std::process::exit(1)`.
async fn handle_ctrl_c(cancel: CancellationToken) {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl_c handler");
    eprintln!("\nReceived Ctrl+C, stopping gracefully...");
    cancel.cancel();

    // Second Ctrl+C: hard abort
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl_c handler");
    eprintln!("\nReceived second Ctrl+C, aborting immediately.");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};
    use crate::loadtest::metrics::{OperationType, RequestSample};

    fn minimal_config() -> LoadTestConfig {
        LoadTestConfig {
            settings: Settings {
                virtual_users: 2,
                duration_secs: 10,
                timeout_ms: 5000,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        }
    }

    #[test]
    fn test_load_test_engine_builder() {
        let config = minimal_config();
        let engine = LoadTestEngine::new(config, "http://localhost:3000".to_string())
            .with_iterations(1000)
            .with_ramp_up(Duration::from_secs(10))
            .with_no_color(true);

        assert_eq!(engine.max_iterations(), Some(1000));
        assert_eq!(engine.ramp_up(), Some(Duration::from_secs(10)));
        assert!(engine.no_color());
        assert_eq!(engine.config().settings.virtual_users, 2);
    }

    #[tokio::test]
    async fn test_metrics_aggregator_processes_samples() {
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(100);
        let initial = MetricsRecorder::new(100).snapshot();
        let (snapshot_tx, snapshot_rx) = watch::channel(initial);
        let cancel = CancellationToken::new();
        let ramp_up_end = Instant::now();

        // Send 5 success samples
        for _ in 0..5 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(10));
            sample_tx.send(sample).await.unwrap();
        }

        // Drop sender to signal completion
        drop(sample_tx);

        // Run aggregator to completion
        metrics_aggregator(sample_rx, snapshot_tx, cancel, 100, ramp_up_end).await;

        let snap = snapshot_rx.borrow().clone();
        assert_eq!(snap.total_requests, 5, "Expected 5 total requests, got {}", snap.total_requests);
    }

    #[tokio::test]
    async fn test_metrics_aggregator_dual_recorder_excludes_ramp_up() {
        let (sample_tx, sample_rx) = mpsc::channel::<RequestSample>(100);
        let initial = MetricsRecorder::new(10_000).snapshot();
        let (snapshot_tx, snapshot_rx) = watch::channel(initial);
        let cancel = CancellationToken::new();

        // Set ramp_up_end 50ms in the future
        let ramp_up_end = Instant::now() + Duration::from_millis(50);

        // Send 3 samples immediately (before ramp_up_end -- ramp-up period)
        for _ in 0..3 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(10));
            sample_tx.send(sample).await.unwrap();
        }

        // Wait past the ramp-up boundary
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Send 2 samples after ramp_up_end (post-ramp-up)
        for _ in 0..2 {
            let sample =
                RequestSample::success(OperationType::ToolsCall, Duration::from_millis(20));
            sample_tx.send(sample).await.unwrap();
        }

        // Drop sender to signal completion
        drop(sample_tx);

        // Run aggregator -- final snapshot uses the report recorder (post-ramp-up only)
        metrics_aggregator(sample_rx, snapshot_tx, cancel, 10_000, ramp_up_end).await;

        let snap = snapshot_rx.borrow().clone();
        // The report recorder should only have the 2 post-ramp-up samples
        assert_eq!(
            snap.total_requests, 2,
            "Expected 2 post-ramp-up requests, got {}",
            snap.total_requests
        );
    }

    #[tokio::test]
    async fn test_engine_run_with_no_color_does_not_panic() {
        // Smoke test: engine handles VU connection failures gracefully.
        // VUs will fail to connect (no server), die after respawn attempts,
        // and engine returns a result with 0 successful requests.
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 1,
                duration_secs: 5,
                timeout_ms: 500,
                expected_interval_ms: 100,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
        };
        let engine = LoadTestEngine::new(config, "http://127.0.0.1:1".to_string())
            .with_no_color(true)
            .with_iterations(1);

        let result = engine.run().await;
        assert!(result.is_ok(), "Engine should not panic or error: {result:?}");
        let result = result.unwrap();
        assert_eq!(result.snapshot.success_count, 0, "No server, so no successes");
    }

    #[test]
    fn test_load_test_result_fields() {
        let snapshot = MetricsRecorder::new(100).snapshot();
        let result = LoadTestResult {
            snapshot,
            elapsed: Duration::from_secs(30),
            final_active_vus: 5,
        };
        assert_eq!(result.elapsed, Duration::from_secs(30));
        assert_eq!(result.final_active_vus, 5);
        assert_eq!(result.snapshot.total_requests, 0);
    }
}
