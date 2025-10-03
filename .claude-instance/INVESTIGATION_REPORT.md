# TypeScript SDK vs Rust SDK: Prompt Implementation Investigation

**Date:** 2025-10-01
**Investigator:** Claude Code
**Objective:** Compare TypeScript and Rust SDK prompt implementations to identify best practices and potential improvements

---

## Executive Summary

The TypeScript SDK provides a straightforward prompt registration API but **does not include specialized helpers** for referencing tools or resources within prompts. The Rust SDK follows a similar pattern with builder-style APIs. Both SDKs rely on manual string construction for prompt text content, though they support structured content types like `ResourceLink` and `EmbeddedResource` in tool responses.

**Key Finding:** Neither SDK provides ergonomic helpers specifically for referencing tools/resources in prompt text. Developers must manually construct references using string interpolation.

---

## 1. TypeScript SDK Prompt Implementation

### 1.1 Core API Design

The TypeScript SDK uses two registration methods:

#### Method 1: `server.prompt()` - Fluent API
```typescript
server.prompt(
  "greeting-template",
  async ({ name }) => ({
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Please greet ${name} in a friendly manner.`,
        },
      },
    ],
  })
);
```

#### Method 2: `server.registerPrompt()` - Config Object
```typescript
server.registerPrompt(
  'greeting-template',
  {
    title: 'Greeting Template',
    description: 'A simple greeting prompt template',
    argsSchema: {
      name: z.string().describe('Name to include in greeting'),
    },
  },
  async ({ name }): Promise<GetPromptResult> => {
    return {
      messages: [
        {
          role: 'user',
          content: {
            type: 'text',
            text: `Please greet ${name} in a friendly manner.`,
          },
        },
      ],
    };
  }
);
```

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/server/mcp.ts` (lines 959-1047)

### 1.2 Schema Validation

TypeScript SDK uses Zod for runtime schema validation:

```typescript
argsSchema: {
  name: z.string().describe('Name to include in greeting'),
  style: z.enum(['formal', 'casual']).optional(),
}
```

Arguments are validated before the callback executes, with automatic error reporting for invalid inputs.

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/server/mcp.ts` (lines 505-514)

### 1.3 Metadata Support

The TypeScript SDK supports:
- `title` - Display name for UI contexts
- `description` - Human-readable explanation
- `arguments` - Auto-generated from Zod schema with descriptions

Example from tests:
```typescript
const listResult = await client.request(
  { method: "prompts/list" },
  ListPromptsResultSchema,
);

// Returns:
{
  prompts: [{
    name: "greeting-template",
    title: "Greeting Template",
    description: "A simple greeting prompt template",
    arguments: [
      { name: "name", description: "Name to include in greeting", required: true }
    ]
  }]
}
```

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/server/mcp.test.ts` (lines 467-485)

---

## 2. Resource and Tool Referencing

### 2.1 ResourceLink Type

Both SDKs define `ResourceLink` as a content type for **tool responses**, not for prompt construction:

```typescript
export const ResourceLinkSchema = ResourceSchema.extend({
  type: z.literal("resource_link"),
});

// Used in tool responses:
const resourceLinks: ResourceLink[] = [
  {
    type: 'resource_link',
    uri: 'file:///example/file1.txt',
    name: 'Example File 1',
    mimeType: 'text/plain',
    description: 'First example file'
  }
];

return {
  content: [
    { type: 'text', text: 'Here are the available files:' },
    ...resourceLinks
  ]
};
```

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/types.ts` (lines 840-842)
**Example:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts` (lines 370-419)

### 2.2 No Specialized Prompt Helpers

The TypeScript SDK **does not provide**:
- Tool name reference helpers (e.g., `toolRef("greet")`)
- Resource URI reference helpers (e.g., `resourceRef("file:///data.txt")`)
- Builder patterns for constructing prompts with tool/resource metadata
- Automatic validation that referenced tools/resources exist

Developers must manually construct references:

```typescript
// Manual string construction
server.registerPrompt(
  'use-greet-tool',
  { description: 'Prompt that references a tool' },
  async () => ({
    messages: [{
      role: 'user',
      content: {
        type: 'text',
        // No helper - must manually reference tool name
        text: 'Please use the "greet" tool to say hello to Alice.'
      }
    }]
  })
);
```

