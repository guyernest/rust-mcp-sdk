# UI Resources

UI resources are the foundation of MCP Apps. They define reusable interface templates that MCP hosts can render when tools are invoked.

## The `ui://` URI Scheme

Just as `http://` identifies web resources and `file://` identifies local files, `ui://` identifies MCP UI resources:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        UI Resource URI Anatomy                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│      ui://hotel/room-gallery                                           │
│      │    │     │                                                      │
│      │    │     └─── Resource name (the specific UI)                   │
│      │    └───────── Namespace (organizational grouping)               │
│      └────────────── Scheme (always "ui")                              │
│                                                                         │
│  Good URI Examples:              Bad URI Examples:                      │
│  ═════════════════               ════════════════                       │
│  ui://charts/sales-dashboard     http://mysite.com/ui    (wrong scheme) │
│  ui://maps/venue-locator         ui://                   (no path)      │
│  ui://forms/user-profile         file:///ui/form.html    (wrong scheme) │
│  ui://hotel/room-gallery         ui:/hotel/gallery       (missing /)    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

The URI scheme serves two purposes:
1. **Identification**: Uniquely identifies the UI within your server
2. **Tool Association**: Links tools to their UIs via the `.with_ui()` method

## UIResource Structure

The core type for declaring UI resources:

```rust
use pmcp::types::ui::{UIResource, UIMimeType};

// Manual construction
let resource = UIResource {
    uri: "ui://hotel/room-gallery".to_string(),
    name: "Hotel Room Gallery".to_string(),
    description: Some("Interactive photo gallery for hotel rooms".to_string()),
    mime_type: "text/html+mcp".to_string(),
};

// Or using the constructor
let resource = UIResource::new(
    "ui://hotel/room-gallery",
    "Hotel Room Gallery",
    UIMimeType::HtmlMcp
)
.with_description("Interactive photo gallery for hotel rooms");
```

### Fields Explained

| Field | Required | Description |
|-------|----------|-------------|
| `uri` | Yes | Must start with `ui://`, identifies the resource |
| `name` | Yes | Human-readable display name |
| `description` | No | Explains what the UI does |
| `mime_type` | Yes | Currently only `text/html+mcp` |

## UIResourceBuilder

For a more ergonomic API, use the builder pattern:

```rust
use pmcp::UIResourceBuilder;

// Build just the resource declaration
let resource = UIResourceBuilder::new("ui://charts/sales", "Sales Dashboard")
    .description("Real-time sales analytics dashboard")
    .html_template(DASHBOARD_HTML)
    .build()?;

// Build both resource and contents (most common pattern)
let (resource, contents) = UIResourceBuilder::new("ui://charts/sales", "Sales Dashboard")
    .description("Real-time sales analytics dashboard")
    .html_template(DASHBOARD_HTML)
    .build_with_contents()?;
```

### Builder Methods

```rust
use pmcp::UIResourceBuilder;
use pmcp::types::ui::UIMimeType;

let builder = UIResourceBuilder::new("ui://example", "Example UI")
    // Set description (optional)
    .description("What this UI does")

    // Set MIME type (defaults to HtmlMcp)
    .mime_type(UIMimeType::HtmlMcp)

    // Provide HTML content - choose one:
    .html_template(r#"<html>...</html>"#)  // Inline string
    // OR
    .html_file(include_str!("../ui/dashboard.html"));  // From file
```

### Validation

The builder validates your UI resource:

```rust
// Error: URI doesn't start with ui://
let result = UIResourceBuilder::new("http://wrong", "Test")
    .html_template("<html></html>")
    .build();
assert!(result.is_err());  // "must start with 'ui://'"

// Error: No content provided
let result = UIResourceBuilder::new("ui://test", "Test")
    .build();  // Missing html_template or html_file
assert!(result.is_err());  // "content must be set"

// Error: Empty path
let result = UIResourceBuilder::new("ui://", "Test")
    .html_template("<html></html>")
    .build();
assert!(result.is_err());  // "must have a path after 'ui://'"
```

## HTML Templates

UI resources use HTML with the `text/html+mcp` MIME type. This is standard HTML with some conventions for MCP communication:

```rust
const GALLERY_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Photo Gallery</title>
    <style>
        /* Your CSS here */
        .gallery {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 16px;
        }
    </style>
</head>
<body>
    <div id="content">Loading...</div>

    <script>
        // MCP communication (covered in ch20-03)
        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                renderData(event.data.result);
            }
        });

        function renderData(data) {
            // Update the UI with tool results
        }
    </script>
</body>
</html>"#;
```

