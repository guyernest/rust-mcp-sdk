# Chapter 7: Prompts & User Workflows

Prompts are one of the most powerful features of the Model Context Protocol. While **tools** let LLMs perform actions and **resources** provide reference data, **prompts** are _user-triggered workflows_ that orchestrate tools and resources to accomplish complex tasks.

Think of prompts as interactive templates that users can invoke to quickly execute common workflows with minimal input.

## What Are Prompts?

Prompts in MCP serve three key purposes:

1. **User-Triggered Workflows**: Pre-defined sequences of operations that users can invoke by name
2. **Workflow Hints**: Show users what your MCP server can do at a glance
3. **LLM Orchestration**: Provide instructions to the LLM on how to use your tools and resources together

### The Three MCP Primitives

Understanding the relationship between the three core MCP primitives is crucial:

| Primitive | Purpose | Who Triggers | Example |
|-----------|---------|--------------|---------|
| **Tools** | Actions the LLM can perform | LLM decides | `add_todo()`, `search_pages()` |
| **Resources** | Reference data for context | LLM or user requests | Documentation, configuration files |
| **Prompts** | Complete workflows | **User triggers** | "Add TODO to project", "Write blog post" |

### Prompt Anatomy

A prompt consists of:

```rust
pub struct GetPromptResult {
    pub description: Option<String>,     // What this prompt does
    pub messages: Vec<PromptMessage>,    // The conversation template
}

pub struct PromptMessage {
    pub role: Role,                      // User, Assistant, or System
    pub content: MessageContent,         // Text, Image, or Resource reference
}
```

**Message roles:**
- `Role::System` - Instructions to the LLM (how to execute the workflow)
- `Role::Assistant` - Contextual information or resources
- `Role::User` - The user's request (often with argument placeholders)

## Simple Prompts

Let's start with a basic example - a code review prompt.

### Example 1: Code Review Assistant

```rust
use pmcp::{SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

fn create_code_review_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("code_review", |args| {
        let language = args.get("language")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let code = args.get("code")
            .ok_or_else(|| pmcp::Error::validation("code argument is required"))?;
        let focus = args.get("focus")
            .map(|s| s.as_str())
            .unwrap_or("general");

        let mut messages = vec![];

        // System message: Instructions to the LLM
        messages.push(PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: format!(
                    "You are an expert {} code reviewer. Focus on {} aspects. \
                     Provide constructive feedback with specific suggestions.",
                    language, focus
                ),
            },
        });

        // User message: The actual request
        messages.push(PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!(
                    "Please review this {} code:\n\n```{}\n{}\n```",
                    language, language, code
                ),
            },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!(
                "Code review for {} code focusing on {}",
                language, focus
            )),
        })
    })
    .with_description("Generate a code review prompt for the provided code")
    .with_argument("language", "Programming language of the code", false)
    .with_argument("code", "The code to review", true)
    .with_argument("focus", "Aspect to focus on (performance, security, style)", false)
}
```

### Registering Prompts

Add prompts to your server:

```rust
use pmcp::Server;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let code_review = create_code_review_prompt();

    let server = Server::builder()
        .name("dev-assistant")
        .version("1.0.0")
        .prompt("code-review", code_review)
        .build()?;

    server.run_stdio().await
}
```

When a user invokes this prompt, it returns a `GetPromptResult` containing a structured conversation:

```json
{
  "description": "Code review for rust code focusing on performance",
  "messages": [
    {
      "role": "system",
      "content": {
        "type": "text",
        "text": "You are an expert rust code reviewer. Focus on performance aspects. Provide constructive feedback with specific suggestions."
      }
    },
    {
      "role": "user",
      "content": {
        "type": "text",
        "text": "Please review this rust code:\n\n```rust\nfn add(a: i32, b: i32) -> i32 { a + b }\n```"
      }
    }
  ]
}
```

This structured output gives the LLM:
- **Context** (System message): Who it is and what to focus on
- **Task** (User message): The specific request with formatted code

The LLM can now provide targeted code review immediately, without the user having to explain the context.

## Advanced Prompts: Workflow Orchestration

