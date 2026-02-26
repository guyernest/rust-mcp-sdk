//! Landing page HTML generation with mock bridge.
//!
//! Generates a single self-contained HTML file that showcases an MCP Apps
//! widget in an iframe with a mock bridge returning hardcoded responses
//! from `mock-data/*.json` files. The landing page is viewable without a
//! running server.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::detect::ProjectInfo;

/// Product-showcase CSS for the landing page.
const LANDING_CSS: &str = r#"
    * {
        box-sizing: border-box;
        margin: 0;
        padding: 0;
    }
    body {
        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #f5f5f7;
        display: flex;
        flex-direction: column;
        align-items: center;
        min-height: 100vh;
        padding: 40px 20px;
    }
    .showcase-header {
        text-align: center;
        margin-bottom: 32px;
        max-width: 960px;
        width: 100%;
    }
    .showcase-header h1 {
        font-size: 2.4rem;
        font-weight: 700;
        color: #1d1d1f;
        margin-bottom: 8px;
    }
    .showcase-header .description {
        font-size: 1.1rem;
        color: #86868b;
        line-height: 1.5;
    }
    .showcase-main {
        max-width: 960px;
        width: 100%;
    }
    .widget-frame {
        background: white;
        border-radius: 12px;
        border: 1px solid #e0e0e0;
        box-shadow: 0 4px 24px rgba(0, 0, 0, 0.06);
        overflow: hidden;
    }
    .widget-frame iframe {
        width: 100%;
        height: 600px;
        border: none;
        display: block;
    }
    .showcase-footer {
        text-align: center;
        margin-top: 32px;
        max-width: 960px;
        width: 100%;
    }
    .showcase-footer p {
        font-size: 0.9rem;
        color: #86868b;
    }
    .showcase-footer a {
        color: #0066cc;
        text-decoration: none;
    }
    .showcase-footer a:hover {
        text-decoration: underline;
    }
    @media (max-width: 640px) {
        body {
            padding: 20px 12px;
        }
        .showcase-header h1 {
            font-size: 1.6rem;
        }
        .widget-frame iframe {
            height: 400px;
        }
    }
"#;

/// Load mock data from the `mock-data/` directory.
///
/// Reads every `.json` file in `mock-data/` and returns a `HashMap` keyed by
/// the file stem (e.g. `mock-data/hello.json` becomes key `"hello"`).
///
/// Returns an error if the directory is missing or contains no `.json` files.
pub fn load_mock_data(project_dir: &Path) -> Result<HashMap<String, serde_json::Value>> {
    let mock_dir = project_dir.join("mock-data");

    if !mock_dir.is_dir() {
        bail!(
            "No mock data found. Create mock-data/tool-name.json for each tool."
        );
    }

    let mut data = HashMap::new();

    let entries = fs::read_dir(&mock_dir).context("Failed to read mock-data/ directory")?;

    for entry in entries {
        let entry = entry.context("Failed to read mock-data/ entry")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let value: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON in {}", path.display()))?;

        data.insert(stem, value);
    }

    if data.is_empty() {
        bail!(
            "No mock data found. Create mock-data/tool-name.json for each tool."
        );
    }

    Ok(data)
}

/// Escape HTML for use in an `srcdoc` attribute value.
///
/// Only `&` and `"` need escaping for the srcdoc context.
/// `<` and `>` are left as-is because the browser parses srcdoc content
/// as HTML. The `&` replacement must happen first to avoid double-escaping.
fn escape_for_srcdoc(html: &str) -> String {
    html.replace('&', "&amp;").replace('"', "&quot;")
}

/// Inject a mock bridge `<script>` tag into widget HTML.
///
/// The script provides a `window.mcpBridge` object that returns mock data
/// for `callTool` calls and dispatches `mcpBridgeReady` once installed.
///
/// Injection strategy (matching `WidgetDir::inject_bridge_script`):
/// 1. Before `</head>` if present
/// 2. After `<body>` if present (when no `</head>`)
/// 3. Prepend to content otherwise
fn inject_mock_bridge(widget_html: &str, mock_data_json: &str) -> String {
    let script = format!(
        r#"<script type="module">
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
    </script>"#
    );

    if let Some(pos) = widget_html.find("</head>") {
        let mut result = String::with_capacity(widget_html.len() + script.len());
        result.push_str(&widget_html[..pos]);
        result.push_str(&script);
        result.push_str(&widget_html[pos..]);
        result
    } else if let Some(pos) = widget_html.find("<body>") {
        let insert_at = pos + "<body>".len();
        let mut result = String::with_capacity(widget_html.len() + script.len());
        result.push_str(&widget_html[..insert_at]);
        result.push_str(&script);
        result.push_str(&widget_html[insert_at..]);
        result
    } else {
        format!("{}{}", script, widget_html)
    }
}

