//! Example: Server with resource support
//!
//! This example demonstrates:
//! - Creating a server that provides resources
//! - Using StaticResource for fixed content
//! - Using ResourceCollection for managing multiple resources
//! - Using DynamicResourceHandler for dynamic content
//! - Resource URIs and MIME types

use pmcp::{types::capabilities::ServerCapabilities, ResourceCollection, Server, StaticResource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .init();

    println!("=== MCP Server Resources Example ===");
    println!("Starting server with various resource types...\n");

    // Create a collection of static resources
    let mut resources = ResourceCollection::new()
        // Add text resources
        .add_resource(
            StaticResource::new_text(
                "config://app/settings.json",
                r#"{"theme": "dark", "language": "en", "debug": false}"#,
            )
            .with_name("Application Settings")
            .with_description("Main application configuration")
            .with_mime_type("application/json"),
        )
        .add_resource(
            StaticResource::new_text(
                "doc://readme.md",
                "# Welcome to MCP\n\nThis is a sample markdown document.\n\n## Features\n- Fast\n- Reliable\n- Extensible",
            )
            .with_name("README")
            .with_description("Project documentation")
            .with_mime_type("text/markdown"),
        )
        .add_resource(
            StaticResource::new_text(
                "template://email/welcome.html",
                r#"<html>
<head><title>Welcome</title></head>
<body>
    <h1>Welcome to our service!</h1>
    <p>Thank you for joining us.</p>
</body>
</html>"#,
            )
            .with_name("Welcome Email Template")
            .with_description("HTML template for welcome emails")
            .with_mime_type("text/html"),
        );

    // Add an image resource (using a simple 1x1 red pixel PNG as example)
    let red_pixel_png = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77,
        0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x99, 0x63, 0xF8, 0xCF,
        0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x5B, 0x71, 0xB7, 0x34, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    resources = resources.add_resource(
        StaticResource::new_image("image://logo.png", &red_pixel_png, "image/png")
            .with_name("Company Logo")
            .with_description("A simple red pixel logo for demonstration"),
    );

    // Build server with resource support
    // Note: In a real application, you would typically use either static resources OR dynamic resources,
    // not both. This example shows static resources, but you could replace it with the dynamic_handler.
    let server = Server::builder()
        .name("resource-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::resources_only())
        .resources(resources)
        .build()?;

    println!("Server ready! Resources will be listed via the list_resources protocol method.");
    println!("\nExample resource URIs:");
    println!("  - config://app/settings.json");
    println!("  - doc://readme.md");
    println!("  - template://email/welcome.html");
    println!("  - image://logo.png");
    println!("\nListening on stdio...");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
