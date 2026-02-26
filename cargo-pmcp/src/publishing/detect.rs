//! MCP Apps project detection.
//!
//! Reads `Cargo.toml` and `widgets/` to determine whether the current
//! directory is a valid MCP Apps project and to collect widget metadata.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

/// Metadata about a discovered widget file.
#[derive(Debug, Clone)]
pub struct WidgetInfo {
    /// File stem used as the tool name (e.g. "board").
    pub name: String,
    /// MCP resource URI (e.g. "ui://app/board").
    pub uri: String,
    /// Full HTML content read from disk (used by packaging pipeline).
    #[allow(dead_code)]
    pub html: String,
}

/// Aggregated project information extracted from `Cargo.toml` and `widgets/`.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    /// Package name from `[package].name`.
    pub name: String,
    /// Package description from `[package].description`.
    pub description: String,
    /// Optional logo URL from `[package.metadata.pmcp].logo`.
    pub logo: Option<String>,
    /// Widgets discovered in the `widgets/` directory.
    pub widgets: Vec<WidgetInfo>,
}

/// Detect an MCP Apps project in the given directory.
///
/// Reads `Cargo.toml` to verify the project depends on `pmcp` with the
/// `mcp-apps` or `full` feature enabled, then scans `widgets/` for `.html`
/// files. Returns a `ProjectInfo` on success or a descriptive error if the
/// directory is not a valid MCP Apps project.
pub fn detect_project(project_dir: &Path) -> Result<ProjectInfo> {
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let raw = fs::read_to_string(&cargo_toml_path)
        .with_context(|| "No Cargo.toml found. Are you in a Rust project directory?")?;

    let doc: toml::Value = toml::from_str(&raw).context("Failed to parse Cargo.toml")?;

    // Verify pmcp dependency has mcp-apps or full feature
    verify_mcp_apps_feature(&doc)?;

    // Extract package metadata
    let name = doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed")
        .to_string();

    let description = doc
        .get("package")
        .and_then(|p| p.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let logo = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("pmcp"))
        .and_then(|pm| pm.get("logo"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Discover widgets
    let widgets_dir = project_dir.join("widgets");
    let widgets = discover_widgets(&widgets_dir)?;

    Ok(ProjectInfo {
        name,
        description,
        logo,
        widgets,
    })
}

/// Verify that the `pmcp` dependency includes `mcp-apps` or `full` features.
fn verify_mcp_apps_feature(doc: &toml::Value) -> Result<()> {
    let pmcp_dep = doc.get("dependencies").and_then(|d| d.get("pmcp"));

    let Some(pmcp) = pmcp_dep else {
        bail!(
            "Not an MCP Apps project. \
             No `pmcp` dependency found in Cargo.toml. \
             Run `cargo pmcp app new` first."
        );
    };

    // Simple string version (e.g. pmcp = "1.10") has no features
    if pmcp.is_str() {
        bail!(
            "Not an MCP Apps project. \
             The `pmcp` dependency does not enable `mcp-apps` or `full` features. \
             Run `cargo pmcp app new` first."
        );
    }

    let features = pmcp.get("features").and_then(|f| f.as_array());

    let has_required_feature = features
        .map(|arr| {
            arr.iter().any(|v| {
                v.as_str()
                    .map(|s| s == "mcp-apps" || s == "full")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    if !has_required_feature {
        bail!(
            "Not an MCP Apps project. \
             The `pmcp` dependency does not enable `mcp-apps` or `full` features. \
             Run `cargo pmcp app new` first."
        );
    }

    Ok(())
}

/// Scan the `widgets/` directory for `.html` files and return sorted
/// `WidgetInfo` entries.
fn discover_widgets(widgets_dir: &Path) -> Result<Vec<WidgetInfo>> {
    if !widgets_dir.is_dir() {
        bail!(
            "No widgets found in widgets/. \
             Add .html files or run `cargo pmcp app new` to scaffold a project."
        );
    }

    let mut entries: Vec<_> = fs::read_dir(widgets_dir)
        .context("Failed to read widgets/ directory")?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "html")
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        bail!(
            "No widgets found in widgets/. \
             Add .html files or run `cargo pmcp app new` to scaffold a project."
        );
    }

    // Sort by filename for deterministic output
    entries.sort_by_key(|e| e.file_name());

    let mut widgets = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let html = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read widget: {}", path.display()))?;

        widgets.push(WidgetInfo {
            uri: format!("ui://app/{}", stem),
            name: stem,
            html,
        });
    }

    Ok(widgets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_cargo_toml(dir: &Path, content: &str) {
        fs::write(dir.join("Cargo.toml"), content).unwrap();
    }

    fn setup_widgets(dir: &Path, files: &[(&str, &str)]) {
        let widgets_dir = dir.join("widgets");
        fs::create_dir_all(&widgets_dir).unwrap();
        for (name, content) in files {
            fs::write(widgets_dir.join(name), content).unwrap();
        }
    }

    #[test]
    fn test_detect_mcp_apps_feature() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "test-app"
description = "A test app"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        setup_widgets(dir.path(), &[("board.html", "<h1>Board</h1>")]);

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.name, "test-app");
        assert_eq!(info.description, "A test app");
        assert_eq!(info.widgets.len(), 1);
        assert_eq!(info.widgets[0].name, "board");
        assert_eq!(info.widgets[0].uri, "ui://app/board");
    }

    #[test]
    fn test_detect_full_feature() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "full-app"
description = "Uses full feature"

[dependencies]
pmcp = { version = "1.10", features = ["full"] }
"#,
        );
        setup_widgets(dir.path(), &[("chat.html", "<p>Chat</p>")]);

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.name, "full-app");
    }

    #[test]
    fn test_detect_missing_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No Cargo.toml found"),
            "Expected Cargo.toml error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_not_mcp_apps_project_simple_string() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "plain-app"

