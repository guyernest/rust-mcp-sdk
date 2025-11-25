//! Hotel Room Gallery Example
//!
//! Demonstrates MCP Apps Extension (SEP-1865) with an image gallery.
//!
//! This example shows:
//! - TypedTool with `.with_ui()` for tool-UI association
//! - UIResourceBuilder for creating HTML-based UIs
//! - Image gallery with lightbox functionality
//! - Responsive grid layout
//!
//! Run with: cargo run --example hotel_gallery

use pmcp::{
    RequestHandlerExtra, ResourceCollection, Server, ServerCapabilities, TypedTool,
    UIResourceBuilder,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// Tool handler that returns hotel room images
async fn get_room_images(
    args: GetRoomImagesArgs,
    _extra: RequestHandlerExtra,
) -> pmcp::Result<serde_json::Value> {
    // In a real implementation, this would fetch from a CDN or database
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
            RoomImage {
                id: "room-2".to_string(),
                url: "https://images.unsplash.com/photo-1582719478250-c89cae4dc85b?w=1200"
                    .to_string(),
                thumbnail_url: "https://images.unsplash.com/photo-1582719478250-c89cae4dc85b?w=400"
                    .to_string(),
                title: "Bathroom".to_string(),
                description: "Marble bathroom with rainfall shower".to_string(),
            },
            RoomImage {
                id: "room-3".to_string(),
                url: "https://images.unsplash.com/photo-1618773928121-c32242e63f39?w=1200"
                    .to_string(),
                thumbnail_url: "https://images.unsplash.com/photo-1618773928121-c32242e63f39?w=400"
                    .to_string(),
                title: "Living Area".to_string(),
                description: "Comfortable seating area with work desk".to_string(),
            },
            RoomImage {
                id: "room-4".to_string(),
                url: "https://images.unsplash.com/photo-1584132967334-10e028bd69f7?w=1200"
                    .to_string(),
                thumbnail_url: "https://images.unsplash.com/photo-1584132967334-10e028bd69f7?w=400"
                    .to_string(),
                title: "City View".to_string(),
                description: "Panoramic city views from floor-to-ceiling windows".to_string(),
            },
        ],
        _ => vec![],
    };

    let result = RoomGalleryResult {
        hotel: args.hotel_id,
        room_type: args.room_type,
        images,
    };

    Ok(serde_json::to_value(result)?)
}

const GALLERY_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Hotel Room Gallery</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }
        .header {
            text-align: center;
            margin-bottom: 30px;
        }
        .header h1 {
            color: #2c3e50;
            font-size: 28px;
            margin-bottom: 8px;
        }
        .header p {
            color: #7f8c8d;
            font-size: 16px;
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
            transition: transform 0.2s, box-shadow 0.2s;
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
            display: block;
        }
        .gallery-item-info {
            padding: 16px;
        }
        .gallery-item-info h3 {
            color: #2c3e50;
            font-size: 18px;
            margin-bottom: 8px;
        }
        .gallery-item-info p {
            color: #7f8c8d;
            font-size: 14px;
            line-height: 1.5;
        }

        /* Lightbox */
        .lightbox {
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0, 0, 0, 0.9);
            z-index: 1000;
            align-items: center;
            justify-content: center;
        }
        .lightbox.active {
            display: flex;
        }
        .lightbox-content {
            max-width: 90%;
            max-height: 90%;
            position: relative;
        }
        .lightbox-content img {
            max-width: 100%;
            max-height: 80vh;
            display: block;
            margin: 0 auto;
        }
        .lightbox-caption {
            color: white;
            text-align: center;
            padding: 20px;
        }
        .lightbox-caption h3 {
            font-size: 24px;
            margin-bottom: 8px;
        }
        .lightbox-caption p {
            font-size: 16px;
            color: #ccc;
        }
        .lightbox-close {
            position: absolute;
            top: 20px;
            right: 30px;
            color: white;
            font-size: 40px;
            font-weight: bold;
            cursor: pointer;
            transition: color 0.2s;
        }
        .lightbox-close:hover {
            color: #3498db;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1 id="room-title">Hotel Room Gallery</h1>
        <p id="room-subtitle">Loading images...</p>
    </div>

    <div class="gallery" id="gallery"></div>

    <!-- Lightbox -->
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

        // Close lightbox on escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') closeLightbox();
        });

        // Close lightbox on background click
        document.getElementById('lightbox').addEventListener('click', (e) => {
            if (e.target.id === 'lightbox') closeLightbox();
        });

        // MCP postMessage handler to receive image data
        window.addEventListener('message', (event) => {
            if (event.data.type === 'mcp-tool-result') {
                const data = event.data.result;
                currentImages = data.images || [];

                // Update header
                document.getElementById('room-title').textContent =
                    `${data.room_type.charAt(0).toUpperCase() + data.room_type.slice(1)} Room`;
                document.getElementById('room-subtitle').textContent =
                    `${data.hotel} - ${currentImages.length} photos`;

                // Render gallery
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

        // Request room images via MCP
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
    </script>
</body>
</html>"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the room gallery UI resource
    let (ui_resource, ui_contents) =
        UIResourceBuilder::new("ui://hotel/room-gallery", "Hotel Room Gallery")
            .description("Interactive photo gallery for hotel rooms")
            .html_template(GALLERY_HTML)
            .build_with_contents()?;

    // Build resource collection with the UI resource
    let resources = ResourceCollection::new().add_ui_resource(ui_resource, ui_contents);

    // Create the tool with UI association (using closure for TypedTool)
    let get_images_tool = TypedTool::new("get_room_images", |args, extra| {
        Box::pin(get_room_images(args, extra))
    })
    .with_description("Get photo gallery for a hotel room")
    .with_ui("ui://hotel/room-gallery");

    // Create server with tool and resources
    let server = Server::builder()
        .name("hotel-gallery-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            resources: Some(Default::default()),
            tools: Some(Default::default()),
            ..Default::default()
        })
        .resources(resources)
        .tool("get_room_images", get_images_tool)
        .build()?;

    // Run with stdio transport
    server.run_stdio().await?;

    Ok(())
}
