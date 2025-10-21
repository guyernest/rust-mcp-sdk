# Ch23 Custom Transports

## Introduction

MCP is **transport-agnostic**: it can run over any bidirectional communication channel that supports JSON-RPC 2.0 message exchange. While the PMCP SDK provides built-in transports (stdio, HTTP, WebSocket, SSE), you can build custom transports for specialized use cases.

### When to Build Custom Transports

Consider custom transports when:

**✅ Good Reasons:**
- **Async messaging systems**: SQS, SNS, Kafka, RabbitMQ for decoupled architectures
- **Custom protocols**: Domain-specific protocols in your infrastructure
- **Performance optimization**: Specialized binary protocols, compression, batching
- **Testing**: In-memory or mock transports for unit/integration tests
- **Legacy integration**: Wrapping existing communication channels
- **Security requirements**: Custom encryption, authentication flows

**❌ Avoid Custom Transports For:**
- Standard HTTP/HTTPS servers → Use built-in `HttpTransport`
- Local processes → Use built-in `StdioTransport`
- Real-time WebSocket → Use built-in `WebSocketTransport`

### Client vs Server Reality

> **Critical Insight**: Like web browsers vs websites, there will be **far fewer MCP clients** than MCP servers.
>
> - **Few clients**: Claude Desktop, IDEs, agent frameworks (like web browsers)
> - **Many servers**: Every tool, API, database, service (like websites)
>
> **Implication**: Custom transports require **both sides** to implement the same protocol. Unless you control both client and server (e.g., internal infrastructure), stick to standard transports for maximum compatibility.

## The Transport Trait

All transports in PMCP implement the `Transport` trait:

```rust
use pmcp::shared::{Transport, TransportMessage};
use async_trait::async_trait;

#[async_trait]
pub trait Transport: Send + Sync + Debug {
    /// Send a message (request, response, or notification)
    async fn send(&mut self, message: TransportMessage) -> Result<()>;

    /// Receive a message (blocks until message arrives)
    async fn receive(&mut self) -> Result<TransportMessage>;

    /// Close the transport gracefully
    async fn close(&mut self) -> Result<()>;

    /// Check if transport is still connected (optional)
    fn is_connected(&self) -> bool {
        true
    }

    /// Transport type name for debugging (optional)
    fn transport_type(&self) -> &'static str {
        "unknown"
    }
}
```

### Message Types

`TransportMessage` represents all MCP communication:

```rust
pub enum TransportMessage {
    /// Request with ID (expects response)
    Request {
        id: RequestId,
        request: Request,  // ClientRequest or ServerRequest
    },

    /// Response to a request
    Response(JSONRPCResponse),

    /// Notification (no response expected)
    Notification(Notification),
}
```

**Key Design Points:**
- **Framing**: Your transport handles message boundaries
- **Serialization**: PMCP handles JSON-RPC serialization
- **Bidirectional**: Both send and receive must work concurrently
- **Error handling**: Use `pmcp::error::TransportError` for transport-specific errors

## Example 1: In-Memory Transport (Testing)

**Use case**: Unit tests, benchmarks, integration tests without network

```rust
use pmcp::shared::{Transport, TransportMessage};
use pmcp::error::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// In-memory transport using channels
#[derive(Debug)]
pub struct InMemoryTransport {
    /// Channel for sending messages
    tx: mpsc::Sender<TransportMessage>,
    /// Channel for receiving messages
    rx: Arc<Mutex<mpsc::Receiver<TransportMessage>>>,
    /// Connected state
    connected: Arc<std::sync::atomic::AtomicBool>,
}

impl InMemoryTransport {
    /// Create a pair of connected transports (client <-> server)
    pub fn create_pair() -> (Self, Self) {
        let (tx1, rx1) = mpsc::channel(100);
        let (tx2, rx2) = mpsc::channel(100);

        let transport1 = Self {
            tx: tx1,
            rx: Arc::new(Mutex::new(rx2)),
            connected: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };

        let transport2 = Self {
            tx: tx2,
            rx: Arc::new(Mutex::new(rx1)),
            connected: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };

        (transport1, transport2)
    }
}

#[async_trait]
impl Transport for InMemoryTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        use std::sync::atomic::Ordering;

        if !self.connected.load(Ordering::Relaxed) {
            return Err(pmcp::error::Error::Transport(
                pmcp::error::TransportError::ConnectionClosed
            ));
        }

        self.tx
            .send(message)
            .await
            .map_err(|_| pmcp::error::Error::Transport(
                pmcp::error::TransportError::ConnectionClosed
            ))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| pmcp::error::Error::Transport(
                pmcp::error::TransportError::ConnectionClosed
            ))
    }

    async fn close(&mut self) -> Result<()> {
        use std::sync::atomic::Ordering;
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        use std::sync::atomic::Ordering;
        self.connected.load(Ordering::Relaxed)
    }

    fn transport_type(&self) -> &'static str {
        "in-memory"
    }
}

// Usage in tests
#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::{Client, Server, ClientCapabilities};

    #[tokio::test]
    async fn test_in_memory_transport() {
        // Create connected pair
        let (client_transport, server_transport) = InMemoryTransport::create_pair();

        // Create client and server
        let mut client = Client::new(client_transport);
        let server = Server::builder()
            .name("test-server")
            .version("1.0.0")
            .build()
            .unwrap();

        // Run server in background
        tokio::spawn(async move {
            server.run(server_transport).await
        });

        // Client can now communicate with server
        let result = client.initialize(ClientCapabilities::minimal()).await;
        assert!(result.is_ok());
    }
}
```

**Benefits:**
- ✅ Zero network overhead
- ✅ Deterministic testing
- ✅ Fast benchmarks
- ✅ Isolated test environments

## Example 2: Async Queue Transport (Production)

**Use case**: Decoupled architectures with message queues (SQS, Kafka, RabbitMQ)

This example shows a conceptual async transport using AWS SQS:

