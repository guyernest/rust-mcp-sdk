# MCP Apps Extension: Interactive UI Integration

## Overview

The MCP Apps Extension (SEP-1865) enables MCP servers to provide rich, interactive user interfaces alongside their tools. This allows servers to offer visual experiences like charts, maps, galleries, and custom dashboards that enhance the tool execution workflow.

This guide demonstrates how to build interactive UIs in your MCP server using the Rust SDK.

## Table of Contents

- [Core Concepts](#core-concepts)
- [Quick Start](#quick-start)
- [Example 1: Interactive Map](#example-1-interactive-map)
- [Example 2: Image Gallery](#example-2-image-gallery)
- [Advanced Patterns](#advanced-patterns)
- [Best Practices](#best-practices)
- [Security Considerations](#security-considerations)

## Core Concepts

### UI Resources

UI Resources are pre-declared HTML templates that can be rendered in the MCP host. They use the `ui://` URI scheme and are delivered as `text/html+mcp` MIME type.

```rust
use pmcp::UIResourceBuilder;

let (ui_resource, ui_contents) = UIResourceBuilder::new(
    "ui://myapp/dashboard",
    "Analytics Dashboard"
)
.description("Real-time analytics visualization")
.html_template(include_str!("dashboard.html"))
.build_with_contents()?;
```

### Tool-UI Association

Tools can be associated with UIs using the `_meta` field in ToolInfo. When a tool is called, the host can display the associated UI:

```rust
use pmcp::TypedTool;

let analytics_tool = TypedTool::new("analyze_data", |args, extra| {
    Box::pin(analyze_data(args, extra))
})
.with_description("Analyze business metrics")
.with_ui("ui://myapp/dashboard");  // Associate with UI
```

### Communication Protocol

The UI (rendered in a sandboxed iframe) communicates with the host via MCP JSON-RPC over `postMessage`:

```javascript
// UI sends tool call request
window.parent.postMessage({
    jsonrpc: '2.0',
    method: 'tools/call',
    params: {
        name: 'analyze_data',
        arguments: { metric: 'sales', period: '30d' }
    },
    id: 1
}, '*');

// UI receives tool result
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        const data = event.data.result;
        // Render the data...
    }
});
```

## Quick Start

Add the `schema-generation` feature to use TypedTool with automatic schema generation:

```toml
[dependencies]
pmcp = { version = "1.8", features = ["schema-generation"] }
schemars = "1.0"
```

Basic example:

```rust
use pmcp::{
    Server, ServerCapabilities, TypedTool, UIResourceBuilder, ResourceCollection,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct DataArgs {
    query: String,
}

async fn get_data(args: DataArgs, _extra: pmcp::RequestHandlerExtra)
    -> pmcp::Result<serde_json::Value>
{
    Ok(serde_json::json!({
        "results": ["item1", "item2", "item3"]
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create UI resource
    let (ui_resource, ui_contents) = UIResourceBuilder::new(
        "ui://app/viewer",
        "Data Viewer"
    )
    .html_template("<html><body>Your UI here</body></html>")
    .build_with_contents()?;

    // Create resource collection
    let resources = ResourceCollection::new()
        .add_ui_resource(ui_resource, ui_contents);

    // Create tool with UI association
    let tool = TypedTool::new("get_data", |args, extra| {
        Box::pin(get_data(args, extra))
    })
    .with_ui("ui://app/viewer");

    // Build and run server
    let server = Server::builder()
        .name("my-app-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_data", tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

## Example 1: Interactive Map

This example demonstrates building a conference venue map using Leaflet.js. Users can visualize multiple venues on an interactive map with popups showing details.

### Use Case

Conference organizers need to show attendees where different sessions, workshops, and events are located across multiple venues in a city.

### Implementation

**Full code:** [`examples/conference_venue_map.rs`](../../examples/conference_venue_map.rs)

#### Step 1: Define Data Types

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct GetVenuesArgs {
    /// Conference ID to get venues for
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

#### Step 2: Implement Tool Handler

```rust
async fn get_conference_venues(
    args: GetVenuesArgs,
    _extra: RequestHandlerExtra,
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
            // ... more venues
        ],
        _ => vec![],
    };

    Ok(serde_json::to_value(VenuesResult {
        conference: args.conference_id,
        venues,
    })?)
}
```

#### Step 3: Create Interactive Map UI

```rust
const VENUE_MAP_HTML: &str = r#"<!DOCTYPE html>
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
        .venue-popup .capacity { font-weight: bold; color: #3498db; }
    </style>
</head>
<body>
    <div id="map"></div>
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <script>
        const map = L.map('map').setView([36.1147, -115.1728], 12);

        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '© OpenStreetMap contributors',
            maxZoom: 19
        }).addTo(map);

        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;
                const bounds = [];

                data.venues.forEach(venue => {
                    const marker = L.marker([venue.lat, venue.lon]).addTo(map);
                    bounds.push([venue.lat, venue.lon]);

                    marker.bindPopup(`
                        <div class="venue-popup">
                            <h3>${venue.name}</h3>
                            <p>${venue.description}</p>
                            <p class="capacity">Capacity: ${venue.capacity.toLocaleString()}</p>
                        </div>
                    `);
                });

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
</html>"#;
```

#### Step 4: Wire Everything Together

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create UI resource
    let (ui_resource, ui_contents) =
        UIResourceBuilder::new("ui://conference/venue-map", "Conference Venue Map")
            .description("Interactive map showing all conference venues")
            .html_template(VENUE_MAP_HTML)
            .build_with_contents()?;

    // Build resource collection
    let resources = ResourceCollection::new()
        .add_ui_resource(ui_resource, ui_contents);

    // Create tool with UI association
    let get_venues_tool = TypedTool::new("get_conference_venues", |args, extra| {
        Box::pin(get_conference_venues(args, extra))
    })
    .with_description("Get all venues for a conference with location data")
    .with_ui("ui://conference/venue-map");

    // Create server
    let server = Server::builder()
        .name("conference-map-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_conference_venues", get_venues_tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

### Running the Example

```bash
cargo run --example conference_venue_map --features schema-generation
```

### Key Features

- **Interactive Map**: Pan, zoom, and click markers
- **Responsive Design**: Works on desktop and mobile
- **Dynamic Data**: Venues are loaded from the tool handler
- **Rich Popups**: Each venue shows name, description, and capacity
- **Auto-fitting**: Map automatically adjusts to show all venues

## Example 2: Image Gallery

This example builds a hotel room gallery with lightbox functionality, demonstrating how to work with images and create rich visual experiences.

### Use Case

Hotels need to showcase room types with multiple high-quality images, allowing potential guests to view detailed photos before booking.

### Implementation

**Full code:** [`examples/hotel_gallery.rs`](../../examples/hotel_gallery.rs)

#### Step 1: Define Data Types

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GetRoomImagesArgs {
    /// Hotel ID
    hotel_id: String,
    /// Room type (e.g., "deluxe", "suite", "standard")
    room_type: String,
}

#[derive(Debug, Serialize)]
struct RoomImage {
    id: String,
    url: String,
    thumbnail_url: String,
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

#### Step 2: Implement Tool Handler

```rust
async fn get_room_images(
    args: GetRoomImagesArgs,
    _extra: RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // In production, fetch from CDN or database
    let images = match (args.hotel_id.as_str(), args.room_type.as_str()) {
        ("grand-resort", "deluxe") => vec![
            RoomImage {
                id: "room-1".to_string(),
                url: "https://images.unsplash.com/photo-1590490360182-c33d57733427?w=1200"
                    .to_string(),
                thumbnail_url: "https://images.unsplash.com/photo-1590490360182-c33d57733427?w=400"
                    .to_string(),
                title: "Deluxe King Room".to_string(),
                description: "Spacious room with king bed and city views".to_string(),
            },
            // ... more images
        ],
        _ => vec![],
    };

    Ok(serde_json::to_value(RoomGalleryResult {
        hotel: args.hotel_id,
        room_type: args.room_type,
        images,
    })?)
}
```

#### Step 3: Create Gallery UI

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hotel Room Gallery</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: system-ui, -apple-system, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }
        .gallery {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 20px;
            max-width: 1400px;
            margin: 0 auto;
        }
        .gallery-item {
            background: white;
            border-radius: 12px;
            overflow: hidden;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            transition: transform 0.2s;
            cursor: pointer;
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
            background: rgba(0, 0, 0, 0.9);
            z-index: 1000;
            align-items: center;
            justify-content: center;
        }
        .lightbox.active { display: flex; }
    </style>
</head>
<body>
    <div class="header">
        <h1 id="room-title">Hotel Room Gallery</h1>
        <p id="room-subtitle">Loading images...</p>
    </div>
    <div class="gallery" id="gallery"></div>
    <div class="lightbox" id="lightbox">
        <span class="lightbox-close" onclick="closeLightbox()">&times;</span>
        <div class="lightbox-content">
            <img id="lightbox-img" src="" alt="">
            <div class="lightbox-caption">
                <h3 id="lightbox-title"></h3>
                <p id="lightbox-desc"></p>
            </div>
        </div>
    </div>

    <script>
        let currentImages = [];

        function openLightbox(index) {
            const image = currentImages[index];
            document.getElementById('lightbox-img').src = image.url;
            document.getElementById('lightbox-title').textContent = image.title;
            document.getElementById('lightbox-desc').textContent = image.description;
            document.getElementById('lightbox').classList.add('active');
        }

        function closeLightbox() {
            document.getElementById('lightbox').classList.remove('active');
        }

        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;
                currentImages = data.images || [];

                document.getElementById('room-title').textContent =
                    `${data.room_type.charAt(0).toUpperCase() + data.room_type.slice(1)} Room`;
                document.getElementById('room-subtitle').textContent =
                    `${data.hotel} - ${currentImages.length} photos`;

                const gallery = document.getElementById('gallery');
                gallery.innerHTML = '';

                currentImages.forEach((image, index) => {
                    const item = document.createElement('div');
                    item.className = 'gallery-item';
                    item.onclick = () => openLightbox(index);
                    item.innerHTML = `
                        <img src="${image.thumbnail_url}" alt="${image.title}" loading="lazy">
                        <div class="gallery-item-info">
                            <h3>${image.title}</h3>
                            <p>${image.description}</p>
                        </div>
                    `;
                    gallery.appendChild(item);
                });
            }
        });

        // Request room images
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

### Running the Example

```bash
cargo run --example hotel_gallery --features schema-generation
```

### Key Features

- **Responsive Grid**: Automatically adjusts to screen size
- **Lazy Loading**: Images load as needed for performance
- **Lightbox View**: Full-screen image viewing
- **Keyboard Support**: ESC key to close lightbox
- **Touch-friendly**: Works on mobile devices
- **Thumbnail Optimization**: Separate thumbnail and full-size URLs

## Advanced Patterns

### 1. Real-time Data Updates

Use server-sent events or polling to update UI with live data:

```javascript
// In your UI
let updateInterval = setInterval(() => {
    window.parent.postMessage({
        jsonrpc: '2.0',
        method: 'tools/call',
        params: { name: 'get_latest_data', arguments: {} },
        id: Date.now()
    }, '*');
}, 5000); // Update every 5 seconds
```

### 2. Multiple Tool Calls

Coordinate multiple tool calls to build complex visualizations:

```javascript
async function loadDashboard() {
    // Load metrics
    const metricsPromise = callTool('get_metrics', { period: '30d' });

    // Load trends
    const trendsPromise = callTool('get_trends', { metric: 'sales' });

    // Load comparisons
    const comparisonsPromise = callTool('get_comparisons', {
        current: '30d',
        previous: '30d'
    });

    const [metrics, trends, comparisons] = await Promise.all([
        metricsPromise,
        trendsPromise,
        comparisonsPromise
    ]);

    renderDashboard({ metrics, trends, comparisons });
}

function callTool(name, args) {
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
```

### 3. Error Handling

Implement robust error handling in your UI:

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

function showError(message) {
    const errorDiv = document.getElementById('error-message');
    errorDiv.textContent = message;
    errorDiv.style.display = 'block';
}
```

### 4. Loading States

Provide feedback during data loading:

```javascript
let pendingRequests = new Map();

function showLoading(requestId) {
    pendingRequests.set(requestId, true);
    document.getElementById('loading-spinner').style.display = 'block';
}

function hideLoading(requestId) {
    pendingRequests.delete(requestId);
    if (pendingRequests.size === 0) {
        document.getElementById('loading-spinner').style.display = 'none';
    }
}

window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        hideLoading(event.data.id);
        // Process result...
    }
});
```

### 5. State Management

For complex UIs, implement state management:

```javascript
class AppState {
    constructor() {
        this.data = null;
        this.filters = {};
        this.listeners = new Set();
    }

    update(changes) {
        Object.assign(this, changes);
        this.notify();
    }

    subscribe(listener) {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    notify() {
        this.listeners.forEach(listener => listener(this));
    }
}

const state = new AppState();

state.subscribe((newState) => {
    renderUI(newState);
});
```

## Best Practices

### 1. Performance Optimization

**Minimize Initial Load**
- Use CDN links for libraries (Leaflet, Chart.js, etc.)
- Lazy load images with `loading="lazy"`
- Minimize inline CSS/JS

**Efficient Rendering**
```javascript
// Good: Update only what changed
function updateChart(newData) {
    chart.data.datasets[0].data = newData;
    chart.update('none'); // Skip animations for performance
}

// Avoid: Full re-render
function updateChart(newData) {
    chart.destroy();
    createChart(newData); // Slow!
}
```

### 2. Accessibility

Make your UIs accessible:

```html
<!-- Add ARIA labels -->
<button
    onclick="openLightbox(0)"
    aria-label="View full size image of Deluxe King Room">
    <img src="thumbnail.jpg" alt="Deluxe King Room">
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

### 3. Mobile Responsiveness

```css
/* Use responsive units */
.container {
    width: min(100% - 2rem, 1400px);
    margin: 0 auto;
}

/* Mobile-first approach */
.gallery {
    grid-template-columns: 1fr;
}

@media (min-width: 640px) {
    .gallery {
        grid-template-columns: repeat(2, 1fr);
    }
}

@media (min-width: 1024px) {
    .gallery {
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    }
}
```

### 4. Resource Cleanup

Clean up resources when UI is closed:

```javascript
let updateInterval;
let eventListeners = [];

function initialize() {
    updateInterval = setInterval(fetchData, 5000);

    const listener = (e) => handleMessage(e);
    window.addEventListener('message', listener);
    eventListeners.push({ type: 'message', listener });
}

window.addEventListener('beforeunload', () => {
    clearInterval(updateInterval);
    eventListeners.forEach(({ type, listener }) => {
        window.removeEventListener(type, listener);
    });
});
```

### 5. Progressive Enhancement

Build UIs that work even if features fail:

```javascript
// Check for required features
if (!window.parent.postMessage) {
    showError('This UI requires postMessage support');
    return;
}

// Graceful degradation
function loadMap() {
    if (typeof L === 'undefined') {
        showStaticMap(); // Fallback to static image
        return;
    }
    initializeLeafletMap();
}
```

## Security Considerations

### 1. Sandboxed Execution

UIs run in sandboxed iframes with restricted permissions:

```html
<!-- Host renders UI in sandbox -->
<iframe
    sandbox="allow-scripts allow-same-origin"
    src="ui://myapp/dashboard">
</iframe>
```

### 2. Content Security Policy

Implement strict CSP in your HTML:

```html
<meta http-equiv="Content-Security-Policy"
      content="default-src 'self';
               script-src 'unsafe-inline' https://unpkg.com;
               style-src 'unsafe-inline';
               img-src https: data:;">
```

### 3. Input Validation

Validate all data from tool results:

```javascript
function validateVenue(venue) {
    if (typeof venue.lat !== 'number' ||
        typeof venue.lon !== 'number') {
        throw new Error('Invalid coordinates');
    }

    if (venue.lat < -90 || venue.lat > 90 ||
        venue.lon < -180 || venue.lon > 180) {
        throw new Error('Coordinates out of range');
    }

    return true;
}

window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        try {
            data.venues.forEach(validateVenue);
            renderVenues(data.venues);
        } catch (err) {
            showError(`Invalid data: ${err.message}`);
        }
    }
});
```

### 4. XSS Prevention

Always sanitize user-provided content:

```javascript
function escapeHTML(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

// Safe rendering
marker.bindPopup(`
    <h3>${escapeHTML(venue.name)}</h3>
    <p>${escapeHTML(venue.description)}</p>
`);
```

### 5. Origin Checking

Verify message origins in production:

```javascript
const ALLOWED_ORIGINS = ['https://your-host.com'];

window.addEventListener('message', (event) => {
    if (!ALLOWED_ORIGINS.includes(event.origin)) {
        console.warn('Ignored message from unknown origin:', event.origin);
        return;
    }

    // Process message...
});
```

## Debugging Tips

### 1. Browser DevTools

Use browser console to debug UI:

```javascript
// Add logging
window.addEventListener('message', (event) => {
    console.log('Received message:', event.data);
    // Process...
});

// Expose state for debugging
window.DEBUG = {
    state,
    callTool,
    currentImages,
};
```

### 2. Error Boundaries

Catch and log errors:

```javascript
window.addEventListener('error', (event) => {
    console.error('Global error:', event.error);
    showError(`Error: ${event.error.message}`);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    showError(`Promise error: ${event.reason}`);
});
```

### 3. Network Inspection

Monitor MCP message flow:

```javascript
const originalPostMessage = window.parent.postMessage;
window.parent.postMessage = function(...args) {
    console.log('Sending MCP message:', args[0]);
    return originalPostMessage.apply(this, args);
};
```

## Conclusion

The MCP Apps Extension enables rich, interactive experiences in MCP servers. By combining:

- **Rust backend**: Type-safe tool handlers with automatic schema generation
- **HTML/CSS/JavaScript frontend**: Rich, responsive UIs
- **MCP protocol**: Seamless communication between UI and tools

You can build powerful applications like:
- Data visualization dashboards
- Interactive maps and geospatial tools
- Image galleries and media browsers
- Real-time monitoring interfaces
- Custom admin panels

## Additional Resources

- [MCP Apps Extension Specification (SEP-1865)](https://spec.modelcontextprotocol.io/specification/draft/2025-01-15/server/ui/)
- [Example Code](../../examples/)
- [API Documentation](https://docs.rs/pmcp/)
- [TypedTool Guide](../TYPED_TOOLS_GUIDE.md)

## Next Steps

Try building your own MCP Apps:

1. **Start simple**: Create a basic UI with one tool call
2. **Add interactivity**: Implement user inputs and dynamic updates
3. **Enhance visually**: Use CSS frameworks or visualization libraries
4. **Optimize**: Profile and improve performance
5. **Deploy**: Test in production with real users

Happy building! 🚀
