//! Connection Pool and Load Balancing Example
//!
//! PMCP-4003: Demonstrates connection pooling with load balancing strategies
//!
//! Run with: cargo run --example 29_connection_pool --features full

use async_trait::async_trait;
use pmcp::error::Result;
use pmcp::shared::{ConnectionPool, ConnectionPoolConfig, LoadBalanceStrategy, TransportMessage};
use std::time::Duration;
use tracing::{info, Level};

/// Mock transport for demonstration purposes
#[derive(Debug, Clone)]
struct MockTransport {
    id: u32,
}

impl MockTransport {
    fn new(id: u32) -> Self {
        Self { id }
    }
}

#[async_trait]
impl pmcp::shared::Transport for MockTransport {
    async fn send(&mut self, _message: TransportMessage) -> Result<()> {
        // Mock sending - just log
        info!("MockTransport {} sending message", self.id);
        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        // Mock receiving - return a progress notification
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(TransportMessage::Notification(
            pmcp::types::Notification::Progress(pmcp::types::ProgressNotification {
                progress_token: pmcp::types::ProgressToken::String(format!("mock-{}", self.id)),
                progress: 50.0,
                message: Some(format!("Mock message from transport {}", self.id)),
            }),
        ))
    }

    async fn close(&mut self) -> Result<()> {
        info!("MockTransport {} closed", self.id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn transport_type(&self) -> &'static str {
        "mock"
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting Connection Pool and Load Balancing Example");

    // Configure connection pool
    let config = ConnectionPoolConfig {
        max_connections: 5,
        min_connections: 2,
        strategy: LoadBalanceStrategy::RoundRobin,
        health_check_interval: Duration::from_secs(10),
        operation_timeout: Duration::from_secs(5),
        max_idle_time: Duration::from_secs(60),
        auto_scaling: true,
        max_retries: 3,
        retry_delay: Duration::from_secs(1),
    };

    info!("✅ Configuration:");
    info!("  • Max connections: {}", config.max_connections);
    info!("  • Min connections: {}", config.min_connections);
    info!("  • Strategy: {:?}", config.strategy);
    info!(
        "  • Health check interval: {:?}",
        config.health_check_interval
    );
    info!("  • Auto scaling: {}", config.auto_scaling);

    // Create connection pool
    let mut pool: ConnectionPool<MockTransport> = ConnectionPool::new(config);

    // Start pool with connection factory
    info!("🔧 Starting connection pool...");

    pool.start(|| {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        info!("Creating new MockTransport connection #{}", id + 1);
        Ok(MockTransport::new(id + 1))
    })
    .await?;

    info!("✓ Connection pool started");

    // Get initial statistics
    let stats = pool.get_stats().await;
    info!("📊 Initial pool stats:");
    info!("  • Total connections: {}", stats.total_connections);
    info!("  • Healthy connections: {}", stats.healthy_connections);
    info!("  • Strategy: {:?}", stats.strategy);

    // Demonstrate different load balancing strategies
    info!("🎯 Testing load balancing strategies...");

    let strategies = vec![
        LoadBalanceStrategy::RoundRobin,
        LoadBalanceStrategy::LeastConnections,
        LoadBalanceStrategy::WeightedRoundRobin,
        LoadBalanceStrategy::Random,
    ];

    for strategy in strategies {
        info!("Testing strategy: {:?}", strategy);

        // Create pool with this strategy
        let test_config = ConnectionPoolConfig {
            strategy,
            max_connections: 3,
            min_connections: 2,
            ..Default::default()
        };

        let mut test_pool: ConnectionPool<MockTransport> = ConnectionPool::new(test_config);
        test_pool
            .start(|| {
                static TEST_COUNTER: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(100);
                let id = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(MockTransport::new(id))
            })
            .await?;

        // Send several test messages
        for i in 0..5 {
            let message = TransportMessage::Notification(pmcp::types::Notification::Progress(
                pmcp::types::ProgressNotification {
                    progress_token: pmcp::types::ProgressToken::String(format!("test-{}", i)),
                    progress: (i as f64 * 20.0),
                    message: Some(format!("Load balancing test {}", i)),
                },
            ));

            if let Ok(connection_id) = test_pool.get_connection().await {
                info!(
                    "  Strategy {:?} selected connection: {}",
                    strategy, connection_id
                );
                let _ = test_pool.send_to_connection(connection_id, message).await;
            }
        }

        test_pool.shutdown().await?;
    }

    // Demonstrate health monitoring
    info!("🏥 Demonstrating health monitoring...");

    let health_stats = pool.get_stats().await;
    info!("Health status distribution:");
    info!("  • Healthy: {}", health_stats.healthy_connections);
    info!("  • Degraded: {}", health_stats.degraded_connections);
    info!("  • Unhealthy: {}", health_stats.unhealthy_connections);

    // Simulate some load
    info!("📈 Simulating load across connections...");

    for i in 0..10 {
        let message = TransportMessage::Notification(pmcp::types::Notification::Progress(
            pmcp::types::ProgressNotification {
                progress_token: pmcp::types::ProgressToken::String(format!("load-test-{}", i)),
                progress: (i as f64 * 10.0),
                message: Some(format!("Load test message {}", i)),
            },
        ));

        if let Err(e) = pool.send_message(message).await {
            info!("Failed to send message {}: {}", i, e);
        } else {
            info!("✓ Message {} sent through pool", i);
        }

        // Small delay to simulate realistic load
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Get final statistics
    let final_stats = pool.get_stats().await;
    info!("📊 Final pool statistics:");
    info!("  • Total connections: {}", final_stats.total_connections);
    info!(
        "  • Healthy connections: {}",
        final_stats.healthy_connections
    );
    info!(
        "  • Total requests processed: {}",
        final_stats.total_requests
    );
    info!(
        "  • Currently active requests: {}",
        final_stats.active_requests
    );

    // Demonstrate pool benefits
    info!("🔄 Connection pool benefits:");
    info!("  • Automatic load distribution across connections");
    info!("  • Health monitoring and automatic failover");
    info!("  • Configurable load balancing strategies");
    info!("  • Connection lifecycle management");
    info!("  • Request/response correlation");
    info!("  • Performance monitoring and statistics");

    // Shutdown pool
    pool.shutdown().await?;
    info!("👋 Connection pool shut down gracefully");

    Ok(())
}
