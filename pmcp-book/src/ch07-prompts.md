# Chapter 7: Prompts — User-Triggered Workflows

Prompts are pre-defined workflows that users explicitly trigger from their MCP client. While **tools** let LLMs perform actions and **resources** provide reference data, **prompts** are _user-controlled workflows_ that orchestrate tools and resources to accomplish complex tasks.

Think of prompts as your MCP server's "quick actions"—common workflows that users can invoke with minimal input.

## Quick Start: Your First Prompt (15 lines)

Let's create a simple code review prompt:

```rust
use pmcp::{Server, SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let code_review = SyncPrompt::new("code-review", |args| {
        let code = args.get("code").and_then(|v| v.as_str())
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;

        Ok(GetPromptResult {
            messages: vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: "You are an expert code reviewer. Provide constructive feedback.".to_string(),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Please review this code:\n\n```\n{}\n```", code),
                    },
                },
            ],
            description: Some("Code review prompt".to_string()),
        })
    })
    .with_description("Generate a code review prompt")
    .with_argument("code", "Code to review", true);

    Server::builder().prompt("code-review", code_review).build()?.run_stdio().await
}
```

**Test it:**
```bash
# Start server
cargo run

# In another terminal:
mcp-tester test stdio --list-prompts
# Shows: code-review

mcp-tester test stdio --get-prompt "code-review" '{"code": "fn main() {}"}'
# Returns structured prompt messages
```

You've created, registered, and tested an MCP prompt! Now let's understand how it works.

## The Prompt Analogy: Website for Agents

Continuing the website analogy from Chapter 4, prompts are your "homepage CTAs" (calls-to-action) for agents.

| Website Element | MCP Prompt | Purpose |
| --- | --- | --- |
| "Get Started" button | Simple prompt | Quick access to common workflow |
| Multi-step wizard | Workflow prompt | Guided multi-tool orchestration |
| Template forms | Prompt with arguments | Pre-filled workflows with user input |
| Help tooltips | Prompt descriptions | Explain what the prompt does |
| Form validation | Argument validation | Ensure user provides correct inputs |

**Key insight**: Prompts are **user-triggered**, not auto-executed by the LLM. They appear in the client UI for users to select explicitly.

## Why Prompts Matter for LLMs

LLMs driving MCP clients benefit from prompts in several ways:

1. **User Intent Clarity**: User selects "Generate weekly report" → LLM knows exact workflow
2. **Tool Orchestration**: Prompt defines sequence (fetch data → calculate → format → save)
3. **Context Pre-loading**: Prompt includes system instructions and resource references
4. **Argument Guidance**: User provides structured inputs (date range, format, recipients)
5. **Consistent Results**: Same prompt + same inputs = predictable workflow execution

**Example workflow:**
```
User action: Selects "Generate weekly report" prompt in Claude Desktop
           ↓
Client calls: prompts/get with arguments {start_date, end_date, format}
           ↓
Server returns: Structured messages with:
  - System instructions (how to generate report)
  - Resource references (templates, previous reports)
  - Tool orchestration (which tools to call in what order)
           ↓
LLM executes: Follows instructions, calls tools, produces report
```

Without prompts, users would need to manually describe the entire workflow every time.

## Prompt Anatomy: Step-by-Step

Every prompt follows this anatomy:
1. **Name + Description** → What the prompt does
2. **Arguments** → User inputs (required vs optional)
3. **Messages** → Structured conversation (System, User, Assistant)
4. **Message Content Types** → Text, Image, or Resource references
5. **Add to Server** → Register and test

Let's build a comprehensive blog post generator following this pattern.

### Step 1: Name + Description

```rust
/// Prompt name: "blog-post"
/// Description: "Generate a complete blog post on any topic.
///              Includes title, introduction, main sections, and conclusion.
///              Supports different writing styles and lengths."
```

**Naming best practices:**
- Use kebab-case: `blog-post`, `weekly-report`, `code-review`
- Descriptive and action-oriented: `generate-blog-post` not `blog`
- User-facing: Users see these names in UI dropdowns

### Step 2: Arguments (with Defaults)

Define required and optional arguments:

```rust
use std::collections::HashMap;

/// Arguments for blog post prompt
///
/// The function receives args as HashMap<String, String>
fn get_arguments(args: &HashMap<String, String>) -> pmcp::Result<(String, String, String)> {
    // Required argument
    let topic = args.get("topic")
        .ok_or_else(|| pmcp::Error::validation(
            "Missing required argument 'topic'. \
             Example: {topic: 'Rust async programming'}"
        ))?;

    // Optional arguments with defaults
    let style = args.get("style")
        .map(|s| s.as_str())
        .unwrap_or("professional");

    let length = args.get("length")
        .map(|s| s.as_str())
        .unwrap_or("medium");

    // Validate style
    if !matches!(style, "professional" | "casual" | "technical") {
        return Err(pmcp::Error::validation(format!(
            "Invalid style '{}'. Must be: professional, casual, or technical",
            style
        )));
    }

    // Validate length
    if !matches!(length, "short" | "medium" | "long") {
        return Err(pmcp::Error::validation(format!(
            "Invalid length '{}'. Must be: short (500w), medium (1000w), or long (2000w)",
            length
        )));
    }

    Ok((topic.to_string(), style.to_string(), length.to_string()))
}
```

**Argument patterns:**
- ✅ **Required**: Must provide (e.g., topic, customer_id)
- ✅ **Optional with defaults**: Fallback if not provided (e.g., style, length)
- ✅ **Validation**: Check values before use
- ✅ **Clear errors**: Tell user exactly what's wrong

### Step 3: Messages (Structured Conversation)

Create a structured conversation with different roles:

```rust
use pmcp::types::{PromptMessage, Role, MessageContent};

fn build_messages(topic: &str, style: &str, length: &str) -> Vec<PromptMessage> {
    vec![
        // System message: Instructions to the LLM
        PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: format!(
                    "You are a professional blog post writer.\n\
                     \n\
                     TASK: Write a {} {} blog post about: {}\n\
                     \n\
                     WORKFLOW:\n\
                     1. Create an engaging title\n\
                     2. Write a compelling introduction\n\
                     3. Develop 3-5 main sections with examples\n\
                     4. Conclude with key takeaways\n\
                     \n\
                     STYLE GUIDE:\n\
                     - Professional: Formal tone, industry terminology\n\
                     - Casual: Conversational, relatable examples\n\
                     - Technical: Deep dives, code examples, references\n\
                     \n\
                     LENGTH TARGETS:\n\
                     - Short: ~500 words (quick overview)\n\
                     - Medium: ~1000 words (balanced coverage)\n\
                     - Long: ~2000 words (comprehensive guide)\n\
                     \n\
                     FORMAT: Use Markdown with proper headings (# ## ###)",
                    length, style, topic
                ),
            },
        },

        // Assistant message: Provide context or resources
        PromptMessage {
            role: Role::Assistant,
            content: MessageContent::Resource {
                uri: format!("resource://blog/style-guide/{}", style),
                text: None,
                mime_type: Some("text/markdown".to_string()),
            },
        },

        // User message: The actual request
        PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!(
                    "Please write a {} {} blog post about: {}",
                    length, style, topic
                ),
            },
        },
    ]
}
```

**Message roles explained:**
- **System**: Instructions for LLM behavior (tone, format, workflow)
- **Assistant**: Context, resources, or examples
- **User**: The user's actual request (with argument placeholders)

**Why this structure works:**
1. **Clear expectations**: System message defines workflow steps
2. **Resource integration**: Assistant provides style guides
3. **User intent**: User message is concise and clear

### Step 4: Message Content Types

PMCP supports three content types for messages:

#### Text Content (Most Common)

```rust
MessageContent::Text {
    text: "Your text here".to_string(),
}
```

Use for: Instructions, user requests, explanations

#### Image Content

```rust
MessageContent::Image {
    data: base64_encoded_image, // Vec<u8> base64-encoded
    mime_type: "image/png".to_string(),
}
```

Use for: Visual references, diagrams, screenshots, design mockups

**Example:**
```rust
let logo_bytes = include_bytes!("../assets/logo.png");
let logo_base64 = base64::encode(logo_bytes);

PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Image {
        data: logo_base64,
        mime_type: "image/png".to_string(),
    },
}
```

#### Resource References

```rust
MessageContent::Resource {
    uri: "resource://app/documentation".to_string(),
    text: None, // Optional inline preview
    mime_type: Some("text/markdown".to_string()),
}
```

Use for: Documentation, configuration files, templates, policies

