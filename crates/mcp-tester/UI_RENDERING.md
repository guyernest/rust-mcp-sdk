# MCP UI Rendering in mcp-tester

This document explains how to use mcp-tester to render and view interactive UIs from MCP servers that implement the MCP Apps Extension (SEP-1865).

## Overview

The mcp-tester now supports:
- ✅ **Automatic UI discovery** - Detects tools with `ui://` resource metadata
- ✅ **UI resource fetching** - Retrieves HTML content from the server
- ✅ **Static HTML rendering** - Generates standalone HTML files with debug panel
- ✅ **PostMessage bridge** - Logs tool calls from the UI (static mode)
- ✅ **Developer-friendly** - Sandboxed iframe with toggleable debug panel

## Quick Start

### 1. Start an MCP Server with UI Support

Example: Conference Venue Map
```bash
cargo run --example conference_venue_map --features schema-generation
```

This starts a server on `http://localhost:3004` with an interactive map UI.

### 2. Render the UI

```bash
cargo run --package mcp-tester --example render_ui -- http://localhost:3004
```

**Output:**
```
🔍 Connecting to MCP server at: http://localhost:3004

📡 Initializing connection...
✅ Connected successfully

🔧 Discovering tools...
✅ Found tools

🎨 Discovering tools with UIs...
✅ Found 1 tool(s) with UIs:
   - get_conference_venues → ui://conference/venue-map

📝 Rendering UIs to HTML files...
✅ Rendered UI for tool 'get_conference_venues' to: get-conference-venues_ui.html
   Open in browser: file:///path/to/get-conference-venues_ui.html

✅ Done! UI files generated.
```

### 3. Open in Browser

```bash
open get-conference-venues_ui.html
```

The HTML file includes:
- **Main UI panel** (left): The interactive map/gallery/UI
- **Debug panel** (right): Logs tool calls and postMessage communication
- **Toggle button**: Show/hide debug panel

## How It Works

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Browser (Static HTML Viewer)                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────┐  ┌──────────────────────────────┐   │
│  │  Debug Panel         │  │  Sandboxed Iframe            │   │
│  │                      │  │                              │   │
│  │  Status: Ready       │  │  ┌─────────────────────┐    │   │
│  │                      │  │  │                     │    │   │
│  │  Tool Calls Log:     │  │  │  Original MCP UI    │    │   │
│  │                      │  │  │  (Map/Gallery/etc)  │    │   │
│  │  [timestamp] Tool    │  │  │                     │    │   │
│  │  call: get_venues    │  │  └─────────────────────┘    │   │
│  │                      │  │                              │   │
│  │  [timestamp] Args:   │  │  Uses postMessage to call    │   │
│  │  {...}               │  │  MCP tools                   │   │
│  │                      │  │                              │   │
│  └──────────────────────┘  └──────────────────────────────┘   │
│         ▲                              │                       │
│         │    PostMessage Bridge        │                       │
│         └──────────────────────────────┘                       │
│                                                                  │
│  Note: Static mode - tool calls logged but not executed        │
└─────────────────────────────────────────────────────────────────┘
```

### Detection Flow

1. **Connect to MCP server** via HTTP/stdio/JSON-RPC
2. **List tools** using `tools/list` method
3. **Inspect `ToolInfo._meta`** for `ui/resourceUri` field
4. **Fetch UI resource** using `resources/read` method
5. **Extract HTML content** from `Content::Text` or `Content::Resource`
6. **Wrap with debug bridge** - Adds debug panel and postMessage logging
7. **Save to file** - Standalone HTML file ready to open

### Code Example

```rust
use mcp_tester::tester::ServerTester;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Create tester
    let mut tester = ServerTester::new(
        "http://localhost:3004",
        Duration::from_secs(30),
        false,  // not insecure
        None,   // no API key
        Some("http"),  // force HTTP transport
        None,   // no middleware
    )?;

    // Initialize
    tester.test_initialize().await;
    tester.test_tools_list().await;

    // Discover and load UIs
    tester.load_all_tool_uis().await?;

    // Get UI info
    for (tool_name, ui_info) in tester.get_tool_uis() {
        println!("Tool: {} -> UI: {}", tool_name, ui_info.ui_resource_uri);

        // Render to HTML
        tester.render_tool_ui(
            tool_name,
            &format!("{}_ui.html", tool_name)
        )?;
    }

    Ok(())
}
```

## API Reference

### `ServerTester` Methods

#### `discover_tool_uis() -> Result<Vec<ToolUIInfo>>`
Scans all tools and extracts those with UI metadata (`_meta["ui/resourceUri"]`).

**Returns:**
```rust
pub struct ToolUIInfo {
    pub tool_name: String,
    pub ui_resource_uri: String,
    pub html_content: Option<String>,
}
```

#### `fetch_ui_resource(uri: &str) -> Result<String>`
Fetches the HTML content for a given `ui://` resource URI.

**Example:**
```rust
let html = tester.fetch_ui_resource("ui://conference/venue-map").await?;
```

#### `load_all_tool_uis() -> Result<()>`
Discovers all tools with UIs and fetches their HTML content in one call.

**Side effect:** Populates internal `tool_uis` HashMap.

#### `get_tool_uis() -> &HashMap<String, ToolUIInfo>`
Returns reference to discovered tool UIs.

