//! Progress reporting support for long-running operations.
//!
//! This module provides the infrastructure for tools to report progress during execution,
//! following the MCP progress notification protocol.

use crate::error::{Error, Result};
use crate::types::{Notification, ProgressNotification, ProgressToken, ServerNotification};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Trait for reporting progress during tool execution.
///
/// Implementations of this trait handle the details of sending progress notifications
/// to clients, including rate limiting and validation.
#[async_trait]
pub trait ProgressReporter: Send + Sync {
    /// Report progress with optional total and message.
    ///
    /// # Arguments
    ///
    /// * `progress` - Current progress value (must increase with each call)
    /// * `total` - Optional total value for the operation
    /// * `message` - Optional human-readable progress message
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Progress does not increase from the last reported value
    /// - The notification sender fails
    async fn report_progress(
        &self,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()>;

    /// Report percentage progress (0-100).
    ///
    /// This is a convenience method that automatically sets total to 100.
    async fn report_percent(&self, percent: f64, message: Option<String>) -> Result<()> {
        self.report_progress(percent, Some(100.0), message).await
    }

    /// Report count-based progress (e.g., "5 of 10 items processed").
    ///
    /// This is a convenience method for operations that process a known number of items.
    async fn report_count(
        &self,
        current: usize,
        total: usize,
        message: Option<String>,
    ) -> Result<()> {
        self.report_progress(current as f64, Some(total as f64), message)
            .await
    }
}

/// Server-side progress reporter implementation.
///
/// This implementation:
/// - Validates that progress values are finite and non-negative
/// - Treats non-increasing progress as a no-op (doesn't error)
/// - Implements rate limiting to prevent notification flooding (except final updates)
/// - Sends progress notifications through the server's notification channel
///
/// # Thread Safety
///
/// This reporter is `Clone` and can be safely shared across tasks for concurrent progress reporting.
#[derive(Clone)]
pub struct ServerProgressReporter {
    /// Progress token for this operation
    progress_token: ProgressToken,
    /// Notification sender callback
    notification_sender: Arc<dyn Fn(Notification) + Send + Sync>,
    /// Last reported progress value (None = no progress reported yet)
    last_progress: Arc<Mutex<Option<f64>>>,
    /// Timestamp of last sent notification (for rate limiting)
    last_sent: Arc<Mutex<Option<Instant>>>,
    /// Minimum interval between notifications (default: 100ms)
    rate_limit_interval: Duration,
}

impl ServerProgressReporter {
    /// Create a new progress reporter.
    ///
    /// # Arguments
    ///
    /// * `progress_token` - Progress token from the request metadata
    /// * `notification_sender` - Callback to send notifications to the client
    pub fn new(
        progress_token: ProgressToken,
        notification_sender: Arc<dyn Fn(Notification) + Send + Sync>,
    ) -> Self {
        Self {
            progress_token,
            notification_sender,
            last_progress: Arc::new(Mutex::new(None)),
            last_sent: Arc::new(Mutex::new(None)),
            rate_limit_interval: Duration::from_millis(100), // Max 10 notifications/second
        }
    }

    /// Create a new progress reporter with custom rate limit.
    ///
    /// # Arguments
    ///
    /// * `progress_token` - Progress token from the request metadata
    /// * `notification_sender` - Callback to send notifications to the client
    /// * `rate_limit_interval` - Minimum duration between notifications
    pub fn with_rate_limit(
        progress_token: ProgressToken,
        notification_sender: Arc<dyn Fn(Notification) + Send + Sync>,
        rate_limit_interval: Duration,
    ) -> Self {
        Self {
            progress_token,
            notification_sender,
            last_progress: Arc::new(Mutex::new(None)),
            last_sent: Arc::new(Mutex::new(None)),
            rate_limit_interval,
        }
    }

    /// Validate progress and total values.
    fn validate_values(progress: f64, total: Option<f64>) -> Result<()> {
        const EPSILON: f64 = 1e-9;

        // Check progress is finite and non-negative
        if !progress.is_finite() {
            return Err(Error::validation("Progress must be a finite number"));
        }
        if progress < 0.0 {
            return Err(Error::validation("Progress cannot be negative"));
        }

        // Check total is finite and non-negative if provided
        if let Some(t) = total {
            if !t.is_finite() {
                return Err(Error::validation("Total must be a finite number"));
            }
            if t < 0.0 {
                return Err(Error::validation("Total cannot be negative"));
            }
            // Check progress doesn't exceed total (with epsilon tolerance)
            if progress > t + EPSILON {
                return Err(Error::validation(format!(
                    "Progress ({}) exceeds total ({})",
                    progress, t
                )));
            }
        }

        Ok(())
    }

    /// Check if enough time has passed since the last notification.
    fn should_send(&self) -> bool {
        let last_sent = self.last_sent.lock().unwrap();
        match *last_sent {
            None => true,
            Some(instant) => instant.elapsed() >= self.rate_limit_interval,
        }
    }

