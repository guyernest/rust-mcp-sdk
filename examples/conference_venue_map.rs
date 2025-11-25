//! Conference Venue Map Example
//!
//! Demonstrates MCP Apps Extension (SEP-1865) with an interactive map.
//!
//! This example shows:
//! - TypedTool with `.with_ui()` for tool-UI association
//! - UIResourceBuilder for creating HTML-based UIs
//! - Leaflet.js integration for interactive maps
//! - Data flow from Rust tool to JavaScript UI
//!
//! Run with: cargo run --example conference_venue_map

use pmcp::{
    RequestHandlerExtra, ResourceCollection, Server, ServerCapabilities, TypedTool,
    UIResourceBuilder,
};
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

/// Tool handler that returns conference venue data
async fn get_conference_venues(
    args: GetVenuesArgs,
    _extra: RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // In a real implementation, this would query a database
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
            Venue {
                id: "aria".to_string(),
                name: "ARIA Resort".to_string(),
                description: "Workshops and labs".to_string(),
                lat: 36.1067,
                lon: -115.1761,
                capacity: 3000,
            },
        ],
        _ => vec![],
    };

    let result = VenuesResult {
        conference: args.conference_id,
        venues,
    };

    Ok(serde_json::to_value(result)?)
}

const VENUE_MAP_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Conference Venue Map</title>
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <style>
        body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; }
        #map { height: 100vh; width: 100%; }
        .venue-popup {
            font-family: system-ui, -apple-system, sans-serif;
        }
        .venue-popup h3 {
            margin: 0 0 8px 0;
            color: #2c3e50;
        }
        .venue-popup p {
            margin: 4px 0;
            color: #555;
        }
        .venue-popup .capacity {
            font-weight: bold;
            color: #3498db;
        }
    </style>
</head>
<body>
    <div id="map"></div>

    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <script>
        // Initialize the map centered on Las Vegas
        const map = L.map('map').setView([36.1147, -115.1728], 12);

        // Add OpenStreetMap tiles
        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '© OpenStreetMap contributors',
            maxZoom: 19
        }).addTo(map);

        // MCP postMessage handler to receive venue data
        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;

                if (data.venues && data.venues.length > 0) {
                    const bounds = [];

                    // Add markers for each venue
                    data.venues.forEach(venue => {
                        const marker = L.marker([venue.lat, venue.lon]).addTo(map);
                        bounds.push([venue.lat, venue.lon]);

                        // Create popup with venue details
                        const popupContent = `
                            <div class="venue-popup">
                                <h3>${venue.name}</h3>
                                <p>${venue.description}</p>
                                <p class="capacity">Capacity: ${venue.capacity.toLocaleString()} people</p>
                            </div>
                        `;
                        marker.bindPopup(popupContent);
                    });

                    // Fit map to show all venues
                    if (bounds.length > 0) {
                        map.fitBounds(bounds, { padding: [50, 50] });
                    }
                }
            }
        });

        // Request venue data via MCP
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
</html>"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the venue map UI resource
    let (ui_resource, ui_contents) =
        UIResourceBuilder::new("ui://conference/venue-map", "Conference Venue Map")
            .description("Interactive map showing all conference venues")
            .html_template(VENUE_MAP_HTML)
            .build_with_contents()?;

    // Build resource collection with the UI resource
    let resources = ResourceCollection::new().add_ui_resource(ui_resource, ui_contents);

    // Create the tool with UI association (using closure for TypedTool)
    let get_venues_tool = TypedTool::new("get_conference_venues", |args, extra| {
        Box::pin(get_conference_venues(args, extra))
    })
    .with_description("Get all venues for a conference with location data")
    .with_ui("ui://conference/venue-map");

    // Create server with tool and resources
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

    // Run with stdio transport
    server.run_stdio().await?;

    Ok(())
}
