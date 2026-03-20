//! Tool with Sampling Server Example
//!
//! This example demonstrates how to create an MCP tool that uses LLM sampling
//! to provide text summarization capabilities, similar to the TypeScript SDK's
//! toolWithSampleServer.ts example.
//!
//! Key features:
//! - Tool that internally uses LLM sampling
//! - Text summarization using sampling API
//! - Error handling and input validation
//! - Structured responses with human-readable content
//!
//! Run with: cargo run --example 49_tool_with_sampling_server --features full

use async_trait::async_trait;
use pmcp::{Error, RequestHandlerExtra, Result, Server, ServerCapabilities, ToolHandler};
use serde_json::{json, Value};

/// Text summarization tool that uses LLM sampling
///
/// This tool demonstrates how to create MCP tools that use sampling APIs
/// for text processing tasks like summarization.
struct SummarizeTool;

#[async_trait]
impl ToolHandler for SummarizeTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // Extract and validate input text
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::validation("Missing required 'text' parameter"))?;

        if text.is_empty() {
            return Err(Error::validation("Text parameter cannot be empty"));
        }

        // In a real implementation, this would call an actual LLM API
        // For this example, we'll simulate the summarization process
        let summary = simulate_llm_summarization(text).await?;

        // Return structured response with both human-readable content and metadata
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Summary: {}", summary)
            }],
            "isError": false,
            "metadata": {
                "original_length": text.len(),
                "summary_length": summary.len(),
                "compression_ratio": (text.len() as f64) / (summary.len() as f64),
                "model": "example-llm-model"
            }
        }))
    }
}

/// Simulates LLM summarization
///
/// In a real implementation, this would make calls to an actual LLM service
/// using the MCP sampling API through server.create_message() or similar.
async fn simulate_llm_summarization(text: &str) -> Result<String> {
    // Simulate processing time
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Simple extractive summarization algorithm for demonstration
    let sentences: Vec<&str> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if sentences.is_empty() {
        return Ok("No meaningful content to summarize".to_string());
    }

    // Take first sentence and last sentence if more than one
    let summary = if sentences.len() == 1 {
        sentences[0].to_string()
    } else if sentences.len() <= 3 {
        sentences.join(". ") + "."
    } else {
        // Take first, middle, and last sentences for longer texts
        let first = sentences[0];
        let middle = sentences[sentences.len() / 2];
        let last = sentences[sentences.len() - 1];
        format!("{}. {}. {}.", first, middle, last)
    };

    // Ensure summary is not longer than original
    if summary.len() >= text.len() {
        let truncated = text.chars().take(100).collect::<String>();
        Ok(format!("{}...", truncated.trim()))
    } else {
        Ok(summary)
    }
}

/// Advanced text analysis tool
///
/// Demonstrates more sophisticated text processing capabilities
struct AnalyzeTextTool;

#[async_trait]
impl ToolHandler for AnalyzeTextTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::validation("Missing required 'text' parameter"))?;

        if text.is_empty() {
            return Err(Error::validation("Text parameter cannot be empty"));
        }

        // Perform text analysis
        let analysis = analyze_text_structure(text);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Text Analysis:\n• {} characters\n• {} words\n• {} sentences\n• {} paragraphs\n• Readability: {}",
                    analysis.char_count,
                    analysis.word_count,
                    analysis.sentence_count,
                    analysis.paragraph_count,
                    analysis.readability_level
                )
            }],
            "isError": false,
            "structuredData": {
                "analysis": analysis
            }
        }))
    }
}

/// Text analysis result structure
#[derive(serde::Serialize)]
struct TextAnalysis {
    char_count: usize,
    word_count: usize,
    sentence_count: usize,
    paragraph_count: usize,
    readability_level: String,
    avg_sentence_length: f64,
    avg_word_length: f64,
}

