# Phase 18: Publishing Pipeline - Research

**Researched:** 2026-02-26
**Domain:** CLI tooling, static HTML generation, JSON manifest generation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Follow ChatGPT ai-plugin.json format -- name, description, auth, API endpoint, logo URL fields
- Server URL specified via required `--url` CLI flag (e.g., `cargo pmcp app manifest --url https://my-app.example.com`)
- Tool-to-widget mappings auto-discovered from `widgets/` directory -- each `.html` file becomes a widget entry mapped to a tool of the same name (convention over config)
- App name from `[package].name`, description from `[package].description` in Cargo.toml; logo via optional `--logo` flag or `[package.metadata.pmcp].logo`
- Widget rendered in a framed showcase -- centered widget iframe with app name, description header, and subtle frame (product demo page feel)
- Mock bridge uses hardcoded sample responses generated at build time from `mock-data/` directory
- Developer creates `mock-data/tool-name.json` files with sample responses; landing generator reads and embeds them
- Output is a single self-contained HTML file -- CSS, JS, widget HTML, mock data all inlined. One file to share, email, or host anywhere.
- Commands live under `cargo pmcp app` namespace: `app manifest`, `app landing`, `app build`
- `app build` is combined command generating both manifest + landing in one invocation; individual commands still available separately
- Default output to `dist/` directory (`dist/manifest.json`, `dist/landing.html`) -- like a build output
- Overwrite existing output files silently -- generated artifacts are disposable build output, matching cargo build semantics
- Detect MCP Apps project by checking Cargo.toml for pmcp dependency with mcp-apps feature; error with "Not an MCP Apps project. Run `cargo pmcp app new` first." if missing
- Widget discovery always looks in `./widgets/` relative to Cargo.toml -- hardcoded path matching Phase 17 convention
- Empty or missing `widgets/` directory produces error: "No widgets found in widgets/. Add .html files or run `cargo pmcp app new` to scaffold a project."
- `mock-data/` directory required for landing page generation -- error if missing: "No mock data found. Create mock-data/tool-name.json for each tool."

### Claude's Discretion
- Exact ChatGPT manifest schema version and field validation
- Landing page CSS styling and responsive behavior
- Mock bridge JavaScript implementation details
- How widget HTML is inlined into the landing page (iframe vs direct embed)
- dist/ directory creation behavior (auto-create if missing)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PUBL-01 | `cargo pmcp manifest` generates ChatGPT-compatible JSON with server URL and tool-to-widget mapping | Manifest schema documented below; Cargo.toml parsing via `toml` crate already established in codebase; widget discovery via existing `WidgetDir` API |
| PUBL-02 | `cargo pmcp landing` generates standalone HTML demo page with mock bridge (no server required) | Mock bridge pattern documented; single-file HTML inlining approach with embedded JS/CSS; widget HTML read from `WidgetDir::read_widget()` |
</phase_requirements>

## Summary

Phase 18 adds three new subcommands to the existing `cargo pmcp app` namespace: `manifest`, `landing`, and `build`. All required infrastructure already exists in the codebase. The `AppCommand` enum in `cargo-pmcp/src/commands/app.rs` currently has only `New`; adding three more variants is a straightforward extension. Widget discovery uses the existing `WidgetDir` from `pmcp::server::mcp_apps::WidgetDir`. Cargo.toml parsing uses the `toml` crate (already a dependency) for reading `[package]` fields. HTML generation follows the same pattern as `cargo-pmcp/src/templates/mcp_app.rs` -- Rust `format!()` string templates returning `String`.

The manifest format follows the ChatGPT ai-plugin.json schema (schema_version "v1"). The MCP Apps twist is that tool-to-widget mappings are included as an extension field. The landing page is a single self-contained HTML file with the widget HTML inlined into an iframe using a `srcdoc` attribute, a mock `window.mcpBridge` that returns hardcoded responses from `mock-data/*.json`, and product-showcase styling.

