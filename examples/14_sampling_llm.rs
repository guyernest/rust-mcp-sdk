//! Example of implementing a sampling/LLM server.

use async_trait::async_trait;
use pmcp::{
    types::{
        capabilities::ServerCapabilities, Content, CreateMessageParams, CreateMessageResult,
        SamplingMessageContent, TokenUsage,
    },
    SamplingHandler, Server,
};
use tracing::info;

struct MockLLM {
    model_name: String,
}

#[async_trait]
impl SamplingHandler for MockLLM {
    async fn create_message(
        &self,
        params: CreateMessageParams,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<CreateMessageResult> {
        info!(
            "Received sampling request with {} messages",
            params.messages.len()
        );

        // In a real implementation, this would call an actual LLM
        let response_text = format!(
            "This is a mock response to: {}",
            params
                .messages
                .last()
                .map(|m| match &m.content {
                    SamplingMessageContent::Text { text, .. } => text.as_str(),
                    SamplingMessageContent::Image { .. } => "[image]",
                    _ => "[other]",
                })
                .unwrap_or("empty")
        );

        Ok(
            CreateMessageResult::new(Content::text(response_text), &self.model_name)
                .with_usage(TokenUsage::new(
                    params.messages.len() as u32 * 10,
                    20,
                    params.messages.len() as u32 * 10 + 20,
                ))
                .with_stop_reason("end_of_text"),
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Creating LLM server");

    let server = Server::builder()
        .name("mock-llm-server")
        .version("1.0.0")
        .capabilities({
            let mut caps = ServerCapabilities::default();
            caps.sampling = Some(Default::default());
            caps
        })
        .sampling(MockLLM {
            model_name: "mock-gpt-4".to_string(),
        })
        .build()?;

    info!("Starting server on stdio");
    server.run_stdio().await?;

    Ok(())
}
