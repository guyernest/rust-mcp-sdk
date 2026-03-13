# Chapter 20 Exercises

These exercises help you build complete MCP Apps powered by the standard SDK APIs.

## AI-Guided Exercises

The following exercises are designed for AI-guided learning. Use an AI assistant with the course MCP server to get personalized guidance, hints, and feedback.

1. **Build a Widget-Enabled MCP Server** (60 min)
   - Create a server with `ToolInfo::with_ui()` and `UIResource::html_mcp_app()`
   - Build a widget using the ext-apps `App` class with all required handlers
   - Return `structuredContent` from a tool and render it in the widget
   - Bundle with Vite and test with `cargo pmcp preview`

2. **Multi-Host Validation** (30 min)
   - Add `with_host_layer(HostType::ChatGpt)` to your server
   - Validate metadata with `mcp-tester apps http://localhost:3000`
   - Run with `--mode chatgpt` and `--mode claude-desktop` to check host-specific compliance
   - Fix any warnings reported by `mcp-tester apps --strict`

3. **External Resource Loading** (45 min)
   - Add external image loading to a widget
   - Declare CDN domains using `WidgetCSP` on the server side
   - Implement the fetch-blob pattern in the widget for cross-host compatibility
   - Test in `cargo pmcp preview` and verify images render correctly

## Prerequisites

Before starting these exercises, ensure you have:
- Completed all previous chapters
- `cargo-pmcp` installed (`cargo install cargo-pmcp`)
- Node.js installed (for Vite bundling)
- Understanding of MCP resource patterns

## Next Steps

Congratulations! You've completed the PMCP course. Continue your learning:
- [Template Gallery](../appendix/template-gallery.md) - Production templates
- [Troubleshooting](../appendix/troubleshooting.md) - Common issues
- Contribute to the PMCP SDK community