**Observation:** There's no type safety or validation ensuring the referenced tool "greet" actually exists.

---

## 3. Rust SDK Prompt Implementation

### 3.1 Core API Design

The Rust SDK uses trait-based handlers with two implementations:

#### SimplePrompt - Async Handler
```rust
let prompt = SimplePrompt::new(
    "code_review",
    Box::new(|args: HashMap<String, String>, _extra| {
        Box::pin(async move {
            let language = args.get("language").unwrap_or(&"unknown".to_string());
            let code = args.get("code")
                .ok_or_else(|| pmcp::Error::validation("code required"))?;

            Ok(GetPromptResult {
                messages: vec![
                    PromptMessage {
                        role: Role::User,
                        content: MessageContent::Text {
                            text: format!("Review this {} code:\n{}", language, code)
                        }
                    }
                ],
                description: Some(format!("Code review for {}", language))
            })
        })
    })
)
.with_description("Generate a code review prompt")
.with_argument("language", "Programming language", false)
.with_argument("code", "Code to review", true);
```

**Source:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/06_server_prompts.rs` (lines 29-83)

#### SyncPrompt - Synchronous Handler
```rust
let prompt = SyncPrompt::new("data_analysis", |args| {
    let data = args.get("data")
        .ok_or_else(|| pmcp::Error::validation("data required"))?;

    Ok(GetPromptResult {
        messages: vec![
            PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Analyze this data:\n{}", data)
                }
            }
        ],
        description: None
    })
})
.with_description("Generate a data analysis prompt")
.with_argument("data", "Data to analyze", true);
```

**Source:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/06_server_prompts.rs` (lines 86-141)

### 3.2 Builder Pattern

Rust SDK uses a consistent builder pattern:

```rust
SimplePrompt::new(name, handler)
    .with_description(description)
    .with_argument(name, description, required)
    .with_arguments(vec![...])
```

**Source:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/simple_prompt.rs` (lines 48-94)

### 3.3 Argument Validation

Rust SDK validates required arguments before invoking the handler:

```rust
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
```

**Source:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/simple_prompt.rs` (lines 106-122)

### 3.4 No Tool/Resource Reference Helpers

Like TypeScript, the Rust SDK **does not provide** specialized helpers for referencing tools or resources in prompts:

```rust
// Manual string construction
let prompt = SimplePrompt::new("use_tool", |args, _| {
    Box::pin(async move {
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    // No helper - manual reference
                    text: "Please use the 'greet' tool to say hello.".to_string()
                }
            }],
            description: None
        })
    })
});
```

**Observation:** Similar to TypeScript, no compile-time or runtime validation that referenced tools exist.

---

## 4. Comparison Matrix

| Feature | TypeScript SDK | Rust SDK | Notes |
|---------|---------------|----------|-------|
| **Prompt Registration** | `prompt()` / `registerPrompt()` | `SimplePrompt` / `SyncPrompt` | Both use builder patterns |
| **Schema Validation** | Zod (runtime) | Manual validation | TS more ergonomic with Zod |
| **Metadata Support** | title, description, arguments | name, description, arguments | Equivalent |
| **Async Support** | Native async/await | Pin<Box<Future>> | TS more ergonomic |
| **Tool Reference Helpers** | ❌ None | ❌ None | **Gap in both SDKs** |
| **Resource Reference Helpers** | ❌ None | ❌ None | **Gap in both SDKs** |
| **Type Safety for References** | ❌ No | ❌ No | String-based, unvalidated |
| **Builder Pattern** | Method chaining | `.with_*()` methods | Both support builders |
| **ResourceLink Support** | ✅ In tool responses | ✅ In tool responses | Not for prompts |
| **EmbeddedResource Support** | ✅ In tool responses | ✅ In tool responses | Not for prompts |

---

## 5. Key Findings

### 5.1 No Helper Functions for Tool/Resource References

**TypeScript SDK:** No utilities for:
- Referencing tool names in prompt text
- Referencing resource URIs in prompt text
- Validating that referenced entities exist
- Auto-completion support for tool/resource names

