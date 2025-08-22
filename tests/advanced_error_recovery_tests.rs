//! Advanced error recovery system tests
//!
//! PMCP-4005: Comprehensive tests for advanced error recovery features

use pmcp::error::recovery::{
    AdvancedRecoveryExecutor, BulkRecoveryResult, HealthCheckResult, HealthStatus,
    JitterCalculator, JitterStrategy, RecoveryCoordinator, RecoveryDeadline, RecoveryEvent,
    RecoveryMetrics, RecoveryPolicy, RecoveryStrategy,
};
use pmcp::error::{Error, ErrorCode};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn test_recovery_deadline() {
    let mut deadline = RecoveryDeadline::new(Duration::from_millis(100));

    // Initially should not be exceeded
    assert!(!deadline.exceeded);
    assert!(deadline.has_time_for(Duration::from_millis(50)));

    // Wait for deadline to pass
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Update should mark as exceeded
    assert!(deadline.update());
    assert!(deadline.exceeded);
    assert!(!deadline.has_time_for(Duration::from_millis(1)));
}

#[tokio::test]
async fn test_recovery_metrics() {
    let metrics = RecoveryMetrics::new();

    // Initially empty
    assert!((metrics.success_rate() - 0.0).abs() < f64::EPSILON);
    assert_eq!(metrics.average_recovery_time(), Duration::ZERO);

    // Record some operations
    metrics.record_attempt();
    metrics.record_success(Duration::from_millis(100));

    metrics.record_attempt();
    metrics.record_failure(Duration::from_millis(200));

    metrics.record_circuit_trip();
    metrics.record_fallback();
    metrics.record_cascade_prevention();

    // Check metrics
    assert!((metrics.success_rate() - 50.0).abs() < f64::EPSILON);
    assert_eq!(metrics.average_recovery_time(), Duration::from_millis(150));

    assert_eq!(metrics.circuit_breaker_trips(), 1);
    assert_eq!(metrics.fallback_executions(), 1);
    assert_eq!(metrics.cascade_preventions(), 1);
}

#[tokio::test]
async fn test_jitter_strategies() {
    let base_delay = Duration::from_millis(1000);

    // No jitter should return exact delay
    let no_jitter = JitterCalculator::calculate_delay(base_delay, JitterStrategy::None);
    assert_eq!(no_jitter, base_delay);

    // Full jitter should be between 0 and base_delay
    let full_jitter = JitterCalculator::calculate_delay(base_delay, JitterStrategy::Full);
    assert!(full_jitter <= base_delay);

    // Equal jitter should be between base_delay/2 and base_delay
    let equal_jitter = JitterCalculator::calculate_delay(base_delay, JitterStrategy::Equal);
    assert!(equal_jitter >= Duration::from_millis(500));
    assert!(equal_jitter <= base_delay);

    // Decorrelated jitter should be around base_delay
    let decorrelated_jitter =
        JitterCalculator::calculate_delay(base_delay, JitterStrategy::Decorrelated);
    assert!(decorrelated_jitter.as_millis() > 0);
}

#[tokio::test]
async fn test_recovery_coordinator() {
    let coordinator = RecoveryCoordinator::new();

    // Add dependencies
    coordinator
        .add_dependency("service_a".to_string(), vec!["database".to_string()])
        .await;
    coordinator
        .add_dependency(
            "service_b".to_string(),
            vec!["service_a".to_string(), "cache".to_string()],
        )
        .await;

    // Test cascade detection
    let affected = coordinator.detect_cascade("database").await;
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(&"service_a".to_string()));

    let affected = coordinator.detect_cascade("service_a").await;
    assert_eq!(affected.len(), 1);
    assert!(affected.contains(&"service_b".to_string()));

    // Test metrics
    let metrics = coordinator.get_metrics();
    assert_eq!(metrics.cascade_preventions(), 2);
}

#[tokio::test]
async fn test_advanced_recovery_executor() {
    let policy = RecoveryPolicy::default();
    let executor = AdvancedRecoveryExecutor::new(policy);

    let attempt_counter = Arc::new(AtomicU32::new(0));
    let counter_clone = attempt_counter.clone();

    // Test adaptive retry
    let result = executor
        .retry_adaptive(
            Error::Internal("test error".to_string()),
            3,
            Duration::from_millis(10),
            Duration::from_millis(100),
            2.0,
            JitterStrategy::None,
            || {
                let counter = counter_clone.clone();
                async move {
                    let count = counter.fetch_add(1, Ordering::Relaxed);
                    if count < 2 {
                        Err(Error::Internal("retry".to_string()))
                    } else {
                        Ok(serde_json::json!({"success": true}))
                    }
                }
            },
        )
        .await;

    assert!(result.is_ok());
    assert_eq!(attempt_counter.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn test_deadline_aware_recovery() {
    let policy = RecoveryPolicy::default();
    let executor = AdvancedRecoveryExecutor::new(policy);

    let mut deadline = RecoveryDeadline::new(Duration::from_millis(200));

    // Test successful operation within deadline
    let result = executor
        .execute_with_deadline(
            "test_op",
            || async { Ok(serde_json::json!({"result": "success"})) },
            &mut deadline,
            &RecoveryStrategy::RetryFixed {
                attempts: 2,
                delay: Duration::from_millis(10),
            },
        )
        .await;

    assert!(result.is_ok());

    // Test operation that exceeds deadline
    let mut short_deadline = RecoveryDeadline::new(Duration::from_millis(50));
    let result = executor
        .execute_with_deadline(
            "slow_op",
            || async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(serde_json::json!({"result": "slow"}))
            },
            &mut short_deadline,
            &RecoveryStrategy::RetryFixed {
                attempts: 1,
                delay: Duration::from_millis(10),
            },
        )
        .await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Timeout(_)));
}

