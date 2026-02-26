//! ChatGPT-compatible manifest JSON generation.
//!
//! Produces a `manifest.json` following the `ai-plugin.json` schema (v1)
//! extended with an `mcp_apps` block containing server URL and widget
//! mappings.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::json;
use std::fs;

use super::detect::ProjectInfo;

/// Generate a ChatGPT-compatible manifest JSON string from project info.
///
/// The manifest follows the `ai-plugin.json` schema version `v1` and
/// includes an `mcp_apps` extension block with the server URL and
/// auto-discovered widget-to-tool mappings.
///
/// `logo_override` takes precedence over `project.logo`; if neither is
/// set the `logo_url` field will be an empty string.
pub fn generate_manifest(
    project: &ProjectInfo,
    server_url: &str,
    logo_override: Option<&str>,
) -> Result<String> {
    let logo_url = logo_override
        .map(|s| s.to_string())
        .or_else(|| project.logo.clone())
        .unwrap_or_default();

    let name_for_model = project.name.replace(['-', ' '], "_");

    let base_url = server_url.trim_end_matches('/');

    let widget_mappings: Vec<serde_json::Value> = project
        .widgets
        .iter()
        .map(|w| {
            json!({
                "tool": w.name,
                "resource_uri": w.uri
            })
        })
        .collect();

    let manifest = json!({
        "schema_version": "v1",
        "name_for_human": project.name,
        "name_for_model": name_for_model,
        "description_for_human": project.description,
        "description_for_model": project.description,
        "auth": { "type": "none" },
        "api": {
            "type": "openapi",
            "url": format!("{}/openapi.json", base_url)
        },
        "logo_url": logo_url,
        "contact_email": "",
        "legal_info_url": "",
        "mcp_apps": {
            "server_url": base_url,
            "widgets": widget_mappings
        }
    });

    serde_json::to_string_pretty(&manifest).context("Failed to serialize manifest JSON")
}

/// Write manifest content to a file in the given output directory.
///
/// Creates the output directory if it does not exist, writes
/// `manifest.json`, and prints a success message to stdout.
pub fn write_manifest(output_dir: &str, content: &str) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir))?;

    let path = std::path::Path::new(output_dir).join("manifest.json");
    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))?;

    println!("  {} Generated {}/manifest.json", "ok".green(), output_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publishing::detect::{ProjectInfo, WidgetInfo};

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            name: "my-chess-app".to_string(),
            description: "A chess game".to_string(),
            logo: Some("https://example.com/chess.png".to_string()),
            widgets: vec![
                WidgetInfo {
                    name: "board".to_string(),
                    uri: "ui://app/board".to_string(),
                    html: "<h1>Board</h1>".to_string(),
                },
                WidgetInfo {
                    name: "timer".to_string(),
                    uri: "ui://app/timer".to_string(),
                    html: "<h1>Timer</h1>".to_string(),
                },
            ],
        }
    }

    #[test]
    fn test_manifest_schema_version() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["schema_version"], "v1");
    }

    #[test]
    fn test_manifest_names() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["name_for_human"], "my-chess-app");
        assert_eq!(parsed["name_for_model"], "my_chess_app");
    }

    #[test]
    fn test_manifest_name_for_model_sanitization() {
        let mut project = sample_project();
        project.name = "my chess-app v2".to_string();

        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["name_for_model"], "my_chess_app_v2");
    }

    #[test]
    fn test_manifest_descriptions() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["description_for_human"], "A chess game");
        assert_eq!(parsed["description_for_model"], "A chess game");
    }

    #[test]
    fn test_manifest_auth_none() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["auth"]["type"], "none");
    }

    #[test]
    fn test_manifest_api_url() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["api"]["type"], "openapi");
        assert_eq!(parsed["api"]["url"], "https://example.com/openapi.json");
    }

    #[test]
    fn test_manifest_server_url_trailing_slash_trimmed() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com/", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["api"]["url"], "https://example.com/openapi.json");
        assert_eq!(parsed["mcp_apps"]["server_url"], "https://example.com");
    }

    #[test]
    fn test_manifest_logo_from_project() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["logo_url"], "https://example.com/chess.png");
    }

    #[test]
    fn test_manifest_logo_override() {
        let project = sample_project();
        let json_str = generate_manifest(
            &project,
            "https://example.com",
            Some("https://cdn.example.com/override.png"),
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["logo_url"], "https://cdn.example.com/override.png");
    }

    #[test]
    fn test_manifest_logo_empty_when_none() {
        let mut project = sample_project();
        project.logo = None;

        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["logo_url"], "");
    }

    #[test]
    fn test_manifest_widget_mappings() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let widgets = parsed["mcp_apps"]["widgets"].as_array().unwrap();
        assert_eq!(widgets.len(), 2);

        assert_eq!(widgets[0]["tool"], "board");
        assert_eq!(widgets[0]["resource_uri"], "ui://app/board");
        assert_eq!(widgets[1]["tool"], "timer");
        assert_eq!(widgets[1]["resource_uri"], "ui://app/timer");
    }

    #[test]
    fn test_manifest_mcp_apps_server_url() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["mcp_apps"]["server_url"], "https://example.com");
    }

    #[test]
    fn test_manifest_empty_fields() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["contact_email"], "");
        assert_eq!(parsed["legal_info_url"], "");
    }

    #[test]
    fn test_manifest_is_valid_json() {
        let project = sample_project();
        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();

        // Verify it round-trips through serde_json
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let re_serialized = serde_json::to_string_pretty(&parsed).unwrap();
        assert_eq!(json_str, re_serialized);
    }

    #[test]
    fn test_write_manifest_creates_directory_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("dist");
        let output_str = output.to_str().unwrap();

        write_manifest(output_str, r#"{"test": true}"#).unwrap();

        let written = fs::read_to_string(output.join("manifest.json")).unwrap();
        assert_eq!(written, r#"{"test": true}"#);
    }

    #[test]
    fn test_write_manifest_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("dist");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("manifest.json"), "old content").unwrap();

        write_manifest(output.to_str().unwrap(), "new content").unwrap();

        let written = fs::read_to_string(output.join("manifest.json")).unwrap();
        assert_eq!(written, "new content");
    }

    #[test]
    fn test_manifest_no_widgets() {
        let project = ProjectInfo {
            name: "empty-app".to_string(),
            description: "No widgets".to_string(),
            logo: None,
            widgets: vec![],
        };

        let json_str = generate_manifest(&project, "https://example.com", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        let widgets = parsed["mcp_apps"]["widgets"].as_array().unwrap();
        assert!(widgets.is_empty());
    }
}
