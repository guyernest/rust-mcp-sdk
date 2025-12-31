::: exercise
id: ch20-01-ui-resources
difficulty: advanced
time: 50 minutes
:::

Create MCP UI resources with graceful degradation. This is an **experimental**
feature - the API may change and most clients don't support it yet.

> **Warning**: MCP Apps (SEP-1865) is experimental. Always design servers
> that work without UI support. UI is an enhancement, not a requirement.

::: objectives
thinking:
  - Why graceful degradation is essential for UI resources
  - The security model for embedded HTML (iframe isolation)
  - When visual presentation adds value vs when it's unnecessary
doing:
  - Build a UIResource with the ui:// scheme
  - Create HTML template with postMessage communication
  - Link tool to UI with .with_ui()
  - Verify graceful degradation for non-UI clients
:::

::: discussion
- What data in your server would genuinely benefit from visual display?
- How do you feel about building on an experimental API?
- What happens if a client ignores your UI resource?
:::

## Step 1: Design for Degradation First

The tool must return complete data without UI:

```rust
/// Tool returns full data - works with or without UI
async fn get_room_images(
    input: RoomImagesInput,
    context: &ToolContext,
) -> Result<Value> {
    let images = fetch_images(&input.hotel_id, &input.room_type).await?;

    // Complete data for all clients
    Ok(json!({
        "hotel_id": input.hotel_id,
        "room_type": input.room_type,
        "image_count": images.len(),
        "images": images.iter().map(|img| json!({
            "id": img.id,
            "thumbnail": img.thumbnail_url,
            "full": img.full_url,
            "caption": img.caption,
            "dimensions": {
                "width": img.width,
                "height": img.height
            }
        })).collect::<Vec<_>>()
    }))
}
```

## Step 2: Create UI Resource

```rust
use pmcp::{UIResourceBuilder, ResourceCollection};

const GALLERY_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: system-ui; margin: 0; padding: 16px; }
        .gallery {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
            gap: 16px;
        }
        .gallery img {
            width: 100%;
            border-radius: 8px;
            cursor: pointer;
            transition: transform 0.2s;
        }
        .gallery img:hover {
            transform: scale(1.05);
        }
        .caption { font-size: 12px; color: #666; margin-top: 4px; }
    </style>
</head>
<body>
    <div class="gallery" id="gallery"></div>
    <script>
        // Wait for data from MCP host
        window.addEventListener('message', (event) => {
            // Validate origin for security
            if (event.data.type === 'mcp-tool-result') {
                renderGallery(event.data.result.images);
            }
        });

        function renderGallery(images) {
            const gallery = document.getElementById('gallery');
            gallery.innerHTML = '';

            images.forEach(img => {
                const container = document.createElement('div');

                const imgEl = document.createElement('img');
                imgEl.src = img.thumbnail;
                imgEl.alt = img.caption;
                imgEl.onclick = () => window.open(img.full, '_blank');

                const caption = document.createElement('div');
                caption.className = 'caption';
                caption.textContent = img.caption;

                container.appendChild(imgEl);
                container.appendChild(caption);
                gallery.appendChild(container);
            });
        }

        // Signal ready to parent
        window.parent.postMessage({ type: 'mcp-ui-ready' }, '*');
    </script>
</body>
</html>"#;

fn create_ui_resources() -> ResourceCollection {
    let (resource, contents) = UIResourceBuilder::new(
        "ui://hotel/gallery",
        "Room Photo Gallery"
    )
        .description("Interactive photo gallery for hotel rooms")
        .html_template(GALLERY_HTML)
        .build_with_contents()
        .expect("Failed to build UI resource");

    ResourceCollection::new()
        .add_ui_resource(resource, contents)
}
```

## Step 3: Link Tool to UI

```rust
let tool = TypedTool::new("get_room_images", get_room_images)
    .with_description("Get photos for a hotel room type")
    .with_ui("ui://hotel/gallery");  // UI-capable clients render this
```

## Step 4: Add to Server

```rust
let server = ServerBuilder::new("hotel-server", "1.0.0")
    .resources(create_ui_resources())
    .tool(tool)
    .build()?;
```

## Step 5: Test Without UI

Verify the tool works without UI support:

```bash
# Tool should return complete JSON data
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "get_room_images",
      "arguments": {"hotel_id": "123", "room_type": "suite"}
    },
    "id": 1
  }'

# Response includes all image data, regardless of UI capability
```

## Step 6: Test With UI (if available)

If you have a UI-capable MCP client:
1. Connect to your server
2. Call the get_room_images tool
3. Observe the gallery UI rendering
4. Verify clicking images opens full-size versions

::: hints
level_1: "Tool result MUST contain complete data as JSON. UI is enhancement only."
level_2: "Validate postMessage origin in production - accept only expected parent origins."
level_3: "Keep UIs simple - display data, handle clicks. Complex logic belongs in tools."
:::

## Success Criteria

- [ ] UIResource created with valid ui:// URI
- [ ] HTML template uses proper message handling
- [ ] Tool linked to UI with .with_ui()
- [ ] Tool returns complete data for non-UI clients
- [ ] PostMessage communication handles data correctly
- [ ] Graceful experience verified without UI support

---

*UI resources are part of the experimental [MCP Apps](../../part8-advanced/ch20-applications.md) specification.*
