# Chapter 12.5: MCP Apps Extension — Interactive UIs

The MCP Apps Extension lets your server provide rich, interactive user interfaces alongside tools—turning your MCP server into a full application platform. Think charts, maps, galleries, and custom dashboards that run securely in the host and communicate seamlessly with your Rust backend.

This chapter shows you how to build interactive UIs that elevate your MCP server from pure API to complete user experience.

## Quick Start: Your First Interactive UI (30 lines)

Let's build a simple data viewer with an interactive UI:

```rust
use pmcp::{Server, TypedTool, UIResourceBuilder, ResourceCollection, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct GetDataArgs {
    query: String,
}

async fn get_data(args: GetDataArgs, _extra: pmcp::RequestHandlerExtra)
    -> pmcp::Result<serde_json::Value>
{
    Ok(serde_json::json!({
        "results": vec!["Apple", "Banana", "Cherry"],
        "count": 3
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create UI resource with embedded HTML
    let (ui_resource, ui_contents) = UIResourceBuilder::new(
        "ui://app/viewer",
        "Data Viewer"
    )
    .html_template(r#"
        <html><body>
            <h1>Data Viewer</h1>
            <div id="results"></div>
            <script>
                window.addEventListener('message', (e) => {
                    if (e.data.type === 'mcp-tool-result') {
                        document.getElementById('results').innerHTML =
                            e.data.result.results.join(', ');
                    }
                });
                window.parent.postMessage({
                    jsonrpc: '2.0',
                    method: 'tools/call',
                    params: { name: 'get_data', arguments: { query: 'fruits' } },
                    id: 1
                }, '*');
            </script>
        </body></html>
    "#)
    .build_with_contents()?;

    // Create tool with UI association
    let tool = TypedTool::new("get_data", |args, extra| {
        Box::pin(get_data(args, extra))
    })
    .with_ui("ui://app/viewer");

    // Build server with resources and tool
    let server = Server::builder()
        .name("data-viewer")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(ResourceCollection::new().add_ui_resource(ui_resource, ui_contents))
        .tool("get_data", tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

**Test it:**
```bash
# In Cargo.toml, enable schema-generation feature:
# pmcp = { version = "1.8", features = ["schema-generation"] }

cargo run

# In Claude Desktop or another MCP host, the tool will show
# an interactive UI when called!
```

That's it! You've created an MCP server with an interactive UI. The UI runs securely in a sandboxed iframe and communicates with your Rust backend via JSON-RPC.

---

## Understanding MCP Apps Extension

### The Three-Piece Architecture

MCP Apps Extension connects three components:

```
┌─────────────────┐      MCP Protocol      ┌──────────────────┐
│   MCP Host      │◄─────JSON-RPC─────────►│  Your Rust       │
│  (Claude, IDE)  │                        │  Server          │
└────────┬────────┘                        └──────────────────┘
         │                                           │
         │ Renders UI                                │ Declares UI
         │ in iframe                                 │ Resources
         ▼                                           │
┌─────────────────┐                                 │
│  UI (HTML/JS)   │                                 │
│  Sandboxed      │                                 │
└─────────────────┘                                 │
         │                                           │
         └──── postMessage (JSON-RPC) ──────────────┘
              Calls tools, receives results
```

1. **Your Rust Server**: Declares UI resources and tools with `.with_ui()`
2. **MCP Host**: Renders the UI in a sandboxed iframe
3. **UI (HTML/JavaScript)**: Calls tools via `postMessage`, renders results

### When to Use MCP Apps

Use MCP Apps when you need:

- **Visualization**: Charts, graphs, maps that are easier to understand visually
- **Rich interaction**: Galleries, forms, dashboards with complex user input
- **Real-time updates**: Live data feeds, monitoring dashboards
- **Complex layouts**: Multi-panel interfaces that don't fit in text

**Don't use MCP Apps** for:
- Simple data retrieval (use regular tools)
- Text-based Q&A (use prompts)
- File access (use resources)

---

## Core Concepts

### UI Resources

UI Resources are HTML templates declared with the `ui://` URI scheme:

```rust
use pmcp::UIResourceBuilder;

let (ui_resource, ui_contents) = UIResourceBuilder::new(
    "ui://myapp/dashboard",        // Unique URI
    "Analytics Dashboard"           // Display name
)
.description("Real-time analytics")
.html_template(include_str!("dashboard.html"))  // Or inline HTML
.build_with_contents()?;
```

**Key points:**
- Use `ui://` scheme (required)
- MIME type is `text/html+mcp` (automatic)
- HTML can be inline or from file
- Multiple UIs per server supported

### Tool-UI Association