**Rust SDK:** Same limitations as TypeScript.

**Example of the Problem:**
```typescript
// Developer must manually track tool names
server.registerPrompt(
  'coding-assistant',
  async ({ task }) => ({
    messages: [{
      role: 'user',
      content: {
        type: 'text',
        // Hardcoded tool name - easy to break if tool is renamed
        text: `Use the "execute_code" tool to run: ${task}`
      }
    }]
  })
);

// If tool is later renamed to "run_code", prompt breaks silently
```

### 5.2 ResourceLink and EmbeddedResource Are Tool-Focused

Both SDKs define `ResourceLink` and `EmbeddedResource` as **content blocks for tool responses**, not for prompt construction.

**TypeScript Example (Tool Response):**
```typescript
server.registerTool(
  'list-files',
  async () => ({
    content: [
      { type: 'text', text: 'Available files:' },
      {
        type: 'resource_link',
        uri: 'file:///example/file1.txt',
        name: 'Example File 1',
        mimeType: 'text/plain'
      }
    ]
  })
);
```

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts` (lines 370-419)

**Key Point:** While tools can return `ResourceLink` objects, prompts cannot leverage this structured type. Prompts must use plain text with manual URI references.

### 5.3 String Interpolation as the Standard

Both SDKs rely on manual string construction for prompt text:

**TypeScript:**
```typescript
text: `Please use the "greet" tool to say hello to ${name}.`
```

**Rust:**
```rust
text: format!("Please use the 'greet' tool to say hello to {}.", name)
```

**Observation:** This approach is:
- ❌ **Error-prone** - No validation of tool/resource existence
- ❌ **Not refactor-safe** - Renaming tools breaks prompts silently
- ❌ **No IDE support** - No autocomplete for tool/resource names
- ✅ **Simple** - Easy to understand and implement

---

## 6. Developer Experience Comparison

### 6.1 TypeScript SDK DX

**Strengths:**
- Zod integration provides excellent schema validation
- Type inference from schemas works well
- Fluent API feels natural for JavaScript/TypeScript developers
- Good error messages from Zod

**Weaknesses:**
- No helpers for referencing server entities (tools/resources)
- Manual string construction required
- No compile-time safety for entity references

**Example from Production Code:**
```typescript
server.registerPrompt(
  'greeting-template',
  {
    title: 'Greeting Template',
    argsSchema: {
      name: z.string().describe('Name to include in greeting'),
    },
  },
  async ({ name }) => ({
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: `Please greet ${name} in a friendly manner.`,
        },
      },
    ],
  })
);
```

**Source:** `/Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts` (lines 245-268)

### 6.2 Rust SDK DX

**Strengths:**
- Builder pattern is idiomatic for Rust
- Compile-time type safety for basic operations
- Explicit error handling with `Result<T>`

**Weaknesses:**
- More verbose than TypeScript
- Async handling with `Pin<Box<Future>>` is complex
- Manual argument validation required
- No helpers for referencing server entities

**Example from Production Code:**
```rust
let prompt = SimplePrompt::new(
    "code_review",
    Box::new(|args, _extra| {
        Box::pin(async move {
            let language = args.get("language").unwrap_or(&"unknown".to_string());
            let code = args.get("code")
                .ok_or_else(|| pmcp::Error::validation("code required"))?;

            Ok(GetPromptResult {
                messages: vec![PromptMessage {
                    role: Role::User,
                    content: MessageContent::Text {
                        text: format!("Review this {} code:\n{}", language, code)
                    }
                }],
                description: Some(format!("Code review for {}", language))
            })
        })
    })
)
.with_description("Generate a code review prompt")
.with_argument("language", "Programming language", false)
.with_argument("code", "Code to review", true);
```

**Source:** `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/06_server_prompts.rs` (lines 29-83)

---

## 7. Potential Improvements for Rust SDK

Based on this investigation, here are recommendations for improving the Rust SDK's prompt handling:

### 7.1 Add Tool/Resource Reference Helpers

**Problem:** Manual string construction is error-prone and not refactor-safe.

**Proposed Solution:**
```rust
// Builder API with validation
let prompt = SimplePrompt::new("use_tools", |args, ctx| {
    Box::pin(async move {
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    // Helper validates tool exists
                    text: format!(
                        "Use {} to greet {} and {} to analyze the data.",
                        ctx.tool_ref("greet")?,        // Validated at runtime
                        args.get("name").unwrap(),
                        ctx.resource_ref("file:///data.csv")?  // Validated at runtime
                    )
                }
            }],
            description: None
        })
    })
})
.requires_tool("greet")           // Compile-time tracking
.requires_resource("file:///data.csv");  // Compile-time tracking
```

**Benefits:**
- Runtime validation that referenced entities exist
- Better error messages when references are invalid
- Easier refactoring (can detect broken references)
- Self-documenting dependencies

### 7.2 Structured Content Builder for Prompts

**Problem:** Prompts can only use plain text, while tools can return structured `ResourceLink` objects.

**Proposed Solution:**
```rust
use pmcp::prompt::{PromptBuilder, ContentBuilder};