#[tokio::test]
async fn test_bulk_recovery_handler() {
    let coordinator = Arc::new(RecoveryCoordinator::new());
    let bulk_handler =
        pmcp::error::recovery::BulkRecoveryHandler::new(coordinator, Duration::from_secs(1));

    // Test all successful operations
    let operations: Vec<_> = (0..3)
        .map(|i| move || async move { Ok(format!("result_{}", i)) })
        .collect();

    let result = bulk_handler.execute_bulk(operations, 0.8, false).await;
    match result {
        BulkRecoveryResult::AllSuccess(results) => {
            assert_eq!(results.len(), 3);
        },
        _ => panic!("Expected all success"),
    }

    // Test partial success
    let operations: Vec<_> = (0..5)
        .map(|i| {
            move || async move {
                if i < 3 {
                    Ok(format!("result_{}", i))
                } else {
                    Err(Error::Internal(format!("error_{}", i)))
                }
            }
        })
        .collect();

    let result = bulk_handler.execute_bulk(operations, 0.5, false).await;
    match result {
        BulkRecoveryResult::PartialSuccess {
            successes,
            failures,
        } => {
            assert_eq!(successes.len(), 3);
            assert_eq!(failures.len(), 2);
        },
        _ => panic!("Expected partial success"),
    }

    // Test fail fast
    let operations: Vec<_> = (0..5)
        .map(|i| {
            move || async move {
                if i == 0 {
                    Err(Error::Internal("first failure".to_string()))
                } else {
                    Ok(format!("result_{}", i))
                }
            }
        })
        .collect();

    let result = bulk_handler.execute_bulk(operations, 0.8, true).await;
    match result {
        BulkRecoveryResult::AllFailed(failures) => {
            assert_eq!(failures.len(), 1); // Should stop at first failure
        },
        _ => panic!("Expected all failed with fail fast"),
    }
}

#[tokio::test]
async fn test_health_check_result() {
    let health_result = HealthCheckResult {
        component: "test_service".to_string(),
        status: HealthStatus::Healthy,
        response_time_us: 1500,
        timestamp: SystemTime::now(),
        message: Some("All systems operational".to_string()),
    };

    assert_eq!(health_result.component, "test_service");
    assert_eq!(health_result.status, HealthStatus::Healthy);
    assert_eq!(health_result.response_time_us, 1500);
    assert_eq!(
        health_result.message,
        Some("All systems operational".to_string())
    );
}

#[tokio::test]
async fn test_recovery_strategies_enum() {
    // Test that recovery strategies can be created and matched
    let strategy = RecoveryStrategy::RetryAdaptive {
        attempts: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        multiplier: 2.0,
        jitter: JitterStrategy::Equal,
    };

    match strategy {
        RecoveryStrategy::RetryAdaptive { attempts, .. } => {
            assert_eq!(attempts, 3);
        },
        _ => panic!("Wrong strategy type"),
    }

    // Test nested strategies
    let deadline_strategy = RecoveryStrategy::DeadlineAware {
        max_recovery_time: Duration::from_secs(30),
        base_strategy: Box::new(RecoveryStrategy::RetryExponential {
            attempts: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }),
    };

    match deadline_strategy {
        RecoveryStrategy::DeadlineAware {
            max_recovery_time,
            base_strategy,
        } => {
            assert_eq!(max_recovery_time, Duration::from_secs(30));
            match *base_strategy {
                RecoveryStrategy::RetryExponential { attempts, .. } => {
                    assert_eq!(attempts, 5);
                },
                _ => panic!("Wrong base strategy type"),
            }
        },
        _ => panic!("Wrong strategy type"),
    }
}

#[tokio::test]
async fn test_recovery_events() {
    #[allow(clippy::no_effect_underscore_binding)]
    let mut _event_received = false;
    let coordinator = RecoveryCoordinator::new();

    // Add event handler
    coordinator
        .add_event_handler(Arc::new(move |_event| {
            // Can't modify local variable in closure, but we can test event emission
        }))
        .await;

    // Test event emission
    let event = RecoveryEvent::RecoveryStarted {
        operation_id: "test_op".to_string(),
        strategy: "retry_fixed".to_string(),
    };

    coordinator.emit_event(event).await;

    // Test cascade event
    let cascade_event = RecoveryEvent::CascadingFailure {
        trigger_component: "database".to_string(),
        affected_components: vec!["api_service".to_string(), "user_service".to_string()],
    };

    coordinator.emit_event(cascade_event).await;

    // Test health change event
    let health_event = RecoveryEvent::HealthChanged {
        component: "cache".to_string(),
        old_status: HealthStatus::Healthy,
        new_status: HealthStatus::Degraded,
    };

    coordinator.emit_event(health_event).await;
}

#[tokio::test]
async fn test_error_code_mapping() {
    let timeout_error = Error::Timeout(1000);
    assert_eq!(timeout_error.error_code(), Some(ErrorCode::REQUEST_TIMEOUT));

    let auth_error = Error::Authentication("invalid token".to_string());
    assert_eq!(
        auth_error.error_code(),
        Some(ErrorCode::AUTHENTICATION_REQUIRED)
    );

    let rate_limit_error = Error::RateLimited;
    assert_eq!(rate_limit_error.error_code(), Some(ErrorCode::RATE_LIMITED));

    let circuit_breaker_error = Error::CircuitBreakerOpen;
    assert_eq!(
        circuit_breaker_error.error_code(),
        Some(ErrorCode::CIRCUIT_BREAKER_OPEN)
    );

    let protocol_error = Error::protocol(ErrorCode::INVALID_REQUEST, "bad request");
    assert_eq!(
        protocol_error.error_code(),
        Some(ErrorCode::INVALID_REQUEST)
    );
}