The real power of prompts emerges when they orchestrate multiple tools and resources to accomplish complex tasks.

### Example 2: Logseq TODO Workflow

This example shows how to create a sophisticated workflow that:
1. Accepts user input (task, project, date)
2. Provides clear orchestration instructions to the LLM
3. References relevant resources
4. Defines expected output format

```rust
use pmcp::{SimplePrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;

fn create_add_todo_prompt() -> SimplePrompt<
    impl Fn(
        HashMap<String, String>,
        pmcp::RequestHandlerExtra,
    ) -> Pin<Box<dyn Future<Output = pmcp::Result<GetPromptResult>> + Send>> + Send + Sync
> {
    SimplePrompt::new("add_todo_to_project", Box::new(
        |args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra| {
            Box::pin(async move {
                let task_description = args.get("task_description")
                    .ok_or_else(|| pmcp::Error::validation("task_description required"))?;
                let project_name = args.get("project_name")
                    .ok_or_else(|| pmcp::Error::validation("project_name required"))?;
                let date = args.get("date").map(|s| s.as_str()).unwrap_or("today");

                let mut messages = vec![];

                // System message: Workflow orchestration instructions
                messages.push(PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Text {
                        text: format!(r#"You are executing a Logseq task creation workflow.

GOAL
Add a TODO item to the journal page for the given date (default: today), tagged to a project page.

TOOLS AVAILABLE
- add-content(date, content): Append a block to a page

INPUTS
- task_description: "{}"
- project_name: "{}"
- date: "{}"

PROCEDURE
1) Normalize project name:
   - Trim whitespace
   - Remove any surrounding [[ ]] if present
   - Final tag format must be [[<project_name>]]

2) Build content exactly as:
   TODO {{task_description}} [[{{project_name}}]]
   - Keep order: TODO keyword, description, single project tag

3) Call add-content with:
   - date = provided date or "today"
   - content = the string above (single line)

4) If tool fails, return structured error (see schema below)

5) On success, return strictly valid JSON response

OUTPUT JSON SCHEMA (no extra fields):
{{
  "status": "ok" | "error",
  "entry_id": "string|null",
  "page": "string|null",
  "block_ref": "string|null",
  "content_preview": "string|null",
  "message": "string|null"
}}

ERROR HANDLING
- On any tool error or invalid inputs (e.g., empty task), set status="error"
  and message with concise reason. Other fields null.
- Do not ask follow-up questions; fail fast with useful message.
"#, task_description, project_name, date),
                    },
                });

                // Assistant message: Reference relevant resources
                messages.push(PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Resource {
                        uri: "resource://logseq/how-to-todo".to_string(),
                        text: None,
                        mime_type: Some("text/markdown".to_string()),
                    },
                });

                messages.push(PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Resource {
                        uri: "resource://logseq/how-to-project-tag".to_string(),
                        text: None,
                        mime_type: Some("text/markdown".to_string()),
                    },
                });

                // User message: The actual request
                messages.push(PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!(
                            "Add a TODO: {} to project: {} on {}",
                            task_description, project_name, date
                        ),
                    },
                });

                Ok(GetPromptResult {
                    messages,
                    description: Some(format!(
                        "Create TODO '{}' in project '{}'",
                        task_description, project_name
                    )),
                })
            }) as Pin<Box<dyn Future<Output = pmcp::Result<GetPromptResult>> + Send>>
        }
    ))
    .with_description("Add a TODO to a Logseq project")
    .with_argument("task_description", "What needs doing", true)
    .with_argument("project_name", "Project/page name (without brackets)", true)
    .with_argument("date", "Defaults to today", false)
}
```

### Why This Prompt Design Works

1. **Clear Orchestration**: The system message provides step-by-step instructions
2. **Tool References**: Explicitly names available tools and their signatures
3. **Expected Output**: Defines exact JSON schema for structured responses
4. **Resource Integration**: References documentation resources for context
5. **Error Handling**: Specifies how to handle failures gracefully

## Prompt Message Types

Prompts can include three types of content:

### Text Content

```rust
MessageContent::Text {
    text: "Your text here".to_string(),
}
```

Use for: Instructions, user requests, explanations

