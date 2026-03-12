# UI Resources and Widget Registration

In this section, you'll learn how to register widget HTML as MCP resources with the correct MIME type, declare Content Security Policy for external domains, and implement the resource handler pattern that connects widgets to the MCP protocol.

## Learning Objectives

By the end of this section, you will be able to:

- Register widget HTML resources using `UIResource::html_mcp_app()` with the correct MIME type
- Serve widget content using `UIResourceContents::html()` for `resources/read`
- Declare CSP for external domains using `WidgetCSP`
- Use `ResourceCollection` to manage widget resources
- Explain why `text/html;profile=mcp-app` is required for Claude Desktop

## The Correct MIME Type

Widgets must use the MIME type `text/html;profile=mcp-app` to be recognized by Claude Desktop, ChatGPT, and other MCP hosts. The SDK constructors produce this automatically:

```rust
use pmcp::types::ui::{UIResource, UIResourceContents};

// For resources/list -- declares the resource
let resource = UIResource::html_mcp_app(
    "ui://my-app/explorer.html",
    "Image Explorer",
);
// resource.mime_type == "text/html;profile=mcp-app"

// For resources/read -- serves the content
let contents = UIResourceContents::html(
    "ui://my-app/explorer.html",
    &html_content,  // your widget HTML string
);
// contents.mime_type == "text/html;profile=mcp-app"
```

Both `UIResource::html_mcp_app()` and `UIResourceContents::html()` produce `mimeType: "text/html;profile=mcp-app"` -- the standard MIME type recognized across all MCP hosts.

> **Warning:** Do not use the legacy `UIResource::html_mcp()` constructor -- it produces `text/html+mcp` which is **not recognized by Claude Desktop**. Always use `html_mcp_app()` for new code.

**Try this:** Look at the type signature of `UIResource::html_mcp_app()` in your IDE. Notice it takes two parameters: the resource URI (always `ui://` scheme) and a display name. The MIME type is set automatically -- you never need to specify it manually.

## Registering Widget Resources

The `ResourceCollection` helper manages widget resource registration. It pairs the resource declaration (for `resources/list`) with the content (for `resources/read`):

```rust
use pmcp::types::ui::{UIResource, UIResourceContents, ResourceCollection};

// Create the collection
let mut resources = ResourceCollection::new();

// Register a widget
let resource = UIResource::html_mcp_app(
    "ui://my-app/explorer.html",
    "Image Explorer",
);

let contents = UIResourceContents::html(
    "ui://my-app/explorer.html",
    &html_content,
);

resources.add_ui_resource(resource, contents);
```

When the host calls `resources/list`, the collection returns all registered `UIResource` entries. When the host calls `resources/read` with a specific URI, the collection returns the matching `UIResourceContents`.

### Embedding Widget HTML

Widget HTML is typically embedded in the Rust binary at compile time using `include_str!`:

```rust
const WIDGET_HTML: &str = include_str!("../../widget/dist/explorer.html");
```

The `include_str!` macro reads the file at compile time, so the built widget HTML must exist before `cargo build` runs. This is why you build the widget first (`cd widget && npm run build`) then build the Rust server.

## Declaring CSP for External Domains

If your widget loads external resources (images, API calls, fonts), you **must** declare them using `WidgetCSP`. Without this, hosts like Claude.ai block all external domains via Content-Security-Policy.

```rust
use pmcp::types::mcp_apps::{WidgetCSP, WidgetMeta};

let csp = WidgetCSP::new()
    .resources("https://*.staticflickr.com")  // img-src: images, scripts, fonts
    .connect("https://*.staticflickr.com");   // connect-src: fetch/XHR

let meta = WidgetMeta::new()
    .resource_uri("ui://my-app/explorer.html")
    .prefers_border(true)
    .csp(csp);
```

This produces `_meta.ui.csp` with `connectDomains` and `resourceDomains` arrays on the `resources/read` response. The host merges these into its iframe CSP, allowing your widget to load resources from the declared domains.

### What Each CSP Method Controls

| Method | CSP Directive | Use Case |
|--------|--------------|----------|
| `.resources("https://cdn.example.com")` | `img-src`, `script-src`, `font-src` | Loading images, scripts, fonts from CDN |
| `.connect("https://api.example.com")` | `connect-src` | `fetch()` and `XMLHttpRequest` calls |

> **Important:** CSP metadata goes on the **resource contents** (returned by `resources/read`), not just the resource listing. The host applies CSP when it renders the widget iframe.