**Why resource references are powerful:**
- ❌ **Bad**: Embed 5000 lines of API docs in prompt text
- ✅ **Good**: Reference `resource://api/documentation` — LLM fetches only if needed
- **Benefit**: Smaller prompts, on-demand context loading

### Step 5: Complete Prompt Implementation

Putting it all together with `SyncPrompt`:

```rust
use pmcp::{SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

fn create_blog_post_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("blog-post", |args| {
        // Step 1: Parse and validate arguments
        let topic = args.get("topic")
            .ok_or_else(|| pmcp::Error::validation("'topic' required"))?;
        let style = args.get("style").map(|s| s.as_str()).unwrap_or("professional");
        let length = args.get("length").map(|s| s.as_str()).unwrap_or("medium");

        // Validate values
        if !matches!(style, "professional" | "casual" | "technical") {
            return Err(pmcp::Error::validation(format!(
                "Invalid style '{}'. Use: professional, casual, or technical",
                style
            )));
        }

        // Step 2: Build messages
        let messages = vec![
            // System: Workflow instructions
            PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!(
                        "You are a {} blog post writer. Write a {} post about: {}\n\
                         \n\
                         STRUCTURE:\n\
                         1. Title (# heading)\n\
                         2. Introduction (hook + overview)\n\
                         3. Main sections (## headings)\n\
                         4. Conclusion (key takeaways)\n\
                         \n\
                         LENGTH: {} (~{} words)",
                        style,
                        style,
                        topic,
                        length,
                        match length {
                            "short" => "500",
                            "long" => "2000",
                            _ => "1000",
                        }
                    ),
                },
            },

            // Assistant: Style guide resource
            PromptMessage {
                role: Role::Assistant,
                content: MessageContent::Resource {
                    uri: format!("resource://blog/style-guide/{}", style),
                    text: None,
                    mime_type: Some("text/markdown".to_string()),
                },
            },

            // User: The request
            PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Write a {} {} blog post about: {}", length, style, topic),
                },
            },
        ];

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
```

**Key components:**
1. **Closure captures arguments**: `|args| { ... }`
2. **Validation**: Check required fields and values
3. **Message construction**: System → Assistant → User
4. **Metadata**: Description helps users understand the prompt
5. **Argument definitions**: Shown in discovery (`prompts/list`)

### Step 6: Add to Server

```rust
use pmcp::Server;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let blog_prompt = create_blog_post_prompt();

    let server = Server::builder()
        .name("content-server")
        .version("1.0.0")
        .prompt("blog-post", blog_prompt)
        // Add the tools this prompt might use:
        // .tool("search_resources", /* ... */)
        // .tool("generate_outline", /* ... */)
        .build()?;

    // Test with: mcp-tester test stdio --get-prompt "blog-post" '{"topic":"Rust"}'

    server.run_stdio().await
}
```

## Simple Text Prompts: Common Patterns

For most use cases, simple text-based prompts with `SyncPrompt` are sufficient.

### Pattern 1: Code Review Prompt

```rust
use pmcp::{SyncPrompt, types::{GetPromptResult, PromptMessage, Role, MessageContent}};
use std::collections::HashMap;

fn create_code_review_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("code-review", |args| {
        let code = args.get("code")
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;
        let language = args.get("language")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let focus = args.get("focus")
            .map(|s| s.as_str())
            .unwrap_or("general");

        Ok(GetPromptResult {
            messages: vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "You are an expert {} code reviewer. Focus on {} aspects.\n\
                             Provide constructive feedback with specific suggestions.",
                            language, focus
                        ),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Review this {} code:\n\n```{}\n{}\n```", language, language, code),
                    },
                },
            ],
            description: Some(format!("Code review for {} focusing on {}", language, focus)),
        })
    })
    .with_description("Generate a code review prompt")
    .with_argument("code", "Code to review", true)
    .with_argument("language", "Programming language", false)
    .with_argument("focus", "Focus area: performance, security, style", false)
}
```

### Pattern 2: Documentation Generator

