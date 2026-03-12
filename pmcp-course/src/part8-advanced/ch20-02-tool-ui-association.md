# Tool-UI Association and Data Flow

In this section, you'll learn how to associate tools with their widgets, return structured data for widget rendering, enable multi-host support, and add output schema validation.

## Learning Objectives

By the end of this section, you will be able to:

- Associate a tool with its widget using `ToolInfo::with_ui()`
- Return data for widget rendering with `with_structured_content()`
- Enable multi-host support with `with_host_layer(HostType::ChatGpt)`
- Add ChatGPT-specific metadata with `WidgetMeta`
- Declare output schemas with `with_output_schema()`
- Explain the difference between `content` and `structuredContent`

## Associating Tools with Widgets

The `ToolInfo::with_ui()` method creates a tool definition that includes UI metadata linking the tool to its widget resource:

```rust
use pmcp::types::protocol::ToolInfo;
use serde_json::json;

let tool = ToolInfo::with_ui(
    "search_images",
    Some("Search for images by class name".to_string()),
    json!({
        "type": "object",
        "properties": {
            "class_name": { "type": "string" }
        },
        "required": ["class_name"]
    }),
    "ui://my-app/explorer.html",  // points to the widget resource
);
```

This produces `_meta: { "ui": { "resourceUri": "ui://my-app/explorer.html" } }` in the `tools/list` response. When a host sees this metadata, it knows that this tool has an associated widget and can render it when the tool is called.

The four parameters are:

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | `&str` | Tool name (e.g., `"search_images"`) |
| `description` | `Option<String>` | Human-readable description for the AI model |
| `input_schema` | `Value` | JSON Schema defining the tool's input parameters |
| `resource_uri` | `&str` | URI of the widget resource (must match `UIResource` registration) |

> **Important:** The `resource_uri` must exactly match the URI used in `UIResource::html_mcp_app()`. If these don't match, the host can't find the widget when the tool is called.

**Try this:** After registering a tool with `ToolInfo::with_ui()`, run `cargo pmcp preview --url http://localhost:3000` and check the Protocol tab. Verify that `tools/list` shows `_meta.ui.resourceUri` on your tool.

## Returning structuredContent

When a tool has an associated widget, the tool response should include `structuredContent` -- the data payload that the widget will render:

```rust
use pmcp::types::protocol::{CallToolResult, Content};
use serde_json::json;

let result = CallToolResult::new(vec![
    Content::text("Found 42 images of dogs"),
])
.with_structured_content(json!({
    "columns": [
        { "name": "image_id", "data_type": "varchar" },
        { "name": "thumbnail_url", "data_type": "varchar" }
    ],
    "rows": [
        { "image_id": "abc123", "thumbnail_url": "https://..." }
    ]
}));
```

### content vs structuredContent

The two fields serve different audiences:

| Field | Audience | Purpose |
|-------|----------|---------|
| `content` | AI model | Text the model reads to understand the result |
| `structuredContent` | Widget | JSON data the widget renders as a UI |

Both fields are visible to the model, but the AI primarily uses `content` for reasoning while the widget uses `structuredContent` for rendering. Always include both -- `content` with a human-readable summary, and `structuredContent` with the full data.

**Try this:** Return a tool result with `structuredContent` containing a JSON object. In your widget, access it via `hostContext.toolOutput` or the `ontoolresult` callback. Log it to the console to see the exact shape.

## Enabling Multi-Host Support

The `with_host_layer()` method on the server builder enables host-specific metadata enrichment. Register the ChatGPT host layer so the server adds `openai/*` keys to `_meta`:

```rust
use pmcp::types::mcp_apps::HostType;

Server::builder()
    .name("my-server")
    .version("1.0.0")
    .with_host_layer(HostType::ChatGpt)  // adds openai/* keys to _meta
    // ... tools, resources, etc.
    .build()
```

### What with_host_layer Does

Without `with_host_layer()`, your server emits standard metadata only (`_meta.ui.resourceUri`). This works for Claude Desktop and standard MCP hosts.

With `with_host_layer(HostType::ChatGpt)`, the server **also** emits ChatGPT-specific keys:

| Key | Value | Purpose |
|-----|-------|---------|
| `openai/outputTemplate` | Widget resource URI | Tells ChatGPT which widget to render |
| `openai/widgetAccessible` | `true` | Marks the tool as widget-capable |
| `openai/toolInvocation/*` | Tool invocation metadata | ChatGPT's tool result routing |

