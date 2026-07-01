//! Standard I/O transport implementation.
//!
//! This transport uses stdin/stdout for communication with newline-delimited
//! JSON-RPC messages as per the MCP specification.

use crate::error::{Result, TransportError};
use crate::shared::transport::{Transport, TransportMessage};
use async_trait::async_trait;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::Mutex;

/// stdio transport for MCP communication.
///
/// Uses newline-delimited JSON-RPC messages as per the MCP specification.
/// Messages are written to stdout and read from stdin.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::shared::StdioTransport;
///
/// # async fn example() -> pmcp::Result<()> {
/// let transport = StdioTransport::new();
/// // Use with Client or Server
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StdioTransport {
    stdin: Mutex<BufReader<tokio::io::Stdin>>,
    stdout: Mutex<tokio::io::Stdout>,
    closed: std::sync::atomic::AtomicBool,
}

impl StdioTransport {
    /// Create a new stdio transport.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::StdioTransport;
    ///
    /// let transport = StdioTransport::new();
    /// // Transport is ready to use
    /// ```
    pub fn new() -> Self {
        Self {
            stdin: Mutex::new(BufReader::new(tokio::io::stdin())),
            stdout: Mutex::new(tokio::io::stdout()),
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        contract_pre_transport_abstraction!();
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let json_bytes = Self::serialize_message(&message)?;
        self.write_message(&json_bytes).await
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        contract_pre_transport_abstraction!();
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err(TransportError::ConnectionClosed.into());
        }

        let buffer = self.read_line().await?;
        Self::parse_message(&buffer)
    }

    async fn close(&mut self) -> Result<()> {
        contract_pre_transport_abstraction!();
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);

        // Flush any pending output
        let mut stdout = self.stdout.lock().await;
        stdout.flush().await.map_err(TransportError::from)?;
        drop(stdout);

        // Note: To send EOF to the server, the spawning process should drop
        // the child process handle or close the pipe. This is handled at the
        // process/spawn level, not here. The server will see EOF on its stdin
        // when the client process terminates or closes its end of the pipe.

        Ok(())
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(std::sync::atomic::Ordering::Acquire)
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }
}

impl StdioTransport {
    /// Serialize transport message to JSON bytes.
    ///
    /// Delegates to [`crate::shared::transport::serialize_message`] — the single
    /// source of truth for the JSON-RPC wire encoding shared by all transports.
    pub fn serialize_message(message: &TransportMessage) -> Result<Vec<u8>> {
        crate::shared::transport::serialize_message(message)
    }

    /// Write message to stdout with newline delimiter.
    async fn write_message(&self, json_bytes: &[u8]) -> Result<()> {
        let mut stdout = self.stdout.lock().await;

        // Write message payload
        stdout
            .write_all(json_bytes)
            .await
            .map_err(TransportError::from)?;

        // Write newline delimiter (MCP spec requirement)
        stdout
            .write_all(b"\n")
            .await
            .map_err(TransportError::from)?;

        // Always flush stdio
        stdout.flush().await.map_err(TransportError::from)?;
        drop(stdout);

        Ok(())
    }

    /// Read a line from stdin (newline-delimited JSON per MCP spec)
    async fn read_line(&self) -> Result<Vec<u8>> {
        let mut stdin = self.stdin.lock().await;
        let mut line = String::new();

        let bytes_read = stdin
            .read_line(&mut line)
            .await
            .map_err(TransportError::from)?;

        if bytes_read == 0 {
            // EOF reached
            drop(stdin);
            self.closed
                .store(true, std::sync::atomic::Ordering::Release);
            return Err(TransportError::ConnectionClosed.into());
        }

        // Remove trailing newline and return as bytes
        let line = line.trim_end_matches('\n').trim_end_matches('\r');

        // Skip empty lines (per MCP spec: messages are delimited by newlines)
        if line.is_empty() {
            drop(stdin);
            return Err(TransportError::InvalidMessage("Empty line received".to_string()).into());
        }

        let bytes = line.as_bytes().to_vec();
        drop(stdin);
        Ok(bytes)
    }

    /// Parse JSON message and determine its type.
    ///
    /// Delegates to [`crate::shared::transport::parse_message`] — the single
    /// source of truth for JSON-RPC frame classification shared by all transports.
    pub fn parse_message(buffer: &[u8]) -> Result<TransportMessage> {
        crate::shared::transport::parse_message(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transport_properties() {
        let transport = StdioTransport::new();
        assert!(transport.is_connected());
        assert_eq!(transport.transport_type(), "stdio");
    }

    #[tokio::test]
    async fn test_close() {
        let mut transport = StdioTransport::new();
        assert!(transport.is_connected());

        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_newline_delimited_format() {
        // Test that serialization produces valid JSON without Content-Length
        let request = TransportMessage::Request {
            id: crate::types::RequestId::Number(1),
            request: crate::types::Request::Client(Box::new(
                crate::types::ClientRequest::Initialize(crate::types::InitializeRequest {
                    protocol_version: "2025-06-18".to_string(),
                    capabilities: crate::types::ClientCapabilities::default(),
                    client_info: crate::types::Implementation::new("test", "1.0.0"),
                }),
            )),
        };

        let json_bytes = StdioTransport::serialize_message(&request).unwrap();
        let json_str = String::from_utf8(json_bytes).unwrap();

        // Should be valid JSON without Content-Length header
        assert!(json_str.starts_with('{'));
        assert!(json_str.contains("jsonrpc\":\"2.0\""));
        assert!(!json_str.contains("Content-Length"));
        assert!(!json_str.contains("\r\n"));
    }
}
