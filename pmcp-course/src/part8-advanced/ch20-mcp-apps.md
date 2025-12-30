# MCP Apps: UI Resources

This chapter covers the experimental MCP Apps Extension (SEP-1865) for adding interactive user interfaces to MCP servers.

## Experimental Status Warning

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    ⚠️  EXPERIMENTAL FEATURE ⚠️                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  MCP Apps and UI Resources are EXPERIMENTAL and subject to change.      │
│                                                                         │
│  What This Means for You:                                               │
│  ════════════════════════                                               │
│                                                                         │
│  1. APIs MAY CHANGE                                                     │
│     The types, methods, and patterns shown here may evolve              │
│     significantly as the specification matures.                         │
│                                                                         │
│  2. LIMITED CLIENT SUPPORT                                              │
│     Most MCP clients (Claude Desktop, IDE plugins, etc.) do NOT         │
│     yet support UI resources. Your UIs may not render in all hosts.     │
│                                                                         │
│  3. ECOSYSTEM FRAGMENTATION                                             │
│     Until the spec stabilizes, different clients may implement          │
│     UI support differently, causing inconsistent behavior.              │
│                                                                         │
│  4. PMCP SDK WILL EVOLVE                                                │
│     As the MCP specification changes, PMCP SDK and cargo-pmcp           │
│     will update their APIs accordingly.                                 │
│                                                                         │
│  Recommendations:                                                       │
│  ═════════════════                                                      │
│  • Learn these patterns to understand where MCP is heading              │
│  • Experiment in development environments                               │
│  • Check client compatibility before production deployment              │
│  • Design servers to gracefully degrade when UI isn't supported         │
│  • Follow MCP specification updates for changes                         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Why MCP Apps?

Traditional MCP interactions are text-based: the AI requests a tool, gets JSON back, and describes results to the user. But some data is inherently visual:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Text vs Visual Information                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Text works well for:           Visual works better for:               │
│  ═════════════════════          ═════════════════════════              │
│  • Status messages              • Image galleries                       │
│  • Simple data retrieval        • Interactive maps                      │
│  • Configuration values         • Charts and dashboards                 │
│  • Error messages               • Forms with validation                 │
│  • List of items                • Data grids with sorting               │
│                                 • Real-time visualizations              │
│                                                                         │
│  Example: Hotel Room Lookup                                             │
│  ──────────────────────────                                             │
│                                                                         │
│  TEXT RESPONSE:                 UI RESPONSE:                            │
│  ┌─────────────────────┐        ┌─────────────────────────────────┐     │
│  │ Deluxe King Room    │        │ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ │     │
│  │ - City views        │   vs   │ │ 📷  │ │ 📷  │ │ 📷  │ │ 📷  │ │     │
│  │ - Marble bathroom   │        │ └─────┘ └─────┘ └─────┘ └─────┘ │     │
│  │ - See photos at...  │        │   Click to expand any image     │     │
│  └─────────────────────┘        └─────────────────────────────────┘     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

MCP Apps solve this by allowing servers to declare UI resources that clients can render, enabling rich visual experiences while maintaining the tool abstraction.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      MCP Apps Architecture                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌───────────────┐                                                      │
│  │  MCP Server   │                                                      │
│  │               │                                                      │
│  │  ┌─────────┐  │  1. Declare UI Resources                            │
│  │  │ Tool    │──────────────────────────────┐                         │
│  │  │ with_ui │  │                           │                         │
│  │  └─────────┘  │                           │                         │
│  │               │                           ▼                         │
│  │  ┌─────────┐  │                   ┌──────────────┐                  │
│  │  │ UI      │──────────────────────│  MCP Host    │                  │
│  │  │ Resource│  │  2. Provide HTML  │  (Client)    │                  │
│  │  └─────────┘  │                   │              │                  │
│  └───────────────┘                   │  ┌────────┐  │                  │
│                                      │  │Sandboxed  │                  │
│         3. Tool Result ─────────────►│  │ iframe  │  │                  │
│                                      │  │        │  │                  │
│         ◄────────── 4. UI Callback   │  │ (UI)   │  │                  │
│                      (postMessage)   │  └────────┘  │                  │
│                                      └──────────────┘                  │
│                                                                         │
│  Key Concepts:                                                          │
│  ═════════════                                                          │
│  • UI Resources use `ui://` scheme (like `file://` or `http://`)       │
│  • Tools link to UIs via `.with_ui("ui://path")`                       │
│  • HTML renders in sandboxed iframe for security                        │
│  • Communication uses JSON-RPC over postMessage                        │
│  • MIME type: `text/html+mcp` (HTML with MCP extensions)               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Graceful Degradation

