# MCP Apps: Interactive Widgets

In this chapter, you'll learn to build rich, interactive widgets -- charts, maps, games, dashboards -- that run alongside your MCP tools. Instead of returning plain JSON that an AI describes in words, your server will serve full HTML interfaces that users can see and interact with directly.

MCP Apps is the standard extension for interactive widget UIs. Your widgets work across multiple hosts -- Claude Desktop, ChatGPT, VS Code, and any standard MCP client -- without writing host-specific code. The SDK handles the differences for you through the **host layer** system.

## Learning Objectives

By the end of this chapter, you will be able to:

- Scaffold an MCP Apps project with `cargo pmcp app new` and preview it in your browser
- Register tools with UI metadata using `ToolInfo::with_ui()` and serve widgets as resources
- Build widgets with the `@modelcontextprotocol/ext-apps` SDK (`App` class) and Vite bundling
- Enable multi-host support with `with_host_layer(HostType::ChatGpt)`
- Return `structuredContent` from tools for widget rendering
- Validate App metadata with `mcp-tester apps` and `cargo pmcp test apps`

## Why Widgets?

Traditional MCP interactions are text-based: the AI calls a tool, gets JSON back, and describes the results in natural language. That works well for simple data, but some information is inherently visual.

```
+-----------------------------------------------------------------------+
|                    Text vs Visual Information                          |
+-----------------------------------------------------------------------+
|                                                                       |
|  Text works well for:           Visual works better for:              |
|  =====================          =========================             |
|  - Status messages              - Image galleries                     |
|  - Simple data retrieval        - Interactive maps                    |
|  - Configuration values         - Charts and dashboards               |
|  - Error messages               - Forms with validation               |
|  - Lists of items               - Data grids with sorting             |
|                                 - Real-time visualizations            |
|                                 - Game boards                         |
|                                                                       |
+-----------------------------------------------------------------------+
```

When your tool returns coordinates for ten conference venues, an interactive map beats a list of latitude/longitude pairs every time. MCP Apps let you serve that map as an HTML widget right alongside the tool that provides the data.

## Architecture at a Glance

Building an MCP App involves two parts:

1. **Server side (Rust, PMCP SDK)** -- registers tools with UI metadata, serves widget HTML as resources, returns `structuredContent` from tool calls
2. **Widget side (JS/TS, ext-apps SDK)** -- the interactive UI that runs in the host's iframe, communicates with the host via the `App` class from `@modelcontextprotocol/ext-apps`

```
+-----------------------------------------------------+
|  Host (Claude Desktop, ChatGPT, VS Code, etc.)      |
|                                                      |
|  tools/list --- _meta.ui.resourceUri ---> knows      |
|                                          which tool  |
|                                          has UI      |
|  tools/call --- structuredContent ------> data for   |
|                                          the widget  |
|  resources/read -- HTML ----------------> widget     |
|                                          code        |
|                                                      |
|  +------------------------------------------------+  |
|  |  Widget iframe                                  |  |
|  |  @modelcontextprotocol/ext-apps (App class)     |  |
|  |  <-- hostContext (theme, toolInput, toolOutput)  |  |
|  |  --> app.callServerTool() --> tools/call          |  |
|  +------------------------------------------------+  |
+-----------------------------------------------------+
```

Here is how each layer works:

| Layer    | What It Is                                  | What It Does                                         |
|----------|---------------------------------------------|------------------------------------------------------|
| Widget   | An HTML file bundled with Vite              | Renders the UI the user sees and interacts with      |
| ext-apps | `@modelcontextprotocol/ext-apps` `App` class | Handles postMessage communication with the host     |
| Host     | Claude Desktop, ChatGPT, VS Code, etc.      | Embeds the widget in a sandboxed iframe              |
| Server   | Your Rust MCP server                        | Processes tool calls, serves widget HTML, returns structuredContent |

Widgets are HTML files bundled with Vite. The ext-apps SDK handles communication. The host renders the widget in an iframe. The server processes tool calls and serves widget resources. You write the widget and the server -- the ext-apps SDK and hosting are handled for you.

## Quick Start: Your First Widget

Let's go from zero to a working interactive widget in three steps.

### Step 1: Scaffold the Project

```bash
cargo pmcp app new my-widget-app
cd my-widget-app
```

This creates a ready-to-run project:

```
my-widget-app/
  src/
    main.rs          # MCP server with tool handlers
  widget/
    mcp-app.html     # Starter widget using ext-apps App class
    package.json     # npm dependencies (ext-apps SDK, Vite)
    vite.config.ts   # Vite bundling config
  Cargo.toml
  README.md
```

### Step 2: Build the Widget

The widget must be bundled into self-contained HTML before the Rust server can embed it. This is required because Claude Desktop's iframe CSP blocks external script loading -- CDN imports fail silently.

```bash
cd widget
npm install
npm run build
cd ..
```

### Step 3: Build and Run the Server

```bash
cargo build
cargo run
```

Then preview your widget:

```bash
cargo pmcp preview --url http://localhost:3000 --open
```

The preview opens in your browser. Interact with the widget -- it communicates with your MCP server through the ext-apps protocol.

## Feature Flag

MCP Apps requires the `mcp-apps` feature flag in your `Cargo.toml`:

```toml
[dependencies]
pmcp = { version = "2.0", features = ["mcp-apps"] }
```

The `full` feature also includes `mcp-apps`. If you scaffolded with `cargo pmcp app new`, this is already configured.

## Developer Tooling

Two tools help you develop and validate MCP Apps:

- **mcp-preview** -- Browser-based testing environment with live MCP bridge. Run `cargo pmcp preview --url http://localhost:3000 --open` to preview widgets, test themes, and inspect protocol messages.
- **mcp-tester** -- CLI-based App metadata validator. Run `mcp-tester apps http://localhost:3000` or `cargo pmcp test apps --url http://localhost:3000` to check metadata compliance without a browser.

## Chapter Contents

This chapter is split into three hands-on sections:

1. **[UI Resources and Widget Registration](./ch20-01-ui-resources.md)** -- Register widget HTML as resources with `UIResource::html_mcp_app()`, declare CSP for external domains with `WidgetCSP`, and implement the standard resource handler pattern

2. **[Tool-UI Association and Data Flow](./ch20-02-tool-ui-association.md)** -- Associate tools with widgets using `ToolInfo::with_ui()`, return `structuredContent` for widget rendering, enable multi-host support with `with_host_layer()`, and add `outputSchema`

3. **[Widget Communication with ext-apps](./ch20-03-postmessage.md)** -- Build widgets using the ext-apps `App` class, implement required protocol handlers, bundle with Vite, and follow cross-host best practices

> **Reference:** For complete API documentation covering every method, type, and edge case, see [Chapter 12.5: MCP Apps Extension](../../pmcp-book/src/ch12-5-mcp-apps.md) in the PMCP book.

## Knowledge Check

{{#quiz ../quizzes/ch20-mcp-apps.toml}}

---

*Continue to [UI Resources and Widget Registration](./ch20-01-ui-resources.md) ->*
