# MCP Apps Extension Implementation Plan

**Status**: Planning
**SEP Reference**: [SEP-1865](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865)
**Target Release**: PMCP v1.9.0
**Last Updated**: 2025-11-23

## Executive Summary

This document outlines the implementation plan for MCP Apps Extension (SEP-1865) in PMCP. The extension enables MCP servers to deliver interactive user interfaces to host applications, addressing a key community request.

**Approach**: Hybrid Progressive Implementation
- **Phase 1**: Spec-compliant HTML support (2 weeks)
- **Phase 2**: Rust developer experience & tooling (2 weeks)
- **Phase 3**: WASM innovation (future, 3-4 weeks)

**Key Differentiators**:
1. Type-safe UI resource builders
2. Compile-time HTML template embedding
3. Tool-UI association validation
4. `cargo pmcp` UI scaffolding & preview
5. Clear path to Rust→WASM UI components

---

## Background

### What is MCP Apps?

MCP Apps Extension standardizes support for interactive user interfaces in the Model Context Protocol. It enables:

- **UI Resources**: Pre-declared resources with `ui://` URI scheme
- **Tool Integration**: Tools reference UI resources via `_meta` field
- **Bidirectional Communication**: UI ↔ Host via MCP JSON-RPC over postMessage
- **Security**: Sandboxed iframes with auditable messages

### Why Now?

1. **Community Demand**: Most requested MCP feature
2. **Proven Patterns**: MCP-UI and OpenAI Apps SDK validate the concept
3. **Strategic Timing**: SEP is in proposal stage - early enough to influence
4. **Competitive Position**: Opportunity to differentiate PMCP with Rust strengths

### Current Limitations

Without UI support:
- Awkward text-based interactions for complex input
- Host must interpret and render specialized data (charts, tables)
- Fragmentation risk across different custom implementations

---

## Architecture Overview

### Protocol Layer

```rust
/// UI Resource declaration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UIResource {
    /// URI with ui:// scheme
    pub uri: String,

    /// Human-readable name
    pub name: String,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// MIME type (e.g., "text/html+mcp")
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// Supported MIME types
pub enum UIMimeType {
    HtmlMcp,           // "text/html+mcp"
    // Future: Wasm, RemoteDom, etc.
}

/// UI Resource content
pub struct UIResourceContents {
    pub uri: String,
    pub mime_type: String,
    pub text: Option<String>,      // HTML content
    pub blob: Option<String>,       // Future: WASM binary
}
```

### Server API

```rust
use pmcp::ui::UIResourceBuilder;

// Build UI resource
let settings_ui = UIResourceBuilder::new("settings/form")
    .name("Settings Form")
    .description("Configure application settings")
    .html_template(include_str!("../ui/settings.html"))
    .build()?;

// Build tool with UI
let configure_tool = TypedTool::new("configure", configure_handler)
    .with_description("Configure settings")
    .with_ui("ui://settings/form");

// Register with server
let server = ServerBuilder::new()
    .ui_resource(settings_ui)
    .tool("configure", configure_tool)
    .build()?;
```

### Key Design Decisions

1. **Pre-declared Resources**: UI templates are resources, referenced in tool metadata
   - Enables prefetching and review before execution
   - Separates static presentation from dynamic data
   - Better caching and security

2. **MCP Transport**: UI ↔ Host communication uses existing MCP JSON-RPC
   - Standard `@modelcontextprotocol/sdk` works
   - Structured and auditable messages
   - Future MCP features automatically supported

3. **HTML First**: Initial support for `text/html+mcp` only
   - Universal browser support
   - Well-understood security model
   - Clear baseline for future extensions

4. **Security-First**:
   - Iframe sandboxing with restricted permissions
   - Predeclared templates (reviewable before rendering)
   - Auditable JSON-RPC messages
   - User consent for UI-initiated tool calls

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

**Goal**: Spec-compliant HTML support that ships value immediately

#### Deliverables

**Protocol Support** (`pmcp/src/protocol/ui.rs`):
- [ ] `UIResource` struct with proper serialization
- [ ] `UIMimeType` enum with `HtmlMcp` variant
- [ ] `UIResourceContents` for content delivery
- [ ] `ToolMetadata` with `ui/resourceUri` field
- [ ] URI validation for `ui://` scheme

