# PostMessage Communication

MCP Apps use the browser's `postMessage` API for secure communication between the sandboxed UI iframe and the MCP host. This chapter explains the communication patterns.

## Security Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Sandboxed Iframe Security                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌────────────────────────────────────────┐                             │
│  │            MCP Host (Client)           │                             │
│  │                                        │                             │
│  │  ┌──────────────────────────────────┐  │                             │
│  │  │      Sandboxed Iframe            │  │                             │
│  │  │      ═════════════════           │  │                             │
│  │  │      ┌────────────────┐          │  │                             │
│  │  │      │  Your HTML/JS  │          │  │                             │
│  │  │      │                │          │  │                             │
│  │  │      └────────────────┘          │  │                             │
│  │  │                                  │  │                             │
│  │  │  ✗ Cannot access parent DOM      │  │                             │
│  │  │  ✗ Cannot read parent cookies    │  │                             │
│  │  │  ✗ Cannot access localStorage    │  │                             │
│  │  │  ✗ Cannot make same-origin reqs  │  │                             │
│  │  │                                  │  │                             │
│  │  │  ✓ CAN use postMessage           │  │                             │
│  │  │  ✓ CAN make cross-origin reqs    │  │                             │
│  │  │  ✓ CAN render any HTML/CSS/JS    │  │                             │
│  │  └──────────────────────────────────┘  │                             │
│  │              ▲              │          │                             │
│  │              │ postMessage  │          │                             │
│  │              │ (MCP JSON-RPC)          │                             │
│  │              ▼              ▼          │                             │
│  │  ┌──────────────────────────────────┐  │                             │
│  │  │     MCP Message Handler          │  │                             │
│  │  └──────────────────────────────────┘  │                             │
│  └────────────────────────────────────────┘                             │
│                                                                         │
│  Why Sandbox?                                                           │
│  • Untrusted HTML from server can't steal data                          │
│  • UI can't interfere with host application                             │
│  • Clear boundary between UI and MCP protocol                           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Message Flow

Communication is bidirectional using JSON-RPC format:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    PostMessage Data Flow                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  UI (Iframe)                              MCP Host                      │
│  ══════════                               ════════                      │
│                                                                         │
│  1. Request Tool Data                                                   │
│  ┌────────────────────────┐               ┌────────────────────────┐    │
│  │ window.parent.postMessage({            │                        │    │
│  │   jsonrpc: '2.0',      │ ────────────► │ Host receives request  │    │
│  │   method: 'tools/call',│               │ Validates & executes   │    │
│  │   params: {...},       │               │                        │    │
│  │   id: 1                │               └────────────────────────┘    │
│  │ }, '*')                │                                             │
│  └────────────────────────┘                                             │
│                                                                         │
│  2. Receive Tool Result                                                 │
│  ┌────────────────────────┐               ┌────────────────────────┐    │
│  │ window.addEventListener(│ ◄──────────── │ Host sends result via  │    │
│  │   'message', (event) => │               │ iframe.contentWindow   │    │
│  │   if (event.data.type   │               │   .postMessage({       │    │
│  │     === 'mcp-tool-result')              │   type: 'mcp-tool-result',  │
│  │     render(event.data.result)           │   result: {...}        │    │
│  │ )                       │               │ })                     │    │
│  └────────────────────────┘               └────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Requesting Tool Data

When your UI loads, it typically requests data from the associated tool:

```javascript
// Request data from the tool that launched this UI
window.parent.postMessage({
    jsonrpc: '2.0',
    method: 'tools/call',
    params: {
        name: 'get_room_images',
        arguments: {
            hotel_id: 'grand-resort',
            room_type: 'deluxe'
        }
    },
    id: 1
}, '*');
```

### Message Format

```javascript
{
    jsonrpc: '2.0',           // Always '2.0' for JSON-RPC
    method: 'tools/call',     // MCP method name
    params: {
        name: 'tool_name',    // Which tool to call
        arguments: {          // Tool-specific arguments
            key: 'value'
        }
    },
    id: 1                     // Request ID for matching responses
}
```