Associate tools with UIs using `.with_ui()`:

```rust
let analytics_tool = TypedTool::new("analyze", |args, extra| {
    Box::pin(analyze_data(args, extra))
})
.with_description("Analyze business metrics")
.with_ui("ui://myapp/dashboard");  // Link to UI
```

When the host calls this tool, it displays the associated UI.

### Communication Protocol

The UI communicates via MCP JSON-RPC over `postMessage`:

**From UI to Server** (call a tool):
```javascript
window.parent.postMessage({
    jsonrpc: '2.0',
    method: 'tools/call',
    params: {
        name: 'analyze',
        arguments: { metric: 'sales', period: '30d' }
    },
    id: 1
}, '*');
```

**From Server to UI** (receive result):
```javascript
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        const data = event.data.result;
        // Render data in your UI
        renderChart(data);
    }
});
```

---

## Example 1: Interactive Conference Map

Let's build a real-world example: an interactive map showing conference venues.

**Goal**: Display multiple conference venues on a map with popups showing details.

### Step 1: Define Data Types

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct GetVenuesArgs {
    conference_id: String,
}

#[derive(Debug, Serialize)]
struct Venue {
    id: String,
    name: String,
    description: String,
    lat: f64,
    lon: f64,
    capacity: u32,
}

#[derive(Debug, Serialize)]
struct VenuesResult {
    conference: String,
    venues: Vec<Venue>,
}
```

### Step 2: Implement Tool Handler

```rust
async fn get_conference_venues(
    args: GetVenuesArgs,
    _extra: pmcp::RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // In production, fetch from database
    let venues = match args.conference_id.as_str() {
        "aws-reinvent-2025" => vec![
            Venue {
                id: "mandalay-bay".to_string(),
                name: "Mandalay Bay Convention Center".to_string(),
                description: "Main keynote venue".to_string(),
                lat: 36.0915,
                lon: -115.1739,
                capacity: 20000,
            },
            Venue {
                id: "venetian".to_string(),
                name: "The Venetian".to_string(),
                description: "Breakout sessions".to_string(),
                lat: 36.1212,
                lon: -115.1697,
                capacity: 5000,
            },
        ],
        _ => vec![],
    };

    Ok(serde_json::to_value(VenuesResult {
        conference: args.conference_id,
        venues,
    })?)
}
```

### Step 3: Create Interactive Map UI

We'll use Leaflet.js for the map. Create a file `venue_map.html`:

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Conference Venue Map</title>
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <style>
        body { margin: 0; padding: 0; }
        #map { height: 100vh; width: 100%; }
        .venue-popup h3 { margin: 0 0 8px 0; color: #2c3e50; }
        .venue-popup p { margin: 4px 0; color: #555; }
        .capacity { font-weight: bold; color: #3498db; }
    </style>
</head>
<body>
    <div id="map"></div>
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <script>
        // Initialize map centered on Las Vegas
        const map = L.map('map').setView([36.1147, -115.1728], 12);

        // Add map tiles
        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '© OpenStreetMap contributors'
        }).addTo(map);

        // Listen for tool results
        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;
                const bounds = [];

                // Add marker for each venue
                data.venues.forEach(venue => {
                    const marker = L.marker([venue.lat, venue.lon]).addTo(map);
                    bounds.push([venue.lat, venue.lon]);

                    // Create popup
                    marker.bindPopup(`
                        <div class="venue-popup">
                            <h3>${venue.name}</h3>
                            <p>${venue.description}</p>
                            <p class="capacity">
                                Capacity: ${venue.capacity.toLocaleString()}
                            </p>
                        </div>
                    `);
                });

                // Fit map to show all venues
                if (bounds.length > 0) {
                    map.fitBounds(bounds, { padding: [50, 50] });
                }
            }
        });

        // Request venue data
        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'get_conference_venues',
                arguments: { conference_id: 'aws-reinvent-2025' }
            },
            id: 1
        }, '*');
    </script>
</body>
</html>
```