```rust
fn create_docs_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("generate-docs", |args| {
        let code = args.get("code")
            .ok_or_else(|| pmcp::Error::validation("'code' required"))?;
        let format = args.get("format")
            .map(|s| s.as_str())
            .unwrap_or("markdown");

        if !matches!(format, "markdown" | "html" | "plaintext") {
            return Err(pmcp::Error::validation(format!(
                "Invalid format '{}'. Use: markdown, html, or plaintext",
                format
            )));
        }

        Ok(GetPromptResult {
            messages: vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "Generate comprehensive documentation in {} format.\n\
                             \n\
                             Include:\n\
                             - Function/class descriptions\n\
                             - Parameter documentation\n\
                             - Return value descriptions\n\
                             - Usage examples\n\
                             - Edge cases and error handling",
                            format
                        ),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Document this code:\n\n```\n{}\n```", code),
                    },
                },
            ],
            description: Some("Generate code documentation".to_string()),
        })
    })
    .with_description("Generate documentation for code")
    .with_argument("code", "Code to document", true)
    .with_argument("format", "Output format: markdown, html, plaintext", false)
}
```

### Pattern 3: Task Creation Prompt

```rust
fn create_task_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("create-task", |args| {
        let title = args.get("title")
            .ok_or_else(|| pmcp::Error::validation("'title' required"))?;
        let project = args.get("project")
            .map(|s| s.as_str())
            .unwrap_or("default");
        let priority = args.get("priority")
            .map(|s| s.as_str())
            .unwrap_or("normal");

        Ok(GetPromptResult {
            messages: vec![
                PromptMessage {
                    role: Role::System,
                    content: MessageContent::Text {
                        text: format!(
                            "Create a task in project '{}' with priority '{}'.\n\
                             \n\
                             Task format:\n\
                             - Title: Brief, actionable\n\
                             - Description: Clear context and requirements\n\
                             - Acceptance criteria: Measurable completion conditions\n\
                             - Labels: Relevant tags for categorization",
                            project, priority
                        ),
                    },
                },
                // Reference project documentation
                PromptMessage {
                    role: Role::Assistant,
                    content: MessageContent::Resource {
                        uri: format!("resource://projects/{}/guidelines", project),
                        text: None,
                        mime_type: Some("text/markdown".to_string()),
                    },
                },
                PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Create a task: {}", title),
                    },
                },
            ],
            description: Some(format!("Create task in {}", project)),
        })
    })
    .with_description("Create a new task in a project")
    .with_argument("title", "Task title", true)
    .with_argument("project", "Project name", false)
    .with_argument("priority", "Priority: low, normal, high", false)
}
```

## Best Practices for Simple Prompts

### 1. Argument Validation: Fail Fast

```rust
// ❌ Bad: Silent defaults for invalid values
let priority = args.get("priority")
    .map(|s| s.as_str())
    .unwrap_or("normal"); // Silently accepts "urgnet" typo

// ✅ Good: Validate and provide clear error
let priority = args.get("priority")
    .map(|s| s.as_str())
    .unwrap_or("normal");

if !matches!(priority, "low" | "normal" | "high") {
    return Err(pmcp::Error::validation(format!(
        "Invalid priority '{}'. Must be: low, normal, or high",
        priority
    )));
}
```

### 2. System Messages: Be Specific

```rust
// ❌ Too vague
"You are a helpful assistant."

// ✅ Specific role and instructions
"You are a senior software engineer specializing in code review.\n\
 Focus on: security vulnerabilities, performance issues, and maintainability.\n\
 Provide actionable feedback with specific file/line references.\n\
 Use a constructive, educational tone."
```

### 3. Resource References: Keep Prompts Lightweight

```rust
// ❌ Bad: Embed large policy doc in prompt
PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Text {
        text: five_thousand_line_policy_document, // Huge prompt!
    },
}

// ✅ Good: Reference resource (LLM fetches if needed)
PromptMessage {
    role: Role::Assistant,
    content: MessageContent::Resource {
        uri: "resource://policies/refund-policy".to_string(),
        text: None,
        mime_type: Some("text/markdown".to_string()),
    },
}
```

### 4. Argument Descriptions: Guide Users

```rust
// ❌ Vague descriptions
.with_argument("style", "The style", false)

// ✅ Clear descriptions with examples
.with_argument(
    "style",
    "Writing style (professional, casual, technical). Default: professional",
    false
)
```

### 5. Optional Arguments: Document Defaults