Because client support is limited, design servers that work with or without UI:

```rust
use pmcp::{TypedTool, Server, ResourceCollection, UIResourceBuilder};

// The tool ALWAYS returns usable JSON data
async fn get_venues(args: GetVenuesArgs, _: RequestHandlerExtra) -> pmcp::Result<Value> {
    let venues = fetch_venues(&args.conference_id).await?;

    // Return complete data that works without UI
    Ok(serde_json::json!({
        "conference": args.conference_id,
        "venue_count": venues.len(),
        "venues": venues.iter().map(|v| {
            serde_json::json!({
                "name": v.name,
                "address": v.address,
                "capacity": v.capacity,
                "coordinates": {
                    "lat": v.lat,
                    "lon": v.lon
                }
            })
        }).collect::<Vec<_>>()
    }))
}

// UI enhancement is OPTIONAL - tool works perfectly without it
let tool = TypedTool::new("get_venues", get_venues)
    .with_description("Get venue information for a conference")
    .with_ui("ui://conference/venue-map");  // UI is bonus, not requirement
```

Clients that support UI render the interactive map. Clients that don't still receive the complete venue data and can describe it textually.

## Chapter Contents

This chapter covers three aspects of MCP Apps:

1. **[UI Resources](./ch20-01-ui-resources.md)** - Creating and managing UI resources
   - The `ui://` URI scheme
   - UIResourceBuilder fluent API
   - HTML templates with `text/html+mcp`

2. **[Tool-UI Association](./ch20-02-tool-ui-association.md)** - Linking tools to UIs
   - The `.with_ui()` method
   - Tool metadata for UI references
   - Resource collection integration

3. **[PostMessage Communication](./ch20-03-postmessage.md)** - Client-UI data flow
   - Sandboxed iframe security model
   - MCP JSON-RPC over postMessage
   - Bidirectional communication patterns

## When to Use MCP Apps

| Use Case | Recommendation | Why |
|----------|----------------|-----|
| Image galleries | Good fit | Thumbnails, lightbox, navigation |
| Interactive maps | Good fit | Leaflet/Mapbox integration |
| Data dashboards | Good fit | Charts, real-time updates |
| Complex forms | Consider | Validation, multi-step flows |
| Simple text data | Skip | Text response is sufficient |
| Mission-critical ops | Careful | Check client support first |

## Example: Hotel Gallery

Here's a complete example from the PMCP SDK:

```rust
use pmcp::{
    Server, TypedTool, ResourceCollection, UIResourceBuilder,
    ServerCapabilities, RequestHandlerExtra,
};

const GALLERY_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Room Gallery</title>
    <style>
        .gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 16px; }
        .gallery img { width: 100%; border-radius: 8px; cursor: pointer; }
    </style>
</head>
<body>
    <div class="gallery" id="gallery"></div>
    <script>
        // Listen for tool results from MCP host
        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const images = event.data.result.images;
                const gallery = document.getElementById('gallery');
                images.forEach(img => {
                    const el = document.createElement('img');
                    el.src = img.thumbnail_url;
                    el.onclick = () => window.open(img.url);
                    gallery.appendChild(el);
                });
            }
        });

        // Request data from the tool
        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: { name: 'get_room_images', arguments: { hotel_id: 'grand-resort' } },
            id: 1
        }, '*');
    </script>
</body>
</html>"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create UI resource
    let (ui_resource, ui_contents) = UIResourceBuilder::new(
        "ui://hotel/room-gallery",
        "Room Photo Gallery"
    )
        .description("Interactive photo gallery for hotel rooms")
        .html_template(GALLERY_HTML)
        .build_with_contents()?;

    // Create resource collection with UI
    let resources = ResourceCollection::new()
        .add_ui_resource(ui_resource, ui_contents);

    // Create tool with UI association
    let tool = TypedTool::new("get_room_images", get_room_images)
        .with_description("Get room photos")
        .with_ui("ui://hotel/room-gallery");

    // Build server
    let server = Server::builder()
        .name("hotel-gallery")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_room_images", tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

## Future Directions

The MCP Apps specification mentions potential future MIME types:

| MIME Type | Status | Description |
|-----------|--------|-------------|
| `text/html+mcp` | Current | HTML in sandboxed iframe |
| `application/wasm+mcp` | Future | WebAssembly modules |
| `application/x-remote-dom+mcp` | Future | Server-rendered UI (like LiveView) |

These would enable:
- **WASM**: High-performance visualizations, complex calculations client-side
- **Remote DOM**: Server-controlled UI without shipping code to client

For now, focus on HTML-based UIs and watch the specification for updates.

---

*Continue to [UI Resources](./ch20-01-ui-resources.md) →*