### Step 4: Wire It Together

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create UI resource
    let (ui_resource, ui_contents) = UIResourceBuilder::new(
        "ui://conference/venue-map",
        "Conference Venue Map"
    )
    .description("Interactive map showing all conference venues")
    .html_template(include_str!("venue_map.html"))
    .build_with_contents()?;

    // Create resource collection
    let resources = ResourceCollection::new()
        .add_ui_resource(ui_resource, ui_contents);

    // Create tool with UI association
    let tool = TypedTool::new("get_conference_venues", |args, extra| {
        Box::pin(get_conference_venues(args, extra))
    })
    .with_description("Get venues with location data")
    .with_ui("ui://conference/venue-map");

    // Build server
    let server = Server::builder()
        .name("conference-map-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_conference_venues", tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

**Run it:**
```bash
cargo run --features schema-generation
```

**Full example:** See [`examples/conference_venue_map.rs`](../../examples/conference_venue_map.rs)

---

## Example 2: Hotel Room Gallery

Build an image gallery with lightbox functionality.

**Goal**: Show multiple hotel room photos in a responsive grid with full-size view.

### Step 1: Define Types

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GetRoomImagesArgs {
    hotel_id: String,
    room_type: String,  // "deluxe", "suite", etc.
}

#[derive(Debug, Serialize)]
struct RoomImage {
    id: String,
    url: String,              // Full-size image
    thumbnail_url: String,    // Thumbnail
    title: String,
    description: String,
}

#[derive(Debug, Serialize)]
struct RoomGalleryResult {
    hotel: String,
    room_type: String,
    images: Vec<RoomImage>,
}
```

### Step 2: Tool Handler

```rust
async fn get_room_images(
    args: GetRoomImagesArgs,
    _extra: pmcp::RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    let images = vec![
        RoomImage {
            id: "room-1".to_string(),
            url: "https://images.unsplash.com/photo-1590490360182-c33d57733427?w=1200"
                .to_string(),
            thumbnail_url:
                "https://images.unsplash.com/photo-1590490360182-c33d57733427?w=400"
                .to_string(),
            title: "Deluxe King Room".to_string(),
            description: "Spacious room with city views".to_string(),
        },
        // ... more images
    ];

    Ok(serde_json::to_value(RoomGalleryResult {
        hotel: args.hotel_id,
        room_type: args.room_type,
        images,
    })?)
}
```

### Step 3: Gallery UI

The UI uses CSS Grid for responsive layout and a lightbox for full-size viewing:

```html
<!DOCTYPE html>
<html>
<head>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: system-ui, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }
        .gallery {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 20px;
        }
        .gallery-item {
            background: white;
            border-radius: 12px;
            overflow: hidden;
            cursor: pointer;
            transition: transform 0.2s;
        }
        .gallery-item:hover {
            transform: translateY(-4px);
            box-shadow: 0 4px 16px rgba(0,0,0,0.15);
        }
        .gallery-item img {
            width: 100%;
            height: 200px;
            object-fit: cover;
        }
        .lightbox {
            display: none;
            position: fixed;
            top: 0; left: 0;
            width: 100%; height: 100%;
            background: rgba(0,0,0,0.9);
            align-items: center;
            justify-content: center;
        }
        .lightbox.active { display: flex; }
    </style>
</head>
<body>
    <h1 id="title">Hotel Gallery</h1>
    <div class="gallery" id="gallery"></div>

    <div class="lightbox" id="lightbox" onclick="closeLightbox()">
        <img id="lightbox-img" src="">
    </div>

    <script>
        let images = [];

        function openLightbox(index) {
            document.getElementById('lightbox-img').src = images[index].url;
            document.getElementById('lightbox').classList.add('active');
        }

        function closeLightbox() {
            document.getElementById('lightbox').classList.remove('active');
        }

        window.addEventListener('message', (e) => {
            if (e.data.type === 'mcp-tool-result') {
                const data = e.data.result;
                images = data.images;

                document.getElementById('title').textContent =
                    `${data.room_type} Room - ${data.hotel}`;

                const gallery = document.getElementById('gallery');
                images.forEach((img, i) => {
                    const item = document.createElement('div');
                    item.className = 'gallery-item';
                    item.onclick = () => openLightbox(i);
                    item.innerHTML = `
                        <img src="${img.thumbnail_url}" alt="${img.title}">
                        <div style="padding: 16px;">
                            <h3>${img.title}</h3>
                            <p>${img.description}</p>
                        </div>
                    `;
                    gallery.appendChild(item);
                });
            }
        });

        // Load images
        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'get_room_images',
                arguments: { hotel_id: 'grand-resort', room_type: 'deluxe' }
            },
            id: 1
        }, '*');
    </script>
</body>
</html>
```

**Full example:** See [`examples/hotel_gallery.rs`](../../examples/hotel_gallery.rs)

---

## Advanced Patterns

### Multiple Tool Calls

Coordinate multiple tool calls for complex UIs:

```javascript
async function callTool(name, args) {
    return new Promise((resolve) => {
        const id = Date.now() + Math.random();

        const handler = (event) => {
            if (event.data.id === id) {
                window.removeEventListener('message', handler);
                resolve(event.data.result);
            }
        };

        window.addEventListener('message', handler);

        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: { name, arguments: args },
            id
        }, '*');
    });
}

