# MCP Apps

MCP Apps is a PMCP extension that enables rich HTML widget UIs for tools.
When a tool has UI metadata, compatible MCP hosts render interactive widgets
alongside the tool's structured output.

## Adding UI to a Tool

Use `.with_ui()` on any typed tool:

```rust
use pmcp::types::UIMimeType;

server_builder.tool_typed(
    MyTool.with_ui(UIMimeType::Html, "widget://my-widget")
);
```

The `UIMimeType` specifies the widget content type:
- `UIMimeType::Html` -- standalone HTML widget
- `UIMimeType::HtmlMcpApp` -- MCP App with bridge integration

## Widget Resources

Widgets are served as resources with `widget://` URIs. The host loads the
widget in an iframe and bridges it with the tool's structured output.

```rust
// Register a widget resource
server_builder.resource_handler("widget://", WidgetResourceHandler);
```

## _meta Emission

When a tool has UI, the SDK automatically emits `_meta` in the tool result:

```json
{
  "structuredContent": { "value": 42 },
  "_meta": {
    "ui": {
      "resourceUri": "widget://my-widget"
    }
  }
}
```

The host uses this to locate and render the widget.

## Widget Runtime Bridge

Widgets communicate with the host via the `ext-apps` bridge:

```html
<script type="module">
import { App } from 'https://cdn.pmcp.run/ext-apps/latest/index.mjs';

const app = new App();
app.onToolResult((result) => {
    // Update widget with tool output
    document.getElementById('output').textContent = JSON.stringify(result);
});
</script>
```

## Host Layer Support

MCP Apps works across multiple hosts:
- **Claude Desktop** -- standard `ui.resourceUri` metadata
- **ChatGPT** -- automatic `openai/outputTemplate` enrichment
- **MCP Inspector** -- preview mode via `mcp-preview`

## Testing MCP Apps

Use `cargo pmcp test apps` to validate:
- Widget resource URIs resolve correctly
- Tool _meta contains required UI keys
- Widget HTML is well-formed
- Cross-reference between tools and widget resources