[dependencies]
pmcp = "1.10"
"#,
        );

        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Not an MCP Apps project"),
            "Expected MCP Apps error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_missing_features() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "no-features"

[dependencies]
pmcp = { version = "1.10", features = ["storage"] }
"#,
        );

        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Not an MCP Apps project"),
            "Expected MCP Apps error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_no_pmcp_dependency() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "other-app"

[dependencies]
serde = "1"
"#,
        );

        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Not an MCP Apps project"),
            "Expected MCP Apps error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_missing_widgets_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "test-app"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );

        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No widgets found"),
            "Expected widgets error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_empty_widgets_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "test-app"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        fs::create_dir_all(dir.path().join("widgets")).unwrap();

        let result = detect_project(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No widgets found"),
            "Expected widgets error, got: {}",
            err
        );
    }

    #[test]
    fn test_detect_multiple_widgets_sorted() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "multi-app"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        setup_widgets(
            dir.path(),
            &[
                ("zebra.html", "<p>Z</p>"),
                ("alpha.html", "<p>A</p>"),
                ("middle.html", "<p>M</p>"),
            ],
        );

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.widgets.len(), 3);
        assert_eq!(info.widgets[0].name, "alpha");
        assert_eq!(info.widgets[1].name, "middle");
        assert_eq!(info.widgets[2].name, "zebra");
    }

    #[test]
    fn test_detect_logo_from_metadata() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "branded-app"
description = "Has a logo"

[package.metadata.pmcp]
logo = "https://example.com/logo.png"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        setup_widgets(dir.path(), &[("hello.html", "<p>Hi</p>")]);

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.logo, Some("https://example.com/logo.png".to_string()));
    }

    #[test]
    fn test_detect_no_logo() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "no-logo"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        setup_widgets(dir.path(), &[("hello.html", "<p>Hi</p>")]);

        let info = detect_project(dir.path()).unwrap();
        assert!(info.logo.is_none());
    }

    #[test]
    fn test_detect_widget_html_content() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "content-test"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        let html = "<html><body><h1>My Widget</h1></body></html>";
        setup_widgets(dir.path(), &[("board.html", html)]);

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.widgets[0].html, html);
    }

    #[test]
    fn test_detect_ignores_non_html_files() {
        let dir = tempfile::tempdir().unwrap();
        write_cargo_toml(
            dir.path(),
            r#"
[package]
name = "mixed-files"

[dependencies]
pmcp = { version = "1.10", features = ["mcp-apps"] }
"#,
        );
        let widgets_dir = dir.path().join("widgets");
        fs::create_dir_all(&widgets_dir).unwrap();
        fs::write(widgets_dir.join("board.html"), "<p>Board</p>").unwrap();
        fs::write(widgets_dir.join("style.css"), "body {}").unwrap();
        fs::write(widgets_dir.join("notes.txt"), "some notes").unwrap();

        let info = detect_project(dir.path()).unwrap();
        assert_eq!(info.widgets.len(), 1);
        assert_eq!(info.widgets[0].name, "board");
    }
}
