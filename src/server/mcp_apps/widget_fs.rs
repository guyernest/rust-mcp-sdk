//! Widget filesystem discovery and hot-reload support.
//!
//! Provides a `WidgetDir` helper that scans a `widgets/` directory for `.html` files
//! and maps each file to an MCP resource URI. Reading from disk on every call enables
//! hot-reload: a browser refresh shows the latest widget HTML without server restart.
//!
//! # Convention
//!
//! - Filename maps directly to MCP resource URI: `widgets/board.html` -> `ui://app/board`
//! - Widgets are single self-contained HTML files
//! - The server auto-injects the bridge script tag via [`WidgetDir::inject_bridge_script`]
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::mcp_apps::WidgetDir;
//!
//! let widgets = WidgetDir::new("examples/mcp-apps-chess/widgets");
//!
//! // Discover all widgets
//! let entries = widgets.discover().unwrap();
//! for entry in &entries {
//!     println!("{} -> {}", entry.filename, entry.uri);
//! }
//!
//! // Read widget HTML from disk (fresh on every call)
//! let html = widgets.read_widget("board").unwrap();
//!
//! // Auto-inject bridge script
//! let html_with_bridge = WidgetDir::inject_bridge_script(&html, "/assets/widget-runtime.mjs");
//! ```

use std::path::{Path, PathBuf};

/// A discovered widget file entry.
#[derive(Debug, Clone)]
pub struct WidgetEntry {
    /// Stem of the HTML file (e.g., "board").
    pub filename: String,
    /// MCP resource URI (e.g., `ui://app/board`).
    pub uri: String,
    /// Absolute path to the `.html` file on disk.
    pub path: PathBuf,
}

/// Widget directory scanner and reader.
///
/// Scans a directory for `.html` files, maps them to MCP resource URIs,
/// and reads content from disk on every request to enable hot-reload.
#[derive(Debug, Clone)]
pub struct WidgetDir {
    path: PathBuf,
}

impl WidgetDir {
    /// Create a new `WidgetDir` pointing at the given directory.
    ///
    /// The path does not need to exist at construction time; errors are
    /// returned when [`discover`](Self::discover) or [`read_widget`](Self::read_widget)
    /// are called.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Return the directory path this `WidgetDir` points to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Scan the widgets directory for `.html` files.
    ///
    /// Returns a sorted list of [`WidgetEntry`] values, one per `.html` file found.
    /// The entries are sorted by filename for deterministic ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read (e.g., does not exist or
    /// permission denied).
    pub fn discover(&self) -> std::io::Result<Vec<WidgetEntry>> {
        let mut entries = Vec::new();

        let dir = std::fs::read_dir(&self.path)?;

        for entry in dir {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) == Some("html") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let abs_path = if path.is_absolute() {
                        path.clone()
                    } else {
                        std::env::current_dir().unwrap_or_default().join(&path)
                    };

                    entries.push(WidgetEntry {
                        filename: stem.to_string(),
                        uri: format!("ui://app/{}", stem),
                        path: abs_path,
                    });
                }
            }
        }

        entries.sort_by(|a, b| a.filename.cmp(&b.filename));

        tracing::debug!(
            "Discovered {} widget(s) in {}",
            entries.len(),
            self.path.display()
        );

        Ok(entries)
    }

    /// Read a widget's HTML content from disk.
    ///
    /// Reads from disk on every call (no caching) to enable hot-reload during
    /// development. If the file does not exist or cannot be read, returns a
    /// styled HTML error page with the filename and error details.
    pub fn read_widget(&self, name: &str) -> String {
        let file_path = self.path.join(format!("{}.html", name));

        match std::fs::read_to_string(&file_path) {
            Ok(content) => {
                tracing::debug!(
                    "Reading widget file: {} ({} bytes)",
                    file_path.display(),
                    content.len()
                );
                content
            },
            Err(err) => {
                tracing::warn!(
                    "Failed to read widget file {}: {}",
                    file_path.display(),
                    err
                );
                Self::error_page(name, &file_path, &err.to_string())
            },
        }
    }

    /// Insert a `<script src="{bridge_url}"></script>` tag into widget HTML.
    ///
    /// Injection strategy:
    /// - If `</head>` is present, inserts the script tag just before it.
    /// - Otherwise, inserts at the start of `<body>` (or at the very beginning
    ///   of the document if no `<body>` tag is found).
    ///
    /// This allows widget authors to write plain HTML without bridge boilerplate;
    /// the server handles injection automatically.
    pub fn inject_bridge_script(html: &str, bridge_url: &str) -> String {
        pmcp_widget_utils::inject_bridge_script(html, bridge_url)
    }

    /// Generate a styled HTML error page for a widget that failed to load.
    fn error_page(name: &str, path: &Path, error: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Widget Error: {name}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            padding: 20px;
        }}
        .error-card {{
            background: #4a1515;
            border: 1px solid #ff6b6b;
            border-radius: 12px;
            padding: 24px 32px;
            max-width: 560px;
            width: 100%;
        }}
        h2 {{
            color: #ff6b6b;
            margin: 0 0 12px 0;
            font-size: 1.2rem;
        }}
        .file-path {{
            font-family: monospace;
            font-size: 0.85rem;
            color: #ffcc00;
            background: rgba(0,0,0,0.3);
            padding: 6px 10px;
            border-radius: 6px;
            word-break: break-all;
            margin-bottom: 12px;
        }}
        .error-message {{
            font-family: monospace;
            font-size: 0.85rem;
            color: #ff9999;
            line-height: 1.5;
        }}
        .hint {{
            margin-top: 16px;
            font-size: 0.85rem;
            color: #888;
        }}
    </style>
