# Tool-UI Association

Once you have UI resources, you need to link them to tools. This chapter covers how tools reference their associated UIs.

## The `.with_ui()` Method

PMCP's `TypedTool` provides a fluent method for UI association:

```rust
use pmcp::TypedTool;

let tool = TypedTool::new("get_room_images", get_room_images)
    .with_description("Get photo gallery for a hotel room")
    .with_ui("ui://hotel/room-gallery");  // Link to UI resource
```

This adds metadata to the tool that MCP hosts can use to render the UI when the tool is called.

## How It Works

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Tool-UI Association Flow                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. SERVER INITIALIZATION                                               │
│     ══════════════════════                                              │
│     Server declares:                                                    │
│     • UI Resource: ui://hotel/gallery with HTML content                │
│     • Tool: get_room_images with _meta["ui/resourceUri"] = ui://...    │
│                                                                         │
│  2. TOOL DISCOVERY (tools/list)                                        │
│     ═══════════════════════════                                        │
│     Client receives tool definition:                                    │
│     {                                                                   │
│       "name": "get_room_images",                                       │
│       "description": "Get photo gallery for a hotel room",             │
│       "_meta": {                                                       │
│         "ui/resourceUri": "ui://hotel/room-gallery"  ← UI link        │
│       }                                                                │
│     }                                                                   │
│                                                                         │
│  3. TOOL INVOCATION (tools/call)                                       │
│     ═══════════════════════════                                        │
│     a. Client calls tool → gets JSON result                            │
│     b. Client sees ui/resourceUri → fetches ui://hotel/room-gallery    │
│     c. Client renders HTML in sandboxed iframe                          │
│     d. Client sends result to iframe via postMessage                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## ToolUIMetadata Structure

Under the hood, `.with_ui()` populates the tool's `_meta` field:

```rust
use pmcp::types::ui::ToolUIMetadata;
use std::collections::HashMap;

// What .with_ui() does internally
let mut meta = HashMap::new();
meta.insert(
    "ui/resourceUri".to_string(),
    serde_json::Value::String("ui://hotel/room-gallery".to_string())
);

// Reading UI metadata back
let ui_meta = ToolUIMetadata::from_metadata(&meta);
assert_eq!(ui_meta.ui_resource_uri, Some("ui://hotel/room-gallery".to_string()));

// Creating metadata manually
let ui_meta = ToolUIMetadata::new()
    .with_ui_resource("ui://hotel/room-gallery");

let meta_map = ui_meta.to_metadata();
// Now meta_map["ui/resourceUri"] = "ui://hotel/room-gallery"
```

## Complete Tool Definition

Here's a full example showing tool creation with UI association:

```rust
use pmcp::{TypedTool, RequestHandlerExtra};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Input type with schema generation
#[derive(Debug, Deserialize, JsonSchema)]
struct GetVenuesArgs {
    /// Conference ID to get venues for
    conference_id: String,
}

// Output type
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

// Tool handler
async fn get_conference_venues(
    args: GetVenuesArgs,
    _extra: RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // Fetch venues from database/API
    let venues = fetch_venues(&args.conference_id).await?;

    let result = VenuesResult {
        conference: args.conference_id,
        venues,
    };

    Ok(serde_json::to_value(result)?)
}

// Create tool with full configuration
let tool = TypedTool::new("get_conference_venues", |args, extra| {
    Box::pin(get_conference_venues(args, extra))
})
    .with_description("Get all venues for a conference with location data")
    .with_ui("ui://conference/venue-map");
```

## Multiple Tools, Multiple UIs

A server can have multiple tools with different UIs:

```rust
use pmcp::{Server, ServerCapabilities, TypedTool, ResourceCollection, UIResourceBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create multiple UI resources
    let (gallery_ui, gallery_contents) = UIResourceBuilder::new(
        "ui://hotel/gallery",
        "Photo Gallery"
    )
        .html_template(GALLERY_HTML)
        .build_with_contents()?;

    let (map_ui, map_contents) = UIResourceBuilder::new(
        "ui://hotel/map",
        "Property Map"
    )
        .html_template(MAP_HTML)
        .build_with_contents()?;

    let (amenities_ui, amenities_contents) = UIResourceBuilder::new(
        "ui://hotel/amenities",
        "Amenities Chart"
    )
        .html_template(AMENITIES_HTML)
        .build_with_contents()?;

    // Resource collection with all UIs
    let resources = ResourceCollection::new()
        .add_ui_resource(gallery_ui, gallery_contents)
        .add_ui_resource(map_ui, map_contents)
        .add_ui_resource(amenities_ui, amenities_contents);

    // Each tool links to its appropriate UI
    let gallery_tool = TypedTool::new("get_room_images", get_room_images)
        .with_description("Get room photos")
        .with_ui("ui://hotel/gallery");

    let map_tool = TypedTool::new("get_property_location", get_property_location)
        .with_description("Get hotel location for map")
        .with_ui("ui://hotel/map");

    let amenities_tool = TypedTool::new("get_amenities", get_amenities)
        .with_description("Get hotel amenities breakdown")
        .with_ui("ui://hotel/amenities");

    // Build server with all tools and resources
    let server = Server::builder()
        .name("hotel-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_room_images", gallery_tool)
        .tool("get_property_location", map_tool)
        .tool("get_amenities", amenities_tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

## Shared UIs

Multiple tools can share the same UI if their output formats are compatible:

```rust
// One UI for multiple search results
let (results_ui, results_contents) = UIResourceBuilder::new(
    "ui://search/results-grid",
    "Search Results Grid"
)
    .html_template(RESULTS_GRID_HTML)
    .build_with_contents()?;

