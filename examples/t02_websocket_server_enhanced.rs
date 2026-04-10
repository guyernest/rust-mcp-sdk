//! Enhanced WebSocket Server Example
//!
//! PMCP-4001: Demonstrates multi-client WebSocket server with advanced features
//!
//! Run with: cargo run --example 27_websocket_server_enhanced --features websocket

use pmcp::server::transport::{EnhancedWebSocketConfig, EnhancedWebSocketServer};
use std::time::Duration;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting Enhanced WebSocket Server Example");

    // Configure the enhanced server
    let config = EnhancedWebSocketConfig {
        bind_addr: "127.0.0.1:9001".parse()?,
        max_connections: 100,
        connection_timeout: Duration::from_secs(60),
        heartbeat_interval: Duration::from_secs(15),
        max_frame_size: Some(10 * 1024 * 1024),   // 10MB
        max_message_size: Some(10 * 1024 * 1024), // 10MB
        enable_pooling: true,
        enable_broadcast: true, // Enable broadcast mode
    };

    // Create and start the server
    let mut server = EnhancedWebSocketServer::new(config);
    server.start().await?;

    info!("✅ Server started on ws://127.0.0.1:9001");
    info!("Features enabled:");
    info!("  • Multi-client support (max 100 connections)");
    info!("  • Connection pooling");
    info!("  • Broadcast messaging");
    info!("  • Heartbeat monitoring (15s interval)");
    info!("  • Automatic stale connection cleanup");

    // Spawn a task to periodically show server stats
    let server_clone = std::sync::Arc::new(tokio::sync::RwLock::new(server));
    let stats_server = server_clone.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            let server = stats_server.read().await;
            let client_count = server.client_count().await;
            info!("📊 Connected clients: {}", client_count);

            if client_count > 0 {
                let clients = server.get_connected_clients().await;
                info!("   Client IDs: {:?}", clients);
            }
        }
    });

    // Main server loop - handle incoming messages
    let server = server_clone.clone();

    loop {
        // Wait for messages from any client
        let server_read = server.read().await;

        match tokio::time::timeout(Duration::from_secs(1), server_read.receive_from_any()).await {
            Ok(Ok((client_id, message))) => {
                info!("📨 Received from client {}: {:?}", client_id, message);

                // Echo the message back to the sender
                if let Err(e) = server_read.send_to_client(client_id, message.clone()).await {
                    info!("Failed to echo to client: {}", e);
                }

                // If broadcast is enabled, send to all clients
                if server_read.client_count().await > 1 {
                    info!("📢 Broadcasting to all clients");
                    if let Err(e) = server_read.broadcast(message).await {
                        info!("Broadcast failed: {}", e);
                    }
                }
            },
            Ok(Err(e)) => {
                // No messages available is expected
                if !e.to_string().contains("No messages available") {
                    info!("Receive error: {}", e);
                }
            },
            Err(_) => {
                // Timeout - normal, continue
            },
        }

        // Check for shutdown signal (Ctrl+C)
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("🛑 Shutdown signal received");
            break;
        }
    }

    // Shutdown the server
    let mut server_write = server_clone.write().await;
    server_write.shutdown().await?;

    info!("👋 Server shut down gracefully");
    Ok(())
}