### Image Content

```rust
MessageContent::Image {
    data: base64_encoded_image,
    mime_type: "image/png".to_string(),
}
```

Use for: Visual references, diagrams, screenshots

### Resource References

```rust
MessageContent::Resource {
    uri: "resource://app/documentation".to_string(),
    text: None,
    mime_type: Some("text/markdown".to_string()),
}
```

Use for: Documentation, configuration files, templates

**Resource references are powerful** because they let the LLM fetch relevant context on-demand without bloating the initial prompt.

## Best Practices

### 1. Design User-Centric Workflows

Think about common tasks your users perform repeatedly:

```rust
// ❌ Bad: Just wrapping a single tool
"calculate_sum" prompt -> calls sum() tool

// ✅ Good: Multi-step workflow
"monthly_report" prompt ->
  1. Fetch data for date range
  2. Calculate statistics
  3. Generate summary
  4. Format as markdown
```

### 2. Provide Clear Orchestration

Tell the LLM **exactly** how to execute the workflow:

```rust
messages.push(PromptMessage {
    role: Role::System,
    content: MessageContent::Text {
        text: r#"
WORKFLOW STEPS
1. Call get_data(start_date, end_date)
2. For each record, call calculate_metrics(record)
3. Aggregate results using aggregate_stats(metrics)
4. Call generate_report(stats) to create final output

ERROR HANDLING
- If get_data fails, return error immediately
- If calculate_metrics fails for any record, log and skip that record
- Minimum 10 records required, otherwise return validation error
        "#.to_string(),
    },
});
```

### 3. Define Output Schemas

For structured workflows, specify exact output format:

```rust
let schema = r#"
OUTPUT JSON SCHEMA:
{
  "status": "success" | "error",
  "data": {
    "records_processed": number,
    "total_amount": number,
    "average": number
  },
  "errors": string[]  // Empty array if no errors
}
"#;
```

### 4. Use Argument Defaults Wisely

```rust
.with_argument("date", "Defaults to today", false)  // Optional with default
.with_argument("project", "Project name", true)     // Required
```

Optional arguments make prompts more user-friendly for common cases.

### 5. Reference Resources for Context

Instead of embedding large documentation in prompts:

```rust
// ❌ Bad: 5000 lines of API docs in prompt text
messages.push(PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Text {
        text: huge_api_documentation,
    },
});

// ✅ Good: Reference resource
messages.push(PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Resource {
        uri: "resource://api/documentation".to_string(),
        text: None,
        mime_type: Some("text/markdown".to_string()),
    },
});
```

The LLM can fetch the resource only if needed.

## Complete Example: Blog Post Generator

Here's a complete example showing all concepts together:

```rust
use pmcp::{Server, SyncPrompt, types::*};
use std::collections::HashMap;

fn create_blog_post_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("generate_blog_post", |args| {
        let topic = args.get("topic")
            .ok_or_else(|| pmcp::Error::validation("topic is required"))?;
        let style = args.get("style").map(|s| s.as_str()).unwrap_or("professional");
        let length = args.get("length").map(|s| s.as_str()).unwrap_or("medium");

        let mut messages = vec![];

        // System: Workflow instructions
        messages.push(PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: format!(r#"You are a blog post generator.

WORKFLOW
1. Call search_resources(topic="{}") to find relevant background info
2. Review returned resources and extract key points
3. Call generate_outline(topic, key_points) to create structure
4. Call write_section(outline, section_name) for each section
5. Call format_markdown(sections) to create final post

STYLE: {}
LENGTH: {} (short=500w, medium=1000w, long=2000w)

OUTPUT
Return a complete markdown blog post with:
- Title (# heading)
- Introduction
- Main sections (## headings)
- Conclusion
- Optional: Code examples if technical topic
                "#, topic, style, length),
            },
        });

        // Assistant: Reference style guide
        messages.push(PromptMessage {
            role: Role::Assistant,
            content: MessageContent::Resource {
                uri: format!("resource://blog/style-guide/{}", style),
                text: None,
                mime_type: Some("text/markdown".to_string()),
            },
        });

        // User: The request
        messages.push(PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!("Write a {} {} blog post about: {}", length, style, topic),
            },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!("Generate {} blog post about {}", style, topic)),
        })
    })
    .with_description("Generate a complete blog post on any topic")
    .with_argument("topic", "The topic to write about", true)
    .with_argument("style", "Writing style: professional, casual, technical", false)
    .with_argument("length", "Post length: short, medium, long", false)
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let blog_prompt = create_blog_post_prompt();

    let server = Server::builder()
        .name("content-server")
        .version("1.0.0")
        .prompt("blog-post", blog_prompt)
        // Add the actual tools this prompt orchestrates:
        // .tool("search_resources", /* ... */)
        // .tool("generate_outline", /* ... */)
        // .tool("write_section", /* ... */)
        // .tool("format_markdown", /* ... */)
        .build()?;

    server.run_stdio().await
}
```