### Template Best Practices

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    HTML Template Guidelines                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ✓ DO:                                                                  │
│  ═════                                                                  │
│  • Include viewport meta tag for responsive design                      │
│  • Use system fonts for consistency with host application               │
│  • Handle the 'mcp-tool-result' message event                          │
│  • Show loading states while waiting for data                           │
│  • Use relative units (rem, %) instead of fixed pixels                 │
│  • Test in multiple MCP clients                                         │
│                                                                         │
│  ✗ DON'T:                                                               │
│  ═══════                                                                │
│  • Rely on cookies (sandboxed iframe restrictions)                      │
│  • Access parent window directly (security blocked)                     │
│  • Use localStorage (may not persist)                                   │
│  • Make assumptions about iframe size                                   │
│  • Include sensitive data in the template                               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Loading from External Files

For larger UIs, store HTML in separate files:

```rust
// In your Rust code, use include_str! at compile time
const DASHBOARD_HTML: &str = include_str!("../ui/dashboard.html");

let (resource, contents) = UIResourceBuilder::new("ui://dashboard", "Dashboard")
    .html_template(DASHBOARD_HTML)  // Content embedded at compile time
    .build_with_contents()?;
```

Project structure:

```
my-server/
├── src/
│   └── main.rs
├── ui/
│   ├── dashboard.html      ← Complex HTML/CSS/JS
│   ├── gallery.html
│   └── map.html
└── Cargo.toml
```

This keeps your Rust code clean while allowing full IDE support for HTML/CSS/JS development.

## UIResourceContents

When the MCP host requests a UI resource, you provide `UIResourceContents`:

```rust
use pmcp::types::ui::UIResourceContents;

// For HTML content
let contents = UIResourceContents::html(
    "ui://hotel/room-gallery",
    "<html><body>Gallery content</body></html>"
);

// The struct fields:
// contents.uri        - The resource URI
// contents.mime_type  - "text/html+mcp"
// contents.text       - Some(html_string) for text content
// contents.blob       - None (used for future binary formats like WASM)
```

## Adding to Resource Collection

UI resources integrate with PMCP's resource system:

```rust
use pmcp::{ResourceCollection, UIResourceBuilder};

// Build the UI resource
let (gallery_resource, gallery_contents) = UIResourceBuilder::new(
    "ui://hotel/gallery",
    "Room Gallery"
)
    .html_template(GALLERY_HTML)
    .build_with_contents()?;

let (map_resource, map_contents) = UIResourceBuilder::new(
    "ui://hotel/map",
    "Property Map"
)
    .html_template(MAP_HTML)
    .build_with_contents()?;

// Add to resource collection
let resources = ResourceCollection::new()
    .add_ui_resource(gallery_resource, gallery_contents)
    .add_ui_resource(map_resource, map_contents);

// Use in server
let server = Server::builder()
    .name("hotel-server")
    .version("1.0.0")
    .resources(resources)
    // ... tools, etc.
    .build()?;
```

## Complete Example

Putting it all together:

```rust
use pmcp::{Server, ServerCapabilities, ResourceCollection, UIResourceBuilder};

const SALES_CHART_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sales Chart</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body { font-family: system-ui; padding: 20px; }
        .chart-container { max-width: 800px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="chart-container">
        <canvas id="salesChart"></canvas>
    </div>
    <script>
        let chart = null;

        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;
                renderChart(data.labels, data.values);
            }
        });

        function renderChart(labels, values) {
            const ctx = document.getElementById('salesChart');
            if (chart) chart.destroy();

            chart = new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: labels,
                    datasets: [{
                        label: 'Sales',
                        data: values,
                        backgroundColor: 'rgba(54, 162, 235, 0.5)',
                        borderColor: 'rgba(54, 162, 235, 1)',
                        borderWidth: 1
                    }]
                }
            });
        }

        // Request data from the tool
        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: { name: 'get_sales_data', arguments: {} },
            id: 1
        }, '*');
    </script>
</body>
</html>"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the UI resource
    let (ui_resource, ui_contents) = UIResourceBuilder::new(
        "ui://analytics/sales-chart",
        "Sales Analytics Chart"
    )
        .description("Interactive bar chart showing sales by category")
        .html_template(SALES_CHART_HTML)
        .build_with_contents()?;

    // Build resource collection
    let resources = ResourceCollection::new()
        .add_ui_resource(ui_resource, ui_contents);

    // Server needs resources capability
    let server = Server::builder()
        .name("analytics-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        // Tools added here...
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

## Summary

| Component | Purpose |
|-----------|---------|
| `UIResource` | Declares a UI with URI, name, description, MIME type |
| `UIResourceBuilder` | Fluent API for creating UI resources |
| `UIResourceContents` | The actual HTML content delivered to hosts |
| `ResourceCollection` | Container for integrating UIs into servers |
| `ui://` scheme | Standard URI format for UI resources |

---

*Continue to [Tool-UI Association](./ch20-02-tool-ui-association.md) →*

