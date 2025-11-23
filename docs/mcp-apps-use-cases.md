# MCP Apps Use Cases - Concrete Examples

**Related**: [MCP Apps Implementation Plan](./mcp-apps-implementation.md)
**Status**: Planning
**Last Updated**: 2025-11-23

## Overview

This document provides concrete use cases and implementation examples for MCP Apps Extension across three key categories:

1. **Maps** - Geographic visualization and navigation
2. **Images** - Photo galleries and visual content
3. **Charts** - Data visualization and analytics

Each use case is mapped to our three-phase implementation strategy.

---

## 1. Maps

### Use Cases

#### A. Conference/Event Planning (Las Vegas)
**Scenario**: AWS re:Invent 2025 conference with 50+ venues across Las Vegas
**User need**: Visualize all session locations, plan routes between talks
**Interactive elements**: Click venue for details, filter by track, route planning

#### B. Transit Planning (London Underground)
**Scenario**: Navigate London's complex transit system
**User need**: See routes, real-time updates, station amenities
**Interactive elements**: Route visualization, live status, journey times

#### C. Interactive Fiction/Gaming
**Scenario**: Text-based adventure game with procedurally generated dungeons
**User need**: Visual map that updates as player explores
**Interactive elements**: Fog of war, dynamic maze generation, custom styling

### Technology Choices

**Phase 1** (HTML):
- **Leaflet.js** - Lightweight, MIT license, 42KB gzipped
- OpenStreetMap tiles (free)
- Marker clustering for many points
- Custom popups with MCP data

**Phase 2** (Templates):
- Pre-built templates for common map types
- Route planning helper functions
- Geocoding integration examples
- Procedural generation patterns

**Phase 3** (WASM):
- **maplibre-rs** - Rust native mapping
- Compile-time coordinate validation
- Custom tile rendering in Rust
- Better performance, smaller bundle

### Example Implementation (Phase 1)

