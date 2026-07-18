//! Shared in-process duplex transport for the pmcp-agent real-loop tests.
//!
//! An mpsc-backed client<->server [`Transport`] pair, mirroring the SDK's own
//! `tests/common/duplex.rs`. Each file in `tests/` compiles as a separate
//! integration crate; include this module per-crate via
//! `#[path = "common/duplex.rs"] mod duplex;`.

#![allow(dead_code)]
#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::{Error, Result};
use tokio::sync::mpsc;

/// One half of an in-process duplex transport. The client side sends Requests
/// and receives Responses; the server side does the reverse.
#[derive(Debug)]
pub struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    /// Create a connected client/server transport pair.
    pub fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}