**Server API** (`pmcp/src/server/ui.rs`):
- [ ] `UIResourceBuilder` with fluent API
- [ ] `UIResourceHandle` for registered resources
- [ ] `html_template()` method for string content
- [ ] `html_file()` method using `include_str!()`
- [ ] URI format validation

**ServerBuilder Integration** (`pmcp/src/server/builder.rs`):
- [ ] `ui_resource()` method for registration
- [ ] `html_ui()` convenience method
- [ ] `list_ui_resources()` capability
- [ ] `get_ui_resource()` content retrieval

**TypedTool Integration** (`pmcp/src/tools/typed.rs`):
- [ ] `with_ui()` method for tool-UI association
- [ ] `has_ui()` query method
- [ ] `ui_uri()` getter method
- [ ] Metadata field population

**Examples**:
- [ ] Example 1: Settings form with theme/notification controls
- [ ] Example 2: Data visualization with bar chart
- [ ] Example 3: Interactive table with filtering

**Testing**:
- [ ] Unit tests for URI validation
- [ ] Unit tests for builder API
- [ ] Integration tests for resource registration
- [ ] Integration tests for content retrieval
- [ ] Example tests

**Documentation**:
- [ ] Protocol types documentation
- [ ] API usage guide
- [ ] Security best practices
- [ ] Example walkthroughs

#### Success Criteria

- ✅ Can declare HTML UI resources
- ✅ Tools can reference UIs via metadata
- ✅ Resources serve correctly to hosts
- ✅ All tests pass with zero warnings
- ✅ Examples work end-to-end
- ✅ Documentation complete

#### Timeline

- Days 1-3: Protocol types and validation
- Days 4-6: Server API and builders
- Days 7-9: Integration and examples
- Days 10-14: Testing and documentation

#### Risk Assessment

**Risk Level**: Low

**Potential Issues**:
- Spec may evolve during implementation → Mitigation: Stay close to SEP-1865 discussion
- HTML embedding edge cases → Mitigation: Comprehensive validation tests
- Resource URI conflicts → Mitigation: Strict URI validation

---

### Phase 2: Developer Experience (Week 3-4)

**Goal**: Best-in-class Rust developer experience for MCP Apps

#### Deliverables

**cargo-pmcp Scaffolding**:
- [ ] `cargo pmcp ui add <name>` command
- [ ] Template selection: `--template form|chart|table|settings`
- [ ] Automatic registration in server code
- [ ] HTML template generation with MCP SDK setup

**UI Preview Server**:
- [ ] `cargo pmcp dev --ui-preview` flag
- [ ] Local server at `http://localhost:3001/ui`
- [ ] Live list of all UI resources
- [ ] Click-through to view rendered UI
- [ ] Hot reload on HTML file changes
- [ ] WebSocket for reload notifications

**Template Library**:
- [ ] Form template (input collection)
- [ ] Chart template (data visualization)
- [ ] Table template (data grid)
- [ ] Settings template (configuration panel)
- [ ] Dashboard template (multi-widget layout)

**mcp-tester Integration**:
- [ ] Validate UI resource declarations
- [ ] Check tool → UI reference integrity
- [ ] HTML syntax validation
- [ ] Security checklist validation

**Documentation**:
- [ ] pmcp-book chapter: "Building UI Apps"
- [ ] Scaffolding guide
- [ ] Template customization guide
- [ ] Security best practices
- [ ] Production deployment guide

#### Success Criteria

- ✅ Can scaffold UI in < 1 minute
- ✅ Live preview works with < 500ms reload
- ✅ 5+ production-ready templates
- ✅ mcp-tester validates UIs correctly
- ✅ Documentation enables self-service

#### Timeline

- Days 15-18: cargo-pmcp commands
- Days 19-22: Preview server
- Days 23-25: Template library
- Days 26-28: Documentation

#### Risk Assessment

**Risk Level**: Low-Medium

**Potential Issues**:
- Hot reload complexity → Mitigation: Use proven file watching libraries
- Template quality → Mitigation: Community review and testing
- Cross-platform issues → Mitigation: Test on Windows, macOS, Linux

---

### Phase 3: WASM Innovation (Future, Month 3+)