**Primary recommendation:** Extend `AppCommand` enum with `Manifest`, `Landing`, and `Build` variants. Implement each as a pure synchronous function (no async needed -- all file I/O). Reuse `WidgetDir::discover()` for widget scanning. Use `toml::Value` for Cargo.toml parsing (established pattern). Generate manifest JSON with `serde_json::to_string_pretty`. Generate landing HTML with Rust string templates.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `toml` | 0.9 | Parse Cargo.toml for package metadata | Already in cargo-pmcp Cargo.toml; used extensively in deployment code |
| `serde_json` | 1 | Generate manifest.json output | Already a dependency; standard JSON serialization |
| `serde` | 1 | Serialize manifest struct to JSON | Already a dependency |
| `clap` | 4 | CLI argument parsing for new subcommands | Already the CLI framework for cargo-pmcp |
| `anyhow` | 1 | Error handling | Already the error strategy throughout cargo-pmcp |
| `colored` | 3 | Terminal output formatting | Already used in app.rs for status messages |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `walkdir` | 2 | Directory traversal (already a dep) | Not needed -- `WidgetDir::discover()` handles widget scanning |
| `cargo_metadata` | 0.19 | Rich Cargo.toml parsing | Alternative to `toml::Value` but heavier; use only if `[package.metadata.pmcp]` access needs structured types |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `toml::Value` for Cargo.toml | `cargo_metadata::MetadataCommand` | cargo_metadata spawns `cargo metadata` subprocess which is slower but more correct for workspace resolution; `toml::Value` is fine for single-package MCP Apps projects |
| Rust format!() for HTML | Template engine (askama, tera) | Adding a template engine for two HTML strings is over-engineering; format!() is the established pattern in this codebase |
| iframe srcdoc for widget embed | Direct HTML injection | srcdoc provides CSS/JS isolation so widget styles don't leak into landing page frame; direct injection risks style conflicts |

## Architecture Patterns

### Recommended Project Structure
```
cargo-pmcp/src/
├── commands/
│   └── app.rs              # Extended with Manifest, Landing, Build variants
├── templates/
│   └── mcp_app.rs          # Existing scaffolding templates
└── publishing/             # NEW module (or inline in app.rs)
    ├── mod.rs              # Module declarations
    ├── manifest.rs         # Manifest JSON generation
    ├── landing.rs          # Landing HTML generation
    └── detect.rs           # Project detection logic (shared)
```

Alternative: Keep everything in `commands/app.rs` if the code stays under ~400 lines. The existing `app.rs` is ~100 lines. With three new commands, a separate `publishing/` module keeps things organized.

### Pattern 1: AppCommand Enum Extension
**What:** Add Manifest, Landing, Build variants to the existing AppCommand enum
**When to use:** This is the only approach -- follows the established pattern
**Example:**
```rust
// Source: cargo-pmcp/src/commands/app.rs (existing pattern)
#[derive(Subcommand)]
pub enum AppCommand {
    /// Create a new MCP Apps project
    New { name: String, #[arg(long)] path: Option<String> },

    /// Generate ChatGPT-compatible manifest JSON
    Manifest {
        /// Server URL (required)
        #[arg(long)]
        url: String,
        /// Logo URL
        #[arg(long)]
        logo: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },

    /// Generate standalone landing page HTML
    Landing {
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },

    /// Generate both manifest and landing page
    Build {
        /// Server URL (required for manifest)
        #[arg(long)]
        url: String,
        /// Logo URL
        #[arg(long)]
        logo: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },
}
```

