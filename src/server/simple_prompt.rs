//! Simple prompt implementations with metadata support.

use crate::types::{GetPromptResult, PromptArgument, PromptInfo};
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use super::cancellation::RequestHandlerExtra;
use super::PromptHandler;

/// A simple prompt implementation with metadata support.
pub struct SimplePrompt<F>
where
    F: Fn(
            HashMap<String, String>,
            RequestHandlerExtra,
        ) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    name: String,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
    handler: F,
}

impl<F> fmt::Debug for SimplePrompt<F>
where
    F: Fn(
            HashMap<String, String>,
            RequestHandlerExtra,
        ) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimplePrompt")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("arguments", &self.arguments)
            .finish()
    }
}

impl<F> SimplePrompt<F>
where
    F: Fn(
            HashMap<String, String>,
            RequestHandlerExtra,
        ) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    /// Create a new prompt with a name and handler.
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            arguments: Vec::new(),
            handler,
        }
    }

    /// Set the description for this prompt.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an argument to this prompt.
    pub fn with_argument(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.arguments.push(PromptArgument {
            name: name.into(),
            description: Some(description.into()),
            required,
            completion: None,
            arg_type: None,
        });
        self
    }

    /// Set all arguments at once.
    pub fn with_arguments(mut self, arguments: Vec<PromptArgument>) -> Self {
        self.arguments = arguments;
        self
    }
}

#[async_trait]
impl<F> PromptHandler for SimplePrompt<F>
where
    F: Fn(
            HashMap<String, String>,
            RequestHandlerExtra,
        ) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
{
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(crate::Error::validation(format!(
                    "Required argument '{}' is missing",
                    arg.name
                )));
            }
        }

        (self.handler)(args, extra).await
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments: if self.arguments.is_empty() {
                None
            } else {
                Some(self.arguments.clone())
            },
        })
    }
}

/// A simpler prompt for synchronous handlers.
pub struct SyncPrompt<F>
where
    F: Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync,
{
    name: String,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
    handler: F,
}

impl<F> fmt::Debug for SyncPrompt<F>
where
    F: Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncPrompt")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("arguments", &self.arguments)
            .finish()
    }
}

impl<F> SyncPrompt<F>
where
    F: Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync,
{
    /// Create a new synchronous prompt with a name and handler.
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: None,
            arguments: Vec::new(),
            handler,
        }
    }

    /// Set the description for this prompt.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an argument to this prompt.
    pub fn with_argument(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.arguments.push(PromptArgument {
            name: name.into(),
            description: Some(description.into()),
            required,
            completion: None,
            arg_type: None,
        });
        self
    }

    /// Set all arguments at once.
    pub fn with_arguments(mut self, arguments: Vec<PromptArgument>) -> Self {
        self.arguments = arguments;
        self
    }
}

#[async_trait]
impl<F> PromptHandler for SyncPrompt<F>
where
    F: Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync,
{
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Validate required arguments
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(crate::Error::validation(format!(
                    "Required argument '{}' is missing",
                    arg.name
                )));
            }
        }

        (self.handler)(args)
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments: if self.arguments.is_empty() {
                None
            } else {
                Some(self.arguments.clone())
            },
        })
    }
}
