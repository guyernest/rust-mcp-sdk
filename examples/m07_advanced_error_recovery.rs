//! Advanced Error Recovery Example
//!
//! PMCP-4005: Demonstrates advanced error recovery strategies including:
//! - Adaptive retry policies with jitter
//! - Bulk operation recovery with partial failures
//! - Deadline-aware recovery with timeout management
//! - Health monitoring and cascade prevention
//! - Recovery coordination and metrics collection
//!
//! Run with: cargo run --example 31_advanced_error_recovery --features full

use async_trait::async_trait;
use pmcp::error::recovery::{
    AdvancedRecoveryExecutor, BulkRecoveryResult, HealthCheckResult, HealthMonitor, HealthStatus,
    JitterStrategy, RecoveryDeadline, RecoveryEvent, RecoveryPolicy, RecoveryStrategy,
};
use pmcp::error::{Error, ErrorCode, Result};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{info, warn, Level};

/// Mock health monitor for demonstration
#[derive(Debug)]
struct MockHealthMonitor {
    failure_rate: Arc<AtomicU32>,
}

impl MockHealthMonitor {
    fn new() -> Self {
        Self {
            failure_rate: Arc::new(AtomicU32::new(0)),
        }
    }

    fn set_failure_rate(&self, rate: u32) {
        self.failure_rate.store(rate, Ordering::Relaxed);
    }
}

#[async_trait]
impl HealthMonitor for MockHealthMonitor {
    async fn check_health(&self, component: &str) -> HealthCheckResult {
        let start_time = std::time::Instant::now();

        // Simulate health check delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        let failure_rate = self.failure_rate.load(Ordering::Relaxed);
        let status = if failure_rate > 50 {
            HealthStatus::Unhealthy
        } else if failure_rate > 20 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthCheckResult {
            component: component.to_string(),
            status,
            response_time_us: start_time.elapsed().as_micros() as u64,
            timestamp: SystemTime::now(),
            message: Some(format!("Mock health check for {}", component)),
        }
    }

    async fn get_health_status(&self, _component: &str) -> HealthStatus {
        let failure_rate = self.failure_rate.load(Ordering::Relaxed);
        if failure_rate > 50 {
            HealthStatus::Unhealthy
        } else if failure_rate > 20 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    async fn subscribe_health_changes(
        &self,
    ) -> Result<Box<dyn std::future::Future<Output = RecoveryEvent> + Send + Unpin>> {
        // Mock implementation - in real scenarios this would be a stream of events
        let future = async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            RecoveryEvent::HealthChanged {
                component: "mock_component".to_string(),
                old_status: HealthStatus::Healthy,
                new_status: HealthStatus::Degraded,
            }
        };
        Ok(Box::new(Box::pin(future)))
    }
}

/// Mock service that simulates various failure scenarios
#[derive(Debug)]
struct MockService {
    failure_count: Arc<AtomicUsize>,
    success_count: Arc<AtomicUsize>,
    total_calls: Arc<AtomicUsize>,
}

impl MockService {
    fn new() -> Self {
        Self {
            failure_count: Arc::new(AtomicUsize::new(0)),
            success_count: Arc::new(AtomicUsize::new(0)),
            total_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Simulate an operation that fails initially but succeeds after retries
    async fn unstable_operation(&self, operation_id: &str) -> Result<serde_json::Value> {
        let call_count = self.total_calls.fetch_add(1, Ordering::Relaxed);

        // Fail first 2 attempts, then succeed
        if call_count < 2 {
            self.failure_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(50)).await;
            return Err(Error::Internal(format!(
                "Unstable operation {} failed on attempt {}",
                operation_id,
                call_count + 1
            )));
        }

        self.success_count.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(serde_json::json!({
            "operation_id": operation_id,
            "result": "success",
            "attempt": call_count + 1
        }))
    }

    /// Simulate a timeout-sensitive operation
    async fn timeout_sensitive_operation(&self, delay_ms: u64) -> Result<serde_json::Value> {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        Ok(serde_json::json!({
            "result": "completed",
            "delay_ms": delay_ms
        }))
    }

    /// Simulate bulk operations with mixed success/failure
    async fn bulk_operation(&self, item_id: usize, fail_rate: f64) -> Result<serde_json::Value> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Use item_id to determine if this operation should fail
        let mut hasher = DefaultHasher::new();
        item_id.hash(&mut hasher);
        let random = (hasher.finish() % 100) as f64 / 100.0;

        tokio::time::sleep(Duration::from_millis(20)).await;

        if random < fail_rate {
            Err(Error::Internal(format!(
                "Bulk operation {} failed",
                item_id
            )))
        } else {
            Ok(serde_json::json!({
                "item_id": item_id,
                "result": "processed",
                "success": true
            }))
        }
    }