**Goal**: Revolutionary Rust→WASM UI components

**Note**: This phase starts after SEP-1865 stabilizes and HTML support is proven in production.

#### Deliverables

**Protocol Extension**:
- [ ] WASM MIME type support (`application/wasm+mcp`)
- [ ] Binary blob content delivery
- [ ] WASM module validation

**Rust Framework Integration**:
- [ ] Leptos integration example
- [ ] Dioxus integration example
- [ ] Yew integration example
- [ ] Framework comparison guide

**Build Tooling**:
- [ ] `#[wasm_ui_component]` procedural macro
- [ ] Compile-time WASM compilation
- [ ] Optimization pipeline (wasm-opt)
- [ ] Size analysis tooling

**Developer Experience**:
- [ ] `cargo pmcp ui add --wasm` flag
- [ ] Hot reload for Rust UI code
- [ ] Type-safe data passing between server and UI
- [ ] Component prop validation

**Examples**:
- [ ] Counter component (hello world)
- [ ] Chart component with real-time updates
- [ ] Form component with validation
- [ ] Complex dashboard

#### Success Criteria

- ✅ Rust UI compiles to < 100KB gzipped WASM
- ✅ Build time < 5s incremental
- ✅ Performance better than equivalent JS
- ✅ Type-safe end-to-end
- ✅ Multiple framework options

#### Timeline

- Week 1: Protocol and framework evaluation
- Week 2: Macro and build tooling
- Week 3: Framework integrations
- Week 4: Examples and documentation

#### Risk Assessment

**Risk Level**: Medium-High

**Potential Issues**:
- WASM size bloat → Mitigation: Aggressive optimization, feature flags
- Framework immaturity → Mitigation: Support multiple frameworks
- Spec divergence → Mitigation: Propose extension to SEP-1865
- Build complexity → Mitigation: Hide complexity behind cargo commands

---

## API Examples

### Example 1: Simple Settings Form

```rust
use pmcp::{ServerBuilder, TypedTool, ui::UIResourceBuilder, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Deserialize, Serialize, JsonSchema)]
struct SettingsArgs {
    theme: String,
    notifications: bool,
}

async fn configure(args: SettingsArgs, _extra: RequestHandlerExtra) -> Result<String> {
    Ok(format!("Configured: theme={}, notifications={}",
               args.theme, args.notifications))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create UI resource
    let settings_ui = UIResourceBuilder::new("settings/form")
        .name("Settings Form")
        .description("Configure application settings")
        .html_template(include_str!("../ui/settings.html"))
        .build()?;

    // Create tool with UI
    let configure_tool = TypedTool::new("configure", |args, extra| {
        Box::pin(configure(args, extra))
    })
    .with_description("Configure application settings")
    .with_ui("ui://settings/form");

    // Build and run server
    let server = ServerBuilder::new()
        .name("settings-server")
        .version("1.0.0")
        .ui_resource(settings_ui)
        .tool("configure", configure_tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

**HTML Template** (`ui/settings.html`):

```html
<!DOCTYPE html>
<html>
<head>
    <title>Settings</title>
    <script src="https://unpkg.com/@modelcontextprotocol/sdk@latest"></script>
    <style>
        body { font-family: system-ui; padding: 20px; }
        form { max-width: 400px; }
        label { display: block; margin: 10px 0; }
        select, input { margin-left: 10px; }
        button { margin-top: 20px; padding: 10px 20px; }
    </style>
</head>
<body>
    <h1>Settings</h1>
    <form id="settings-form">
        <label>
            Theme:
            <select name="theme">
                <option value="light">Light</option>
                <option value="dark">Dark</option>
            </select>
        </label>
        <label>
            <input type="checkbox" name="notifications" />
            Enable Notifications
        </label>
        <button type="submit">Save Settings</button>
    </form>

    <script type="module">
        import { Client } from '@modelcontextprotocol/sdk/client/index.js';

        // Initialize MCP client
        const client = new Client({
            name: "settings-ui",
            version: "1.0.0"
        });

        // Handle form submission
        document.getElementById('settings-form').addEventListener('submit', async (e) => {
            e.preventDefault();
            const formData = new FormData(e.target);

            try {
                const result = await client.request({
                    method: 'tools/call',
                    params: {
                        name: 'configure',
                        arguments: {
                            theme: formData.get('theme'),
                            notifications: formData.get('notifications') === 'on'
                        }
                    }
                });

                alert('Settings saved: ' + result.content[0].text);
            } catch (error) {
                alert('Error saving settings: ' + error.message);
            }
        });
    </script>