// Use it:
async function loadDashboard() {
    const [metrics, trends] = await Promise.all([
        callTool('get_metrics', { period: '30d' }),
        callTool('get_trends', { metric: 'sales' })
    ]);

    renderDashboard(metrics, trends);
}
```

### Real-Time Updates

Poll for live data:

```javascript
let updateInterval = setInterval(async () => {
    const data = await callTool('get_latest_data', {});
    updateChart(data);
}, 5000);  // Update every 5 seconds

// Clean up on close
window.addEventListener('beforeunload', () => {
    clearInterval(updateInterval);
});
```

### Error Handling

Handle errors gracefully:

```javascript
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        const data = event.data.result;

        if (data.isError) {
            showError(data.content[0].text);
            return;
        }

        try {
            renderData(data);
        } catch (err) {
            showError(`Rendering failed: ${err.message}`);
        }
    }
});
```

---

## Best Practices

### Performance

**Minimize initial load:**
- Use CDN links for libraries
- Lazy load images: `<img loading="lazy">`
- Minimize inline CSS/JS

**Efficient rendering:**
```javascript
// Good: Update only what changed
chart.data.datasets[0].data = newData;
chart.update('none');

// Avoid: Full re-render
chart.destroy();
createChart(newData);  // Slow!
```

### Accessibility

Make UIs accessible:

```html
<!-- Add ARIA labels -->
<button onclick="openLightbox(0)" aria-label="View full image">
    <img src="thumb.jpg" alt="Hotel room">
</button>

<!-- Keyboard navigation -->
<script>
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeLightbox();
    if (e.key === 'ArrowRight') nextImage();
    if (e.key === 'ArrowLeft') previousImage();
});
</script>
```

### Mobile Responsiveness

Use mobile-first CSS:

```css
/* Mobile first */
.gallery {
    grid-template-columns: 1fr;
}

/* Tablet */
@media (min-width: 640px) {
    .gallery {
        grid-template-columns: repeat(2, 1fr);
    }
}

/* Desktop */
@media (min-width: 1024px) {
    .gallery {
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    }
}
```

---

## Security Considerations

### Sandboxed Execution

UIs run in sandboxed iframes with restricted permissions. The host controls:
- Network access
- Storage access
- Script execution context

### Input Validation

Always validate tool results:

```javascript
function validateVenue(venue) {
    if (typeof venue.lat !== 'number' ||
        typeof venue.lon !== 'number') {
        throw new Error('Invalid coordinates');
    }

    if (venue.lat < -90 || venue.lat > 90) {
        throw new Error('Latitude out of range');
    }

    return true;
}

// Use it:
data.venues.forEach(validateVenue);
```

### XSS Prevention

Sanitize user content:

```javascript
function escapeHTML(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

// Safe rendering
element.innerHTML = `<h3>${escapeHTML(venue.name)}</h3>`;
```

---

## Testing Your UIs

### Browser DevTools

Debug in the browser:

```javascript
// Add logging
window.addEventListener('message', (event) => {
    console.log('Received:', event.data);
});

// Expose for debugging
window.DEBUG = { state, callTool, currentData };
```

### Manual Testing

Test with MCP Inspector or Claude Desktop:

1. Start your server: `cargo run --features schema-generation`
2. Connect from an MCP host
3. Call your UI-associated tool
4. Verify the UI renders correctly
5. Test interactions (clicks, keyboard, etc.)

### Error Boundaries

Catch all errors:

```javascript
window.addEventListener('error', (event) => {
    console.error('Error:', event.error);
    showError(`Error: ${event.error.message}`);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Promise rejection:', event.reason);
});
```

---

## Summary

MCP Apps Extension turns your server into a complete application platform:

✅ **Declare UI resources** with `UIResourceBuilder`
✅ **Associate tools with UIs** using `.with_ui()`
✅ **Communicate via postMessage** with JSON-RPC
✅ **Build rich experiences**: maps, galleries, dashboards
✅ **Run securely** in sandboxed iframes
✅ **Optimize for performance** and accessibility

**Next steps:**
1. Try the [conference map example](../../examples/conference_venue_map.rs)
2. Build the [hotel gallery](../../examples/hotel_gallery.rs)
3. Create your own interactive UI for your use case
4. Share your creations with the community!

**Further reading:**
- [MCP Apps Extension Spec (SEP-1865)](https://spec.modelcontextprotocol.io/)
- [Chapter 6: Resources](ch06-resources.md)
- [Chapter 5: Tools](ch05-tools.md)
- [Advanced guide](../docs/advanced/mcp-apps-extension.md)