These keys are required for ChatGPT but harmless for other hosts -- they simply ignore unknown `_meta` keys. This is why `with_host_layer()` is the recommended default for any server that might be used with ChatGPT.

### When to Use It

- **Always recommended** if your server might be used with ChatGPT
- **Required** if you're targeting ChatGPT specifically
- **Optional** if you're only targeting Claude Desktop (standard metadata is sufficient)
- **Harmless** for hosts that don't understand `openai/*` keys

## ChatGPT-Specific Metadata with WidgetMeta

For additional ChatGPT customization, use `WidgetMeta`:

```rust
use pmcp::types::mcp_apps::WidgetMeta;

let tool = ToolInfo::with_ui("my_tool", None, schema, "ui://my-app/widget.html")
    .with_widget_meta(WidgetMeta::new().prefers_border(true));
```

### WidgetMeta Fields

| Method | Effect | Description |
|--------|--------|-------------|
| `.prefers_border(true)` | Adds border around widget | ChatGPT-specific visual preference |
| `.description("...")` | Widget self-description | Helps the AI understand the widget |
| `.csp(widget_csp)` | Content Security Policy | Declares external domains (see ch20-01) |
| `.resource_uri("ui://...")` | Resource URI override | Usually set automatically by `with_ui()` |

## Adding outputSchema

`outputSchema` tells the host the shape of `structuredContent`, enabling validation. It is a top-level field on `ToolInfo` (per MCP spec 2025-06-18), not in annotations:

```rust
let tool = ToolInfo::with_ui(
    "search_images",
    None,
    input_schema,
    "ui://my-app/explorer.html",
)
.with_output_schema(json!({
    "type": "object",
    "properties": {
        "columns": { "type": "array" },
        "rows": { "type": "array" }
    }
}));
```

Adding `outputSchema` is optional but recommended. It enables hosts to validate `structuredContent` against the declared schema and helps AI models understand the structure of tool output.

**Try this:** Add an `outputSchema` to one of your tools, then run `mcp-tester apps http://localhost:3000` to verify the validator detects and checks it.

## Complete Server Example

Here is a complete server setup combining all the concepts from this section:

```rust
use pmcp::types::mcp_apps::{HostType, WidgetMeta, WidgetCSP};
use pmcp::types::protocol::{ToolInfo, CallToolResult, Content};
use pmcp::types::ui::{UIResource, UIResourceContents};
use serde_json::json;

// Widget HTML embedded at compile time
const EXPLORER_HTML: &str = include_str!("../../widget/dist/explorer.html");

// 1. Define the tool with UI metadata
let tool = ToolInfo::with_ui(
    "search_images",
    Some("Search for images by class name".to_string()),
    json!({
        "type": "object",
        "properties": {
            "class_name": { "type": "string" }
        },
        "required": ["class_name"]
    }),
    "ui://my-app/explorer.html",
)
.with_output_schema(json!({
    "type": "object",
    "properties": {
        "columns": { "type": "array" },
        "rows": { "type": "array" }
    }
}))
.with_widget_meta(WidgetMeta::new().prefers_border(true));

// 2. Define the widget resource
let resource = UIResource::html_mcp_app(
    "ui://my-app/explorer.html",
    "Image Explorer",
);
let contents = UIResourceContents::html(
    "ui://my-app/explorer.html",
    EXPLORER_HTML,
);

// 3. Build the server with host layer
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .with_host_layer(HostType::ChatGpt)
    // register tool and resources...
    .build()?;
```

## Summary and Next Steps

Let's recap what you've learned:

- **`ToolInfo::with_ui()`** associates a tool with its widget by adding `_meta.ui.resourceUri` to the `tools/list` response
- **`with_structured_content()`** returns data alongside text so widgets can render results
- **`content`** is for the AI model; **`structuredContent`** is for the widget (both are visible to the model)
- **`with_host_layer(HostType::ChatGpt)`** enables ChatGPT-specific metadata enrichment while remaining compatible with all other hosts
- **`WidgetMeta`** provides ChatGPT-specific customization (border, description, CSP)
- **`with_output_schema()`** declares the shape of `structuredContent` for validation

In the next section, you'll learn how to build widgets using the ext-apps `App` class, implement required protocol handlers, and bundle with Vite.

---

*Continue to [Widget Communication with ext-apps](./ch20-03-postmessage.md) ->*