let prompt = PromptBuilder::new("data_analysis")
    .with_description("Analyze data from resources")
    .with_argument("source", "Data source URI", true)
    .with_handler(|args, ctx| async move {
        let source_uri = args.get("source").unwrap();

        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: ContentBuilder::new()
                    .text("Please analyze the following data:")
                    .resource_link(source_uri, ctx)?  // Structured reference
                    .text("Provide a summary of key insights.")
                    .build()
            }],
            description: None
        })
    })
    .build();
```

**Benefits:**
- Structured content composition
- Type-safe resource references
- Consistent with tool response patterns

### 7.3 Macro-Based Tool/Resource References

**Problem:** Verbose boilerplate for simple prompts.

**Proposed Solution:**
```rust
use pmcp::prompt_ref;

#[prompt_ref]
fn create_analysis_prompt(
    #[arg] data_uri: String,
    #[tool] analyze_data: ToolRef,     // Macro generates validation
    #[resource] source: ResourceRef,   // Macro generates validation
) -> GetPromptResult {
    GetPromptResult {
        messages: vec![PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!(
                    "Use {analyze_data} to process data from {source}."
                )
            }
        }],
        description: None
    }
}
```

**Benefits:**
- Declarative dependency tracking
- Compile-time tool/resource reference validation
- Less boilerplate code

### 7.4 Server-Level Reference Registry

**Problem:** No central tracking of what tools/resources prompts depend on.

**Proposed Solution:**
```rust
let server = Server::builder()
    .name("analysis-server")
    .version("1.0.0")
    // Tools
    .tool("greet", greet_handler)
    .tool("analyze_data", analyze_handler)
    // Resources
    .resource("data.csv", read_csv_handler)
    // Prompts with automatic validation
    .prompt("use_analysis", analysis_prompt)
    .validate_references()  // Fails if prompt references non-existent tool/resource
    .build()?;
```

**Benefits:**
- Server startup fails if references are invalid
- Prevents runtime errors
- Better developer feedback

---

## 8. Specific File References

### TypeScript SDK Core Files
- `/Users/guy/Development/mcp/sdk/typescript-sdk/src/server/mcp.ts` (lines 959-1047) - Prompt registration
- `/Users/guy/Development/mcp/sdk/typescript-sdk/src/types.ts` (lines 680-881) - Prompt type definitions
- `/Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts` (lines 245-268) - Example prompt
- `/Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts` (lines 370-419) - ResourceLink in tool response

### Rust SDK Core Files
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/src/server/simple_prompt.rs` - Prompt implementations
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/06_server_prompts.rs` - Prompt examples
- `/Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/17_completable_prompts.rs` - Completable arguments example

---

## 9. Conclusions

### 9.1 Current State

**Neither SDK provides helpers for:**
1. Referencing tools by name in prompt text
2. Referencing resources by URI in prompt text
3. Validating that referenced entities exist
4. Type-safe or compile-time checked references
5. Structured content composition for prompts (only for tool responses)

**Both SDKs rely on:**
- Manual string interpolation
- Developer discipline to keep references in sync
- Runtime discovery of broken references (if at all)

### 9.2 Rust SDK Opportunities

The Rust SDK could differentiate itself by:

1. **Adding a Reference System:**
   - `ctx.tool_ref("name")` and `ctx.resource_ref("uri")` helpers
   - Runtime validation of references
   - Compile-time tracking via builder methods

2. **Structured Content for Prompts:**
   - Allow `ResourceLink` and similar types in prompt messages
   - Parity with tool response capabilities

3. **Macro-Based Ergonomics:**
   - `#[prompt_ref]` attribute macro for dependency tracking
   - Compile-time validation of tool/resource existence