</head>
<body>
    <div class="error-card">
        <h2>Widget Load Error</h2>
        <div class="file-path">{path}</div>
        <div class="error-message">{error}</div>
        <div class="hint">
            Create or fix the widget file and refresh the browser to retry.
        </div>
    </div>
</body>
</html>"#,
            name = name,
            path = path.display(),
            error = error,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_finds_html_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("board.html"), "<html>board</html>").unwrap();
        fs::write(dir.path().join("map.html"), "<html>map</html>").unwrap();
        fs::write(dir.path().join("readme.txt"), "not a widget").unwrap();

        let widget_dir = WidgetDir::new(dir.path());
        let entries = widget_dir.discover().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].filename, "board");
        assert_eq!(entries[0].uri, "ui://app/board");
        assert_eq!(entries[1].filename, "map");
        assert_eq!(entries[1].uri, "ui://app/map");
    }

    #[test]
    fn test_discover_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let widget_dir = WidgetDir::new(dir.path());
        let entries = widget_dir.discover().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_discover_nonexistent_directory() {
        let widget_dir = WidgetDir::new("/tmp/nonexistent-widget-dir-12345");
        assert!(widget_dir.discover().is_err());
    }

    #[test]
    fn test_read_widget_success() {
        let dir = tempfile::tempdir().unwrap();
        let content = "<html><body>Hello Widget</body></html>";
        fs::write(dir.path().join("test.html"), content).unwrap();

        let widget_dir = WidgetDir::new(dir.path());
        let result = widget_dir.read_widget("test");
        assert_eq!(result, content);
    }

    #[test]
    fn test_read_widget_missing_returns_error_page() {
        let dir = tempfile::tempdir().unwrap();
        let widget_dir = WidgetDir::new(dir.path());
        let result = widget_dir.read_widget("nonexistent");

        assert!(result.contains("Widget Load Error"));
        assert!(result.contains("nonexistent"));
    }

    #[test]
    fn test_inject_bridge_script_before_head_close() {
        let html = "<html><head><title>Test</title></head><body>Content</body></html>";
        let result = WidgetDir::inject_bridge_script(html, "/assets/widget-runtime.mjs");

        assert!(
            result.contains(r#"<script type="module" src="/assets/widget-runtime.mjs"></script>"#)
        );
        // Script should appear before </head>
        let script_pos = result.find("widget-runtime.mjs").unwrap();
        let head_close_pos = result.find("</head>").unwrap();
        assert!(script_pos < head_close_pos);
    }

    #[test]
    fn test_inject_bridge_script_after_body_open() {
        let html = "<html><body>Content</body></html>";
        let result = WidgetDir::inject_bridge_script(html, "/assets/widget-runtime.mjs");

        assert!(
            result.contains(r#"<script type="module" src="/assets/widget-runtime.mjs"></script>"#)
        );
        // Script should appear after <body>
        let body_pos = result.find("<body>").unwrap();
        let script_pos = result.find("widget-runtime.mjs").unwrap();
        assert!(script_pos > body_pos);
    }

    #[test]
    fn test_inject_bridge_script_no_head_no_body() {
        let html = "<div>Just content</div>";
        let result = WidgetDir::inject_bridge_script(html, "/assets/widget-runtime.mjs");

        assert!(result
            .starts_with(r#"<script type="module" src="/assets/widget-runtime.mjs"></script>"#));
    }

    #[test]
    fn test_error_page_contains_details() {
        let page =
            WidgetDir::error_page("board", Path::new("/widgets/board.html"), "file not found");

        assert!(page.contains("Widget Load Error"));
        assert!(page.contains("/widgets/board.html"));
        assert!(page.contains("file not found"));
        assert!(page.contains("board"));
    }
}