```rust
use pmcp::shared::{Transport, TransportMessage};
use pmcp::error::Result;
use async_trait::async_trait;
use aws_sdk_sqs::Client as SqsClient;
use aws_config;  // For load_from_env
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SQS-based async transport
#[derive(Debug)]
pub struct SqsTransport {
    /// SQS client
    sqs: SqsClient,
    /// Request queue URL (for sending)
    request_queue_url: String,
    /// Response queue URL (for receiving)
    response_queue_url: String,
    /// Local message buffer
    message_rx: Arc<Mutex<mpsc::Receiver<TransportMessage>>>,
    message_tx: mpsc::Sender<TransportMessage>,
    /// Background poller handle
    poller_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SqsTransport {
    pub async fn new(
        request_queue_url: String,
        response_queue_url: String,
    ) -> Result<Self> {
        let config = aws_config::load_from_env().await;
        let sqs = SqsClient::new(&config);

        let (tx, rx) = mpsc::channel(100);

        let mut transport = Self {
            sqs: sqs.clone(),
            request_queue_url,
            response_queue_url: response_queue_url.clone(),
            message_rx: Arc::new(Mutex::new(rx)),
            message_tx: tx,
            poller_handle: None,
        };

        // Start background poller for incoming messages
        transport.start_poller().await?;

        Ok(transport)
    }

    async fn start_poller(&mut self) -> Result<()> {
        let sqs = self.sqs.clone();
        let queue_url = self.response_queue_url.clone();
        let tx = self.message_tx.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Long-poll SQS for messages (20 seconds)
                match sqs
                    .receive_message()
                    .queue_url(&queue_url)
                    .max_number_of_messages(10)
                    .wait_time_seconds(20)
                    .send()
                    .await
                {
                    Ok(output) => {
                        if let Some(messages) = output.messages {
                            for msg in messages {
                                if let Some(body) = msg.body {
                                    // Parse JSON-RPC message
                                    if let Ok(transport_msg) =
                                        serde_json::from_str::<TransportMessage>(&body)
                                    {
                                        let _ = tx.send(transport_msg).await;
                                    }

                                    // Delete message from queue
                                    if let Some(receipt) = msg.receipt_handle {
                                        let _ = sqs
                                            .delete_message()
                                            .queue_url(&queue_url)
                                            .receipt_handle(receipt)
                                            .send()
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("SQS polling error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        self.poller_handle = Some(handle);
        Ok(())
    }
}

#[async_trait]
impl Transport for SqsTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // Serialize message to JSON
        let json = serde_json::to_string(&message)
            .map_err(|e| pmcp::error::Error::Transport(
                pmcp::error::TransportError::InvalidMessage(e.to_string())
            ))?;

        // Send to SQS request queue
        self.sqs
            .send_message()
            .queue_url(&self.request_queue_url)
            .message_body(json)
            .send()
            .await
            .map_err(|e| pmcp::error::Error::Transport(
                pmcp::error::TransportError::InvalidMessage(e.to_string())
            ))?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| pmcp::error::Error::Transport(
                pmcp::error::TransportError::ConnectionClosed
            ))
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(handle) = self.poller_handle.take() {
            handle.abort();
        }
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "sqs-async"
    }
}
```

**Architecture Benefits:**
- ✅ **Decoupled**: Client/server don't need to be online simultaneously
- ✅ **Scalable**: Multiple servers can consume from same queue
- ✅ **Reliable**: Message persistence and retry mechanisms
- ✅ **Async workflows**: Long-running operations without blocking

**Trade-offs:**
- ❌ **Latency**: Higher than direct connections (100ms+)
- ❌ **Cost**: Message queue service fees
- ❌ **Complexity**: Requires queue infrastructure setup

## Example 3: WebSocket Transport (Built-in)

The SDK includes a production-ready WebSocket transport. Here's how it works internally:

```rust
// From src/shared/websocket.rs (simplified)
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures::{SinkExt, StreamExt};

pub struct WebSocketTransport {
    config: WebSocketConfig,
    state: Arc<RwLock<ConnectionState>>,
    message_tx: mpsc::Sender<TransportMessage>,
    message_rx: Arc<AsyncMutex<mpsc::Receiver<TransportMessage>>>,
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // Serialize to JSON
        let json = serde_json::to_vec(&message)?;

        // Send as WebSocket text frame
        let ws_msg = Message::Text(String::from_utf8(json)?);
        self.sink.send(ws_msg).await?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        // Wait for incoming WebSocket frame
        let mut rx = self.message_rx.lock().await;
        rx.recv().await.ok_or(TransportError::ConnectionClosed)
    }

    // ... reconnection logic, ping/pong handling, etc.
}
```

**Key WebSocket Features:**
- ✅ **Bidirectional**: True full-duplex communication
- ✅ **Low latency**: Direct TCP connection
- ✅ **Server push**: Notifications without polling
- ✅ **Auto-reconnect**: Built-in resilience

## Example 4: Kafka Transport with Topic Design

**Use case**: Event-driven, scalable, multi-client/server architectures

Kafka provides a robust foundation for MCP when you need decoupled, event-driven communication. The key challenge is **topic design** for request-response patterns in a pub/sub system.

### Topic Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Kafka Cluster                           │
│                                                             │
│  mcp.requests.*                  mcp.responses.*           │
│  ├─ mcp.requests.global          ├─ mcp.responses.client-A │
│  ├─ mcp.requests.tool-analysis   ├─ mcp.responses.client-B │
│  └─ mcp.requests.data-proc       └─ mcp.responses.client-C │
│                                                             │
│  mcp.server.discovery                                       │
│  └─ Heartbeats from servers with capabilities              │
└─────────────────────────────────────────────────────────────┘

     ▲                │                    ▲                │
     │ Produce        │ Consume            │ Consume        │ Produce
     │ requests       │ requests           │ responses      │ responses
     │                ▼                    │                ▼

┌─────────┐                          ┌──────────────┐
│ Client  │                          │ MCP Server   │
│ (Agent) │                          │ Pool         │
└─────────┘                          └──────────────┘
```

### Key Design Patterns

**1. Request Routing:**
- **Global topic**: `mcp.requests.global` - Any server can handle
- **Capability-based**: `mcp.requests.tool-analysis` - Only servers with specific tools
- **Dedicated**: `mcp.requests.server-id-123` - Target specific server instance

**2. Response Routing:**
- **Client-specific topics**: Each client subscribes to `mcp.responses.{client-id}`
- **Correlation IDs**: Message headers contain `request-id` + `client-id`
- **TTL**: Messages expire after configurable timeout

**3. Server Discovery:**
- Servers publish heartbeats to `mcp.server.discovery` with:
  - Server ID
  - Capabilities (tools, resources, prompts)
  - Load metrics
  - Status (online/busy/draining)

### Implementation

```rust
use pmcp::shared::{Transport, TransportMessage};
use pmcp::error::Result;
use async_trait::async_trait;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::message::Message;
use std::time::Duration;
use tokio::sync::mpsc;

/// Kafka-based MCP transport
#[derive(Debug)]
pub struct KafkaTransport {
    /// Kafka producer for sending
    producer: FutureProducer,
    /// Kafka consumer for receiving
    consumer: StreamConsumer,
    /// Client/Server ID for routing
    instance_id: String,
    /// Request topic name
    request_topic: String,
    /// Response topic name (client-specific)
    response_topic: String,
    /// Local message buffer
    message_rx: Arc<Mutex<mpsc::Receiver<TransportMessage>>>,
    message_tx: mpsc::Sender<TransportMessage>,
}

