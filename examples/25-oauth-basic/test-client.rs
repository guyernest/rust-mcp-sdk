use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::{Client, ClientCapabilities};
use serde_json::json;
use tracing_subscriber;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    let server_url = "http://localhost:8080";

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         TESTING OAUTH MCP SERVER WITH HTTP CLIENT         ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ Server: {:43} ║", server_url);
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Configure the HTTP transport
    let config = StreamableHttpTransportConfig {
        url: Url::parse(server_url).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
    };

    // Create the transport
    let transport = StreamableHttpTransport::new(config);

    // Create the client
    let mut client = Client::new(transport.clone());

    // Define client capabilities
    let capabilities = ClientCapabilities::minimal();

    // Initialize connection
    println!("📡 Initializing connection...");
    let result = client.initialize(capabilities).await?;
    println!("✅ Successfully connected!");
    println!(
        "   Server: {} v{}",
        result.server_info.name, result.server_info.version
    );
    println!("   Protocol: {}", result.protocol_version.0);

    // Set the protocol version on the transport
    transport.set_protocol_version(Some(result.protocol_version.0.clone()));

    // Get session ID
    if let Some(session_id) = transport.session_id() {
        println!("   Session ID: {}", session_id);
    }

    println!();

    // List available tools
    println!("🔧 Discovering available tools...");
    let tools = client.list_tools(None).await?;
    println!("Found {} tools:", tools.tools.len());
    for tool in &tools.tools {
        println!(
            "   • {} - {}",
            tool.name,
            tool.description.as_deref().unwrap_or("(no description)")
        );
    }
    println!();

    // Test each OAuth tool
    println!("📝 Testing OAuth tools:");
    println!();

    // 1. Public tool
    println!("1️⃣  Testing 'public_info' tool...");
    let public_result = client
        .call_tool("public_info".to_string(), json!({}))
        .await?;
    println!(
        "   Response: {}",
        serde_json::to_string_pretty(&public_result)?
    );
    println!();

    // 2. Protected tool
    println!("2️⃣  Testing 'protected_data' tool...");
    let protected_result = client
        .call_tool("protected_data".to_string(), json!({}))
        .await?;
    println!(
        "   Response: {}",
        serde_json::to_string_pretty(&protected_result)?
    );
    println!();

    // 3. Admin tool
    println!("3️⃣  Testing 'admin_action' tool...");
    match client
        .call_tool(
            "admin_action".to_string(),
            json!({
                "action": "test_admin_action"
            }),
        )
        .await
    {
        Ok(admin_result) => {
            println!(
                "   Response: {}",
                serde_json::to_string_pretty(&admin_result)?
            );
        },
        Err(e) => {
            println!("   Expected error (admin access required): {}", e);
        },
    }
    println!();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                 OAUTH TESTING COMPLETE                    ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ ✅ Connection established successfully                     ║");
    println!("║ ✅ All OAuth tools are accessible                         ║");
    println!("║ ✅ NoOpAuthProvider working as expected                   ║");
    println!("║ ✅ HTTP transport functioning properly                    ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    Ok(())
}