4. **Server-Level Validation:**
   - `.validate_references()` method to check all prompt dependencies
   - Fail-fast at server startup if references are broken

### 9.3 Recommended Next Steps

1. **Add Basic Reference Helpers** (Low Effort, High Value)
   - Implement `RequestHandlerExtra` methods for tool/resource validation
   - Provide runtime errors for invalid references

2. **Design Structured Content API** (Medium Effort, High Value)
   - Allow prompts to use `ResourceLink` in content
   - Maintain consistency with tool response patterns

3. **Explore Macro Solutions** (High Effort, Medium Value)
   - Prototype `#[prompt_ref]` for ergonomic DX
   - Evaluate complexity vs. benefit

4. **Document Current Limitations** (Low Effort, Medium Value)
   - Update docs to explain manual reference requirements
   - Provide best practices for keeping references in sync

---

## Appendix: Code Snippets

### A.1 TypeScript Prompt with Manual Tool Reference

```typescript
// From: /Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts
server.registerPrompt(
  'greeting-template',
  {
    title: 'Greeting Template',
    description: 'A simple greeting prompt template',
    argsSchema: {
      name: z.string().describe('Name to include in greeting'),
    },
  },
  async ({ name }): Promise<GetPromptResult> => {
    return {
      messages: [
        {
          role: 'user',
          content: {
            type: 'text',
            // Manual string construction - no helpers
            text: `Please greet ${name} in a friendly manner.`,
          },
        },
      ],
    };
  }
);
```

### A.2 TypeScript Tool with ResourceLink Response

```typescript
// From: /Users/guy/Development/mcp/sdk/typescript-sdk/src/examples/server/simpleStreamableHttp.ts
server.registerTool(
  'list-files',
  {
    title: 'List Files with ResourceLinks',
    description: 'Returns a list of files as ResourceLinks',
    inputSchema: {
      includeDescriptions: z.boolean().optional(),
    },
  },
  async ({ includeDescriptions = true }): Promise<CallToolResult> => {
    const resourceLinks: ResourceLink[] = [
      {
        type: 'resource_link',
        uri: 'https://example.com/greetings/default',
        name: 'Default Greeting',
        mimeType: 'text/plain',
        ...(includeDescriptions && { description: 'A simple greeting resource' })
      },
      {
        type: 'resource_link',
        uri: 'file:///example/file1.txt',
        name: 'Example File 1',
        mimeType: 'text/plain',
        ...(includeDescriptions && { description: 'First example file' })
      }
    ];

    return {
      content: [
        { type: 'text', text: 'Here are the available files:' },
        ...resourceLinks
      ]
    };
  }
);
```

### A.3 Rust SimplePrompt with Manual String Construction

```rust
// From: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/examples/06_server_prompts.rs
let code_review = SimplePrompt::new(
    "code_review",
    Box::new(|args: HashMap<String, String>, _extra: pmcp::RequestHandlerExtra| {
        Box::pin(async move {
            let language = args.get("language")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            let code = args.get("code")
                .ok_or_else(|| pmcp::Error::validation("code argument is required"))?;
            let focus = args.get("focus")
                .map(|s| s.as_str())
                .unwrap_or("general");

            let mut messages = vec![];

            // Manual string construction
            messages.push(PromptMessage {
                role: Role::System,
                content: MessageContent::Text {
                    text: format!(
                        "You are an expert {} code reviewer. Focus on {} aspects.",
                        language, focus
                    ),
                },
            });

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
    })
)
.with_description("Generate a code review prompt")
.with_argument("language", "Programming language", false)
.with_argument("code", "Code to review", true)
.with_argument("focus", "Specific aspect to focus on", false);
```

---

**End of Report**
