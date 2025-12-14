//! Internal types for building prompts
//!
//! These types support both loose (text-only) and strict (handle-based) prompt construction.
//! They are converted to protocol types at the edge.

use super::handles::{ResourceHandle, ToolHandle};
use crate::types::Role;
use smallvec::SmallVec;

/// Internal representation of prompt content
/// Supports both loose (text-only) and strict (handle-based) construction
#[derive(Clone, Debug)]
#[non_exhaustive] // Can add variants without breaking changes
pub enum PromptContent {
    /// Plain text (loose mode - easy migration)
    Text(String),

    /// Image data
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },

    /// Resource URI as string (loose mode)
    ResourceUri(String),

    /// Tool handle (strict mode - type-safe)
    ToolHandle(ToolHandle),

    /// Resource handle (strict mode - type-safe)
    ResourceHandle(ResourceHandle),

    /// Multiple content parts
    /// `SmallVec` optimized for 2-4 parts (common case)
    Multi(SmallVec<[Box<Self>; 3]>),
}

/// Internal representation of a prompt message
#[derive(Clone, Debug)]
pub struct InternalPromptMessage {
    /// The role of the message sender
    pub role: Role,
    /// The message content
    pub content: PromptContent,
}

impl InternalPromptMessage {
    /// Create a new prompt message
    pub fn new(role: Role, content: impl Into<PromptContent>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Create a text message with the given role
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: PromptContent::Text(text.into()),
        }
    }

    /// Create a system message
    pub fn system(text: impl Into<String>) -> Self {
        Self::text(Role::System, text)
    }

    /// Create a user message
    pub fn user(text: impl Into<String>) -> Self {
        Self::text(Role::User, text)
    }

    /// Create an assistant message
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::text(Role::Assistant, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_content_text() {
        let content = PromptContent::Text("Hello".to_string());
        assert!(matches!(content, PromptContent::Text(_)));
    }

    #[test]
    fn test_prompt_content_image() {
        let content = PromptContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        assert!(matches!(content, PromptContent::Image { .. }));
    }

    #[test]
    fn test_prompt_content_tool_handle() {
        let handle = ToolHandle::new("greet");
        let content = PromptContent::ToolHandle(handle);
        assert!(matches!(content, PromptContent::ToolHandle(_)));
    }

    #[test]
    fn test_internal_prompt_message_text() {
        let msg = InternalPromptMessage::text(Role::User, "Hello");
        assert!(matches!(msg.role, Role::User));
        assert!(matches!(msg.content, PromptContent::Text(_)));
    }

    #[test]
    fn test_internal_prompt_message_helpers() {
        let system_msg = InternalPromptMessage::system("System prompt");
        assert!(matches!(system_msg.role, Role::System));

        let user_msg = InternalPromptMessage::user("User message");
        assert!(matches!(user_msg.role, Role::User));

        let assistant_msg = InternalPromptMessage::assistant("Assistant message");
        assert!(matches!(assistant_msg.role, Role::Assistant));
    }

    #[test]
    fn test_prompt_content_multi_smallvec() {
        let parts = smallvec::smallvec![
            Box::new(PromptContent::Text("Part 1".to_string())),
            Box::new(PromptContent::Text("Part 2".to_string())),
        ];
        let content = PromptContent::Multi(parts);

        if let PromptContent::Multi(parts) = content {
            assert_eq!(parts.len(), 2);
        } else {
            panic!("Expected Multi variant");
        }
    }
}