See complete code in [use cases analysis document](/tmp/mcp-apps-use-cases.md#maps-implementation)

**Key features**:
```rust
// Rust server provides venue data
async fn get_conference_venues(
    args: ConferenceVenuesArgs
) -> Result<Vec<Venue>> {
    Ok(vec![
        Venue {
            id: "mandalay-bay",
            name: "Mandalay Bay Convention Center",
            lat: 36.0915,
            lon: -115.1739,
            capacity: 2000,
        },
        // More venues...
    ])
}

// UI consumes via MCP
const result = await client.request({
    method: 'tools/call',
    params: {
        name: 'get_conference_venues',
        arguments: { conference_id: 'aws-reinvent-2025' }
    }
});
```

---

## 2. Images

### Use Cases

#### A. E-commerce/Hospitality
**Scenario**: Hotel booking site showing room options
**User need**: Browse high-quality room photos
**Interactive elements**: Thumbnail grid, lightbox zoom, 360° views

#### B. Data Visualization
**Scenario**: System architecture diagram generator
**User need**: Visualize complex diagrams
**Interactive elements**: SVG with clickable elements, annotations

#### C. Creative Content
**Scenario**: AI image generation results
**User need**: Display and compare generated images
**Interactive elements**: Side-by-side comparison, metadata display

### Technology Choices

**Phase 1** (HTML):
- **Lightbox2** - Popular image viewer, 12KB gzipped
- Lazy loading with IntersectionObserver
- Responsive image srcset
- WebP with JPEG fallback

**Phase 2** (Templates):
- Gallery grid template
- Lightbox template
- Image comparison template
- 360° viewer template

**Phase 3** (WASM):
- **image-rs** - Rust image processing
- Client-side thumbnail generation
- WebP/AVIF encoding in browser
- Advanced filters (blur, sharpen, color adjust)

### Example Implementation (Phase 1)

**Key features**:
```rust
// Rust server provides image data
#[derive(Serialize, JsonSchema)]
struct HotelImage {
    id: String,
    thumbnail_url: String,
    full_url: String,
    description: String,
    alt_text: String,
}

async fn get_hotel_images(
    args: HotelImagesArgs
) -> Result<Vec<HotelImage>> {
    // Fetch from database/CDN
    Ok(images)
}
```

```html
<!-- UI displays with lightbox -->
<div class="gallery">
    <a href="full.jpg" data-lightbox="rooms" data-title="Deluxe King">
        <img src="thumb.jpg" alt="Deluxe King Room" />
    </a>
</div>
```

---

## 3. Charts

### Use Cases

#### A. Data Analysis (SQL/Polars)
**Scenario**: Business analyst queries sales database
**User need**: Visualize query results as interactive charts
**Interactive elements**: Hover tooltips, zoom, filter, export

#### B. Business Intelligence
**Scenario**: Executive dashboard with KPIs
**User need**: Real-time metrics visualization
**Interactive elements**: Time range selector, drill-down, comparisons

#### C. Scientific/Research
**Scenario**: ML experiment results
**User need**: Plot model performance, parameter sweeps
**Interactive elements**: Multi-line comparison, confidence intervals

### Technology Choices

**Phase 1** (HTML):
- **Vega-Lite** - Declarative, Altair-compatible, 150KB gzipped
- Grammar of Graphics approach
- Polars → Vega-Lite pipeline
- Export to PNG/SVG

**Phase 2** (Templates):
- Chart type templates (line, bar, scatter, etc.)
- Polars integration helper
- Dashboard layout template
- Real-time update patterns

**Phase 3** (WASM):
- **plotters-rs** - Rust native charting
- Type-safe data binding
- Canvas rendering from Rust
- Better performance for large datasets

### Example Implementation (Phase 1)

**Polars Integration**:
```rust
use polars::prelude::*;

async fn analyze_sales(args: AnalyzeArgs) -> Result<QueryResult> {
    // Execute query with Polars
    let df = CsvReader::from_path("sales.csv")?
        .finish()?
        .lazy()
        .group_by([col("date")])
        .agg([col("amount").sum().alias("revenue")])
        .collect()?;

    // Convert to JSON for Vega-Lite
    let results: Vec<serde_json::Value> = df_to_json(&df)?;

    Ok(QueryResult { results })
}
```

**Vega-Lite Visualization**:
```javascript
const spec = {
    "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
    "data": { "values": queryResults },
    "mark": "line",
    "encoding": {
        "x": { "field": "date", "type": "temporal" },
        "y": { "field": "revenue", "type": "quantitative" }
    }
};
vegaEmbed('#chart', spec);
```

---

## Implementation Priority

### Phase 1 Examples (Must Have)

1. **Conference Venue Map**
   - Demonstrates Leaflet integration
   - Shows marker clustering
   - MCP data binding pattern
   - ~200 lines HTML + ~100 lines Rust

2. **Hotel Room Gallery**
   - Image handling best practices
   - Lightbox interaction
   - Lazy loading demonstration
   - ~150 lines HTML + ~80 lines Rust

3. **Sales Analytics Chart**
   - Vega-Lite integration
   - Polars → Vega pipeline
   - Interactive filtering
   - ~180 lines HTML + ~120 lines Rust

**Estimated effort**: 4-6 days total

### Phase 2 Templates (Should Have)

4. **London Transit Map**
   - Route visualization template
   - Real-time update pattern
   - Multi-layer maps

5. **Product Catalog Gallery**
   - E-commerce template
   - Grid + lightbox combo
   - Filtering/sorting UI

6. **Business Dashboard**
   - Multi-chart template
   - Shared data source
   - Responsive layout

7. **Game Maze Generator**
   - Canvas-based procedural map
   - Dynamic updates
   - Custom styling

**Estimated effort**: 6-8 days total

### Phase 3 WASM (Nice to Have)

8. **High-Performance Map** (maplibre-rs)
9. **Client-Side Image Editor** (image-rs)
10. **Real-Time Chart** (plotters-rs)

**Estimated effort**: 2-3 weeks

---

## Template Library Structure

```
cargo-pmcp/templates/ui/
├── maps/
│   ├── leaflet-markers/
│   │   ├── template.html
│   │   ├── example.rs
│   │   └── README.md
│   ├── leaflet-routes/
│   ├── procedural-maze/
│   └── maplibre-wasm/     # Phase 3
├── images/
│   ├── gallery-grid/
│   ├── gallery-lightbox/
│   ├── image-compare/
│   └── image-editor-wasm/ # Phase 3
└── charts/
    ├── vega-line/
    ├── vega-bar/
    ├── vega-dashboard/
    └── plotters-wasm/     # Phase 3
```

---

## Developer Experience

### Scaffolding

```bash
# Phase 1: Manual setup
cargo pmcp new my-server
# ... manually create UI resources

# Phase 2: Scaffolding
cargo pmcp ui add venue-map --template map-leaflet
# Generates:
# - ui/venue-map.html (customizable)
# - Registers in server code
# - Includes example Rust handler

# Phase 3: WASM
cargo pmcp ui add fast-chart --template plotters-wasm --wasm
# Generates:
# - ui/fast-chart/src/lib.rs (Rust component)
# - Automatic WASM build setup
# - Hot reload configuration
```

### Preview

```bash
cargo pmcp dev --ui-preview

# Opens http://localhost:3001/ui
# Shows:
# - All registered UI resources
# - Filter by type (Maps, Images, Charts)
# - Live preview with mock data
# - Side-by-side: UI + Tool output
```

---

## Polars Integration Helper

### Phase 2 Addition

```rust
// In pmcp crate
pub mod ui {
    pub mod charts {
        use polars::prelude::*;

        /// Convert Polars DataFrame to Vega-Lite spec
        pub fn dataframe_to_vega(
            df: &DataFrame,
            chart_type: ChartType,
            x_field: &str,
            y_field: &str,
        ) -> Result<serde_json::Value> {
            let data: Vec<serde_json::Value> = df
                .iter()
                .map(|row| row_to_json(row))
                .collect();

            Ok(serde_json::json!({
                "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
                "data": { "values": data },
                "mark": chart_type.as_str(),
                "encoding": {
                    "x": { "field": x_field, "type": "temporal" },
                    "y": { "field": y_field, "type": "quantitative" }
                }
            }))
        }
    }
}
```

**Usage**:
```rust
use pmcp::ui::charts::dataframe_to_vega;

async fn analyze(args: Args) -> Result<ChartOutput> {
    let df = execute_query(&args.query)?;

    Ok(ChartOutput {
        vega_spec: dataframe_to_vega(&df, ChartType::Line, "date", "revenue")?
    })
}
```

---

## Success Metrics by Use Case

### Maps
- [ ] Conference map loads < 1 second
- [ ] Handles 100+ markers smoothly
- [ ] Click-to-details latency < 200ms
- [ ] Mobile responsive

### Images
- [ ] Gallery loads < 2 seconds
- [ ] Lightbox transitions smooth (60fps)
- [ ] Lazy loading works correctly
- [ ] Supports 50+ images

### Charts
- [ ] Chart renders < 500ms
- [ ] Interactive hover responsive
- [ ] Handles 10,000+ data points
- [ ] Export works correctly

---

## Security Considerations by Use Case

### Maps
- **Threat**: Malicious tile server URLs
- **Mitigation**: Whitelist tile providers, CSP headers
- **Validation**: Check coordinate bounds

### Images
- **Threat**: XSS via image URLs
- **Mitigation**: URL validation, CSP img-src directive
- **Validation**: Check file types, size limits

### Charts
- **Threat**: Code injection via data values
- **Mitigation**: Vega-Lite sandboxing, data sanitization
- **Validation**: Type checking in Rust, schema validation

---

## Next Steps

1. **This Week**: Review use cases, prioritize Phase 1 examples
2. **Week 1-2**: Implement conference map, hotel gallery, sales chart
3. **Week 3-4**: Create templates, Polars helper, preview server
4. **Month 3+**: Evaluate WASM frameworks, prototype examples

---

**Document Version**: 1.0
**Authors**: Claude Code, Guy Ernest
**Status**: Planning
**Related**: [Implementation Plan](./mcp-apps-implementation.md)