```rust
fn create_prompt() -> SyncPrompt<
    impl Fn(HashMap<String, String>) -> pmcp::Result<GetPromptResult> + Send + Sync
> {
    SyncPrompt::new("example", |args| {
        // Document defaults in code
        let format = args.get("format")
            .map(|s| s.as_str())
            .unwrap_or("markdown"); // Default: markdown

        let verbosity = args.get("verbosity")
            .map(|s| s.as_str())
            .unwrap_or("normal"); // Default: normal

        // ... use format and verbosity
        Ok(GetPromptResult { messages: vec![], description: None })
    })
    // Document defaults in argument descriptions
    .with_argument("format", "Output format (markdown, html). Default: markdown", false)
    .with_argument("verbosity", "Detail level (brief, normal, verbose). Default: normal", false)
}
```

## AsyncPrompt vs SyncPrompt

Choose based on your handler's needs:

### SyncPrompt (Recommended for Most Cases)

For simple, CPU-bound prompt generation:

```rust
use pmcp::SyncPrompt;

let prompt = SyncPrompt::new("simple", |args| {
    // Synchronous logic only
    let topic = args.get("topic").unwrap_or(&"default".to_string());

    Ok(GetPromptResult {
        messages: vec![
            PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!("Talk about {}", topic),
                },
            },
        ],
        description: None,
    })
});
```

### SimplePrompt (Async)

For prompts that need async operations (database queries, API calls):

```rust
use pmcp::SimplePrompt;
use std::pin::Pin;
use std::future::Future;

let prompt = SimplePrompt::new("async-example", Box::new(
    |args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra| {
        Box::pin(async move {
            // Can await async operations
            let data = fetch_from_database(&args["id"]).await?;
            let template = generate_messages(&data).await?;

            Ok(GetPromptResult {
                messages: template,
                description: Some("Generated from database".to_string()),
            })
        }) as Pin<Box<dyn Future<Output = pmcp::Result<GetPromptResult>> + Send>>
    }
));
```

**When to use which:**
- ✅ **SyncPrompt**: 95% of cases (simple message construction)
- ✅ **SimplePrompt**: Database lookups, API calls, file I/O

## Listing Prompts