/// Analyze text structure and readability
fn analyze_text_structure(text: &str) -> TextAnalysis {
    let char_count = text.len();
    let word_count = text.split_whitespace().count();
    let sentence_count = text.matches(['.', '!', '?']).count();
    let paragraph_count = text.split("\n\n").filter(|p| !p.trim().is_empty()).count();

    let avg_sentence_length = if sentence_count > 0 {
        word_count as f64 / sentence_count as f64
    } else {
        0.0
    };

    let avg_word_length = if word_count > 0 {
        text.chars().filter(|c| !c.is_whitespace()).count() as f64 / word_count as f64
    } else {
        0.0
    };

    let readability_level = determine_readability_level(avg_sentence_length, avg_word_length);

    TextAnalysis {
        char_count,
        word_count,
        sentence_count,
        paragraph_count: paragraph_count.max(1), // At least 1 paragraph
        readability_level,
        avg_sentence_length,
        avg_word_length,
    }
}

/// Determine readability level based on sentence and word length
fn determine_readability_level(avg_sentence_length: f64, avg_word_length: f64) -> String {
    match (avg_sentence_length, avg_word_length) {
        (s, w) if s <= 15.0 && w <= 4.5 => "Easy".to_string(),
        (s, w) if s <= 20.0 && w <= 5.5 => "Moderate".to_string(),
        (s, w) if s <= 25.0 && w <= 6.5 => "Challenging".to_string(),
        _ => "Advanced".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🤖 Tool with Sampling Server Example");
    println!("====================================");

    // Create server with sampling-based tools
    let server = Server::builder()
        .name("sampling-tools-server")
        .version("1.0.0")
        .capabilities({
            let mut caps = ServerCapabilities::tools_only();
            caps.sampling = Some(Default::default());
            caps
        })
        .tool("summarize", SummarizeTool)
        .tool("analyze_text", AnalyzeTextTool)
        .build()?;

    println!("📋 Available tools:");
    println!("  • summarize - Summarize text using LLM sampling");
    println!("    Parameters: {{ \"text\": \"your text here\" }}");
    println!("  • analyze_text - Analyze text structure and readability");
    println!("    Parameters: {{ \"text\": \"your text here\" }}");
    println!();
    println!("🚀 Server starting on stdio...");
    println!("💡 Tools use simulated LLM sampling for text processing");
    println!("🔧 In production, replace simulate_llm_summarization() with real LLM API calls");
    println!();

    // Run the server
    server.run_stdio().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_analysis() {
        let text = "This is a test. It has multiple sentences! How interesting?";
        let analysis = analyze_text_structure(text);

        assert_eq!(analysis.sentence_count, 3);
        assert!(analysis.word_count > 0);
        assert!(!analysis.readability_level.is_empty());
    }

    #[tokio::test]
    async fn test_summarization() {
        let text = "This is a long piece of text that needs to be summarized. It contains multiple sentences and ideas. The summarization should extract key information.";
        let result = simulate_llm_summarization(text).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(!summary.is_empty());
        assert!(summary.len() <= text.len()); // Summary should not be longer than original
    }

    #[tokio::test]
    async fn test_empty_text_handling() {
        let result = simulate_llm_summarization("").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No meaningful content to summarize");
    }

    #[test]
    fn test_readability_levels() {
        assert_eq!(determine_readability_level(10.0, 4.0), "Easy");
        assert_eq!(determine_readability_level(18.0, 5.0), "Moderate");
        assert_eq!(determine_readability_level(22.0, 6.0), "Challenging");
        assert_eq!(determine_readability_level(30.0, 7.0), "Advanced");
    }

    #[tokio::test]
    async fn test_summarize_tool_handler() {
        let tool = SummarizeTool;
        let args = json!({"text": "This is a test sentence."});
        let extra = RequestHandlerExtra::new(
            "test".to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            tokio_util::sync::CancellationToken::new(),
        );

        let result = tool.handle(args, extra).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response["isError"].as_bool().unwrap_or(true));
        assert!(response["content"].is_array());
        assert!(response["metadata"].is_object());
    }

    #[tokio::test]
    async fn test_analyze_text_tool_handler() {
        let tool = AnalyzeTextTool;
        let args = json!({"text": "This is a test. It has two sentences."});
        let extra = RequestHandlerExtra::new(
            "test".to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            tokio_util::sync::CancellationToken::new(),
        );

        let result = tool.handle(args, extra).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(!response["isError"].as_bool().unwrap_or(true));
        assert!(response["content"].is_array());
        assert!(response["structuredData"]["analysis"].is_object());
    }
}
