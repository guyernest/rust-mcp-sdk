//! Conversion trait for ergonomic prompt content construction

use super::{
    handles::{ResourceHandle, ToolHandle},
    prompt_content::PromptContent,
};

/// Convert various types into `PromptContent` (internal type)
/// This is the developer-facing API
pub trait IntoPromptContent {
    /// Convert self into `PromptContent`
    fn into_prompt_content(self) -> PromptContent;
}

// Implement for handles (strict mode)
impl IntoPromptContent for ToolHandle {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::ToolHandle(self)
    }
}

impl IntoPromptContent for ResourceHandle {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::ResourceHandle(self)
    }
}

// Implement for strings (loose mode)
impl IntoPromptContent for String {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::Text(self)
    }
}

impl IntoPromptContent for &str {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::Text(self.to_string())
    }
}

// Implement for images (tuple of data and mime_type)
impl IntoPromptContent for (String, String) {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::Image {
            data: self.0,
            mime_type: self.1,
        }
    }
}

// Blanket From implementation for ergonomics
impl<T: IntoPromptContent> From<T> for PromptContent {
    fn from(value: T) -> Self {
        value.into_prompt_content()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_prompt_content_string() {
        let content: PromptContent = "Hello".into();
        assert!(matches!(content, PromptContent::Text(_)));
    }

    #[test]
    fn test_into_prompt_content_tool_handle() {
        let handle = ToolHandle::new("greet");
        let content: PromptContent = handle.into();
        assert!(matches!(content, PromptContent::ToolHandle(_)));
    }

    #[test]
    fn test_into_prompt_content_resource_handle() {
        let handle = ResourceHandle::new("resource://test/path").unwrap();
        let content: PromptContent = handle.into();
        assert!(matches!(content, PromptContent::ResourceHandle(_)));
    }

    #[test]
    fn test_into_prompt_content_image() {
        let image = ("base64data".to_string(), "image/png".to_string());
        let content: PromptContent = image.into();
        assert!(matches!(content, PromptContent::Image { .. }));
    }

    #[test]
    fn test_trait_usage() {
        fn takes_content(content: impl IntoPromptContent) -> PromptContent {
            content.into_prompt_content()
        }

        let text_content = takes_content("text");
        assert!(matches!(text_content, PromptContent::Text(_)));

        let tool_content = takes_content(ToolHandle::new("greet"));
        assert!(matches!(tool_content, PromptContent::ToolHandle(_)));
    }
}
