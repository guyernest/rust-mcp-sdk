# Prompts

Prompts provide guided workflow templates that MCP clients can present to users.
They return structured messages that seed conversations with relevant context.

## PromptHandler Trait

```rust
use pmcp::server::PromptHandler;
use pmcp::types::{GetPromptResult, PromptMessage, PromptInfo, PromptArgument, Role, Content};
use pmcp::RequestHandlerExtra;
use async_trait::async_trait;
use std::collections::HashMap;

struct GreetPrompt;

#[async_trait]
impl PromptHandler for GreetPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        let name = args.get("name").cloned().unwrap_or_else(|| "World".into());
        Ok(GetPromptResult {
            description: Some("A greeting prompt".into()),
            messages: vec![PromptMessage {
                role: Role::Assistant,
                content: Content::Text { text: format!("Hello, {name}!") },
            }],
            meta: None,
        })
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(PromptInfo {
            name: "greet".into(),
            description: Some("Generate a greeting".into()),
            arguments: Some(vec![PromptArgument {
                name: "name".into(),
                description: Some("Name to greet".into()),
                required: false,
                completion: None,
                arg_type: None,
            }]),
        })
    }
}
```

Register with the server builder:

```rust
server_builder.prompt(GreetPrompt);
```

## SimplePrompt Helper

For simpler cases without a dedicated struct:

```rust
use pmcp::server::SimplePrompt;

let prompt = SimplePrompt::new("quickstart", |args, _extra| {
    Box::pin(async move {
        Ok(GetPromptResult {
            description: Some("Quickstart guide".into()),
            messages: vec![PromptMessage {
                role: Role::Assistant,
                content: Content::Text { text: "Step 1: ...".into() },
            }],
            meta: None,
        })
    })
})
.with_description("Get started with PMCP")
.with_argument("template", "Template to use", false);
```

## Prompt Design Principles

- Return actionable, concise messages (20-50 lines)
- Use User role for context/question, Assistant role for guidance
- Include code snippets with correct imports
- Mark required arguments in metadata
- Prompts are conversation starters, not full documentation