impl KafkaTransport {
    pub async fn new_client(
        brokers: &str,
        client_id: String,
    ) -> Result<Self> {
        use rdkafka::config::ClientConfig;

        // Create producer
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        // Create consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", &client_id)
            .set("bootstrap.servers", brokers)
            .set("enable.auto.commit", "true")
            .create()?;

        // Subscribe to client-specific response topic
        let response_topic = format!("mcp.responses.{}", client_id);
        consumer.subscribe(&[&response_topic])?;

        let (tx, rx) = mpsc::channel(100);

        let mut transport = Self {
            producer,
            consumer,
            instance_id: client_id,
            request_topic: "mcp.requests.global".to_string(),
            response_topic,
            message_rx: Arc::new(Mutex::new(rx)),
            message_tx: tx,
        };

        // Start background consumer
        transport.start_consumer().await?;

        Ok(transport)
    }

    async fn start_consumer(&mut self) -> Result<()> {
        let consumer = self.consumer.clone();
        let tx = self.message_tx.clone();

        tokio::spawn(async move {
            loop {
                match consumer.recv().await {
                    Ok(msg) => {
                        if let Some(payload) = msg.payload() {
                            // Parse JSON-RPC message
                            if let Ok(transport_msg) =
                                serde_json::from_slice::<TransportMessage>(payload)
                            {
                                let _ = tx.send(transport_msg).await;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Kafka consumer error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl Transport for KafkaTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // Serialize message
        let json = serde_json::to_vec(&message)?;

        // Extract request ID for correlation
        let correlation_id = match &message {
            TransportMessage::Request { id, .. } => format!("{:?}", id),
            _ => uuid::Uuid::new_v4().to_string(),
        };

        // Build Kafka record with headers
        let record = FutureRecord::to(&self.request_topic)
            .payload(&json)
            .key(&correlation_id)
            .headers(rdkafka::message::OwnedHeaders::new()
                .insert(rdkafka::message::Header {
                    key: "client-id",
                    value: Some(self.instance_id.as_bytes()),
                })
                .insert(rdkafka::message::Header {
                    key: "response-topic",
                    value: Some(self.response_topic.as_bytes()),
                }));

        // Send to Kafka
        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| pmcp::error::Error::Transport(
                pmcp::error::TransportError::InvalidMessage(e.to_string())
            ))?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut rx = self.message_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| pmcp::error::Error::Transport(
                pmcp::error::TransportError::ConnectionClosed
            ))
    }

    async fn close(&mut self) -> Result<()> {
        // Graceful shutdown
        Ok(())
    }

    fn transport_type(&self) -> &'static str {
        "kafka"
    }
}
```

### Server-Side Topic Patterns

```rust
impl KafkaTransport {
    /// Create server transport with capability-based subscription
    pub async fn new_server(
        brokers: &str,
        server_id: String,
        capabilities: Vec<String>,  // ["tool-analysis", "data-proc"]
    ) -> Result<Self> {
        use rdkafka::config::ClientConfig;

        // Create producer for server discovery
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        // Create consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", &format!("mcp-server-{}", server_id))
            .set("bootstrap.servers", brokers)
            .create()?;

        // Subscribe to relevant request topics
        let mut topics = vec!["mcp.requests.global".to_string()];
        for cap in &capabilities {
            topics.push(format!("mcp.requests.{}", cap));
        }

        consumer.subscribe(&topics.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;

        // Publish server discovery
        Self::publish_discovery(
            &producer,
            &server_id,
            &capabilities,
        ).await?;

        // ... rest of initialization (similar to new_client)
        let (tx, rx) = mpsc::channel(100);

        let mut transport = Self {
            producer,
            consumer,
            instance_id: server_id,
            request_topic: "mcp.requests.global".to_string(),
            response_topic: String::new(), // Server doesn't have response topic
            message_rx: Arc::new(Mutex::new(rx)),
            message_tx: tx,
        };

        transport.start_consumer().await?;
        Ok(transport)
    }

    async fn publish_discovery(
        producer: &FutureProducer,
        server_id: &str,
        capabilities: &[String],
    ) -> Result<()> {
        let discovery_msg = serde_json::json!({
            "server_id": server_id,
            "capabilities": capabilities,
            "status": "online",
            "timestamp": SystemTime::now(),
        });

        let record = FutureRecord::to("mcp.server.discovery")
            .payload(&serde_json::to_vec(&discovery_msg)?);

        producer.send(record, Duration::from_secs(1)).await?;
        Ok(())
    }

    /// Send response back to specific client
    async fn send_response(
        &self,
        response: TransportMessage,
        client_id: &str,
        correlation_id: &str,
    ) -> Result<()> {
        let json = serde_json::to_vec(&response)?;

        let response_topic = format!("mcp.responses.{}", client_id);

        let record = FutureRecord::to(&response_topic)
            .payload(&json)
            .key(correlation_id);

        self.producer
            .send(record, Duration::from_secs(5))
            .await?;

        Ok(())
    }
}
```

### Kafka Benefits for MCP

**✅ Decoupled Communication:**
- Clients/servers operate independently
- No direct connections required
- Timeouts managed at application layer

**✅ Event-Driven Architecture:**
- React to MCP requests as events
- Multiple consumers can process same request stream
- Event sourcing: Full message history

**✅ Scalability:**
- Horizontal scaling: Add more consumer groups
- Partitioning: Distribute load across brokers
- Retention: Replay historical requests

**✅ Multi-Tenant Support:**
- Topic isolation per client/tenant
- ACLs for security boundaries
- Quota management per client

**❌ Trade-offs:**
- Higher latency (50-200ms vs 1-5ms for WebSocket)
- Complex topic design required
- Kafka infrastructure overhead
- Request-response correlation complexity

> **Note on GraphQL**: GraphQL APIs belong at the MCP **server layer** (as tools exposing queries/mutations), not as a custom transport. Use standard transports (HTTP, WebSocket) and build MCP servers that wrap GraphQL backends.

## Best Practices

### 1. Preserve JSON-RPC Message Format

Your transport must not modify MCP messages:

```rust
// ✅ Correct: Transport as dumb pipe
async fn send(&mut self, message: TransportMessage) -> Result<()> {
    let bytes = serde_json::to_vec(&message)?;
    self.underlying_channel.write(&bytes).await?;
    Ok(())
}

// ❌ Wrong: Modifying message structure
async fn send(&mut self, message: TransportMessage) -> Result<()> {
    // Don't do this!
    let mut custom_msg = CustomMessage::from(message);
    custom_msg.add_custom_field("timestamp", SystemTime::now());
    self.underlying_channel.write(&custom_msg).await?;
    Ok(())
}
```

### 2. Handle Framing Correctly

Ensure message boundaries are preserved:

```rust
// ✅ Correct: Length-prefixed framing
async fn send(&mut self, message: TransportMessage) -> Result<()> {
    let json = serde_json::to_vec(&message)?;

    // Send length prefix (4 bytes, big-endian)
    let len = (json.len() as u32).to_be_bytes();
    self.writer.write_all(&len).await?;

    // Send message
    self.writer.write_all(&json).await?;
    Ok(())
}

async fn receive(&mut self) -> Result<TransportMessage> {
    // Read length prefix
    let mut len_buf = [0u8; 4];
    self.reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // Read exact message
    let mut buf = vec![0u8; len];
    self.reader.read_exact(&mut buf).await?;

    serde_json::from_slice(&buf).map_err(Into::into)
}
```

### 3. Implement Proper Error Handling

Map transport-specific errors to `TransportError`:

```rust
use pmcp::error::{Error, TransportError};

async fn send(&mut self, message: TransportMessage) -> Result<()> {
    match self.underlying_send(&message).await {
        Ok(()) => Ok(()),
        Err(e) if e.is_connection_error() => {
            Err(Error::Transport(TransportError::ConnectionClosed))
        }
        Err(e) if e.is_timeout() => {
            Err(Error::Timeout(5000))  // 5 seconds
        }
        Err(e) => {
            Err(Error::Transport(TransportError::InvalidMessage(
                format!("Send failed: {}", e)
            )))
        }
    }
}
```

### 4. Support Concurrent send/receive

Transports must handle concurrent operations:

```rust
// ✅ Correct: Separate channels for send/receive
pub struct MyTransport {
    send_tx: mpsc::Sender<TransportMessage>,
    recv_rx: Arc<Mutex<mpsc::Receiver<TransportMessage>>>,
}

// ❌ Wrong: Single shared state without synchronization
pub struct BadTransport {
    connection: TcpStream,  // Can't safely share between send/receive
}
```

### 5. Security Considerations

Even for internal transports:

```rust
// ✅ Bind to localhost only
let listener = TcpListener::bind("127.0.0.1:8080").await?;

// ❌ Binding to all interfaces exposes to network
let listener = TcpListener::bind("0.0.0.0:8080").await?;

// ✅ Validate message sizes to prevent DoS
const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;  // 10 MB

async fn receive(&mut self) -> Result<TransportMessage> {
    let len = self.read_length().await?;

    if len > MAX_MESSAGE_SIZE {
        return Err(Error::Transport(TransportError::InvalidMessage(
            format!("Message too large: {} bytes", len)
        )));
    }

    // ... continue reading
}
```

### 6. Message Encryption (End-to-End Security)

For high-security environments (defense, healthcare, finance), encrypt messages at the transport layer:

> **⚠️ Important Caveat**: This example wraps messages in a custom `Notification` envelope. Both client and server MUST use `EncryptedTransport` - this is **not interoperable** with standard MCP clients/servers. For transparent encryption, use **TLS/mTLS at the transport layer** instead (e.g., `wss://` for WebSocket, HTTPS for HTTP).
>
> This pattern is appropriate when you control both ends and need application-layer encryption with custom key management.

```rust
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

/// Encrypted transport wrapper
pub struct EncryptedTransport<T: Transport> {
    inner: T,
    cipher: Aes256Gcm,
}

impl<T: Transport> EncryptedTransport<T> {
    /// Create encrypted transport with 256-bit AES-GCM
    pub fn new(inner: T, key: &[u8; 32]) -> Result<Self> {
        let cipher = Aes256Gcm::new(key.into());
        Ok(Self { inner, cipher })
    }

    /// Encrypt message payload
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Generate random nonce (12 bytes for AES-GCM)
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| Error::Transport(TransportError::InvalidMessage(
                format!("Encryption failed: {}", e)
            )))?;

        // Prepend nonce to ciphertext for decryption
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt message payload
    fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(Error::Transport(TransportError::InvalidMessage(
                "Message too short for nonce".to_string()
            )));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::Transport(TransportError::InvalidMessage(
                format!("Decryption failed: {}", e)
            )))?;

        Ok(plaintext)
    }
}

#[async_trait]
impl<T: Transport> Transport for EncryptedTransport<T> {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // Serialize message
        let plaintext = serde_json::to_vec(&message)?;

        // Encrypt
        let encrypted = self.encrypt(&plaintext)?;

        // Wrap in envelope
        let envelope = TransportMessage::Notification(Notification::Custom {
            method: "encrypted".to_string(),
            params: serde_json::json!({
                "data": base64::encode(&encrypted),
            }),
        });

        // Send via underlying transport
        self.inner.send(envelope).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        // Receive encrypted envelope
        let envelope = self.inner.receive().await?;

        // Extract encrypted data
        let encrypted_b64 = match envelope {
            TransportMessage::Notification(Notification::Custom { params, .. }) => {
                params["data"].as_str().ok_or_else(|| {
                    Error::Transport(TransportError::InvalidMessage(
                        "Missing encrypted data".to_string()
                    ))
                })?
            }
            _ => return Err(Error::Transport(TransportError::InvalidMessage(
                "Expected encrypted envelope".to_string()
            ))),
        };

        // Decode and decrypt
        let encrypted = base64::decode(encrypted_b64)
            .map_err(|e| Error::Transport(TransportError::InvalidMessage(e.to_string())))?;

        let plaintext = self.decrypt(&encrypted)?;

        // Deserialize message
        serde_json::from_slice(&plaintext).map_err(Into::into)
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn transport_type(&self) -> &'static str {
        "encrypted"
    }
}

// Usage
let base_transport = HttpTransport::new(/* ... */);
let encryption_key: [u8; 32] = derive_key_from_password("secure-password");
let encrypted_transport = EncryptedTransport::new(base_transport, &encryption_key)?;

let client = Client::new(encrypted_transport);
```

**Security Benefits:**
- ✅ **End-to-end encryption**: Only sender/receiver can decrypt
- ✅ **Authenticated encryption**: AES-GCM provides integrity checks
- ✅ **Nonce-based**: Each message has unique nonce (prevents replay)
- ✅ **Tamper-evident**: Modified ciphertext fails decryption

**Use Cases:**
- Defense contractor systems (provably encrypted)
- Healthcare data (HIPAA compliance)
- Financial transactions (PCI-DSS requirements)
- Cross-border data transfers (GDPR encryption at rest/in transit)

### 7. Performance Optimization

For high-throughput scenarios:

```rust
// Message batching
pub struct BatchingTransport {
    pending: Vec<TransportMessage>,
    flush_interval: Duration,
}

impl BatchingTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.pending.push(message);

        if self.pending.len() >= 100 {  // Batch size threshold
            self.flush().await?;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }

        // Send all pending messages in one operation
        let batch = std::mem::take(&mut self.pending);
        self.underlying_send_batch(batch).await?;
        Ok(())
    }
}

// Compression for large messages
async fn send_with_compression(
    &mut self,
    message: TransportMessage,
) -> Result<()> {
    let json = serde_json::to_vec(&message)?;

    if json.len() > 1024 {  // Compress large messages
        let compressed = compress(&json)?;
        self.send_compressed(compressed).await?;
    } else {
        self.send_raw(json).await?;
    }

    Ok(())
}
```

## Testing Your Transport

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::types::*;

    #[tokio::test]
    async fn test_send_receive() {
        let mut transport = MyTransport::new().await.unwrap();

        // Create test message
        let request = TransportMessage::Request {
            id: RequestId::from(1),
            request: Request::Client(Box::new(ClientRequest::Ping)),
        };

        // Send and receive
        transport.send(request.clone()).await.unwrap();
        let received = transport.receive().await.unwrap();

        // Verify round-trip
        assert!(matches!(received, TransportMessage::Request { .. }));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut transport = MyTransport::new().await.unwrap();

        // Close transport
        transport.close().await.unwrap();

        // Verify sends fail after close
        let result = transport.send(/* ... */).await;
        assert!(result.is_err());
    }
}
```

### Integration Tests with mcp-tester

Use [mcp-tester](../ch15-testing.md#mcp-server-tester) to validate transport behavior:

```bash
# Test custom transport server
cargo build --release --example my_custom_transport_server

# Run integration tests
mcp-tester test <CUSTOM_TRANSPORT_URL> \
  --with-tools \
  --with-resources \
  --format json > results.json
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_transport_roundtrip(
        id in any::<i64>(),
        method in "[a-z]{1,20}",
    ) {
        tokio_test::block_on(async {
            let mut transport = MyTransport::new().await.unwrap();

            let message = TransportMessage::Request {
                id: RequestId::from(id),
                request: Request::Client(Box::new(
                    ClientRequest::ListTools(ListToolsParams { cursor: None })
                )),
            };

            transport.send(message.clone()).await.unwrap();
            let received = transport.receive().await.unwrap();

            // Verify message integrity
            prop_assert!(matches!(received, TransportMessage::Request { .. }));
        });
    }
}
```

## Business Use Cases for Custom Transports

Custom transports require significant development effort (both client and server implementations). Here are 8 business scenarios where the investment pays off:

### 1. Long-Running Operations (Fintech, Legal-Tech)

**Business Problem:**
A risk modeling platform runs MCP tools that perform complex calculations taking 5-30 minutes (credit risk analysis, compliance checks, multi-agent simulations).

**Why Async Transport:**
```
┌──────────┐     Request      ┌──────────┐     Job Queue    ┌──────────┐
│  Client  │─────────────────►│   SQS    │─────────────────►│  Lambda  │
│  (Agent) │                  │  Queue   │                  │  Worker  │
└──────────┘                  └──────────┘                  └──────────┘
     │                              ▲                             │
     │ Job ID                       │ Completed                   │ Process
     │ returned                     │ notification                │ 5-30 min
     │ immediately                  └─────────────────────────────┘
     │
     │ Poll/Subscribe
     │ for results
     ▼
┌──────────┐
│ Results  │
│ Queue    │
└──────────┘
```

**Benefits:**
- ✅ **No timeouts**: Client doesn't wait; gets job ID immediately
- ✅ **Job orchestration**: Multiple workers process queue in parallel
- ✅ **UX improvement**: Frontend shows "Processing..." with progress updates
- ✅ **Cost optimization**: Workers scale based on queue depth

**Transport Choice**: SQS (AWS), Azure Service Bus, Google Cloud Tasks

---

### 2. Regulated/Air-Gapped Environments (Healthcare, Government)

**Business Problem:**
Healthcare organization needs MCP tools to access sensitive patient data inside secure network, but analysts/AI agents run outside that zone (HIPAA compliance).

**Why Async Transport:**
```
Public Zone          Security Boundary      Secure Zone
┌──────────┐             │            ┌──────────┐
│ AI Agent │             │            │  MCP     │
│ (Client) │             │            │  Server  │
└──────────┘             │            │  (PHI    │
     │                   │            │  Access) │
     │ Encrypted         │            └──────────┘
     │ request           │                  ▲
     ▼                   │                  │
┌──────────┐             │            ┌──────────┐
│  Kafka   │◄────────────┼───────────►│  Kafka   │
│  Public  │   Firewall  │  Consumer  │  Secure  │
│  Broker  │   Rules     │  Group     │  Broker  │
└──────────┘             │            └──────────┘
```

**Benefits:**
- ✅ **Security isolation**: No direct socket connections across zones
- ✅ **Encryption at rest**: Kafka stores encrypted messages
- ✅ **Audit trail**: All requests/responses logged for compliance
- ✅ **Policy enforcement**: Messages routed based on classification level

**Transport Choice**: Kafka (with encryption), AWS SQS (cross-account), Azure Event Hubs

---

### 3. Burst Load & Elastic Scaling (SaaS)

**Business Problem:**
SaaS provider offers MCP servers for text-to-structured-data conversion. During peak hours (9-11am), thousands of clients issue requests simultaneously.

**Why Async Transport:**
```
                    ┌──────────┐
Peak: 10k req/min───►│  Kafka   │
                    │  Topic   │
                    │  (Buffer)│
                    └──────────┘
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
        ┌─────────┐ ┌─────────┐ ┌─────────┐
        │  MCP    │ │  MCP    │ │  MCP    │
        │ Server  │ │ Server  │ │ Server  │
        │ Pod 1   │ │ Pod 2   │ │ Pod N   │
        └─────────┘ └─────────┘ └─────────┘

        Auto-scales based on queue depth (lag)
```

**Benefits:**
- ✅ **Elastic scaling**: Workers scale up/down based on queue lag
- ✅ **Cost optimization**: Turn off servers when idle (queue empty)
- ✅ **Smooth bursts**: Queue absorbs spikes; no client rate limit errors
- ✅ **Priority queues**: High-priority clients use separate topic

**Transport Choice**: Kafka (for throughput), RabbitMQ (for priority queues), Redis Streams

---

### 4. Multi-Tenant, Multi-Region (Enterprise)

**Business Problem:**
Global enterprise has data centers in EU, US, Asia. MCP clients and servers may reside in different regions. Data sovereignty requires EU data stays in EU.

**Why Async Transport:**
```
EU Region                  US Region                 Asia Region
┌──────────┐             ┌──────────┐            ┌──────────┐
│  Client  │             │  Client  │            │  Client  │
│ (Germany)│             │  (Ohio)  │            │ (Tokyo)  │
└──────────┘             └──────────┘            └──────────┘
     │                         │                       │
     ▼                         ▼                       ▼
┌──────────┐             ┌──────────┐            ┌──────────┐
│  Kafka   │             │  Kafka   │            │  Kafka   │
│  EU      │             │  US      │            │  Asia    │
└──────────┘             └──────────┘            └──────────┘
     │                         │                       │
     ▼                         ▼                       ▼
┌──────────┐             ┌──────────┐            ┌──────────┐
│  MCP     │             │  MCP     │            │  MCP     │
│  Servers │             │  Servers │            │  Servers │
│  (EU)    │             │  (US)    │            │  (Asia)  │
└──────────┘             └──────────┘            └──────────┘
```

**Benefits:**
- ✅ **Latency optimization**: Process requests close to data source
- ✅ **Regulatory compliance**: EU data never leaves EU (GDPR)
- ✅ **Failover**: Route EU requests to US if EU region down
- ✅ **Topic-based routing**: Geo-tagged messages route automatically

**Transport Choice**: Kafka (multi-region replication), AWS SQS (cross-region)

---

### 5. Cross-System Integration (AI + Legacy)

**Business Problem:**
MCP client acts as AI orchestrator needing to trigger workflows on legacy ERP/CRM systems that cannot maintain live socket connections (batch-oriented, mainframe integration).

**Why Async Transport:**
```
┌──────────┐     Modern     ┌──────────┐    Bridge    ┌──────────┐
│  AI      │     MCP        │  Message │    Adapter   │  Legacy  │
│  Agent   │───────────────►│  Queue   │─────────────►│  ERP     │
│ (Claude) │   JSON-RPC     │  (SQS)   │   XML/SOAP   │ (SAP)    │
└──────────┘                └──────────┘              └──────────┘
     ▲                            │                         │
     │                            │                         │
     │ Result                     │ Poll every              │ Batch
     │ notification               │ 30 seconds              │ process
     └────────────────────────────┴─────────────────────────┘
```

**Benefits:**
- ✅ **Bridge async systems**: AI doesn't need to know if ERP is online
- ✅ **Decouple failure domains**: AI continues working if ERP down
- ✅ **Extend MCP reach**: Wrap legacy services with MCP-compatible async interface
- ✅ **Protocol translation**: Queue adapter converts JSON-RPC ↔ XML/SOAP

**Transport Choice**: SQS (simple integration), Apache Camel + Kafka (complex routing)

---

### 6. High-Security Messaging (Defense, Blockchain)

**Business Problem:**
Defense contractor uses MCP tools to perform sensitive computations that must be provably encrypted, tamper-evident, and non-repudiable (zero-trust architecture).

**Why Custom Transport:**
```
┌──────────┐   Encrypted    ┌──────────┐   Encrypted    ┌──────────┐
│ Client   │   MCP          │  Kafka   │   MCP          │  MCP     │
│ (Secret) │───────────────►│  (TLS +  │───────────────►│  Server  │
│  Agent   │   AES-256-GCM  │  ACLs)   │   AES-256-GCM  │ (Secure  │
└──────────┘                └──────────┘                │  Enclave)│
                                   │                    └──────────┘
                                   │
                                   ▼
                            ┌──────────┐
                            │ Immutable│
                            │ Audit    │
                            │ Log      │
                            └──────────┘
```

**Benefits:**
- ✅ **End-to-end encryption**: Payloads encrypted by client, decrypted by server only
- ✅ **Non-repudiation**: Kafka's immutable log proves message history
- ✅ **Policy enforcement**: Custom headers enforce classification levels (TOP SECRET, etc.)
- ✅ **Compliance**: FIPS 140-2 validated encryption modules

**Transport Choice**: Encrypted Kafka transport (Example 6 above), AWS KMS + SQS

---

### 7. Event-Driven Workflows (Analytics)

**Business Problem:**
Analytics platform uses MCP clients embedded in microservices that react to real-time business events (user signup, transaction alert, IoT sensor data).

**Why Async Transport:**
```
Event Sources              Kafka Streams           MCP Processors
┌──────────┐                                      ┌──────────┐
│ User     │─┐                                   ┌►│  MCP     │
│ Signups  │ │            ┌──────────┐          │ │  Tool:   │
└──────────┘ ├───────────►│  Kafka   │──────────┤ │  Enrich  │
             │            │  Topic:  │          │ │  Profile │
┌──────────┐ │            │  events  │          │ └──────────┘
│ Trans-   │─┤            └──────────┘          │
│ actions  │ │                  │               │ ┌──────────┐
└──────────┘ │                  │               └►│  MCP     │
             │                  │                 │  Tool:   │
┌──────────┐ │                  ▼                 │  Score   │
│ IoT      │─┘            ┌──────────┐            │  Risk    │
│ Sensors  │              │  MCP     │            └──────────┘
└──────────┘              │  Clients │
                          │(Consumers)│            ┌──────────┐
                          └──────────┘            ┌►│  MCP     │
                                                  │ │  Tool:   │
                           Process each event     │ │  Alert   │
                           through MCP tools      │ │  Webhook │
                                                  │ └──────────┘
                                                  │
                                                  │ ┌──────────┐
                                                  └►│  MCP     │
                                                    │  Tool:   │
                                                    │  ML      │
                                                    │  Predict │
                                                    └──────────┘
```

**Benefits:**
- ✅ **Reactive design**: Kafka events trigger MCP workflows automatically
- ✅ **Backpressure control**: MCP servers process events at their own pace
- ✅ **Composability**: Multiple event-driven MCP clients coordinate on shared bus
- ✅ **Stream processing**: Kafka Streams integrates with MCP tools for windowing, joins

**Transport Choice**: Kafka (for event streaming), AWS Kinesis, Apache Pulsar

---

### 8. Offline/Intermittent Connectivity (Edge Devices)

**Business Problem:**
Ships, drones, field sensors act as MCP clients but can't maintain stable network links. They collect data offline and sync when connectivity returns.

**Why Async Transport:**
```
Edge Device (Ship)         Satellite Link       Cloud
┌──────────┐                                    ┌──────────┐
│  MCP     │   Offline:                        │  Kafka   │
│  Client  │   Queue locally                   │  Broker  │
│          │        │                           │          │
│  Local   │        ▼                           └──────────┘
│  Queue   │◄──┐ ┌──────┐                            │
│  (SQLite)│   └─│ Net  │                            ▼
└──────────┘     │ Down │                      ┌──────────┐
     │           └──────┘                      │  MCP     │
     │                                         │  Servers │
     │  Online:                                │  (Cloud) │
     │  Sync queue                             └──────────┘
     ▼
┌──────────┐     Network Up     ┌──────────┐
│  Sync    │────────────────────►│  Kafka   │
│  Process │◄────────────────────│  Topic   │
└──────────┘      ACKs           └──────────┘
```

**Benefits:**
- ✅ **Offline queuing**: Requests queued locally (SQLite, LevelDB)
- ✅ **Resilience**: Automatic retry when connection restored
- ✅ **Simplified sync**: Kafka acts as durable buffer for unreliable endpoints
- ✅ **Conflict resolution**: Last-write-wins or custom merge strategies

**Transport Choice**: Local queue + Kafka sync, MQTT (IoT-optimized), AWS IoT Core

---

### Summary: When to Build Custom Transports

| Use Case | Latency Tolerance | Complexity | ROI |
|----------|------------------|------------|-----|
| Long-running ops | High (minutes) | Medium | ✅ High |
| Air-gapped security | Medium (seconds) | High | ✅ High |
| Burst scaling | Medium (100-500ms) | Medium | ✅ High |
| Multi-region | Medium (100-500ms) | High | ✅ Medium |
| Legacy integration | High (minutes) | Medium | ✅ High |
| High-security | Low (milliseconds) | High | ✅ Medium |
| Event-driven | Medium (100-500ms) | Medium | ✅ High |
| Offline/edge | High (minutes-hours) | Medium | ✅ High |

**Decision Criteria:**
- ✅ Build custom transport if: Regulatory requirements, legacy constraints, or operational patterns prevent standard transports
- ❌ Avoid if: Standard HTTP/WebSocket/stdio meets needs (99% of cases)

## Gateway/Proxy Pattern (Recommended)

**The Right Way to Use Custom Transports**: Instead of requiring all clients to implement custom transports, use a **gateway** that translates between standard and custom transports.

### Architecture

```
Standard MCP Clients          Gateway              Custom Backend
┌──────────┐                                       ┌──────────┐
│ Claude   │    WebSocket     ┌──────────┐  Kafka │  MCP     │
│ Desktop  │◄────────────────►│          │◄──────►│  Server  │
└──────────┘                  │          │        │  Pool    │
                              │  MCP     │        └──────────┘
┌──────────┐                  │ Gateway  │        ┌──────────┐
│ Custom   │     HTTP         │          │  SQS   │  Lambda  │
│ Client   │◄────────────────►│  Bridge  │◄──────►│  Workers │
└──────────┘                  │          │        └──────────┘
                              │  Policy  │        ┌──────────┐
┌──────────┐                  │  Layer   │  HTTP  │  Legacy  │
│  IDE     │    stdio/HTTP    │          │◄──────►│  Backend │
│  Plugin  │◄────────────────►│          │        └──────────┘
└──────────┘                  └──────────┘
```

### Benefits

**✅ Client Compatibility:**
- Standard MCP clients work unchanged (Claude Desktop, IDEs)
- No client-side custom transport implementation needed
- One gateway serves all clients

**✅ Control Point:**
- **Authorization**: Check permissions before routing
- **DLP**: Redact PII, filter sensitive data
- **Rate Limiting**: Per-client quotas
- **Schema Validation**: Reject malformed requests
- **Audit**: Centralized logging of all MCP traffic

**✅ Backend Flexibility:**
- Route to different backends based on capability
- Load balancing across server pool
- Circuit breakers for failing backends
- Automatic retries with backoff
- Protocol translation (JSON-RPC ↔ XML/SOAP)

**✅ Observability:**
- Centralized metrics (latency, throughput, errors)
- Distributed tracing across transports
- Real-time monitoring dashboards

### Implementation

**Minimal Bridge (Bidirectional Forwarder):**

```rust
use pmcp::shared::{Transport, TransportMessage};
use pmcp::error::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Bridge that forwards messages between two transports
pub async fn bridge_transports(
    mut client_side: Box<dyn Transport + Send>,
    mut backend_side: Box<dyn Transport + Send>,
) -> Result<()> {
    // Client -> Backend
    let c2b = tokio::spawn(async move {
        loop {
            match client_side.receive().await {
                Ok(msg) => {
                    if let Err(e) = backend_side.send(msg).await {
                        tracing::error!("Failed to forward to backend: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    tracing::info!("Client disconnected: {}", e);
                    break;
                }
            }
        }
        Result::<()>::Ok(())
    });

    // Backend -> Client
    let b2c = tokio::spawn(async move {
        loop {
            match backend_side.receive().await {
                Ok(msg) => {
                    if let Err(e) = client_side.send(msg).await {
                        tracing::error!("Failed to forward to client: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    tracing::info!("Backend disconnected: {}", e);
                    break;
                }
            }
        }
        Result::<()>::Ok(())
    });

    // If either side exits, close the other
    tokio::select! {
        r = c2b => { let _ = r?; }
        r = b2c => { let _ = r?; }
    }

    Ok(())
}

// Usage
let client_transport = WebSocketTransport::new(config)?;
let backend_transport = KafkaTransport::new_client(brokers, client_id).await?;

tokio::spawn(bridge_transports(
    Box::new(client_transport),
    Box::new(backend_transport),
));
```

**Policy-Enforcing Gateway:**

```rust
/// Transport wrapper that enforces policies
pub struct PolicyTransport<T: Transport> {
    inner: T,
    policy: Arc<PolicyEngine>,
}

#[async_trait]
impl<T: Transport + Send + Sync> Transport for PolicyTransport<T> {
    async fn send(&mut self, msg: TransportMessage) -> Result<()> {
        // Enforce outbound policies (rate limits, quotas)
        self.policy.check_send(&msg).await?;

        // Log for audit
        tracing::info!("Sending: {:?}", msg);

        self.inner.send(msg).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let msg = self.inner.receive().await?;

        // Enforce inbound policies (schema validation, PII redaction)
        let sanitized = self.policy.sanitize(msg).await?;

        // Log for audit
        tracing::info!("Received: {:?}", sanitized);

        Ok(sanitized)
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn transport_type(&self) -> &'static str {
        "policy"
    }
}

// Usage
let base = KafkaTransport::new_client(brokers, client_id).await?;
let policy_engine = Arc::new(PolicyEngine::new());
let policy_transport = PolicyTransport {
    inner: base,
    policy: policy_engine,
};
```

**Full Gateway Service:**

```rust
use pmcp::shared::{Transport, WebSocketTransport, WebSocketConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    // Listen for WebSocket connections from clients
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Gateway listening on port 8080");

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("New client connection from {}", addr);

        tokio::spawn(async move {
            // Create client-facing transport (WebSocket)
            let ws_config = WebSocketConfig { /* ... */ };
            let client_transport = WebSocketTransport::from_stream(stream, ws_config);

            // Create backend transport (Kafka/SQS based on routing logic)
            let backend_transport = select_backend_transport(addr).await?;

            // Wrap in policy layer
            let policy_transport = PolicyTransport {
                inner: backend_transport,
                policy: Arc::new(PolicyEngine::new()),
            };

            // Bridge the two transports
            bridge_transports(
                Box::new(client_transport),
                Box::new(policy_transport),
            ).await
        });
    }
}

async fn select_backend_transport(client_addr: SocketAddr) -> Result<Box<dyn Transport>> {
    // Route based on client, capability, tenant, etc.
    if client_addr.ip().is_loopback() {
        // Local clients -> direct HTTP
        Ok(Box::new(HttpTransport::new(/* ... */)))
    } else {
        // External clients -> Kafka
        let client_id = format!("gateway-{}", uuid::Uuid::new_v4());
        Ok(Box::new(KafkaTransport::new_client(KAFKA_BROKERS, client_id).await?))
    }
}
```

### Routing Strategies

**1. Capability-Based:**
```rust
async fn route_by_capability(request: &TransportMessage) -> String {
    match request {
        TransportMessage::Request { request, .. } => {
            match request {
                Request::Client(ClientRequest::CallTool(params)) => {
                    // Route to Kafka topic based on tool name
                    if params.name.starts_with("ml_") {
                        "mcp.requests.ml-tools".to_string()
                    } else if params.name.starts_with("data_") {
                        "mcp.requests.data-tools".to_string()
                    } else {
                        "mcp.requests.global".to_string()
                    }
                }
                _ => "mcp.requests.global".to_string(),
            }
        }
        _ => "mcp.requests.global".to_string(),
    }
}
```

**2. Load Balancing:**
```rust
struct BackendPool {
    backends: Vec<Box<dyn Transport>>,
    current_index: AtomicUsize,
}

impl BackendPool {
    fn get_next(&self) -> &Box<dyn Transport> {
        let idx = self.current_index.fetch_add(1, Ordering::Relaxed);
        &self.backends[idx % self.backends.len()]
    }
}
```

**3. Circuit Breaker:**
```rust
struct CircuitBreakerTransport<T: Transport> {
    inner: T,
    state: Arc<Mutex<CircuitState>>,
}

enum CircuitState {
    Closed { failures: u32 },
    Open { until: Instant },
    HalfOpen,
}

impl<T: Transport> Transport for CircuitBreakerTransport<T> {
    async fn send(&mut self, msg: TransportMessage) -> Result<()> {
        let state = self.state.lock().await;
        match *state {
            CircuitState::Open { until } if Instant::now() < until => {
                return Err(Error::Transport(TransportError::ConnectionClosed));
            }
            _ => {}
        }
        drop(state);

        match self.inner.send(msg).await {
            Ok(()) => {
                // Success - reset failures
                let mut state = self.state.lock().await;
                *state = CircuitState::Closed { failures: 0 };
                Ok(())
            }
            Err(e) => {
                // Failure - increment counter
                let mut state = self.state.lock().await;
                if let CircuitState::Closed { failures } = *state {
                    if failures + 1 >= 5 {
                        // Trip circuit breaker
                        *state = CircuitState::Open {
                            until: Instant::now() + Duration::from_secs(30),
                        };
                    } else {
                        *state = CircuitState::Closed { failures: failures + 1 };
                    }
                }
                Err(e)
            }
        }
    }
    // ... rest of implementation
}
```

### Testing with Gateway

**Integration Testing:**

```bash
# Start gateway
cargo run --bin mcp-gateway

# Test with mcp-tester against gateway (WebSocket)
mcp-tester test ws://localhost:8080 \
  --with-tools \
  --format json > results.json

# Gateway exercises custom backend transport (Kafka/SQS)
# under realistic conditions
```

**Key Advantages:**
- ✅ **Client compatibility**: Standard clients work without modification
- ✅ **Flexibility**: Change backend transport without client changes
- ✅ **Testability**: `mcp-tester` validates end-to-end flow
- ✅ **Incremental migration**: Gradually move backends to custom transports

## Advanced Topics

### Connection Pooling

For HTTP-like transports, implement pooling:

```rust
use deadpool::managed::{Manager, Pool, RecycleResult};

struct MyTransportManager;

#[async_trait]
impl Manager for MyTransportManager {
    type Type = MyTransport;
    type Error = Error;

    async fn create(&self) -> Result<MyTransport, Error> {
        MyTransport::connect().await
    }

    async fn recycle(&self, conn: &mut MyTransport) -> RecycleResult<Error> {
        if conn.is_connected() {
            Ok(())
        } else {
            Err(RecycleResult::StaticMessage("Connection lost"))
        }
    }
}

// Usage
let pool: Pool<MyTransportManager> = Pool::builder(MyTransportManager)
    .max_size(10)
    .build()
    .unwrap();

let transport = pool.get().await?;
```

### Middleware Support

Wrap transports with cross-cutting concerns:

```rust
pub struct LoggingTransport<T: Transport> {
    inner: T,
}

#[async_trait]
impl<T: Transport> Transport for LoggingTransport<T> {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        tracing::info!("Sending message: {:?}", message);
        let start = std::time::Instant::now();

        let result = self.inner.send(message).await;

        tracing::info!("Send completed in {:?}", start.elapsed());
        result
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        tracing::debug!("Waiting for message...");
        let message = self.inner.receive().await?;
        tracing::info!("Received message: {:?}", message);
        Ok(message)
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}
```

## Conclusion

Custom transports unlock MCP's full potential for specialized environments:

**Simple Use Cases:**
- ✅ Use **in-memory transports** for testing
- ✅ Use **built-in transports** (HTTP, WebSocket, stdio) for standard cases

**Advanced Use Cases:**
- ✅ **Async messaging** (SQS, Kafka) for decoupled architectures
- ✅ **Custom protocols** when integrating with legacy systems
- ✅ **Performance optimizations** for high-throughput scenarios

**Remember:**
- GraphQL → Build MCP servers with GraphQL tools (not transport)
- Custom transports require both client and server support
- Stick to standard transports unless you have specific infrastructure needs
- Test thoroughly with unit tests, integration tests, and mcp-tester

**Additional Resources:**
- [MCP Specification: Transports](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [asyncmcp GitHub](https://github.com/bh-rat/asyncmcp): AWS SQS/SNS transport examples
- [Chapter 15: Testing](./ch15-testing.md): Use mcp-tester to validate custom transports
- [Apollo MCP Server](https://www.apollographql.com/docs/apollo-mcp-server): GraphQL + MCP example