    /// Update the last sent timestamp.
    fn update_last_sent(&self) {
        *self.last_sent.lock().unwrap() = Some(Instant::now());
    }
}

#[async_trait]
impl ProgressReporter for ServerProgressReporter {
    async fn report_progress(
        &self,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> Result<()> {
        const EPSILON: f64 = 1e-9;

        // Validate input values
        Self::validate_values(progress, total)?;

        // Check if progress increases (or is first report)
        let mut last_progress_guard = self.last_progress.lock().unwrap();
        let last_progress = *last_progress_guard;

        match last_progress {
            None => {
                // First progress report - always allowed
                tracing::debug!(
                    progress = progress,
                    total = ?total,
                    "First progress report"
                );
            },
            Some(last) if progress <= last + EPSILON => {
                // Progress did not increase - check if it's the final notification
                let is_final = total.is_some_and(|t| (progress - t).abs() < EPSILON);

                if is_final {
                    // Final notification - always send
                    tracing::debug!(
                        progress = progress,
                        total = ?total,
                        "Final progress notification"
                    );
                } else {
                    // Non-increasing progress - treat as no-op
                    tracing::debug!(
                        progress = progress,
                        last = last,
                        "Skipping non-increasing progress update"
                    );
                    return Ok(());
                }
            },
            Some(_) => {
                // Progress increased - normal case
            },
        }

        // Rate limiting: skip notification if too soon after last one
        // Exception: Always send final notification (progress == total)
        let is_final = total.is_some_and(|t| (progress - t).abs() < EPSILON);
        if !is_final && !self.should_send() {
            // Skip due to rate limiting
            tracing::trace!(
                progress = progress,
                "Skipping progress notification due to rate limiting"
            );
            // Still update last_progress so future reports work correctly
            *last_progress_guard = Some(progress);
            return Ok(());
        }

        // Update tracked state
        *last_progress_guard = Some(progress);
        drop(last_progress_guard); // Release lock before async operations

        self.update_last_sent();

        // Log before creating notification (to avoid borrow issues)
        tracing::debug!(
            progress = progress,
            total = ?total,
            message = ?message,
            "Sending progress notification"
        );

        // Create and send progress notification
        let notification =
            Notification::Server(ServerNotification::Progress(ProgressNotification {
                progress_token: self.progress_token.clone(),
                progress,
                total,
                message,
            }));

        (self.notification_sender)(notification);
        Ok(())
    }
}

impl std::fmt::Debug for ServerProgressReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerProgressReporter")
            .field("progress_token", &self.progress_token)
            .field("last_progress", &self.last_progress.lock().unwrap())
            .field("rate_limit_interval", &self.rate_limit_interval)
            .finish()
    }
}

/// A no-op progress reporter that drops all reports.
#[derive(Debug, Clone, Default)]
pub struct NoopProgressReporter;

#[async_trait]
impl ProgressReporter for NoopProgressReporter {
    async fn report_progress(
        &self,
        _progress: f64,
        _total: Option<f64>,
        _message: Option<String>,
    ) -> crate::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProgressToken;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_initial_zero_progress_allowed() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::with_rate_limit(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::ZERO, // No rate limiting for tests
        );

        // Initial 0.0 progress: should be allowed now
        reporter
            .report_progress(0.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Progress from 0.0 to 10.0: OK
        reporter
            .report_progress(10.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_progress_increases() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::with_rate_limit(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::ZERO, // No rate limiting for tests
        );

        // First progress: OK
        reporter
            .report_progress(10.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Increasing progress: OK
        reporter
            .report_progress(20.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        // Non-increasing progress: no-op (not error), no notification sent
        reporter
            .report_progress(15.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2); // Still 2 - was skipped

        // Continuing to increase: OK
        reporter
            .report_progress(30.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_validation_rejects_invalid_values() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::new(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        // NaN progress: error
        let result = reporter.report_progress(f64::NAN, Some(100.0), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("finite"));

        // Negative progress: error
        let result = reporter.report_progress(-10.0, Some(100.0), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("negative"));

        // Progress > total: error
        let result = reporter.report_progress(150.0, Some(100.0), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds"));

        // No notifications should have been sent
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::with_rate_limit(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(50),
        );

        // First notification: sent
        reporter
            .report_progress(10.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Immediate second notification: rate-limited
        reporter
            .report_progress(20.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Still 1 - second was rate-limited

        // Wait for rate limit to expire
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Third notification: sent
        reporter
            .report_progress(30.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_final_notification_always_sent() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::with_rate_limit(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(100),
        );

        // Send initial progress
        reporter
            .report_progress(50.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Immediately send final progress (100 of 100) - should bypass rate limit
        reporter
            .report_progress(100.0, Some(100.0), None)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2); // Final always sent
    }

    #[tokio::test]
    async fn test_percent_helper() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::new(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        reporter
            .report_percent(50.0, Some("Halfway done".to_string()))
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_count_helper() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let reporter = ServerProgressReporter::new(
            ProgressToken::String("test".to_string()),
            Arc::new(move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }),
        );

        reporter
            .report_count(5, 10, Some("5 of 10 processed".to_string()))
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
