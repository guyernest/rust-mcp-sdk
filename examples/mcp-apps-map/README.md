# World City Explorer MCP App Example

An interactive map widget that demonstrates the **MCP Apps** pattern with geospatial data visualization using Leaflet.js.

```
┌─────────────────────────────────────────────────────────────────┐
│                    World City Explorer                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ [Search cities...]  [Category ▼]  [Search]              │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                                                         │   │
│  │            🔵 Tokyo                                     │   │
│  │                         🔴 Beijing                      │   │
│  │     🟢 London                                           │   │
│  │              🟣 Paris                                   │   │
│  │                                                         │   │
│  │  🔵 San Francisco    🔵 New York                        │   │
│  │                                                         │   │
│  │                           🟠 Cairo                      │   │
│  │                                    🟢 Singapore         │   │
│  │                                                         │   │
│  │                              🟣 Sydney                  │   │
│  │                                                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│  Cities (10) ─────────────────────────────────────────────────  │
│  │ Tokyo         Japan        [Tech]     │                     │
│  │ Paris         France       [Cultural] │                     │
│  │ New York      USA          [Financial]│                     │
│  └────────────────────────────────────────                     │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Build and Start the Server

```bash
cd examples/mcp-apps-map
cargo build --release
./target/release/mcp-apps-map
```

The server will start on port 3001 by default:
```
City Explorer MCP server listening on http://0.0.0.0:3001
Press Ctrl+C to stop the server
```

You can configure the port with the `PORT` environment variable:
```bash
PORT=8080 ./target/release/mcp-apps-map
```

### 2. Preview the Widget (with Mock Bridge)

For the best development experience, use the preview page with a mock MCP bridge:

```bash
open preview.html
# Or on Linux: xdg-open preview.html
```

This gives you a fully functional map explorer with:
- All 10 world cities with real coordinates
- Category filtering and search
- Real-time tool call logging in the dev panel
- Distance calculations using Haversine formula
- Simulated network latency for realistic testing

Alternatively, view just the widget UI:
```bash
open widget/map.html
```

### 3. Test the Server

Test the server with curl:

```bash
# Initialize the MCP connection
curl -s -X POST http://localhost:3001 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# List available tools
curl -s -X POST http://localhost:3001 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'

# Search for tech cities
curl -s -X POST http://localhost:3001 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_cities","arguments":{"filter":"tech"}}}'
```

### 4. Connect with cargo pmcp

Use the PMCP CLI for interactive testing:

```bash
cargo pmcp connect http://localhost:3001
```

This provides an interactive REPL for exploring the server:

```
Connected to city-explorer-server v1.0.0
> tools/list
> tools/call search_cities {"filter": "tech"}
> tools/call get_city_details {"city_id": "tokyo"}
> resources/list
```

### 5. Use with Claude Code

Add the server as an MCP endpoint:

```bash
claude mcp add city-explorer --transport http http://localhost:3001
```

Then test the tools:

```
You: Search for tech cities around the world
Claude: [Calls search_cities with filter "tech"]

You: Tell me more about Tokyo
Claude: [Calls get_city_details with city_id "tokyo"]

You: What cities are within 1000km of Paris?
Claude: [Calls get_nearby_cities with center and radius]
```

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                     MAP WIDGET ARCHITECTURE                    │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Widget (HTML/JS)                      │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │  │
│  │  │ Leaflet Map │  │ City List   │  │ Detail Panel    │   │  │
│  │  │ (tiles +    │  │ (filtered   │  │ (selected city  │   │  │
│  │  │  markers)   │  │  results)   │  │  info)          │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘   │  │
│  │                         │                                 │  │
│  │                    MapState                               │  │
│  │              { center, zoom, filter }                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                 │
│                         MCP Bridge                             │
│                              │                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Server (Rust)                         │  │
│  │                                                          │  │
│  │  search_cities(query?, filter?, map_state?)              │  │
│  │  └─> Returns matching cities with coordinates            │  │
│  │                                                          │  │
│  │  get_city_details(city_id)                               │  │
│  │  └─> Returns full city info + suggested map view         │  │
│  │                                                          │  │
│  │  get_nearby_cities(center, radius_km)                    │  │
│  │  └─> Returns cities within radius using Haversine        │  │
│  │                                                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
mcp-apps-map/
├── Cargo.toml
├── README.md
├── src/
│   └── main.rs
│       ├── City types (Coordinates, City, CityCategory)
│       ├── Mock city database (10 world cities)
│       ├── Haversine distance calculation
│       ├── Tool handlers
│       └── Resource handler
└── widget/
    └── map.html
        ├── Leaflet.js map integration
        ├── Category-colored markers
        ├── Search and filter UI
        ├── City list sidebar
        └── Detail panel
```