## Receiving Results

Listen for the `mcp-tool-result` message type:

```javascript
window.addEventListener('message', (event) => {
    // Check message type
    if (event.data.type === 'mcp-tool-result') {
        const result = event.data.result;

        // result contains the tool's JSON output
        console.log('Received:', result);

        // Update your UI
        renderData(result);
    }
});

function renderData(data) {
    if (data.images) {
        data.images.forEach(img => {
            // Create gallery items
        });
    }
}
```

### Result Message Format

```javascript
{
    type: 'mcp-tool-result',   // Message type identifier
    result: {                   // Tool's JSON output
        hotel: 'grand-resort',
        room_type: 'deluxe',
        images: [
            { id: 'img-1', url: '...', title: 'Bedroom' },
            { id: 'img-2', url: '...', title: 'Bathroom' }
        ]
    },
    requestId: 1               // Matches your request ID
}
```

## Complete Example: Interactive Map

Here's a full UI example using Leaflet.js for an interactive map:

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
        .loading {
            position: absolute;
            top: 50%; left: 50%;
            transform: translate(-50%, -50%);
            font-family: system-ui;
            color: #666;
        }
    </style>
</head>
<body>
    <div id="map">
        <div class="loading">Loading venue data...</div>
    </div>

    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <script>
        // Initialize map
        const map = L.map('map').setView([36.1147, -115.1728], 12);

        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '© OpenStreetMap contributors'
        }).addTo(map);

        // Listen for tool results from MCP host
        window.addEventListener('message', (event) => {
            // Only handle MCP tool results
            if (event.data.type !== 'mcp-tool-result') return;

            const data = event.data.result;

            // Remove loading message
            document.querySelector('.loading')?.remove();

            if (data.venues && data.venues.length > 0) {
                const bounds = [];

                // Add marker for each venue
                data.venues.forEach(venue => {
                    const marker = L.marker([venue.lat, venue.lon]).addTo(map);
                    bounds.push([venue.lat, venue.lon]);

                    // Create popup with venue info
                    marker.bindPopup(`
                        <strong>${venue.name}</strong><br>
                        ${venue.description}<br>
                        <em>Capacity: ${venue.capacity.toLocaleString()}</em>
                    `);
                });

                // Zoom to fit all markers
                map.fitBounds(bounds, { padding: [50, 50] });
            }
        });

        // Request venue data
        window.parent.postMessage({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'get_conference_venues',
                arguments: {
                    conference_id: 'aws-reinvent-2025'
                }
            },
            id: 1
        }, '*');
    </script>
</body>
</html>
```

## Error Handling

Handle potential errors in your UI:

```javascript
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-error') {
        // Handle error from MCP host
        const error = event.data.error;
        showError(`Error: ${error.message}`);
        return;
    }

    if (event.data.type === 'mcp-tool-result') {
        const result = event.data.result;

        // Check for error in result
        if (result.error) {
            showError(`Tool error: ${result.error}`);
            return;
        }

        // Normal processing
        renderData(result);
    }
});

function showError(message) {
    document.body.innerHTML = `
        <div style="padding: 20px; color: #c0392b; font-family: system-ui;">
            <h2>Something went wrong</h2>
            <p>${message}</p>
        </div>
    `;
}
```

## Loading States

Always show loading states since tool calls are asynchronous:

```javascript
// Initial loading state
document.getElementById('content').innerHTML = `
    <div class="loading">
        <div class="spinner"></div>
        <p>Loading data...</p>
    </div>
`;

window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        // Remove loading, show content
        document.querySelector('.loading')?.remove();
        renderData(event.data.result);
    }
});

// Start the request
window.parent.postMessage({...}, '*');
```

CSS for a simple spinner:

```css
.loading {
    text-align: center;
    padding: 40px;
    color: #666;
}