/// Generate a self-contained landing page HTML string.
///
/// Embeds the selected widget in an iframe using `srcdoc`, with a mock bridge
/// that returns data from the provided `mock_data` HashMap. The page includes
/// product-showcase styling and is viewable without a running server.
///
/// If `widget_name` is `Some`, the matching widget is selected. Otherwise the
/// first widget alphabetically is used. Returns an error if the requested
/// widget is not found or the project has no widgets.
pub fn generate_landing(
    project: &ProjectInfo,
    mock_data: &HashMap<String, serde_json::Value>,
    widget_name: Option<&str>,
) -> Result<String> {
    let widget = if let Some(name) = widget_name {
        project
            .widgets
            .iter()
            .find(|w| w.name == name)
            .with_context(|| {
                format!(
                    "Widget '{}' not found. Available: {}",
                    name,
                    project
                        .widgets
                        .iter()
                        .map(|w| w.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?
    } else {
        project
            .widgets
            .first()
            .context("No widgets found in project")?
    };

    let mock_data_json = serde_json::to_string(mock_data)
        .context("Failed to serialize mock data")?;

    let widget_with_bridge = inject_mock_bridge(&widget.html, &mock_data_json);
    let escaped = escape_for_srcdoc(&widget_with_bridge);

    let html = format!(
        r#"<!DOCTYPE html>
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
            <iframe srcdoc="{escaped}" sandbox="allow-scripts" loading="lazy"></iframe>
        </div>
    </main>
    <footer class="showcase-footer">
        <p>Built with <a href="https://crates.io/crates/pmcp">pmcp</a></p>
    </footer>
</body>
</html>"#,
        name = project.name,
        css = LANDING_CSS,
        description = project.description,
        escaped = escaped,
    );

    Ok(html)
}

/// Write landing page HTML to the output directory.
///
/// Creates the output directory if it does not exist and writes the content
/// to `{output_dir}/landing.html`.
pub fn write_landing(output_dir: &str, content: &str) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir))?;

    let path = std::path::Path::new(output_dir).join("landing.html");
    fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    println!("  {} Generated {}/landing.html", "ok".green(), output_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publishing::detect::{ProjectInfo, WidgetInfo};

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            name: "demo-app".to_string(),
            description: "A demo application".to_string(),
            logo: None,
            widgets: vec![WidgetInfo {
                name: "hello".to_string(),
                uri: "ui://app/hello".to_string(),
                html: r#"<html><head><title>Hello</title></head><body><p>Hello</p></body></html>"#
                    .to_string(),
            }],
        }
    }

    fn sample_mock_data() -> HashMap<String, serde_json::Value> {
        let mut data = HashMap::new();
        data.insert(
            "hello".to_string(),
            serde_json::json!({"greeting": "Hello, World!"}),
        );
        data
    }

    #[test]
    fn test_escape_for_srcdoc() {
        let input = r#"<div class="test">&amp;</div>"#;
        let escaped = escape_for_srcdoc(input);
        // & becomes &amp;, " becomes &quot;, < stays <
        assert!(escaped.contains("&amp;amp;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("<div"));
        assert!(!escaped.contains("\"test\""));

        // Verify & is replaced before " to avoid double-escaping issues
        let simple = escape_for_srcdoc("a&b\"c");
        assert_eq!(simple, "a&amp;b&quot;c");
    }

    #[test]
    fn test_inject_mock_bridge_before_head() {
        let html = "<html><head><title>Test</title></head><body>Hi</body></html>";
        let result = inject_mock_bridge(html, r#"{"hello":{}}"#);
        assert!(result.contains("<script type=\"module\">"));
        // Script should appear before </head>
        let script_pos = result.find("<script type=\"module\">").unwrap();
        let head_close_pos = result.find("</head>").unwrap();
        assert!(script_pos < head_close_pos, "Script should be before </head>");
    }

    #[test]
    fn test_inject_mock_bridge_after_body() {
        let html = "<html><body><p>Content</p></body></html>";
        let result = inject_mock_bridge(html, r#"{"hello":{}}"#);
        assert!(result.contains("<script type=\"module\">"));
        // Script should appear after <body>
        let body_pos = result.find("<body>").unwrap();
        let script_pos = result.find("<script type=\"module\">").unwrap();
        assert!(
            script_pos > body_pos,
            "Script should be after <body>"
        );
    }

    #[test]
    fn test_inject_mock_bridge_prepend() {
        let html = "<div>Just content</div>";
        let result = inject_mock_bridge(html, r#"{"hello":{}}"#);
        assert!(result.starts_with("<script type=\"module\">"));
        assert!(result.ends_with("<div>Just content</div>"));
    }

    #[test]
    fn test_generate_landing_basic() {
        let project = sample_project();
        let mock_data = sample_mock_data();

        let html = generate_landing(&project, &mock_data, None).unwrap();

        // Contains project info
        assert!(html.contains("demo-app"));
        assert!(html.contains("A demo application"));

        // Contains escaped widget content in srcdoc
        assert!(html.contains("srcdoc="));

        // Contains showcase styling
        assert!(html.contains("showcase-header"));
        assert!(html.contains("widget-frame"));
        assert!(html.contains("showcase-footer"));

        // Contains pmcp attribution
        assert!(html.contains("Built with"));
        assert!(html.contains("crates.io/crates/pmcp"));

        // Contains mock data reference (escaped in srcdoc)
        assert!(html.contains("Hello, World!"));
    }

    #[test]
    fn test_generate_landing_widget_selection() {
        let project = ProjectInfo {
            name: "multi-app".to_string(),
            description: "Multiple widgets".to_string(),
            logo: None,
            widgets: vec![
                WidgetInfo {
                    name: "alpha".to_string(),
                    uri: "ui://app/alpha".to_string(),
                    html: "<p>Alpha Widget</p>".to_string(),
                },
                WidgetInfo {
                    name: "beta".to_string(),
                    uri: "ui://app/beta".to_string(),
                    html: "<p>Beta Widget</p>".to_string(),
                },
            ],
        };
        let mock_data = sample_mock_data();

        // Select specific widget
        let html = generate_landing(&project, &mock_data, Some("beta")).unwrap();
        assert!(html.contains("Beta Widget"));

        // Default selects first
        let html_default = generate_landing(&project, &mock_data, None).unwrap();
        assert!(html_default.contains("Alpha Widget"));

        // Non-existent widget errors
        let err = generate_landing(&project, &mock_data, Some("nonexistent"));
        assert!(err.is_err());
        let err_msg = err.unwrap_err().to_string();
        assert!(err_msg.contains("nonexistent"));
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn test_generate_landing_empty_widgets() {
        let project = ProjectInfo {
            name: "empty".to_string(),
            description: "No widgets".to_string(),
            logo: None,
            widgets: vec![],
        };
        let mock_data = sample_mock_data();

        let err = generate_landing(&project, &mock_data, None);
        assert!(err.is_err());
    }

    #[test]
    fn test_load_mock_data() {
        let dir = tempfile::tempdir().unwrap();
        let mock_dir = dir.path().join("mock-data");
        fs::create_dir_all(&mock_dir).unwrap();
        fs::write(
            mock_dir.join("hello.json"),
            r#"{"greeting": "Hi"}"#,
        )
        .unwrap();
        fs::write(
            mock_dir.join("greet.json"),
            r#"{"message": "Greetings"}"#,
        )
        .unwrap();

        let data = load_mock_data(dir.path()).unwrap();
        assert_eq!(data.len(), 2);
        assert!(data.contains_key("hello"));
        assert!(data.contains_key("greet"));
        assert_eq!(data["hello"]["greeting"], "Hi");
    }

    #[test]
    fn test_load_mock_data_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_mock_data(dir.path());
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("No mock data found"));
    }

    #[test]
    fn test_load_mock_data_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("mock-data")).unwrap();

        let err = load_mock_data(dir.path());
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("No mock data found"));
    }

    #[test]
    fn test_load_mock_data_ignores_non_json() {
        let dir = tempfile::tempdir().unwrap();
        let mock_dir = dir.path().join("mock-data");
        fs::create_dir_all(&mock_dir).unwrap();
        fs::write(mock_dir.join("hello.json"), r#"{"ok": true}"#).unwrap();
        fs::write(mock_dir.join("notes.txt"), "not json").unwrap();

        let data = load_mock_data(dir.path()).unwrap();
        assert_eq!(data.len(), 1);
        assert!(data.contains_key("hello"));
    }

    #[test]
    fn test_write_landing_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("dist");
        let output_str = output.to_str().unwrap();

        write_landing(output_str, "<html>test</html>").unwrap();

        let written = fs::read_to_string(output.join("landing.html")).unwrap();
        assert_eq!(written, "<html>test</html>");
    }

    #[test]
    fn test_write_landing_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("dist");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("landing.html"), "old").unwrap();

        write_landing(output.to_str().unwrap(), "new").unwrap();

        let written = fs::read_to_string(output.join("landing.html")).unwrap();
        assert_eq!(written, "new");
    }
}
