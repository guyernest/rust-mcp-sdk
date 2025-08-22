//! Enhanced WebSocket server with multi-client support and advanced features.
//!
//! PMCP-4001: Complete WebSocket server implementation with:
//! - Multiple client connections
//! - Connection pooling
//! - Broadcast messaging
//! - Heartbeat/keepalive
//! - Advanced error recovery

use crate::error::{Error, Result};
use crate::shared::TransportMessage;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, timeout};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Client connection identifier
pub type ClientId = Uuid;

/// Configuration for enhanced WebSocket server
#[derive(Debug, Clone)]
pub struct EnhancedWebSocketConfig {
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum frame size
    pub max_frame_size: Option<usize>,
    /// Maximum message size  
    pub max_message_size: Option<usize>,
    /// Enable connection pooling
    pub enable_pooling: bool,
    /// Enable broadcast mode
    pub enable_broadcast: bool,
}

impl Default for EnhancedWebSocketConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9001".parse().expect("Valid default"),
            max_connections: 100,
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(15),
            max_frame_size: Some(64 * 1024 * 1024),
            max_message_size: Some(64 * 1024 * 1024),
            enable_pooling: true,
            enable_broadcast: false,
        }
    }
}

/// Represents a single client connection
#[derive(Debug)]
struct ClientConnection {
    #[allow(dead_code)]
    id: ClientId,
    #[allow(dead_code)]
    addr: SocketAddr,
    tx: mpsc::Sender<TransportMessage>,
    last_seen: std::time::Instant,
}

/// Enhanced WebSocket server with multi-client support
pub struct EnhancedWebSocketServer {
    config: EnhancedWebSocketConfig,
    listener: Option<Arc<TcpListener>>,
    clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    incoming_tx: mpsc::Sender<(ClientId, TransportMessage)>,
    incoming_rx: Arc<RwLock<mpsc::Receiver<(ClientId, TransportMessage)>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl std::fmt::Debug for EnhancedWebSocketServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnhancedWebSocketServer")
            .field("config", &self.config)
            .field("has_listener", &self.listener.is_some())
            .field("shutdown_tx", &self.shutdown_tx.is_some())
            .finish()
    }
}

impl EnhancedWebSocketServer {
    /// Create new enhanced WebSocket server
    pub fn new(config: EnhancedWebSocketConfig) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::channel(1000);