## AsyncPrompt vs SyncPrompt

Choose based on your handler's needs:

### SyncPrompt

For simple, CPU-bound prompt generation:

```rust
SyncPrompt::new("simple", |args| {
    // Synchronous logic only
    let result = format_template(args);
    Ok(GetPromptResult { messages: result, description: None })
})
```

### SimplePrompt (Async)

For prompts that need async operations (database, API calls):

```rust
SimplePrompt::new("complex", Box::new(
    |args, extra| Box::pin(async move {
        // Can await async operations
        let data = fetch_from_db(&args["id"]).await?;
        let template = generate_messages(data).await?;
        Ok(GetPromptResult { messages: template, description: None })
    })
))
```

## Listing Prompts

Users discover your prompts via the `prompts/list` method:

```json
{
  "method": "prompts/list"
}
```

Response:
```json
{
  "prompts": [
    {
      "name": "code-review",
      "description": "Generate a code review prompt",
      "arguments": [
        {"name": "language", "description": "Programming language", "required": false},
        {"name": "code", "description": "Code to review", "required": true}
      ]
    },
    {
      "name": "add-todo",
      "description": "Add TODO to Logseq project",
      "arguments": [
        {"name": "task_description", "description": "What needs doing", "required": true},
        {"name": "project_name", "description": "Project name", "required": true},
        {"name": "date", "description": "Defaults to today", "required": false}
      ]
    }
  ]
}
```

## When to Use Prompts

Use prompts when:

✅ **Users need quick access to common workflows**
- "Generate weekly report"
- "Create pull request description"
- "Add TODO to project"

✅ **Multiple tools must be orchestrated in a specific order**
- Data analysis pipelines
- Content generation workflows
- Complex queries

✅ **You want to guide LLM behavior for specific tasks**
- "Review code focusing on security"
- "Summarize in executive style"
- "Generate test cases for function"

Don't use prompts when:

❌ **It's just a single tool call**
- Use tools directly instead

❌ **The workflow is user-specific and can't be templated**
- Let the LLM figure it out from available tools

❌ **The task changes based on dynamic conditions**
- Use tools with conditional logic instead

## Testing Prompts

Test your prompts programmatically:

```rust
#[tokio::test]
async fn test_code_review_prompt() {
    let prompt = create_code_review_prompt();

    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert("code".to_string(), "fn test() {}".to_string());
    args.insert("focus".to_string(), "style".to_string());

    let result = prompt.handle(args, RequestHandlerExtra::default()).await.unwrap();

    assert_eq!(result.messages.len(), 2);
    assert!(matches!(result.messages[0].role, Role::System));
    assert!(matches!(result.messages[1].role, Role::User));
}
```

## Summary

Prompts are the user-facing workflows of your MCP server. They:

- **Provide quick access** to common tasks users perform repeatedly
- **Orchestrate multiple tools** to accomplish complex goals
- **Guide LLM behavior** with clear instructions and context
- **Reference resources** for additional context
- **Define structured outputs** for predictable results

**Key Design Principles:**
1. Think user workflows, not just tool wrappers
2. Provide clear orchestration instructions
3. Define expected outputs
4. Use resource references for large context
5. Make common cases easy with optional arguments

In the next chapter, we'll explore error handling and recovery strategies to make your prompts and tools robust in production.
