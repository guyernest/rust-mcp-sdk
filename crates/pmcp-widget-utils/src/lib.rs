//! Shared widget utilities for PMCP SDK.
//!
//! Provides common HTML manipulation functions used by both the core `pmcp` crate
//! and the `mcp-preview` crate, eliminating code duplication.

/// Inject a bridge script tag into widget HTML.
///
/// Inserts `<script type="module" src="{bridge_url}"></script>` at the best
/// location in the HTML document:
///
/// 1. Before `</head>` if present
/// 2. After `<body>` opening tag if present
/// 3. Prepended to the document otherwise
///
/// # Example
///
/// ```
/// use pmcp_widget_utils::inject_bridge_script;
///
/// let html = r#"<html><head><title>Widget</title></head><body>Hello</body></html>"#;
/// let result = inject_bridge_script(html, "/assets/widget-runtime.mjs");
/// assert!(result.contains(r#"<script type="module" src="/assets/widget-runtime.mjs"></script>"#));
/// ```
pub fn inject_bridge_script(html: &str, bridge_url: &str) -> String {
    let script_tag = format!(
        r#"<script type="module" src="{}"></script>"#,
        bridge_url
    );

    if let Some(pos) = html.find("</head>") {
        // Insert before </head>
        let mut result = String::with_capacity(html.len() + script_tag.len() + 1);
        result.push_str(&html[..pos]);
        result.push_str(&script_tag);
        result.push('\n');
        result.push_str(&html[pos..]);
        result
    } else if let Some(pos) = html.find("<body") {
        // Find the closing '>' of the <body> tag
        if let Some(close) = html[pos..].find('>') {
            let insert_at = pos + close + 1;
            let mut result = String::with_capacity(html.len() + script_tag.len() + 1);
            result.push_str(&html[..insert_at]);
            result.push('\n');
            result.push_str(&script_tag);
            result.push_str(&html[insert_at..]);
            result
        } else {
            format!("{}\n{}", script_tag, html)
        }
    } else {
        // No </head> or <body> — prepend
        format!("{}\n{}", script_tag, html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_before_head_close() {
        let html = "<html><head><title>T</title></head><body></body></html>";
        let result = inject_bridge_script(html, "/bridge.mjs");
        assert!(result.contains(r#"<script type="module" src="/bridge.mjs"></script>"#));
        assert!(result.find("bridge.mjs").unwrap() < result.find("</head>").unwrap());
    }

    #[test]
    fn injects_after_body_open_when_no_head() {
        let html = "<body><div>Content</div></body>";
        let result = inject_bridge_script(html, "/bridge.mjs");
        assert!(result.contains(r#"<script type="module" src="/bridge.mjs"></script>"#));
        assert!(result.find("bridge.mjs").unwrap() > result.find("<body>").unwrap());
    }

    #[test]
    fn prepends_when_no_head_or_body() {
        let html = "<div>Simple widget</div>";
        let result = inject_bridge_script(html, "/bridge.mjs");
        assert!(result.starts_with(r#"<script type="module" src="/bridge.mjs"></script>"#));
    }
}