### Pattern 2: Project Detection via Cargo.toml Parsing
**What:** Check if current directory is an MCP Apps project by parsing Cargo.toml
**When to use:** Before running any publishing command
**Example:**
```rust
// Source: established pattern in cargo-pmcp/src/commands/deploy/init.rs line 184-198
fn detect_mcp_apps_project() -> Result<ProjectInfo> {
    let cargo_toml_path = PathBuf::from("Cargo.toml");
    if !cargo_toml_path.exists() {
        anyhow::bail!("No Cargo.toml found. Are you in a Rust project directory?");
    }

    let content = std::fs::read_to_string(&cargo_toml_path)?;
    let cargo_toml: toml::Value = toml::from_str(&content)?;

    // Check for pmcp dependency with mcp-apps feature
    let has_mcp_apps = cargo_toml
        .get("dependencies")
        .and_then(|d| d.get("pmcp"))
        .and_then(|p| p.get("features"))
        .and_then(|f| f.as_array())
        .map(|features| features.iter().any(|f| f.as_str() == Some("mcp-apps")))
        .unwrap_or(false);

    if !has_mcp_apps {
        anyhow::bail!("Not an MCP Apps project. Run `cargo pmcp app new` first.");
    }

    // Extract metadata
    let name = cargo_toml.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unnamed")
        .to_string();

    let description = cargo_toml.get("package")
        .and_then(|p| p.get("description"))
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .to_string();

    // Check for [package.metadata.pmcp] logo
    let logo = cargo_toml.get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("pmcp"))
        .and_then(|pm| pm.get("logo"))
        .and_then(|l| l.as_str())
        .map(String::from);

    Ok(ProjectInfo { name, description, logo })
}
```

### Pattern 3: Manifest JSON Generation
**What:** Generate ai-plugin.json compatible manifest with MCP Apps extensions
**When to use:** `cargo pmcp app manifest --url <URL>`
**Example:**
```rust
// Manifest structure following ChatGPT ai-plugin.json schema
#[derive(Serialize)]
struct Manifest {
    schema_version: String,         // "v1"
    name_for_human: String,         // from [package].name
    name_for_model: String,         // sanitized [package].name (no spaces)
    description_for_human: String,  // from [package].description
    description_for_model: String,  // from [package].description
    auth: ManifestAuth,             // { type: "none" } default
    api: ManifestApi,               // { type: "openapi", url: "--url flag + /openapi.json" }
    logo_url: String,               // from --logo flag or [package.metadata.pmcp].logo
    contact_email: String,          // empty or from metadata
    legal_info_url: String,         // empty or from metadata
    // MCP Apps extension
    mcp_apps: McpAppsExtension,
}

#[derive(Serialize)]
struct McpAppsExtension {
    server_url: String,             // from --url flag
    widgets: Vec<WidgetMapping>,    // auto-discovered from widgets/
}

#[derive(Serialize)]
struct WidgetMapping {
    tool: String,                   // widget filename (e.g., "board")
    resource_uri: String,           // e.g., "ui://app/board"
    html_file: String,              // e.g., "widgets/board.html"
}
```

### Pattern 4: Mock Bridge for Landing Page
**What:** JavaScript mock bridge that intercepts `callTool()` and returns canned responses
**When to use:** Landing page HTML -- replaces the real MCP bridge
**Example:**
```javascript
// Mock bridge injected into landing page
// Tool responses loaded from mock-data/*.json at build time
window.mcpBridge = {
    _mockData: { /* embedded at build time from mock-data/*.json */ },

    callTool: async function(name, args) {
        const response = this._mockData[name];
        if (response) {
            return response;
        }
        return { error: 'No mock data for tool: ' + name };
    },

    getState: function() { return this._state || {}; },
    setState: function(s) { this._state = { ...this._state, ...s }; },
    get theme() { return 'light'; },
    get locale() { return 'en-US'; },
    get displayMode() { return 'inline'; }
};
window.dispatchEvent(new Event('mcpBridgeReady'));
```