.spinner {
    width: 40px;
    height: 40px;
    border: 3px solid #f3f3f3;
    border-top: 3px solid #3498db;
    border-radius: 50%;
    animation: spin 1s linear infinite;
    margin: 0 auto 16px;
}

@keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}
```

## UI-to-Tool Callbacks

UIs can call tools to request additional data or perform actions:

```javascript
// User clicks a venue marker
function onVenueClick(venueId) {
    // Show loading in popup
    showPopupLoading();

    // Request detailed info
    window.parent.postMessage({
        jsonrpc: '2.0',
        method: 'tools/call',
        params: {
            name: 'get_venue_details',
            arguments: { venue_id: venueId }
        },
        id: 2  // Different ID from initial request
    }, '*');
}

// Handle multiple response types
window.addEventListener('message', (event) => {
    if (event.data.type !== 'mcp-tool-result') return;

    const requestId = event.data.requestId;

    switch (requestId) {
        case 1:  // Initial venue list
            renderVenueMarkers(event.data.result);
            break;
        case 2:  // Venue details
            updatePopupContent(event.data.result);
            break;
    }
});
```

## Best Practices

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    PostMessage Best Practices                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ✓ DO:                                                                  │
│  ═════                                                                  │
│  • Use unique request IDs when making multiple calls                    │
│  • Always show loading states                                           │
│  • Handle errors gracefully with user-friendly messages                 │
│  • Check event.data.type before processing                              │
│  • Use event.data.requestId to match responses                          │
│  • Design for async - data may arrive out of order                      │
│                                                                         │
│  ✗ DON'T:                                                               │
│  ═══════                                                                │
│  • Assume immediate responses                                           │
│  • Ignore error handling                                                │
│  • Send sensitive data through postMessage                              │
│  • Rely on message ordering                                             │
│  • Make synchronous assumptions                                         │
│  • Trust event.origin without validation (if needed)                    │
│                                                                         │
│  ORIGIN VALIDATION (optional but recommended):                          │
│  ════════════════════════════════════════════                           │
│                                                                         │
│  window.addEventListener('message', (event) => {                        │
│      // Optionally validate origin for sensitive operations             │
│      // if (event.origin !== expectedOrigin) return;                    │
│                                                                         │
│      if (event.data.type === 'mcp-tool-result') {...}                  │
│  });                                                                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Message Types Reference

| Message Type | Direction | Purpose |
|--------------|-----------|---------|
| `tools/call` | UI → Host | Request tool execution |
| `mcp-tool-result` | Host → UI | Return tool output |
| `mcp-error` | Host → UI | Report error to UI |

## Debugging Tips

```javascript
// Log all messages for debugging
window.addEventListener('message', (event) => {
    console.log('[MCP Message]', event.data);

    // Normal processing...
});

// Add visual debug info
function debugLog(message) {
    const debug = document.getElementById('debug');
    if (debug) {
        debug.innerHTML += `<p>${new Date().toISOString()}: ${message}</p>`;
    }
}
```

During development, add a debug panel:

```html
<div id="debug" style="
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    max-height: 150px;
    overflow-y: auto;
    background: rgba(0,0,0,0.8);
    color: #0f0;
    font-family: monospace;
    font-size: 12px;
    padding: 10px;
    display: none; /* Show with ?debug=true */
"></div>

<script>
    if (location.search.includes('debug=true')) {
        document.getElementById('debug').style.display = 'block';
    }
</script>
```

## Summary

| Concept | Description |
|---------|-------------|
| `postMessage` | Browser API for cross-origin iframe communication |
| JSON-RPC | Message format used for MCP protocol |
| `window.parent.postMessage()` | How UI sends requests to host |
| `window.addEventListener('message')` | How UI receives responses |
| `mcp-tool-result` | Message type for tool output |
| Request ID | Correlates requests with responses |

---

*← Back to [Chapter Index](./ch20-mcp-apps.md)*