// Multiple tools use the same UI
let search_rooms_tool = TypedTool::new("search_rooms", search_rooms)
    .with_ui("ui://search/results-grid");

let search_restaurants_tool = TypedTool::new("search_restaurants", search_restaurants)
    .with_ui("ui://search/results-grid");

let search_activities_tool = TypedTool::new("search_activities", search_activities)
    .with_ui("ui://search/results-grid");
```

The UI needs to handle different data shapes:

```javascript
window.addEventListener('message', (event) => {
    if (event.data.type === 'mcp-tool-result') {
        const result = event.data.result;

        // Handle different result types
        if (result.rooms) {
            renderRooms(result.rooms);
        } else if (result.restaurants) {
            renderRestaurants(result.restaurants);
        } else if (result.activities) {
            renderActivities(result.activities);
        }
    }
});
```

## Tools Without UIs

Not every tool needs a UI. Tools that return simple data work fine without one:

```rust
// Simple status check - no UI needed
let status_tool = TypedTool::new("get_server_status", get_status)
    .with_description("Check server health");
// No .with_ui() - client displays result as text

// Configuration retrieval - no UI needed
let config_tool = TypedTool::new("get_config", get_config)
    .with_description("Get current configuration");
// No .with_ui() - AI describes config in natural language
```

## Checking UI Association

You can verify a tool's UI metadata:

```rust
use pmcp::types::ui::ToolUIMetadata;

// Get tool metadata from server (after building)
let tool_list = server.list_tools().await?;

for tool in tool_list.tools {
    if let Some(meta) = &tool._meta {
        let ui_meta = ToolUIMetadata::from_metadata(meta);
        if let Some(uri) = ui_meta.ui_resource_uri {
            println!("Tool '{}' has UI: {}", tool.name, uri);
        } else {
            println!("Tool '{}' has no UI", tool.name);
        }
    }
}
```

## URI Matching

The UI resource URI in the tool must exactly match a declared UI resource:

```rust
// ✅ Correct: URIs match exactly
let (ui, contents) = UIResourceBuilder::new(
    "ui://hotel/gallery",  // ← Declared URI
    "Gallery"
).html_template(HTML).build_with_contents()?;

let tool = TypedTool::new("get_images", handler)
    .with_ui("ui://hotel/gallery");  // ← Same URI

// ❌ Wrong: Trailing slash mismatch
let tool = TypedTool::new("get_images", handler)
    .with_ui("ui://hotel/gallery/");  // ← Won't match!

// ❌ Wrong: Case mismatch
let tool = TypedTool::new("get_images", handler)
    .with_ui("ui://Hotel/Gallery");  // ← Won't match!
```

## Capability Requirements

For UI resources to work, declare the resources capability:

```rust
use pmcp::{Server, ServerCapabilities, ResourceCapabilities};

let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities {
        // Must have resources capability for UI resources
        resources: Some(ResourceCapabilities {
            subscribe: Some(false),
            list_changed: Some(false),
        }),
        tools: Some(Default::default()),
        ..Default::default()
    })
    .resources(resources)  // Contains UI resources
    .build()?;
```

## Summary

| Concept | Description |
|---------|-------------|
| `.with_ui(uri)` | Associates a tool with a UI resource |
| `ui/resourceUri` | The metadata key in tool's `_meta` |
| `ToolUIMetadata` | Type for reading/writing UI metadata |
| URI matching | Tool URI must exactly match declared resource |
| Shared UIs | Multiple tools can reference the same UI |
| Optional | Tools work without UI for simple responses |

---

*Continue to [PostMessage Communication](./ch20-03-postmessage.md) →*