### Pattern 5: Single-File Landing Page with iframe srcdoc
**What:** Widget HTML embedded in an iframe using srcdoc attribute for CSS/JS isolation
**When to use:** Landing page generation
**Example:**
```html
<!-- Landing page structure -->
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{app_name}</title>
    <style>/* all CSS inlined */</style>
</head>
<body>
    <header>
        <h1>{app_name}</h1>
        <p>{app_description}</p>
    </header>
    <main>
        <div class="widget-showcase">
            <iframe
                srcdoc="{escaped_widget_html_with_mock_bridge}"
                sandbox="allow-scripts"
                style="width:100%;height:600px;border:1px solid #e0e0e0;border-radius:12px;"
            ></iframe>
        </div>
    </main>
    <script>/* mock bridge injected into iframe via srcdoc */</script>
</body>
</html>
```

**Key detail:** The widget HTML must have the mock bridge script injected before it is escaped and placed into `srcdoc`. The bridge replaces what would normally be the server-injected bridge. HTML entities in srcdoc must be escaped (`"` -> `&quot;`, etc.).

### Anti-Patterns to Avoid
- **Dynamic imports in landing page:** The landing page must be fully self-contained. No `<script src="...">` to external files. Everything inlined.
- **Using WidgetDir at runtime in landing page:** WidgetDir is a server-side Rust API. The landing page is static HTML. Read widget content at build time, embed it.
- **Generating landing page with `window.openai` bridge:** The landing page is standalone -- it does not run inside ChatGPT. Use a mock bridge that returns hardcoded data, not the ChatGPT bridge from `ChatGptAdapter`.
- **Trusting widget filenames as tool names without validation:** A widget named `board.html` maps to tool `board`, but the tool must actually exist in the server. The manifest generator should list discovered widgets; it cannot validate tools exist without running the server.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Widget discovery | Custom directory scanner | `WidgetDir::discover()` from `pmcp::server::mcp_apps` | Already tested, handles edge cases (sorting, non-HTML filtering) |
| Cargo.toml parsing | Custom TOML parser | `toml::from_str::<toml::Value>()` | Already used throughout cargo-pmcp deployment code |
| JSON output | Manual string concatenation | `serde_json::to_string_pretty()` on typed struct | Handles escaping, nested objects correctly |
| HTML entity escaping for srcdoc | Custom escaper | Standard HTML entity replacement (`&` -> `&amp;`, `"` -> `&quot;`, `<` -> `&lt;`, `>` -> `&gt;`) | Only 4 replacements needed; `String::replace()` chain is sufficient |
| CLI argument parsing | Manual arg parsing | `clap` derive macros on AppCommand enum | Already the pattern for all cargo-pmcp commands |

**Key insight:** This phase is primarily string assembly (JSON and HTML from templates). No network calls, no async, no complex state. The heavy lifting (widget discovery, Cargo.toml format) is already solved by existing code.

## Common Pitfalls