Users discover prompts via `prompts/list`:

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
        {"name": "code", "description": "Code to review", "required": true},
        {"name": "language", "description": "Programming language", "required": false},
        {"name": "focus", "description": "Focus area", "required": false}
      ]
    },
    {
      "name": "blog-post",
      "description": "Generate a complete blog post",
      "arguments": [
        {"name": "topic", "description": "Topic to write about", "required": true},
        {"name": "style", "description": "Writing style", "required": false},
        {"name": "length", "description": "Post length", "required": false}
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
- "Review code focusing on security"

✅ **Multiple tools must be orchestrated in a specific order**
- Data analysis pipelines
- Content generation workflows
- Multi-step validation processes

✅ **You want to guide LLM behavior for specific tasks**
- "Write in executive summary style"
- "Focus on security vulnerabilities"
- "Generate tests for this function"

Don't use prompts when:

❌ **It's just a single tool call**
- Use tools directly instead

❌ **The workflow is user-specific and can't be templated**
- Let the LLM figure it out from available tools

❌ **The task changes based on dynamic runtime conditions**
- Use tools with conditional logic instead

---

## Advanced: Workflow-Based Prompts

For complex multi-tool orchestration with data flow between steps, PMCP provides a powerful workflow system. This advanced section demonstrates building sophisticated prompts that compose multiple tools.

**When to use workflows:**
- ✅ Multi-step processes with data dependencies
- ✅ Complex tool orchestration (step 2 uses output from step 1)
- ✅ Validated workflows with compile-time checks
- ✅ Reusable tool compositions

**When NOT to use workflows:**
- ❌ Simple single-message prompts (use `SyncPrompt`)
- ❌ One-off custom requests
- ❌ Highly dynamic workflows that can't be templated

### Workflow Anatomy: Quadratic Formula Solver

Let's build a workflow that solves quadratic equations (ax² + bx + c = 0) step by step.

From `examples/50_workflow_minimal.rs`:

```rust
use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use serde_json::json;

fn create_quadratic_solver_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "quadratic_solver",
        "Solve quadratic equations using the quadratic formula"
    )
    // Define required prompt arguments
    .argument("a", "Coefficient a (x² term)", true)
    .argument("b", "Coefficient b (x term)", true)
    .argument("c", "Coefficient c (constant term)", true)

    // Add instruction messages
    .instruction(InternalPromptMessage::system(
        "Solve the quadratic equation ax² + bx + c = 0"
    ))

    // Step 1: Calculate discriminant (b² - 4ac)
    .step(
        WorkflowStep::new("calc_discriminant", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("discriminant")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("c", prompt_arg("c"))
            .bind("discriminant") // ← Bind output as "discriminant"
    )

    // Step 2: Calculate first root
    .step(
        WorkflowStep::new("calc_root1", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value")) // ← Reference binding
            .arg("sign", constant(json!("+")))
            .bind("root1") // ← Bind output as "root1"
    )

    // Step 3: Calculate second root
    .step(
        WorkflowStep::new("calc_root2", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value"))
            .arg("sign", constant(json!("-")))
            .bind("root2")
    )

    // Step 4: Format the solution
    .step(
        WorkflowStep::new("format_solution", ToolHandle::new("formatter"))
            .arg("discriminant_result", from_step("discriminant")) // ← Entire output
            .arg("root1_result", from_step("root1"))
            .arg("root2_result", from_step("root2"))
            .arg("format_template", constant(json!("Solution: x = {root1} or x = {root2}")))
            .bind("formatted_solution")
    )
}
```

**Key concepts:**

1. **SequentialWorkflow**: Defines a multi-step workflow
2. **WorkflowStep**: Individual steps that call tools
3. **Bindings**: `.bind("name")` creates named outputs
4. **DSL helpers**:
   - `prompt_arg("a")` - Reference workflow argument
   - `from_step("discriminant")` - Use entire output from previous step
   - `field("discriminant", "value")` - Extract specific field from output
   - `constant(json!("value"))` - Provide constant value

5. **Data flow**: Step 2 uses output from Step 1 via bindings

### Workflow DSL: The Four Mapping Helpers

From `examples/52_workflow_dsl_cookbook.rs`:

```rust
WorkflowStep::new("step_name", ToolHandle::new("tool"))
    // 1. prompt_arg("arg_name") - Get value from workflow arguments
    .arg("input", prompt_arg("user_input"))

    // 2. constant(json!(...)) - Provide a constant value
    .arg("mode", constant(json!("auto")))
    .arg("count", constant(json!(42)))

    // 3. from_step("binding") - Get entire output from previous step
    .arg("data", from_step("result1"))

    // 4. field("binding", "field") - Get specific field from output
    .arg("style", field("result1", "recommended_style"))

    .bind("result2") // ← Create binding for this step's output
```

**Important distinction:**
- **Step name** (first arg): Identifies the step internally
- **Binding name** (via `.bind()`): How other steps reference the output
- ✅ Use **binding names** in `from_step()` and `field()`
- ❌ Don't use step names to reference outputs

### Chaining Steps with Bindings

From `examples/52_workflow_dsl_cookbook.rs`:

```rust
SequentialWorkflow::new("content-pipeline", "Multi-step content creation")
    .argument("topic", "Topic to write about", true)

    .step(
        // Step 1: Create draft
        WorkflowStep::new("create_draft", ToolHandle::new("writer"))
            .arg("topic", prompt_arg("topic"))
            .arg("format", constant(json!("markdown")))
            .bind("draft") // ← Bind as "draft"
    )

    .step(
        // Step 2: Review draft (uses output from step 1)
        WorkflowStep::new("review_draft", ToolHandle::new("reviewer"))
            .arg("content", from_step("draft")) // ← Reference "draft" binding
            .arg("criteria", constant(json!(["grammar", "clarity"])))
            .bind("review") // ← Bind as "review"
    )

    .step(
        // Step 3: Revise (uses outputs from steps 1 & 2)
        WorkflowStep::new("revise_draft", ToolHandle::new("editor"))
            .arg("original", from_step("draft")) // ← Reference "draft"
            .arg("feedback", field("review", "suggestions")) // ← Extract field from "review"
            .bind("final") // ← Bind as "final"
    )
```

**Pattern**: Each step binds its output, allowing later steps to reference it.

### Validation and Error Messages

Workflows are validated at build time. From `examples/51_workflow_error_messages.rs`:

**Common errors:**

1. **Unknown binding** - Referencing a binding that doesn't exist:
```rust
.step(
    WorkflowStep::new("create", ToolHandle::new("creator"))
        .bind("content") // ← Binds as "content"
)
.step(
    WorkflowStep::new("review", ToolHandle::new("reviewer"))
        .arg("text", from_step("draft")) // ❌ ERROR: "draft" doesn't exist
)

// Error: Unknown binding 'draft'. Available bindings: content
// Fix: Change to from_step("content")
```

2. **Undefined prompt argument** - Using an undeclared argument:
```rust
SequentialWorkflow::new("workflow", "...")
    .argument("topic", "The topic", true)
    // Missing: .argument("style", ...)
    .step(
        WorkflowStep::new("create", ToolHandle::new("creator"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", prompt_arg("writing_style")) // ❌ ERROR: not declared
    )

// Error: Undefined prompt argument 'writing_style'
// Fix: Add .argument("writing_style", "Writing style", false)
```

3. **Step without binding cannot be referenced:**
```rust
.step(
    WorkflowStep::new("create", ToolHandle::new("creator"))
        .arg("topic", prompt_arg("topic"))
        // ❌ Missing: .bind("content")
)
.step(
    WorkflowStep::new("review", ToolHandle::new("reviewer"))
        .arg("text", from_step("create")) // ❌ ERROR: "create" has no binding
)

// Error: Step 'create' has no binding. Add .bind("name") to reference it.
// Fix: Add .bind("content") to first step
```

**Best practice**: Call `.validate()` early to catch errors:

```rust
let workflow = create_my_workflow();

match workflow.validate() {
    Ok(()) => println!("✅ Workflow is valid"),
    Err(e) => {
        eprintln!("❌ Validation failed: {}", e);
        // Error messages are actionable - they tell you exactly what's wrong
    }
}
```

### Registering Workflows as Prompts

Use `.prompt_workflow()` to register and validate workflows:

```rust
use pmcp::Server;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let workflow = create_quadratic_solver_workflow();

    let server = Server::builder()
        .name("math-server")
        .version("1.0.0")

        // Register tools that the workflow uses
        .tool("calculator", CalculatorTool)
        .tool("formatter", FormatterTool)

        // Register workflow as prompt (validates automatically)
        .prompt_workflow(workflow)?

        .build()?;

    server.run_stdio().await
}
```

**What `.prompt_workflow()` does:**
1. Validates the workflow (checks bindings, arguments, etc.)
2. Registers it as a prompt (discoverable via `prompts/list`)
3. Returns error if validation fails

### Integration with Typed Tools

From `examples/53_typed_tools_workflow_integration.rs`:

Workflows integrate seamlessly with typed tools:

```rust
use pmcp::Server;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Typed tool input
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AnalyzeCodeInput {
    code: String,
    language: String,
    depth: u8,
}

async fn analyze_code(input: AnalyzeCodeInput, _extra: RequestHandlerExtra) -> Result<Value> {
    // Implementation
    Ok(json!({
        "analysis": "...",
        "issues_found": 3
    }))
}

// Workflow that uses the typed tool
fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new("code_review", "Review code comprehensively")
        .argument("code", "Source code", true)
        .argument("language", "Programming language", false)

        .step(
            WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
                .arg("code", prompt_arg("code"))
                .arg("language", prompt_arg("language"))
                .arg("depth", constant(json!(2)))
                .bind("analysis")
        )
        // ... more steps
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    Server::builder()
        .name("code-server")
        .version("1.0.0")
        // Register typed tool (automatic schema generation)
        .tool_typed("analyze_code", analyze_code)
        // Register workflow that references the tool
        .prompt_workflow(create_code_review_workflow())?
        .build()?
        .run_stdio()
        .await
}
```

**Benefits:**
- ✅ Type-safe tool inputs (compile-time checked)
- ✅ Automatic JSON schema generation
- ✅ Workflow validates tool references exist
- ✅ Single source of truth for tool definitions

### Workflow Best Practices

1. **Use descriptive binding names**:
```rust
// ❌ Bad: Unclear
.bind("r1")
.bind("out")

// ✅ Good: Clear purpose
.bind("analysis_result")
.bind("formatted_output")
```

2. **Declare all arguments before using**:
```rust
SequentialWorkflow::new("workflow", "...")
    // ✅ Declare all arguments first
    .argument("topic", "Topic", true)
    .argument("style", "Style", false)
    .argument("length", "Length", false)
    // Then use them in steps
    .step(...)
```

3. **Add `.bind()` only when output is needed**:
```rust
.step(
    WorkflowStep::new("log", ToolHandle::new("logger"))
        .arg("message", from_step("result"))
        // No .bind() - logging is a side-effect, output not needed
)
```

4. **Use `field()` to extract specific data**:
```rust
// ❌ Bad: Pass entire large object
.arg("data", from_step("analysis")) // Entire analysis result

// ✅ Good: Extract only what's needed
.arg("summary", field("analysis", "summary"))
.arg("score", field("analysis", "confidence_score"))
```

5. **Validate workflows early**:
```rust
let workflow = create_my_workflow();
workflow.validate()?; // ← Catch errors before registration
```

### When to Use Workflows vs Simple Prompts

| Feature | Simple Prompt (`SyncPrompt`) | Workflow (`SequentialWorkflow`) |
|---------|----------------------------|--------------------------------|
| **Use case** | Single-message prompts | Multi-step tool orchestration |
| **Complexity** | Simple | Moderate to complex |
| **Tool composition** | LLM decides | Pre-defined sequence |
| **Data flow** | None | Explicit bindings |
| **Validation** | Argument checks | Full workflow validation |
| **Examples** | Code review, blog post generation | Quadratic solver, content pipeline |

**Decision guide:**
- ✅ Use **simple prompts** for: One-shot requests, LLM-driven tool selection
- ✅ Use **workflows** for: Multi-step processes with known sequence, data dependencies

---

## Testing Prompts

### Unit Testing Simple Prompts

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_review_prompt() {
        let prompt = create_code_review_prompt();

        let mut args = HashMap::new();
        args.insert("code".to_string(), "fn test() {}".to_string());
        args.insert("language".to_string(), "rust".to_string());

        let result = prompt.handle(args, RequestHandlerExtra::default()).await;

        assert!(result.is_ok());
        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 2);
        assert!(matches!(prompt_result.messages[0].role, Role::System));
        assert!(matches!(prompt_result.messages[1].role, Role::User));
    }

    #[tokio::test]
    async fn test_missing_required_argument() {
        let prompt = create_code_review_prompt();

        let args = HashMap::new(); // Missing "code"

        let result = prompt.handle(args, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
    }
}
```

### Testing Workflows

```rust
#[test]
fn test_workflow_validation() {
    let workflow = create_quadratic_solver_workflow();

    // Workflow should validate successfully
    assert!(workflow.validate().is_ok());

    // Check arguments
    assert_eq!(workflow.arguments().len(), 3);
    assert!(workflow.arguments().contains_key(&"a".into()));

    // Check steps
    assert_eq!(workflow.steps().len(), 4);

    // Check bindings
    let bindings = workflow.output_bindings();
    assert!(bindings.contains(&"discriminant".into()));
    assert!(bindings.contains(&"root1".into()));
}
```

### Integration Testing with mcp-tester

```bash
# List all prompts
mcp-tester test stdio --list-prompts

# Get specific prompt
mcp-tester test stdio --get-prompt "code-review" '{"code": "fn main() {}", "language": "rust"}'

# Get workflow prompt
mcp-tester test stdio --get-prompt "quadratic_solver" '{"a": 1, "b": -3, "c": 2}'
```

## Summary

Prompts are user-triggered workflows that orchestrate tools and resources. PMCP provides two approaches:

**Simple Prompts (`SyncPrompt`):**
- ✅ Quick message templates with arguments
- ✅ Minimal boilerplate
- ✅ Perfect for single-message prompts
- ✅ User provides inputs, LLM decides tool usage

**Workflow Prompts (`SequentialWorkflow`):**
- ✅ Multi-step tool orchestration
- ✅ Explicit data flow with bindings
- ✅ Compile-time validation
- ✅ Pre-defined tool sequences

**Key takeaways:**
1. Start with `SyncPrompt` for simple prompts
2. Use workflows when you need multi-step orchestration
3. Validate arguments thoroughly
4. Provide clear system messages
5. Reference resources instead of embedding large content
6. Test with `mcp-tester` and unit tests

**Next chapters:**
- **Chapter 8**: Error Handling & Recovery
- **Chapter 9**: Integration Patterns

Prompts + Tools + Resources = complete MCP server. You now understand how to provide user-triggered workflows that make your server easy and efficient to use.
