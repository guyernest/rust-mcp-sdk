//! Code Mode instruction and policy templates.
//!
//! This module provides a simple template engine for generating Code Mode
//! `code-mode://instructions` and `code-mode://policies` resources from
//! compiled-in templates. Templates use `{{key}}` substitution markers.
//!
//! ## Architecture
//!
//! Each server type crate embeds its own `.md` template files via `include_str!()`.
//! The shared template engine here provides:
//!
//! - **`render()`** — Simple `{{key}}` → value substitution
//! - **`TemplateContext`** — Builder for template variables
//! - **URI constants** — Standard resource URIs for auto-generated resources
//!
//! ## Override Mechanism
//!
//! If an admin defines a resource with the same URI in the TOML config,
//! the admin's version takes precedence. The server startup code should
//! check for existing resources before registering auto-generated ones.

use std::collections::HashMap;

/// Standard URI for auto-generated instructions resource.
pub const INSTRUCTIONS_URI: &str = "code-mode://instructions";

/// Standard URI for auto-generated policies resource.
pub const POLICIES_URI: &str = "code-mode://policies";

/// Template context holding key-value pairs for substitution.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    vars: HashMap<String, String>,
}

impl TemplateContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable.
    pub fn set(mut self, key: &str, value: impl Into<String>) -> Self {
        self.vars.insert(key.to_string(), value.into());
        self
    }

    /// Set a boolean variable (renders as "true"/"false").
    pub fn set_bool(self, key: &str, value: bool) -> Self {
        self.set(key, if value { "true" } else { "false" })
    }

    /// Set a numeric variable.
    pub fn set_num(self, key: &str, value: impl std::fmt::Display) -> Self {
        self.set(key, value.to_string())
    }

    /// Render a template string, replacing `{{key}}` with values.
    ///
    /// Unknown keys are left as-is (not replaced). This allows templates
    /// to contain literal `{{` in code examples by using keys that don't
    /// match any variable.
    pub fn render(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.vars {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

/// Render a template with the given context.
///
/// Convenience function equivalent to `ctx.render(template)`.
pub fn render(template: &str, ctx: &TemplateContext) -> String {
    ctx.render(template)
}

/// Conditionally include a section based on a boolean.
///
/// Returns the section content if `condition` is true, otherwise empty string.
/// Useful for building template content with optional sections.
pub fn conditional(condition: bool, content: &str) -> String {
    if condition {
        content.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_render() {
        let ctx = TemplateContext::new()
            .set("name", "IMDB")
            .set_num("timeout", 300);

        let result = ctx.render("Server: {{name}}, Timeout: {{timeout}}s");
        assert_eq!(result, "Server: IMDB, Timeout: 300s");
    }

    #[test]
    fn test_unknown_keys_preserved() {
        let ctx = TemplateContext::new().set("name", "test");
        let result = ctx.render("{{name}} and {{unknown}}");
        assert_eq!(result, "test and {{unknown}}");
    }

    #[test]
    fn test_conditional() {
        assert_eq!(conditional(true, "hello"), "hello");
        assert_eq!(conditional(false, "hello"), "");
    }

    #[test]
    fn test_code_examples_not_broken() {
        // Ensure that code examples with JS template literals aren't affected
        let ctx = TemplateContext::new().set("server_name", "Test");
        let template = r#"{{server_name}} API

```javascript
const path = `/users/${userId}`;
```"#;
        let result = ctx.render(template);
        assert!(result.contains("Test API"));
        assert!(result.contains("${userId}"));
    }
}