#### `render_tool_ui(tool_name: &str, output_path: &str) -> Result<()>`
Renders a tool's UI to an HTML file with debug panel and postMessage bridge.

**Example:**
```rust
tester.render_tool_ui("get_conference_venues", "venue_map.html")?;
```

## Debug Panel Features

The generated HTML includes a debug panel with:

### Visual Design
- **Dark theme** for comfortable viewing
- **Fixed position** on the right side
- **Scrollable log** with syntax highlighting
- **Toggleable** - Click button to show/hide

### Logged Events
- ✅ **UI loaded** - When iframe initializes
- ✅ **PostMessage bridge** - When communication is ready
- 🔧 **Tool calls** - Method name and arguments (JSON formatted)
- ⚠️ **Warnings** - Static mode notices
- ❌ **Errors** - If tool call fails

### Log Entry Types
- `info` - Blue border - General information
- `success` - Green border - Successful operations
- `warning` - Orange border - Notices
- `error` - Red border - Errors

## Limitations (Static Mode)

The current implementation is a **static viewer**:

### What Works
- ✅ UI renders correctly in iframe
- ✅ Tool calls are detected and logged
- ✅ PostMessage events are captured
- ✅ Inspect tool call arguments and structure
- ✅ Debug UI communication flow

### What Doesn't Work
- ❌ Tool calls are **not executed** (no connection to MCP server)
- ❌ UI receives error responses (static mode message)
- ❌ No real-time data from server
- ❌ Can't test end-to-end tool execution

### Future: Interactive Mode
For interactive testing with real tool execution, use:
```bash
cargo pmcp test --serve-ui
```

This will:
- Start HTTP server with MCP proxy
- Serve UI with live postMessage ↔ MCP bridge
- Execute actual tool calls
- Show real-time results

## Example Servers with UIs

### 1. Conference Venue Map
```bash
cargo run --example conference_venue_map --features schema-generation
```

**Features:**
- Interactive Leaflet.js map
- Venue markers with popups
- Tool: `get_conference_venues`
- UI: `ui://conference/venue-map`

**Test:**
```bash
cargo run --package mcp-tester --example render_ui -- http://localhost:3004
```

### 2. Hotel Room Gallery
```bash
cargo run --example hotel_gallery --features schema-generation
```

**Features:**
- Responsive image gallery
- Lightbox for full-screen viewing
- Tool: `get_room_images`
- UI: `ui://hotel/room-gallery`

**Test:**
```bash
cargo run --package mcp-tester --example render_ui -- http://localhost:3005
```

## Troubleshooting

### "No tools with UI metadata found"

**Cause:** Server doesn't implement MCP Apps Extension

**Solution:**
1. Verify server uses `UIResourceBuilder` to create UI resources
2. Verify tools use `.with_ui("ui://...")` to associate with UI
3. Check `ToolInfo._meta` field contains `"ui/resourceUri"`

### "No text content found in UI resource"

**Cause:** UI resource doesn't return HTML content

**Solution:**
1. Verify `ReadResourceResult.contents` contains `Content::Text` or `Content::Resource` with text
2. Check UI resource handler returns HTML string
3. Ensure MIME type is `text/html+mcp` or compatible

### "Failed to fetch UI for tool 'X'"

**Cause:** Resource read failed

**Solution:**
1. Check resource URI is correct (`ui://...`)
2. Verify server implements `resources/read` handler
3. Check network connection to server
4. Enable debug logging: `RUST_LOG=debug`

### UI doesn't load in browser

**Cause:** Browser security restrictions

**Solution:**
1. Open from `file://` protocol (drag into browser)
2. Use local HTTP server: `python -m http.server` then open `http://localhost:8000/file.html`
3. Check browser console for errors

## Next Steps

1. ✅ **Try the example**: Run `render_ui` with conference venue map
2. ✅ **Inspect the HTML**: Open generated file and toggle debug panel
3. ✅ **Check tool calls**: Interact with UI and watch debug log
4. 🔜 **Interactive mode**: Wait for `--serve-ui` implementation
5. 🔜 **Custom scenarios**: Add UI rendering to test scenarios

## Contributing

To add UI rendering to test scenarios:

1. **Add to scenario YAML:**
```yaml
steps:
  - name: render_ui
    type: custom
    tool: get_data
    expect_ui: true
    render_to: output/data_viewer.html
```

2. **Implement in test runner:**
```rust
if step.expect_ui {
    tester.load_all_tool_uis().await?;
    tester.render_tool_ui(&step.tool, &step.render_to)?;
}
```

## Resources

- **MCP Apps Extension Spec**: [SEP-1865](https://spec.modelcontextprotocol.io/)
- **UI Examples**: `examples/conference_venue_map.rs`, `examples/hotel_gallery.rs`
- **Book Chapter**: `pmcp-book/src/ch12-5-mcp-apps.md`
- **Advanced Guide**: `docs/advanced/mcp-apps-extension.md`

## Summary

The mcp-tester UI rendering feature provides a simple way to:
- **Discover** tools with interactive UIs
- **Extract** and **render** HTML content
- **Debug** postMessage communication
- **Visualize** UI behavior in a browser

This is perfect for:
- ✅ Manual testing and inspection
- ✅ Demo and documentation
- ✅ UI development iteration
- ✅ Protocol compliance verification

For full end-to-end testing with live tool execution, stay tuned for the interactive mode!