### Pitfall 1: srcdoc HTML Escaping
**What goes wrong:** Widget HTML contains `"` characters. When placed into `srcdoc="..."`, unescaped quotes break the HTML attribute boundary, corrupting the landing page.
**Why it happens:** Developers forget that srcdoc content is an HTML attribute value that needs entity escaping.
**How to avoid:** Escape the full widget HTML string before embedding: `&` -> `&amp;`, `"` -> `&quot;`, `<` is fine inside srcdoc (it's parsed as HTML content), but `"` MUST be escaped. Use a dedicated `escape_for_srcdoc()` function.
**Warning signs:** Landing page renders as blank or broken HTML in browser.

### Pitfall 2: Mock Bridge Must Match Real Bridge API Shape
**What goes wrong:** Landing page mock bridge returns data in a different shape than the real bridge (e.g., `{ result: data }` vs `data` directly), causing widget JavaScript to fail silently.
**Why it happens:** The bridge API shape varies by context: ChatGPT bridge returns `window.openai.callTool()` result directly; preview proxy bridge returns `{ success, content }`; WASM bridge returns `{ success, content }`.
**How to avoid:** The mock bridge `callTool()` should return the mock-data JSON directly (the same shape the widget expects from the real bridge). Document that mock-data files should contain the exact response shape the widget's JavaScript code expects.
**Warning signs:** Widget loads but shows "undefined" or blank data areas.

### Pitfall 3: WidgetDir Not Available in cargo-pmcp Crate
**What goes wrong:** `WidgetDir` is defined in `pmcp::server::mcp_apps` which requires the `mcp-apps` feature of the `pmcp` crate. The `cargo-pmcp` crate depends on `mcp-preview` and `mcp-tester` but may not directly depend on `pmcp` with `mcp-apps`.
**Why it happens:** Build tool (cargo-pmcp) and runtime library (pmcp) are separate crates.
**How to avoid:** Check if cargo-pmcp can depend on pmcp with mcp-apps feature, OR reimplement the simple widget directory scan inline (it's ~30 lines: read_dir, filter .html, sort). Given WidgetDir is simple, reimplementing avoids pulling the full pmcp crate into the CLI tool.
**Warning signs:** Compile error when trying to import WidgetDir in cargo-pmcp code.

### Pitfall 4: Package.metadata.pmcp Parsing
**What goes wrong:** `[package.metadata.pmcp]` is a custom section. The `toml` crate reads it fine as `toml::Value`, but developers might try to deserialize it into a strict struct that fails if unexpected fields are present.
**How to avoid:** Use `toml::Value` navigation (`.get("package").and_then(...)`) instead of strongly-typed deserialization for the metadata section. This is the pattern already used throughout cargo-pmcp.
**Warning signs:** Manifest generation fails with "unknown field" deserialization error.

### Pitfall 5: Relative Paths in Generated Artifacts
**What goes wrong:** Generated manifest.json or landing.html contains paths like `widgets/board.html` instead of absolute or server-relative URLs.
**Why it happens:** At build time, widgets are local files. But the manifest's tool-to-widget mapping should reference resource URIs (`ui://app/board`), and the landing page should have widget HTML inlined (not linked).
**How to avoid:** Manifest uses `ui://app/{name}` URIs. Landing page inlines full HTML content. No relative file paths appear in output artifacts.
**Warning signs:** Manifest contains file system paths; landing page tries to load files that don't exist on the target host.

## Code Examples

### Example 1: Project Detection (reusable for all three commands)
```rust
// Reuse the toml::Value approach from deploy/init.rs
struct ProjectInfo {
    name: String,
    description: String,
    logo: Option<String>,
    widgets: Vec<WidgetInfo>,
}

struct WidgetInfo {
    name: String,       // "board" (file stem)
    uri: String,        // "ui://app/board"
    html: String,       // full HTML content
}

fn detect_project() -> Result<ProjectInfo> {
    // 1. Parse Cargo.toml
    let cargo_str = fs::read_to_string("Cargo.toml")
        .context("No Cargo.toml found")?;
    let cargo: toml::Value = toml::from_str(&cargo_str)?;

    // 2. Verify mcp-apps feature
    let has_feature = /* check dependencies.pmcp.features contains "mcp-apps" */;
    if !has_feature {
        bail!("Not an MCP Apps project. Run `cargo pmcp app new` first.");
    }

    // 3. Discover widgets
    let widgets_dir = PathBuf::from("widgets");
    if !widgets_dir.exists() {
        bail!("No widgets found in widgets/. Add .html files or run `cargo pmcp app new` to scaffold a project.");
    }

    let mut widgets = Vec::new();
    for entry in fs::read_dir(&widgets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("html") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let html = fs::read_to_string(&path)?;
                widgets.push(WidgetInfo {
                    name: stem.to_string(),
                    uri: format!("ui://app/{}", stem),
                    html,
                });
            }
        }
    }
    widgets.sort_by(|a, b| a.name.cmp(&b.name));

    if widgets.is_empty() {
        bail!("No widgets found in widgets/. Add .html files or run `cargo pmcp app new` to scaffold a project.");
    }

    // 4. Extract package metadata
    let name = cargo.get("package").and_then(|p| p.get("name"))
        .and_then(|n| n.as_str()).unwrap_or("unnamed").to_string();
    let description = cargo.get("package").and_then(|p| p.get("description"))
        .and_then(|d| d.as_str()).unwrap_or("").to_string();
    let logo = cargo.get("package").and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("pmcp")).and_then(|pm| pm.get("logo"))
        .and_then(|l| l.as_str()).map(String::from);

    Ok(ProjectInfo { name, description, logo, widgets })
}
```

### Example 2: Manifest JSON Generation
```rust
fn generate_manifest(project: &ProjectInfo, server_url: &str, logo_override: Option<&str>) -> Result<String> {
    let logo_url = logo_override
        .map(String::from)
        .or_else(|| project.logo.clone())
        .unwrap_or_default();

    let name_for_model = project.name.replace('-', "_").replace(' ', "_");

    let manifest = serde_json::json!({
        "schema_version": "v1",
        "name_for_human": project.name,
        "name_for_model": name_for_model,
        "description_for_human": project.description,
        "description_for_model": project.description,
        "auth": { "type": "none" },
        "api": {
            "type": "openapi",
            "url": format!("{}/openapi.json", server_url.trim_end_matches('/'))
        },
        "logo_url": logo_url,
        "contact_email": "",
        "legal_info_url": "",
        "mcp_apps": {
            "server_url": server_url,
            "widgets": project.widgets.iter().map(|w| {
                serde_json::json!({
                    "tool": w.name,
                    "resource_uri": w.uri,
                })
            }).collect::<Vec<_>>()
        }
    });

    serde_json::to_string_pretty(&manifest).context("Failed to serialize manifest")
}
```

### Example 3: Landing Page HTML Generation
```rust
fn generate_landing(project: &ProjectInfo, mock_data: &HashMap<String, String>) -> Result<String> {
    // Pick first widget for showcase (or allow selection via flag)
    let widget = project.widgets.first()
        .context("No widgets found")?;

    // Build mock bridge script with embedded data
    let mock_data_json = serde_json::to_string(mock_data)?;
    let mock_bridge = format!(r#"<script>
window.mcpBridge = {{
    _mockData: {mock_data_json},
    _state: {{}},
    callTool: async function(name, args) {{
        const data = this._mockData[name];
        return data || {{ error: 'No mock data for: ' + name }};
    }},
    getState: function() {{ return this._state; }},
    setState: function(s) {{ Object.assign(this._state, s); }},
    get theme() {{ return 'light'; }},
    get locale() {{ return 'en-US'; }},
    get displayMode() {{ return 'inline'; }}
}};
window.dispatchEvent(new Event('mcpBridgeReady'));
</script>"#);

    // Inject mock bridge into widget HTML (replacing server bridge)
    let widget_with_bridge = inject_mock_bridge(&widget.html, &mock_bridge);

    // Escape for srcdoc attribute
    let escaped = escape_for_srcdoc(&widget_with_bridge);

    // Assemble landing page
    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{name}</title>
    <style>{css}</style>
</head>
<body>
    <header class="showcase-header">
        <h1>{name}</h1>
        <p class="description">{description}</p>
    </header>
    <main class="showcase-main">
        <div class="widget-frame">
            <iframe srcdoc="{srcdoc}" sandbox="allow-scripts" loading="lazy"></iframe>
        </div>
    </main>
    <footer class="showcase-footer">
        <p>Built with <a href="https://crates.io/crates/pmcp">pmcp</a></p>
    </footer>
</body>
</html>"#,
        name = project.name,
        description = project.description,
        css = LANDING_CSS,
        srcdoc = escaped,
    );

    Ok(html)
}

fn escape_for_srcdoc(html: &str) -> String {
    html.replace('&', "&amp;").replace('"', "&quot;")
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ChatGPT plugins (ai-plugin.json) | ChatGPT Actions + Apps (ai-plugin.json still used) | 2024 | ai-plugin.json schema is stable at "v1"; no breaking changes |
| Separate CSS/JS files for landing pages | Single-file HTML with all resources inlined | Ongoing best practice | Zero deployment complexity; works from filesystem |

**Deprecated/outdated:**
- ChatGPT plugin beta program was sunset; replaced by ChatGPT Actions and Apps. The ai-plugin.json manifest format remains compatible.

## Open Questions

1. **Should `app manifest` validate that tool names match widget names?**
   - What we know: Widget filenames are discovered from `widgets/`. Tools are registered in `src/main.rs`. These are separate concerns -- manifest generator only sees the file system.
   - What's unclear: Whether a mismatch (widget `board.html` but no tool `board`) should warn or silently include.
   - Recommendation: Include all discovered widgets in the manifest without tool validation. The manifest is a declaration of capabilities. Tool validation would require running the server, which is out of scope for a build-time command. Add a comment in the output suggesting the developer verify tool names.

2. **How should multiple widgets be handled in the landing page?**
   - What we know: A project can have multiple widgets (e.g., chess has `board.html`, map has `map.html`). The landing page renders "a widget" in a showcase.
   - What's unclear: Whether to show all widgets (tabbed? scrolling?) or just the first one.
   - Recommendation: Default to the first widget (alphabetically sorted). Add an optional `--widget <name>` flag to select which widget to showcase. This keeps the default simple while supporting multi-widget projects.

3. **Should the scaffolding (`cargo pmcp app new`) also create `mock-data/` directory?**
   - What we know: Phase 17 scaffolding creates `src/`, `widgets/`, and project files. Phase 18 requires `mock-data/` for landing page generation.
   - What's unclear: Whether to update the Phase 17 template or require manual creation.
   - Recommendation: Update the mcp_app template to also create `mock-data/hello.json` with a sample response matching the hello tool. This is a one-line addition to `templates/mcp_app.rs`.

4. **Feature detection approach: simple string check or full TOML parsing?**
   - What we know: The decision says "check Cargo.toml for pmcp dependency with mcp-apps feature." The pmcp dependency could be: `pmcp = { version = "1.10", features = ["mcp-apps"] }` or `pmcp = { version = "1.10", features = ["full"] }` (which includes mcp-apps).
   - What's unclear: Whether to also accept the "full" feature as a valid MCP Apps indicator.
   - Recommendation: Check for either `"mcp-apps"` or `"full"` in the features array. The "full" feature is a superset. This avoids false negatives for projects using `features = ["full"]` like the chess example.

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `cargo-pmcp/src/commands/app.rs` -- existing AppCommand enum pattern
- Codebase inspection: `cargo-pmcp/src/templates/mcp_app.rs` -- template generation pattern
- Codebase inspection: `src/server/mcp_apps/widget_fs.rs` -- WidgetDir API for widget discovery
- Codebase inspection: `src/server/mcp_apps/adapter.rs` -- Bridge injection patterns (ChatGPT, MCP Apps)
- Codebase inspection: `cargo-pmcp/src/commands/deploy/init.rs` -- Cargo.toml parsing with toml::Value
- Codebase inspection: `crates/mcp-preview/assets/widget-runtime.js` -- WASM bridge API shape
- Codebase inspection: `examples/mcp-apps-chess/src/main.rs` -- Real MCP Apps project structure

### Secondary (MEDIUM confidence)
- [ChatGPT Plugin Manifest](https://www.hackwithgpt.com/blog/what-is-the-chatgpt-plugin-manifest/) -- ai-plugin.json schema fields (verified against multiple sources)
- [ChatGPT Plugin Quickstart](https://community.openai.com/t/chatgpt-plugin-quickstart/150783) -- Schema version "v1" confirmation

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in cargo-pmcp Cargo.toml; no new dependencies needed
- Architecture: HIGH -- follows established patterns (AppCommand enum, toml::Value parsing, format!() templates)
- Pitfalls: HIGH -- pitfalls identified from codebase inspection (srcdoc escaping, bridge shape, WidgetDir availability)

**Research date:** 2026-02-26
**Valid until:** 2026-03-28 (stable domain -- no moving targets)
