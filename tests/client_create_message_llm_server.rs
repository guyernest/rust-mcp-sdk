//! End-to-end regression for CR-01 (Phase 106): `Client::create_message`
//! against a sampling-enabled pmcp `Server` (the "LLM-server pattern").
//!
//! Before this fix `Client::assert_capability` had no `"sampling"` arm, so
//! **every** `create_message` call errored with a spurious "Server does not
//! support sampling" capability error — regardless of what the server
//! advertised — because the match fell through to `_ => false` and the
//! assertion unconditionally failed. No test or example exercised the path,
//! which is why the dead branch shipped.
//!
//! This test drives a real `Client` against a real `Server::builder()
//! .sampling(handler)` over the shared in-process duplex transport and asserts
//! the completion round-trips end to end.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use async_trait::async_trait;

use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResult, SamplingMessage, SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Content, Role};
use pmcp::{Client, RequestHandlerExtra, Result, SamplingHandler, Server};

/// Server-side sampling handler (the LLM-server side of the pattern).
struct EchoLlm;

#[async_trait]
impl SamplingHandler for EchoLlm {
    async fn create_message(
        &self,
        _params: CreateMessageParams,
        _extra: RequestHandlerExtra,
    ) -> Result<CreateMessageResult> {
        Ok(CreateMessageResult::new(
            Content::text("llm-server completion"),
            "server-model",
        ))
    }
}

/// CR-01: a `create_message` call must reach the wire and return the server's
/// completion when the server advertises `sampling` (it does automatically via
/// `.sampling(handler)`).
#[tokio::test]
async fn create_message_succeeds_against_sampling_server() {
    let server = Server::builder()
        .name("llm-server")
        .version("1.0.0")
        .sampling(EchoLlm)
        .build()
        .expect("build sampling server");

    let (client_t, server_t) = duplex::DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against sampling server");

    let params = CreateMessageParams::new(vec![SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: "hello".to_string(),
            meta: None,
        },
    )]);

    let result = client
        .create_message(params)
        .await
        .expect("create_message must succeed against a sampling-enabled server");

    assert_eq!(result.model, "server-model");
}