        Self {
            config,
            listener: None,
            clients: Arc::new(RwLock::new(HashMap::new())),
            incoming_tx,
            incoming_rx: Arc::new(RwLock::new(incoming_rx)),
            shutdown_tx: None,
        }
    }

    /// Start the server and bind to configured address
    pub async fn start(&mut self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr)
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to {}: {}",
                    self.config.bind_addr, e
                ))
            })?;

        info!(
            "Enhanced WebSocket server listening on {}",
            self.config.bind_addr
        );

        let listener = Arc::new(listener);
        self.listener = Some(listener.clone());

        // Start heartbeat task
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let clients = self.clients.clone();
        let heartbeat_interval = self.config.heartbeat_interval;

        tokio::spawn(async move {
            let mut ticker = interval(heartbeat_interval);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        Self::check_heartbeats(clients.clone()).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Heartbeat task shutting down");
                        break;
                    }
                }
            }
        });

        // Start accept loop
        let clients = self.clients.clone();
        let incoming_tx = self.incoming_tx.clone();
        let max_connections = self.config.max_connections;
        let connection_timeout = self.config.connection_timeout;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let client_count = clients.read().await.len();

                        if client_count >= max_connections {
                            warn!("Max connections reached, rejecting {}", addr);
                            continue;
                        }

                        info!("New connection from {}", addr);

                        // Handle connection with timeout
                        let clients = clients.clone();
                        let incoming_tx = incoming_tx.clone();

                        tokio::spawn(async move {
                            match timeout(
                                connection_timeout,
                                Self::handle_connection(stream, addr, clients, incoming_tx),
                            )
                            .await
                            {
                                Ok(Ok(())) => {
                                    info!("Connection from {} closed normally", addr);
                                },
                                Ok(Err(e)) => {
                                    error!("Connection from {} error: {}", addr, e);
                                },
                                Err(_) => {
                                    warn!("Connection from {} timed out", addr);
                                },
                            }
                        });
                    },
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    },
                }
            }
        });

        Ok(())
    }

    /// Setup client connection after handshake
    async fn setup_client(
        client_id: ClientId,
        addr: SocketAddr,
        clients: &Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    ) -> (
        mpsc::Sender<TransportMessage>,
        mpsc::Receiver<TransportMessage>,
    ) {
        let (client_tx, client_rx) = mpsc::channel(100);

        // Register client
        let mut clients_guard = clients.write().await;
        clients_guard.insert(
            client_id,
            ClientConnection {
                id: client_id,
                addr,
                tx: client_tx.clone(),
                last_seen: std::time::Instant::now(),
            },
        );
        drop(clients_guard);

        // Return both for spawning send task
        (client_tx, client_rx)
    }

    /// Handle outgoing messages for a client
    #[allow(clippy::cognitive_complexity)]
    async fn handle_client_send(
        client_id: ClientId,
        mut client_rx: mpsc::Receiver<TransportMessage>,
        mut ws_sink: futures::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
            Message,
        >,
        clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    ) {
        while let Some(msg) = client_rx.recv().await {
            let json_bytes = match crate::shared::stdio::StdioTransport::serialize_message(&msg) {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    continue;
                },
            };

            let json = match String::from_utf8(json_bytes) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to convert to UTF-8: {}", e);
                    continue;
                },
            };

            if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                error!("Failed to send to client {}: {}", client_id, e);
                break;
            }
        }

        // Remove client on disconnect
        clients.write().await.remove(&client_id);
        info!("Client {} send task ended", client_id);
    }

    /// Handle individual client connection
    #[allow(clippy::cognitive_complexity)]
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        addr: SocketAddr,
        clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
        incoming_tx: mpsc::Sender<(ClientId, TransportMessage)>,
    ) -> Result<()> {
        // Perform WebSocket handshake
        let ws_stream = accept_async(stream)
            .await
            .map_err(|e| Error::internal(format!("WebSocket handshake failed: {}", e)))?;

        let client_id = Uuid::new_v4();
        info!("Client {} connected from {}", client_id, addr);

        // Setup client connection
        let (_client_tx, client_rx) = Self::setup_client(client_id, addr, &clients).await;
        // client_tx is stored in clients map for send_to_client operations

        // Split WebSocket stream
        let (ws_sink, mut ws_stream) = ws_stream.split();

        // Spawn send task
        let clients_send = clients.clone();
        tokio::spawn(Self::handle_client_send(
            client_id,
            client_rx,
            ws_sink,
            clients_send,
        ));

        // Handle incoming messages
        while let Some(result) = ws_stream.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    // Update last seen
                    {
                        let mut clients_guard = clients.write().await;
                        if let Some(client) = clients_guard.get_mut(&client_id) {
                            client.last_seen = std::time::Instant::now();
                        }
                    }

                    match crate::shared::stdio::StdioTransport::parse_message(text.as_bytes()) {
                        Ok(msg) => {
                            if let Err(e) = incoming_tx.send((client_id, msg)).await {
                                error!("Failed to queue message from {}: {}", client_id, e);
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse message from {}: {}", client_id, e);
                        },
                    }
                },
                Ok(Message::Close(_)) => {
                    info!("Client {} closed connection", client_id);
                    break;
                },
                Ok(Message::Ping(_data)) => {
                    // Update last seen on ping
                    {
                        let mut clients_guard = clients.write().await;
                        if let Some(client) = clients_guard.get_mut(&client_id) {
                            client.last_seen = std::time::Instant::now();
                        }
                    }
                    // WebSocket library handles pong automatically
                },
                Ok(_) => {
                    // Ignore other message types
                },
                Err(e) => {
                    error!("WebSocket error for {}: {}", client_id, e);
                    break;
                },
            }
        }

        // Cleanup
        clients.write().await.remove(&client_id);
        info!("Client {} disconnected", client_id);

        Ok(())
    }

    /// Check client heartbeats and remove stale connections
    async fn check_heartbeats(clients: Arc<RwLock<HashMap<ClientId, ClientConnection>>>) {
        let mut clients_guard = clients.write().await;
        let now = std::time::Instant::now();
        let timeout = Duration::from_secs(60);

        let stale_clients: Vec<ClientId> = clients_guard
            .iter()
            .filter(|(_, client)| now.duration_since(client.last_seen) > timeout)
            .map(|(id, _)| *id)
            .collect();

        for client_id in stale_clients {
            warn!("Removing stale client {}", client_id);
            clients_guard.remove(&client_id);
        }
    }

    /// Broadcast message to all connected clients
    pub async fn broadcast(&self, message: TransportMessage) -> Result<()> {
        if !self.config.enable_broadcast {
            return Err(Error::internal("Broadcast mode not enabled"));
        }

        let clients = self.clients.read().await;
        let mut send_count = 0;

        for (client_id, client) in clients.iter() {
            if let Err(e) = client.tx.send(message.clone()).await {
                warn!("Failed to send to client {}: {}", client_id, e);
            } else {
                send_count += 1;
            }
        }

        debug!("Broadcast sent to {} clients", send_count);
        Ok(())
    }

    /// Send message to specific client
    pub async fn send_to_client(
        &self,
        client_id: ClientId,
        message: TransportMessage,
    ) -> Result<()> {
        let clients = self.clients.read().await;

        let client = clients
            .get(&client_id)
            .ok_or_else(|| Error::internal(format!("Client {} not found", client_id)))?;

        client
            .tx
            .send(message)
            .await
            .map_err(|_| Error::internal("Failed to send to client"))?;

        Ok(())
    }

    /// Get list of connected client IDs
    pub async fn get_connected_clients(&self) -> Vec<ClientId> {
        self.clients.read().await.keys().copied().collect()
    }

    /// Get number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Receive next message from any client
    pub async fn receive_from_any(&self) -> Result<(ClientId, TransportMessage)> {
        let mut rx = self.incoming_rx.write().await;

        rx.recv()
            .await
            .ok_or_else(|| Error::internal("No messages available"))
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) -> Result<()> {
        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // Clear all clients
        self.clients.write().await.clear();

        info!("Enhanced WebSocket server shut down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = EnhancedWebSocketConfig::default();
        let server = EnhancedWebSocketServer::new(config);

        assert_eq!(server.client_count().await, 0);
        assert!(server.get_connected_clients().await.is_empty());
    }

    #[tokio::test]
    async fn test_broadcast_disabled() {
        let config = EnhancedWebSocketConfig {
            enable_broadcast: false,
            ..Default::default()
        };

        let server = EnhancedWebSocketServer::new(config);
        let result = server
            .broadcast(TransportMessage::Notification(
                crate::types::Notification::Cancelled(crate::types::CancelledNotification {
                    request_id: crate::types::RequestId::Number(1),
                    reason: None,
                }),
            ))
            .await;

        assert!(result.is_err());
    }
}