    fn get_stats(&self) -> (usize, usize, usize) {
        (
            self.total_calls.load(Ordering::Relaxed),
            self.success_count.load(Ordering::Relaxed),
            self.failure_count.load(Ordering::Relaxed),
        )
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting Advanced Error Recovery Example");

    // Create mock services
    let mock_service = Arc::new(MockService::new());
    let health_monitor = Arc::new(MockHealthMonitor::new());

    // Set up recovery policy with advanced strategies
    let mut policy = RecoveryPolicy::new(RecoveryStrategy::RetryAdaptive {
        attempts: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        multiplier: 2.0,
        jitter: JitterStrategy::Equal,
    });

    // Add specific strategies for different error codes
    policy.add_strategy(
        ErrorCode::REQUEST_TIMEOUT,
        RecoveryStrategy::DeadlineAware {
            max_recovery_time: Duration::from_secs(10),
            base_strategy: Box::new(RecoveryStrategy::RetryExponential {
                attempts: 5,
                initial_delay: Duration::from_millis(50),
                max_delay: Duration::from_secs(2),
                multiplier: 1.5,
            }),
        },
    );

    // Create advanced recovery executor
    let executor = AdvancedRecoveryExecutor::new(policy);
    let coordinator = executor.coordinator();

    // Set up health monitoring
    let _health_monitor_clone = health_monitor.clone();
    coordinator
        .add_event_handler(Arc::new(move |event| match event {
            RecoveryEvent::HealthChanged {
                component,
                old_status,
                new_status,
            } => {
                info!(
                    "🏥 Health changed for {}: {:?} → {:?}",
                    component, old_status, new_status
                );
            },
            RecoveryEvent::RecoveryStarted {
                operation_id,
                strategy,
            } => {
                info!(
                    "🔄 Recovery started for {}: strategy={}",
                    operation_id, strategy
                );
            },
            RecoveryEvent::RecoveryCompleted {
                operation_id,
                success,
                duration,
            } => {
                if success {
                    info!(
                        "✅ Recovery completed for {} in {:?}",
                        operation_id, duration
                    );
                } else {
                    warn!(
                        "❌ Recovery failed for {} after {:?}",
                        operation_id, duration
                    );
                }
            },
            RecoveryEvent::CascadingFailure {
                trigger_component,
                affected_components,
            } => {
                warn!(
                    "🌊 Cascading failure detected! Trigger: {}, Affected: {:?}",
                    trigger_component, affected_components
                );
            },
        }))
        .await;

    // Set up component dependencies for cascade detection
    coordinator
        .add_dependency("api_gateway".to_string(), vec!["database".to_string()])
        .await;
    coordinator
        .add_dependency(
            "user_service".to_string(),
            vec!["api_gateway".to_string(), "cache".to_string()],
        )
        .await;
    coordinator
        .add_dependency(
            "order_service".to_string(),
            vec!["user_service".to_string(), "payment_service".to_string()],
        )
        .await;

    info!("✅ Recovery system configured with advanced strategies");

    // Demonstrate 1: Adaptive Retry with Jitter
    info!("\n🎯 Demonstrating adaptive retry with jitter...");

    let service_clone = mock_service.clone();
    let result = executor
        .retry_adaptive(
            Error::Internal("Initial failure".to_string()),
            4,
            Duration::from_millis(100),
            Duration::from_secs(2),
            2.0,
            JitterStrategy::Equal,
            || {
                let service = service_clone.clone();
                async move { service.unstable_operation("adaptive_retry_test").await }
            },
        )
        .await;

    match result {
        Ok(value) => info!("  ✅ Adaptive retry succeeded: {}", value),
        Err(e) => warn!("  ❌ Adaptive retry failed: {}", e),
    }

    let (total_calls, successes, failures) = mock_service.get_stats();
    info!(
        "  📊 Service stats: {} total, {} successes, {} failures",
        total_calls, successes, failures
    );

    // Demonstrate 2: Deadline-Aware Recovery
    info!("\n⏰ Demonstrating deadline-aware recovery...");

    let mut deadline = RecoveryDeadline::new(Duration::from_millis(500));
    let service_clone = mock_service.clone();

    let result = executor
        .execute_with_deadline(
            "deadline_test",
            || {
                let service = service_clone.clone();
                async move { service.timeout_sensitive_operation(200).await }
            },
            &mut deadline,
            &RecoveryStrategy::RetryFixed {
                attempts: 3,
                delay: Duration::from_millis(100),
            },
        )
        .await;

    match result {
        Ok(value) => info!("  ✅ Deadline-aware operation succeeded: {}", value),
        Err(e) => warn!("  ⏰ Deadline-aware operation failed: {}", e),
    }

    // Demonstrate 3: Bulk Operation Recovery
    info!("\n📦 Demonstrating bulk operation recovery...");

    let bulk_handler = executor.bulk_handler();
    let service_clone = mock_service.clone();

    // Create bulk operations with 30% failure rate
    let operations: Vec<_> = (0..10)
        .map(|i| {
            let service = service_clone.clone();
            move || {
                let service = service.clone();
                async move { service.bulk_operation(i, 0.3).await }
            }
        })
        .collect();

    let bulk_result = bulk_handler
        .execute_bulk(
            operations, 0.6,   // 60% minimum success rate
            false, // don't fail fast
        )
        .await;

    match bulk_result {
        BulkRecoveryResult::AllSuccess(results) => {
            info!("  ✅ All {} bulk operations succeeded", results.len());
        },
        BulkRecoveryResult::PartialSuccess {
            successes,
            failures,
        } => {
            info!(
                "  🔄 Partial success: {} succeeded, {} failed",
                successes.len(),
                failures.len()
            );
            info!(
                "    Successes: {:?}",
                successes.iter().map(|(i, _)| i).collect::<Vec<_>>()
            );
            info!(
                "    Failures: {:?}",
                failures.iter().map(|(i, _)| i).collect::<Vec<_>>()
            );
        },
        BulkRecoveryResult::AllFailed(failures) => {
            warn!("  ❌ All {} bulk operations failed", failures.len());
        },
    }

    // Demonstrate 4: Health Monitoring and Cascade Detection
    info!("\n🏥 Demonstrating health monitoring and cascade detection...");

    // Simulate component health degradation
    health_monitor.set_failure_rate(60); // Make component unhealthy

    let health_result = health_monitor.check_health("database").await;
    info!(
        "  Health check result: {:?} ({}μs)",
        health_result.status, health_result.response_time_us
    );

    // Trigger cascade detection
    let affected_components = coordinator.detect_cascade("database").await;
    if !affected_components.is_empty() {
        info!(
            "  🌊 Cascade detected! Components that depend on database: {:?}",
            affected_components
        );

        // Simulate recovery actions
        for component in &affected_components {
            info!(
                "  🔧 Initiating recovery for dependent component: {}",
                component
            );
        }
    }

    // Demonstrate 5: Recovery Metrics and Monitoring
    info!("\n📊 Recovery metrics and monitoring...");

    let metrics = coordinator.get_metrics();
    info!("  Success rate: {:.1}%", metrics.success_rate());
    info!(
        "  Average recovery time: {:?}",
        metrics.average_recovery_time()
    );
    info!(
        "  Circuit breaker trips: {}",
        metrics.circuit_breaker_trips()
    );
    info!("  Fallback executions: {}", metrics.fallback_executions());
    info!("  Cascade preventions: {}", metrics.cascade_preventions());

    // Demonstrate 6: Different Jitter Strategies
    info!("\n🎲 Demonstrating different jitter strategies...");

    let base_delay = Duration::from_millis(1000);
    let strategies = vec![
        ("No Jitter", JitterStrategy::None),
        ("Full Jitter", JitterStrategy::Full),
        ("Equal Jitter", JitterStrategy::Equal),
        ("Decorrelated Jitter", JitterStrategy::Decorrelated),
    ];

    for (name, strategy) in strategies {
        let mut delays = Vec::new();
        for _ in 0..5 {
            let jittered =
                pmcp::error::recovery::JitterCalculator::calculate_delay(base_delay, strategy);
            delays.push(jittered.as_millis());
        }
        info!("  {} sample delays: {:?}ms", name, delays);
    }

    info!("\n🔄 Advanced error recovery features demonstrated:");
    info!("  • Adaptive retry with configurable jitter strategies");
    info!("  • Deadline-aware recovery with timeout management");
    info!("  • Bulk operation recovery with partial failure handling");
    info!("  • Health monitoring with cascade failure detection");
    info!("  • Recovery coordination with event-driven architecture");
    info!("  • Comprehensive metrics collection and monitoring");
    info!("  • Advanced circuit breakers with health integration");

    info!("👋 Advanced error recovery demonstration complete");

    Ok(())
}