> **Important:** Always use HTTPS for external domains -- `http://` URLs are blocked by mixed-content policy even with CSP declarations.

**Try this:** If your widget loads images from an external CDN, add a `WidgetCSP` with the CDN domain. Run `cargo pmcp preview` and check the Protocol tab to verify the CSP metadata appears in the `resources/read` response.

## The Resource Handler Pattern

Every MCP Apps server needs a resource handler that connects widget HTML to the MCP protocol. The pattern has two methods: `list()` for discovery and `read()` for serving content.

### Implementing resources/list

The `list()` method returns all available widget resources:

```rust
use pmcp::types::protocol::{ListResourcesResult, ResourceInfo};
use pmcp::types::ui::UIResource;

// Return all registered widget resources
fn list_resources(&self) -> ListResourcesResult {
    let resources = vec![
        UIResource::html_mcp_app(
            "ui://my-app/explorer.html",
            "Image Explorer",
        ).into(),
    ];
    ListResourcesResult::new(resources)
}
```

### Implementing resources/read

The `read()` method extracts the widget name from the URI and returns the HTML content:

```rust
use pmcp::types::ui::UIResourceContents;

fn read_resource(&self, uri: &str) -> Result<ReadResourceResult> {
    match uri {
        "ui://my-app/explorer.html" => {
            let contents = UIResourceContents::html(
                uri,
                WIDGET_HTML,  // embedded at compile time
            );
            Ok(ReadResourceResult::from(contents))
        }
        _ => Err(Error::resource_not_found(uri)),
    }
}
```

### The Three-Step Read Pattern

Every resource read follows three steps:

| Step | What Happens | Code |
|------|-------------|------|
| 1 | Extract widget name from `ui://app/{name}` URI | `uri.strip_prefix("ui://my-app/")` |
| 2 | Look up the widget HTML content | `include_str!` or `ResourceCollection` |
| 3 | Return as `UIResourceContents` with correct MIME type | `UIResourceContents::html(uri, &html)` |

This pattern is consistent across all MCP Apps servers. Whether you have one widget or ten, the structure is the same.

## Complete Example: Registering a Widget

Here is a complete example combining resource registration with the server builder:

```rust
use pmcp::types::mcp_apps::HostType;
use pmcp::types::protocol::ToolInfo;
use pmcp::types::ui::{UIResource, UIResourceContents, ResourceCollection};
use serde_json::json;

// 1. Embed widget HTML at compile time
const EXPLORER_HTML: &str = include_str!("../../widget/dist/explorer.html");

// 2. Create resource collection
let mut resources = ResourceCollection::new();

let resource = UIResource::html_mcp_app(
    "ui://my-app/explorer.html",
    "Image Explorer",
);

let contents = UIResourceContents::html(
    "ui://my-app/explorer.html",
    EXPLORER_HTML,
);

resources.add_ui_resource(resource, contents);

// 3. Build server with host layer and resources
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .with_host_layer(HostType::ChatGpt)  // multi-host support
    .resources(resources)
    .build()?;
```

## Hot-Reload During Development

During development, you can read widget HTML from disk instead of embedding it:

```rust
use std::fs;

fn read_resource(&self, uri: &str) -> Result<ReadResourceResult> {
    // Read from disk on every request -- changes appear on browser refresh
    let html = fs::read_to_string("widget/dist/explorer.html")?;
    let contents = UIResourceContents::html(uri, &html);
    Ok(ReadResourceResult::from(contents))
}
```

Reading from disk on every request enables hot-reload: edit your widget HTML, refresh the browser, and see changes instantly. No server restart needed.

**When do you need to restart the server?** Only when you change Rust code (tool handlers, `main.rs`). Widget HTML changes during development are always instant -- just refresh the browser.

## Summary and Next Steps

Let's recap what you've learned:

- **`UIResource::html_mcp_app()`** creates resource declarations with the correct MIME type (`text/html;profile=mcp-app`)
- **`UIResourceContents::html()`** creates resource content for serving widget HTML
- **`WidgetCSP`** declares external domains that widgets need to load resources from
- **`ResourceCollection`** manages the pairing of resource declarations and content
- **The three-step read pattern** is consistent across all MCP Apps servers: extract name, look up content, return with correct MIME type
- **Hot-reload** works by reading widget HTML from disk on every request during development

In the next section, you'll learn how to associate tools with widgets using `ToolInfo::with_ui()` and return `structuredContent` for widget rendering.

---

*Continue to [Tool-UI Association and Data Flow](./ch20-02-tool-ui-association.md) ->*