## Server Tools

### search_cities

Search for cities by name, country, or category.

```json
// Request
{
    "query": "york",
    "filter": "financial",
    "map_state": { "center": { "lat": 40, "lon": -74 }, "zoom": 8 }
}

// Response
{
    "count": 1,
    "cities": [{
        "id": "new-york",
        "name": "New York",
        "country": "United States",
        "population": 18800000,
        "coordinates": { "lat": 40.7128, "lon": -74.0060 },
        "description": "The Big Apple, global center of finance and culture.",
        "category": "financial"
    }]
}
```

### get_city_details

Get detailed information about a specific city.

```json
// Request
{ "city_id": "tokyo" }

// Response
{
    "found": true,
    "city": { /* full city object */ },
    "recommended_zoom": 12,
    "suggested_view": {
        "center": { "lat": 35.6762, "lon": 139.6503 },
        "zoom": 12
    }
}
```

### get_nearby_cities

Find cities within a radius of a point.

```json
// Request
{
    "center": { "lat": 48.8566, "lon": 2.3522 },
    "radius_km": 500
}

// Response
{
    "center": { "lat": 48.8566, "lon": 2.3522 },
    "radius_km": 500,
    "count": 2,
    "cities": [
        { "city": { /* Paris */ }, "distance_km": 0 },
        { "city": { /* London */ }, "distance_km": 343.5 }
    ]
}
```

## City Categories

The example includes cities in five categories, each with a distinct marker color:

| Category   | Color  | Example Cities           |
|------------|--------|--------------------------|
| Capital    | Red    | London, Beijing          |
| Tech       | Blue   | Tokyo, San Francisco     |
| Cultural   | Purple | Paris, Sydney            |
| Financial  | Green  | New York, Singapore      |
| Historical | Orange | Rome, Cairo              |

## Extending the Example

### Add Real Data Sources

Replace the mock database with a real API:

```rust
// In src/main.rs
async fn search_cities_handler(
    input: SearchCitiesInput,
    _extra: RequestHandlerExtra
) -> Result<Value> {
    // Call external API
    let response = reqwest::get(format!(
        "https://api.example.com/cities?q={}",
        input.query.unwrap_or_default()
    )).await?;

    let cities: Vec<City> = response.json().await?;
    Ok(json!({ "count": cities.len(), "cities": cities }))
}
```

### Add New Features

**Weather integration:**
```rust
#[derive(Deserialize, JsonSchema)]
struct GetWeatherInput {
    city_id: String,
}

fn get_weather_handler(input: GetWeatherInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Fetch weather data for city
    Ok(json!({
        "city_id": input.city_id,
        "temperature_c": 22,
        "conditions": "Partly cloudy"
    }))
}
```

**Route planning:**
```rust
#[derive(Deserialize, JsonSchema)]
struct GetRouteInput {
    from_city: String,
    to_city: String,
}
```

### Customize the Map

Modify `widget/map.html`:

```javascript
// Change tile provider (e.g., satellite imagery)
L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}', {
    attribution: 'Tiles &copy; Esri'
}).addTo(map);

// Custom marker icons
function createMarkerIcon(category) {
    return L.icon({
        iconUrl: `/icons/${category}.png`,
        iconSize: [32, 32],
        iconAnchor: [16, 32]
    });
}

// Add clustering for many markers
const markers = L.markerClusterGroup();
cities.forEach(city => {
    markers.addLayer(L.marker([city.coordinates.lat, city.coordinates.lon]));
});
map.addLayer(markers);
```

## Performance Considerations

### Large Datasets

For thousands of cities:

1. **Server-side pagination:**
```rust
#[derive(Deserialize, JsonSchema)]
struct SearchCitiesInput {
    query: Option<String>,
    limit: Option<usize>,  // Default 50
    offset: Option<usize>,
}
```

2. **Viewport-based loading:**
```javascript
map.on('moveend', async () => {
    const bounds = map.getBounds();
    const cities = await callTool('search_cities_in_bounds', {
        north: bounds.getNorth(),
        south: bounds.getSouth(),
        east: bounds.getEast(),
        west: bounds.getWest()
    });
    renderMarkers(cities);
});
```

3. **Marker clustering:** Use Leaflet.markercluster for better performance with many markers.

### Caching

```javascript
const cityCache = new Map();

async function getCityDetails(cityId) {
    if (cityCache.has(cityId)) {
        return cityCache.get(cityId);
    }
    const result = await callTool('get_city_details', { city_id: cityId });
    cityCache.set(cityId, result);
    return result;
}
```

## Testing

### Unit Tests (Rust)

```bash
# From repository root
cargo test --features "mcp-apps" -- map
```

### Widget Testing with Preview

The `preview.html` file provides a complete testing environment:

1. Open `preview.html` in your browser
2. Search for cities, filter by category
3. Click markers and cities to see tool calls in the dev panel
4. Check state persistence by refreshing the page

### Integration Testing with Claude Code

After connecting to Claude Code (see Quick Start), test the full flow:

```
You: Show me all the tech hub cities in the world
Claude: [Calls search_cities with filter "tech"]

You: What's the closest city to London within 500km?
Claude: [Calls get_nearby_cities with London coordinates and 500km radius]
```

### Direct JSON-RPC Testing

Test the server directly via HTTP:

```bash
# Start the server in one terminal
./target/release/mcp-apps-map

# In another terminal, test the endpoints:

# Initialize handshake
curl -s -X POST http://localhost:3001 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# Call search_cities tool
curl -s -X POST http://localhost:3001 \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_cities","arguments":{"filter":"tech"}}}'
```

### Widget Testing with Playwright (Optional)

```bash
cd ../../tests/playwright
npm install
npm test -- --grep "map"
```

### Manual Testing Checklist

- [ ] Map loads with correct initial view
- [ ] Search filters cities correctly
- [ ] Category dropdown filters work
- [ ] Clicking marker shows popup
- [ ] Clicking city in list zooms to location
- [ ] Detail panel shows correct information
- [ ] Map state persists (in ChatGPT)
- [ ] Mobile viewport renders correctly

## Deployment

### Local Development

```bash
cargo build --release
./target/release/mcp-apps-map
# Server runs on http://localhost:3001
```

### Production Deployment

The server is a standalone HTTP service that can be deployed anywhere:

```bash
# Build the release binary
cargo build --release

# Run with custom port
PORT=8080 ./target/release/mcp-apps-map
```

### With External Tile Server

For production, consider:
- Using a commercial tile provider (Mapbox, Google Maps)
- Self-hosting tiles with OpenMapTiles
- Caching tiles for offline use

### Environment Variables

```bash
# Server port (default: 3001)
export PORT=8080

# Optional: Configure tile server
export MAP_TILE_URL="https://your-tile-server/{z}/{x}/{y}.png"

# Optional: Configure city data source
export CITY_API_URL="https://your-api.com/cities"
```

## Troubleshooting

### Map tiles not loading

- Check internet connection (tiles load from OpenStreetMap)
- Verify no CORS issues in browser console
- Try a different tile provider

### Cities not appearing

- Check browser console for tool call errors
- Verify server is running and responding
- Check that coordinates are valid (lat: -90 to 90, lon: -180 to 180)

### Markers in wrong position

- Leaflet uses [lat, lon] order, not [lon, lat]
- Verify coordinate data in server response

## Learn More

- [Leaflet.js Documentation](https://leafletjs.com/reference.html)
- [MCP Apps Specification](https://github.com/anthropics/mcp/blob/main/proposals/sep-1865-mcp-apps.md)
- [OpenStreetMap Tile Usage Policy](https://operations.osmfoundation.org/policies/tiles/)
- [PMCP SDK Documentation](https://docs.rs/pmcp)