</body>
</html>
```

### Example 2: Data Visualization

```rust
use pmcp::{ServerBuilder, TypedTool, ui::UIResourceBuilder, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Deserialize, Serialize, JsonSchema)]
struct VisualizeArgs {
    data: Vec<f64>,
    labels: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
struct ChartOutput {
    chart_data: serde_json::Value,
}

async fn visualize(args: VisualizeArgs, _extra: RequestHandlerExtra) -> Result<ChartOutput> {
    // Return data in format expected by UI
    Ok(ChartOutput {
        chart_data: serde_json::json!({
            "labels": args.labels,
            "datasets": [{
                "data": args.data,
                "backgroundColor": "rgba(75, 192, 192, 0.2)",
                "borderColor": "rgba(75, 192, 192, 1)"
            }]
        })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let chart_ui = UIResourceBuilder::new("charts/bar")
        .name("Bar Chart")
        .description("Interactive bar chart visualization")
        .html_template(include_str!("../ui/chart.html"))
        .build()?;

    let visualize_tool = TypedTool::new("visualize_data", |args, extra| {
        Box::pin(visualize(args, extra))
    })
    .with_description("Visualize data as a bar chart")
    .with_ui("ui://charts/bar");

    let server = ServerBuilder::new()
        .name("visualization-server")
        .ui_resource(chart_ui)
        .tool("visualize_data", visualize_tool)
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

### Example 3: Compile-Time Template Validation

```rust
// Future enhancement: Validate templates at compile time

use pmcp::ui::UIResourceBuilder;

// This would fail at compile time if template is invalid HTML
let ui = UIResourceBuilder::new("my-ui")
    .html_template(include_str!("template.html"))
    .validate_at_compile_time()  // Future feature
    .build()?;
```

---

## Security Considerations

### Threat Model

**Threats**:
1. Malicious HTML injection
2. XSS attacks via UI content
3. Unauthorized tool execution
4. Data exfiltration via postMessage
5. Resource exhaustion (large UIs)

### Mitigations

#### 1. Iframe Sandboxing

**Requirement**: All UI content MUST be rendered in sandboxed iframes.

**Recommended sandbox attributes**:
```html
<iframe
  sandbox="allow-scripts allow-same-origin"
  src="ui-resource-uri"
  csp="default-src 'self'; script-src 'unsafe-inline' https://unpkg.com/@modelcontextprotocol/sdk">
</iframe>
```

**Documentation**:
- Explain required permissions
- Warn against unsafe configurations
- Provide secure examples

#### 2. Content Security Policy

**Recommended CSP for UI resources**:
```
default-src 'self';
script-src 'self' 'unsafe-inline' https://unpkg.com/@modelcontextprotocol/sdk;
style-src 'self' 'unsafe-inline';
img-src 'self' data: https:;
connect-src 'self';
```

#### 3. Template Validation

**At build time**:
- [ ] Validate HTML syntax
- [ ] Check for dangerous patterns (e.g., `<script src="http://evil.com">`)
- [ ] Validate MCP SDK usage
- [ ] Size limits (warn on >1MB templates)

**At runtime**:
- [ ] URI format validation
- [ ] MIME type verification
- [ ] Content sanitization (optional, host-side)

#### 4. Audit Logging

**Log all UI interactions**:
```rust
// Example: Log tool calls initiated from UI
server.on_tool_call(|name, args, source| {
    if source.is_ui() {
        audit_log!("UI tool call: {} from {}", name, source.ui_uri());
    }
});
```

#### 5. User Consent

**Host should prompt for**:
- First-time UI resource load
- Tool execution from UI
- Access to sensitive data

**Best practice**: Show UI template preview before first use.

### Security Checklist

Before shipping UI Apps:

- [ ] All HTML templates reviewed for XSS
- [ ] Sandbox attributes documented
- [ ] CSP headers configured
- [ ] Template validation implemented
- [ ] Audit logging in place
- [ ] User consent flow designed
- [ ] Security guide published
- [ ] Example security configs provided

---

## Testing Strategy

### Unit Tests

**Protocol Layer** (`pmcp/src/protocol/ui.rs`):
```rust
#[test]
fn test_ui_resource_uri_validation() {
    // Valid URIs
    assert!(UIResourceBuilder::new("charts/bar").build().is_ok());

    // Invalid URIs
    assert!(UIResourceBuilder::new("").build().is_err());
    assert!(UIResourceBuilder::new("http://evil.com").build().is_err());
}

#[test]
fn test_mime_type_serialization() {
    let mime = UIMimeType::HtmlMcp;
    assert_eq!(mime.as_str(), "text/html+mcp");
}
```

**Builder API** (`pmcp/src/server/ui.rs`):
```rust
#[test]
fn test_ui_resource_builder() {
    let ui = UIResourceBuilder::new("test/ui")
        .name("Test UI")
        .description("Test description")
        .html_template("<html><body>Test</body></html>")
        .build()
        .unwrap();

    assert_eq!(ui.uri(), "ui://test/ui");
    assert_eq!(ui.resource().name, "Test UI");
}
```

### Integration Tests

**Resource Registration**:
```rust
#[tokio::test]
async fn test_ui_resource_registration() {
    let ui = UIResourceBuilder::new("test/ui")
        .name("Test")
        .html_template("<html></html>")
        .build()
        .unwrap();

    let server = ServerBuilder::new()
        .name("test-server")
        .ui_resource(ui)
        .build()
        .unwrap();

    let resources = server.list_ui_resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "ui://test/ui");
}
```

**Tool-UI Association**:
```rust
#[tokio::test]
async fn test_tool_ui_metadata() {
    let tool = TypedTool::new("test", test_handler)
        .with_ui("ui://test/ui");

    assert!(tool.has_ui());
    assert_eq!(tool.ui_uri(), Some("ui://test/ui"));
}
```

### mcp-tester Support

**New validation modes**:
```bash
# Validate UI resources
mcp-tester ui http://localhost:8080

# Checks:
# - All UI resources are accessible
# - Tools with UI reference valid resources
# - HTML templates are valid
# - Security best practices
```

**Output**:
```
UI Validation Report:
✅ 3 UI resources found
✅ All tool → UI references valid
✅ HTML syntax valid
⚠️  Warning: ui://charts/bar missing CSP headers
❌ Error: ui://settings/form has inline eval()
```

### Example Tests

Each example must include:
- [ ] README with usage instructions
- [ ] Automated test script
- [ ] Expected output verification
- [ ] Screenshot or recording

---

## Documentation Plan

### pmcp-book Chapter

**New chapter**: "Building UI Apps with MCP Apps Extension"

**Sections**:
1. Introduction to MCP Apps
2. Creating Your First UI Resource
3. Tool-UI Integration
4. Security Best Practices
5. Advanced Patterns
6. Deployment Considerations
7. Troubleshooting

### API Documentation

**rust-doc coverage**:
- [ ] All public types documented
- [ ] Examples for each major API
- [ ] Security warnings where applicable
- [ ] Links to SEP-1865

### Guides

**Quick Start Guide**:
- 5-minute tutorial
- Simple form example
- Copy-paste ready code

**Migration Guide**:
- For existing servers adding UI
- Step-by-step conversion
- Before/after comparisons

**Template Guide**:
- How to use built-in templates
- Customization patterns
- Creating custom templates

**Security Guide**:
- Threat model explanation
- Best practices checklist
- Secure configuration examples
- Common vulnerabilities

---

## Community Engagement

### Communication Plan

**Announcement**:
- [ ] Blog post: "MCP Apps in PMCP"
- [ ] Reddit: r/rust, r/Programming
- [ ] Twitter/X: Rust and MCP communities
- [ ] Discord: MCP Contributors #ui-wg

**Call for Feedback**:
- [ ] Create RFC for WASM extension
- [ ] Request template contributions
- [ ] Beta tester recruitment

**Support**:
- [ ] GitHub Discussions for Q&A
- [ ] Examples repository
- [ ] Template showcase

### Contribution Opportunities

**Easy contributions**:
- UI templates
- Examples
- Documentation improvements
- Security reviews

**Advanced contributions**:
- WASM framework integrations
- Preview server enhancements
- New MIME type support

---

## Success Metrics

### Phase 1 Success Criteria

**Adoption**:
- [ ] 10+ developers using UI Apps
- [ ] 5+ production deployments
- [ ] 3+ community-contributed examples

**Quality**:
- [ ] Zero P0 bugs in production
- [ ] All tests passing
- [ ] Documentation rated 4.5+/5

**Performance**:
- [ ] < 1ms overhead per UI resource
- [ ] < 100KB memory per registered UI
- [ ] No measurable impact on non-UI operations

### Phase 2 Success Criteria

**Developer Experience**:
- [ ] UI scaffolding < 60 seconds
- [ ] Preview server < 500ms reload
- [ ] 90% positive feedback on tooling

**Templates**:
- [ ] 5+ production-ready templates
- [ ] 10+ forks/customizations
- [ ] Template showcase published

### Phase 3 Success Criteria

**WASM Performance**:
- [ ] < 100KB gzipped bundle
- [ ] < 5s incremental build
- [ ] Faster than JS equivalent

**Adoption**:
- [ ] 5+ WASM UI Apps in production
- [ ] Framework integration docs complete
- [ ] Community WASM examples

---

## Risk Management

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| SEP-1865 changes | Medium | High | Stay close to discussions, design for flexibility |
| WASM bloat | Medium | Medium | Aggressive optimization, feature flags |
| Browser compatibility | Low | Medium | Test on major browsers, document requirements |
| Performance regression | Low | High | Benchmark suite, continuous monitoring |

### Market Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| MCP-UI dominance | Medium | Medium | Focus on Rust strengths, WASM differentiation |
| Low adoption | Low | High | Excellent docs, examples, community engagement |
| Spec fragmentation | Low | High | Active SEP-1865 participation, standards compliance |

### Operational Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Resource constraints | Low | Medium | Phased approach, community contributions |
| Timeline slippage | Medium | Low | Buffer in estimates, MVP focus |
| Security vulnerability | Low | High | Security-first design, third-party audits |

---

## Open Questions

### Awaiting Decisions

1. **WASM Priority**:
   - Q: Is WASM support critical for v1.9.0?
   - Options: Include in Phase 1, Delay to v1.10.0
   - Decision: [TBD]

2. **Framework Choice**:
   - Q: Which WASM framework to prioritize?
   - Options: Leptos, Dioxus, Yew, All
   - Decision: [TBD]

3. **Template Hosting**:
   - Q: Where to host template library?
   - Options: Embedded, CDN, Separate repo
   - Decision: [TBD]

4. **Preview Server**:
   - Q: Standalone or integrated with `cargo pmcp dev`?
   - Options: Separate command, Integrated flag
   - Decision: [TBD]

### Research Needed

- [ ] WASM framework comparison (performance, size, DX)
- [ ] CSP best practices for MCP Apps
- [ ] Template rendering performance benchmarks
- [ ] Community template requirements survey

---

## Appendix

### References

- [SEP-1865: MCP Apps Extension](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865)
- [MCP Apps Blog Post](https://modelcontextprotocol.io/blog/mcp-apps)
- [MCP-UI Project](https://github.com/mcp-ui)
- [OpenAI Apps SDK](https://platform.openai.com/docs/apps)
- [PMCP Documentation](https://paiml.github.io/rust-mcp-sdk/)

### Related Work

- **MCP-UI**: TypeScript-based UI extension, large community
- **OpenAI Apps SDK**: ChatGPT app platform using MCP
- **Leptos**: Rust WASM framework for web apps
- **Dioxus**: Cross-platform Rust UI framework
- **Yew**: Rust framework for building web apps with WASM

### Glossary

- **MCP**: Model Context Protocol
- **SEP**: Specification Enhancement Proposal
- **UI Resource**: Pre-declared user interface template
- **MIME Type**: Multipurpose Internet Mail Extensions type
- **WASM**: WebAssembly
- **CSP**: Content Security Policy
- **XSS**: Cross-Site Scripting
- **DX**: Developer Experience

---

**Document Version**: 1.0
**Authors**: Claude Code, Guy Ernest
**Status**: Draft
**Next Review**: After Phase 1 completion
