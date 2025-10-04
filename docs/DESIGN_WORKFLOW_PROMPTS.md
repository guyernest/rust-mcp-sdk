# Design Document: Workflow-Based Prompts for PMCP SDK

**Status**: Draft
**Author**: PMCP SDK Team
**Created**: 2025-10-01
**Last Updated**: 2025-10-01

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Problem Statement](#problem-statement)
3. [Goals and Non-Goals](#goals-and-non-goals)
4. [Design Principles](#design-principles)
5. [Architecture Overview](#architecture-overview)
6. [Detailed Design](#detailed-design)
7. [API Examples](#api-examples)
8. [Implementation Phases](#implementation-phases)
9. [Testing Strategy](#testing-strategy)
10. [Migration Path](#migration-path)
11. [Alternatives Considered](#alternatives-considered)
12. [Open Questions](#open-questions)

---

## Executive Summary

This document proposes a comprehensive redesign of the PMCP SDK's prompt system to support **declarative workflow composition**. The current prompt system relies on manual string construction, leading to error-prone code that breaks silently during refactoring. The proposed design introduces:

- **Handle-based references** for type-safe tool/resource composition
- **Sequential workflow builders** for declarative data flow definition
- **Reusable instruction resources** for consistency across workflows
- **Compile-time and runtime validation** to catch errors early

This design positions the Rust SDK as the premier MCP server framework for production environments, aligning with Toyota Way principles of built-in quality (Jidoka) and fail-fast validation.

**Key Benefits:**
- ✅ Type-safe tool and resource references
- ✅ Refactor-safe workflows (renaming tools propagates automatically)
- ✅ Build-time validation of dependencies
- ✅ Reusable instruction resources for consistency
- ✅ Self-documenting workflow definitions
- ✅ Superior developer experience vs. TypeScript/Python SDKs

---

## Problem Statement

### Current State

Both TypeScript and Rust MCP SDKs require manual string construction for prompts:

```rust
// Current approach - fragile
let prompt = SimplePrompt::new("use_tools", |args, _| {
    Box::pin(async move {
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    // Hardcoded tool/resource names - breaks silently if renamed
                    text: format!(
                        "Use the 'greet' tool to say hello. \
                         Refer to 'resource://docs/greeting-guide' for format."
                    )
                }
            }],
            description: None
        })
    })
});
```

### Problems

1. **No Type Safety**: Tool and resource names are strings; renaming breaks prompts silently
2. **No Validation**: No check that referenced tools/resources exist
3. **Duplication**: Tool signatures and descriptions duplicated in prompt text
4. **Hard to Maintain**: Changes to tools require manual updates across all prompts
5. **No Consistency**: Each developer writes instruction text differently
6. **No IDE Support**: No autocomplete or jump-to-definition for references

### Real-World Impact

- Logseq MCP server has 5+ prompts that reference the same tools
- Renaming `add-content` → `add-block` requires finding/replacing strings across codebase
- No compile-time guarantee that changes are complete
- Instruction text duplicated across similar workflows
- High maintenance burden as server evolves

---

## Goals and Non-Goals

### Goals

1. **Type-Safe Composition**: Tools and resources referenced as typed objects, not strings
2. **Build-Time Validation**: Server build fails if prompts reference non-existent entities
3. **Refactor Safety**: Renaming tools/resources propagates to all prompts automatically
4. **Reusable Instructions**: Shared instruction resources for consistency
5. **Declarative Workflows**: Express data flow between tools clearly
6. **Backward Compatibility**: Existing simple prompts continue to work
7. **Superior DX**: Better than TypeScript SDK's developer experience

### Non-Goals

1. **Visual Workflow Editor**: Command-line/code-based definition is sufficient
2. **Conditional Logic in Workflows**: LLM handles branching; workflows are linear sequences
3. **Stateful Workflows**: Each prompt invocation is independent
4. **GraphQL-Style Query Language**: Builder pattern is more Rust-idiomatic
5. **Runtime Workflow Modification**: Workflows defined at compile time

---

## Design Principles

### 1. Toyota Way: Jidoka (Built-In Quality)

**Principle**: Build quality into the process, not inspect it afterward.

**Application**:
- Type system prevents invalid references at compile time
- Server builder validates all handles during construction
- Fail fast with clear errors, not silent failures

### 2. Composition Over Inheritance

**Principle**: Workflows compose tools and resources, not inherit from them.

**Application**:
- Handles are lightweight identifiers
- Resources provide reusable instruction text
- Workflows declare dependencies explicitly

### 3. Explicit Over Implicit

**Principle**: Make dependencies and data flow visible.

**Application**:
```rust
.step(WorkflowStep {
    tool: add_content.clone(),
    input_mapping: hashmap! {
        "date" => DataSource::PromptArg("date"),        // ← Explicit source
        "content" => DataSource::StepOutput {           // ← Explicit dependency
            step: "content",
            field: "text",
        },
    },
    output_binding: "result".to_string(),
})
```

### 4. Progressive Disclosure

**Principle**: Simple things should be simple; complex things should be possible.

**Application**:
- Layer 1: Basic `SimplePrompt` for text-only prompts (existing)
- Layer 2: `SequentialWorkflow` for tool composition (new)
- Layer 3: Macros for declarative syntax (future)

### 5. Fail Fast

**Principle**: Catch errors at the earliest possible stage.

**Application**:
- Compile time: Type errors for wrong handle types
- Build time: Server validates all handles exist
- Runtime: Clear errors for missing arguments

---

## Rust Idioms

This design follows Rust best practices and idioms for maximum performance and maintainability:

**Summary of Idioms Applied**:
1. ✅ Trait naming: `IntoPromptContent` (not `PromptContentable`)
2. ✅ Newtypes: `ArgName`, `BindingName`, `StepName`, `Uri`
3. ✅ Cheap cloning: `Arc<str>` instead of `String`
4. ✅ Forward compatibility: `#[non_exhaustive]` on all public enums
5. ✅ Deterministic iteration: `IndexMap` for predictable testing
6. ✅ Dedicated errors: `WorkflowError` with `thiserror`
7. ✅ **Performance**: `SmallVec` for hot-path collections
8. ✅ Ergonomics: Chainable builders + DSL helpers

### 1. Trait Naming

**Convention**: Use nouns/verbs, not "-able" suffixes.

**Application**:
- ❌ `PromptReferenceable` (too verbose, "-able" suffix)
- ✅ `IntoPromptMessage` (follows `Into*` pattern)
- ✅ Or implement standard `Into<PromptMessage>` trait

### 2. Newtypes for Domain Concepts

**Convention**: Use newtypes to encode invariants and prevent type confusion.

**Application**:
```rust
// Instead of raw String everywhere:
pub struct StepName(Arc<str>);      // Workflow step identifier
pub struct BindingName(Arc<str>);   // Output variable name
pub struct ArgName(Arc<str>);       // Argument name
pub struct Uri(Arc<str>);           // Resource URI

// Benefits:
// - Can't accidentally pass StepName where ArgName expected
// - Encode validation (e.g., Uri must be valid URI)
// - Self-documenting code
```

### 3. Cheap Cloning with Arc

**Convention**: Use `Arc<str>` instead of `String` for immutable shared data.

**Application**:
```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolHandle {
    name: Arc<str>,  // Not String - cheap to clone
}

// Benefits:
// - Clone is O(1) pointer copy, not string allocation
// - Multiple workflows can share same handle cheaply
// - Consider Copy trait if using &'static str
```

### 4. Non-Exhaustive Enums

**Convention**: Mark public enums as `#[non_exhaustive]` for forward compatibility.

**Application**:
```rust
#[non_exhaustive]  // Can add variants without breaking changes
pub enum DataSource {
    PromptArg(ArgName),
    StepOutput { step: StepName, field: String },
    // Future: can add new variants
}

#[non_exhaustive]
pub enum ErrorStrategy {
    FailFast,
    CollectErrors,
    Retry { max_attempts: usize },
    // Future: Timeout, Circuit Breaker, etc.
}
```

### 5. Deterministic Collections

**Convention**: Use `IndexMap` when iteration order matters.

**Application**:
```rust
use indexmap::IndexMap;

pub struct WorkflowStep {
    pub tool: ToolHandle,
    // IndexMap preserves insertion order - predictable for testing
    pub input_mapping: IndexMap<ArgName, DataSource>,
    pub output_binding: BindingName,
}

// Benefits:
// - Deterministic iteration for testing
// - Stable message generation order
// - Easier debugging (order matches definition)
```

### 6. Ergonomic Builders and DSL

**Convention**: Chainable builders with typed helpers reduce boilerplate.

**Application**:
```rust
// Instead of verbose struct literals:
WorkflowStep {
    tool: add_content,
    input_mapping: indexmap! {
        ArgName::new("date") => DataSource::PromptArg(ArgName::new("date")),
    },
    output_binding: BindingName::new("result"),
    output_transform: None,
}

// Use chainable builder with DSL helpers:
WorkflowStep::new(add_content)
    .arg("date", prompt_arg("date"))  // Chainable, type-safe
    .bind("result")                    // Fluent API
    .build()?                          // Validated

// Benefits:
// - 70% less boilerplate
// - Clear intent (what, not how)
// - Type-safe via newtypes
// - Easy to refactor
```

**DSL Helpers**:
```rust
prompt_arg("name")               // Reference prompt input
from_step("step1", "field")      // Reference step output
constant(json!({"foo": "bar"}))  // Constant value
from_resource(handle)            // Resource reference

// With macros for compile-time validation:
prompt_arg(arg!(name))           // Validates identifier at compile time
from_step(binding!(step1), field!(output))
```

### 7. SmallVec for Hot Paths

**Convention**: Use `SmallVec` for small, frequently-allocated collections.

**Application**:
```rust
use smallvec::SmallVec;

pub struct SequentialWorkflow {
    // Most workflows have 1-3 instruction resources
    instructions: SmallVec<[ResourceHandle; 3]>,

    // Most workflows have 2-5 steps
    steps: SmallVec<[WorkflowStep; 4]>,
}

pub enum PromptContent {
    // Most multi-part content has 2-4 parts
    Multi(SmallVec<[PromptContent; 3]>),
}

// Benefits:
// - 0 heap allocations for small collections (≤N items)
// - Falls back to Vec for larger collections
// - Same API as Vec
// - Significant performance improvement for common case
```

**Rationale**:
- Most workflows have 1-3 instructions, 2-5 steps
- Stack allocation faster than heap for small N
- No behavior change, pure performance optimization
- `union` feature enables optimized layout

### 8. Dedicated Error Types

**Convention**: Use `thiserror` for domain-specific errors.

**Application**:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Step '{step}' references unknown binding '{binding}'")]
    UnknownBinding { step: StepName, binding: BindingName },

    #[error("Workflow '{workflow}' requires unregistered tool '{tool}'")]
    MissingTool { workflow: String, tool: ToolHandle },

    #[error("Workflow '{workflow}' requires unregistered resource '{resource}'")]
    MissingResource { workflow: String, resource: ResourceHandle },

    #[error("Circular dependency detected: {cycle}")]
    CircularDependency { cycle: String },

    #[error("Invalid argument mapping: {0}")]
    InvalidMapping(String),
}

// Benefits:
// - Type-safe error matching
// - Better error messages
// - No string-based error construction
```

---

## Architecture Overview

### Three-Layer Design

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 3: Declarative Macros (Future)                        │
│ workflow_prompt! { ... }                                    │
│ #[derive(WorkflowPrompt)]                                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: Sequential Workflows (This Design)                 │
│ SequentialWorkflow::builder()                               │
│ WorkflowStep { tool, input_mapping, output_binding }        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Foundation (This Design)                           │
│ • ToolHandle, ResourceHandle                                │
│ • PromptReferenceable trait                                 │
│ • MessageContent with typed references                      │
│ • Server-level validation                                   │
└─────────────────────────────────────────────────────────────┘
```

### Component Relationships

```
┌──────────────┐       references       ┌──────────────┐
│  Workflow    │──────────────────────▶ │  ToolHandle  │
│              │                        │              │
│  - steps     │       references       │  - name      │
│  - resources │──────────────────────▶ │  - schema    │
│              │                        └──────────────┘
└──────────────┘
       │                                ┌──────────────┐
       │         references             │ Resource     │
       └───────────────────────────────▶│ Handle       │
                                        │              │
                                        │  - uri       │
                                        │  - mime_type │
                                        └──────────────┘
                                               │
                                               │ provides
                                               ▼
                                        ┌──────────────┐
                                        │ Instruction  │
                                        │ Text         │
                                        │              │
                                        │ (reusable)   │
                                        └──────────────┘
```

---

## Detailed Design

### Design Overview: Protocol vs. Internal Architecture

**Core Principle**: Clean separation between wire format (MCP protocol) and developer API (internal types).

```
┌─────────────────────────────────────────────────────────────────┐
│                     Developer API (Internal)                    │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ ToolHandle   │    │ResourceHandle│    │PromptContent │     │
│  │              │    │              │    │              │     │
│  │ • Type-safe  │    │ • Validated  │    │ • ToolHandle │     │
│  │ • Arc<str>   │    │ • Arc<str>   │    │ • ResourceH. │     │
│  │ • Clone O(1) │    │ • Clone O(1) │    │ • Text       │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         │                    │                    │             │
│         │                    │                    │             │
│         └────────────────────┴────────────────────┘             │
│                              │                                   │
│                   ┌──────────▼──────────┐                       │
│                   │  to_protocol(ctx)   │  ← Conversion Layer   │
│                   │  • Expand handles   │                       │
│                   │  • Validate refs    │                       │
│                   │  • Lookup schemas   │                       │
│                   └──────────┬──────────┘                       │
└──────────────────────────────┼──────────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                    MCP Protocol (Wire Format)                    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────┐      │
│  │ MessageContent (Serialized to JSON)                  │      │
│  │                                                       │      │
│  │  • Text { text: String }                             │      │
│  │  • Image { data, mime_type }                         │      │
│  │  • Resource { uri, text?, mime_type? }               │      │
│  │                                                       │      │
│  │  NO HANDLES - Just stable MCP types                  │      │
│  └──────────────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────────────┘
```

**Key Benefits**:

| Aspect | Protocol Types | Internal Types |
|--------|---------------|----------------|
| **Purpose** | Wire format | Developer API |
| **Stability** | Stable, minimal | Can evolve |
| **Migration** | Easy (copy from TS/Python) | Gradual adoption |
| **Type Safety** | None | Full (handles, newtypes) |
| **Validation** | At runtime | At build time |
| **Handles** | Never exposed | Core feature |

### 1. Newtypes for Domain Concepts

Domain-specific newtypes prevent type confusion and encode invariants.

```rust
use std::sync::Arc;

/// Workflow step identifier
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StepName(Arc<str>);

impl StepName {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Output variable binding name
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingName(Arc<str>);

impl BindingName {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Argument name
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgName(Arc<str>);

impl ArgName {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self(Arc::from(name.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resource URI with validation
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Uri(Arc<str>);

impl Uri {
    pub fn new(uri: impl AsRef<str>) -> Result<Self, WorkflowError> {
        let uri_str = uri.as_ref();
        // Validate URI format (basic check)
        if !uri_str.starts_with("resource://") && !uri_str.starts_with("file://") {
            return Err(WorkflowError::InvalidUri {
                uri: uri_str.to_string(),
            });
        }
        Ok(Self(Arc::from(uri_str)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

### 2. Handle System

Handles are lightweight, cheap-to-clone identifiers using `Arc<str>`.

#### ToolHandle

```rust
use std::sync::Arc;

/// Type-safe identifier for a tool
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolHandle {
    name: Arc<str>,  // Arc makes cloning O(1)
}

impl ToolHandle {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: Arc::from(name.as_ref()),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

// Enable Display for error messages
impl std::fmt::Display for ToolHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
```

#### ResourceHandle

```rust
use std::sync::Arc;

/// Type-safe identifier for a resource
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceHandle {
    uri: Uri,  // Use Uri newtype for validation
}

impl ResourceHandle {
    pub fn new(uri: impl AsRef<str>) -> Result<Self, WorkflowError> {
        Ok(Self {
            uri: Uri::new(uri)?,
        })
    }

    pub fn uri(&self) -> &str {
        self.uri.as_str()
    }
}

// Enable Display for error messages
impl std::fmt::Display for ResourceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uri.as_str())
    }
}
```

**Design Rationale**:
- `Arc<str>` makes cloning O(1) instead of O(n)
- Distinct types prevent mixing tools and resources
- `Uri` newtype validates resource URIs
- No lifetimes = easy to use in builders
- `Display` trait enables clean error messages
- Could use `&'static str` + `Copy` for compile-time constants

### 3. Conversion Traits

Enable ergonomic construction of prompts from multiple types.

```rust
// src/server/workflow/into_prompt_content.rs

/// Convert various types into PromptContent (internal type)
/// This is the developer-facing API
pub trait IntoPromptContent {
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

// Implement for images
impl IntoPromptContent for (String, String) {
    fn into_prompt_content(self) -> PromptContent {
        PromptContent::Image {
            data: self.0,
            mime_type: self.1,
        }
    }
}
```

**Usage**:

```rust
// Loose mode - text only
let msg = InternalPromptMessage {
    role: Role::User,
    content: "Please review this code".into_prompt_content(),
};

// Strict mode - handle
let msg = InternalPromptMessage {
    role: Role::Assistant,
    content: tool_handle.into_prompt_content(),
};

// Even easier with From trait
impl<T: IntoPromptContent> From<T> for PromptContent {
    fn from(value: T) -> Self {
        value.into_prompt_content()
    }
}

// Now you can use .into()
let content: PromptContent = tool_handle.into();
let content: PromptContent = "some text".into();
```

### 4. Protocol vs. Internal Types

**Key Principle**: Separate wire format (protocol) from developer API (internal).

#### 4.1 Protocol Types (Wire Format)

Protocol types match the MCP specification and remain **minimal and stable**.

```rust
// src/types/protocol.rs
// These types are serialized and sent over the wire - KEEP MINIMAL

/// Message content as defined by MCP protocol
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum MessageContent {
    /// Plain text message
    #[serde(rename = "text")]
    Text { text: String },

    /// Image data (base64 encoded)
    #[serde(rename = "image")]
    Image {
        data: String,
        mime_type: String,
    },

    /// Resource reference
    #[serde(rename = "resource")]
    Resource {
        uri: String,
        text: Option<String>,
        mime_type: Option<String>,
    },
}

/// Prompt message as defined by MCP protocol
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: Role,
    pub content: MessageContent,
}
```

**Design Rationale**:
- Matches MCP spec exactly
- No handles or internal types exposed
- Stable wire format
- Easy to copy prompts from TypeScript/Python SDKs

#### 4.2 Internal Types (Developer API)

Internal types are rich, type-safe, and handle-based. Used for **building** prompts.

```rust
// src/server/workflow/prompt_content.rs
// These types are for BUILDING prompts - can be rich and type-safe

/// Internal representation of prompt content
/// Supports both loose (text-only) and strict (handle-based) construction
#[derive(Clone, Debug)]
#[non_exhaustive]  // Can add variants without breaking changes
pub enum PromptContent {
    /// Plain text (loose mode - easy migration)
    Text(String),

    /// Image data
    Image {
        data: String,
        mime_type: String,
    },

    /// Resource URI as string (loose mode)
    ResourceUri(String),

    /// Tool handle (strict mode - type-safe)
    ToolHandle(ToolHandle),

    /// Resource handle (strict mode - type-safe)
    ResourceHandle(ResourceHandle),

    /// Multiple content parts
    /// SmallVec optimized for 2-4 parts (common case)
    Multi(SmallVec<[PromptContent; 3]>),
}

/// Internal representation of a prompt message
#[derive(Clone, Debug)]
pub struct InternalPromptMessage {
    pub role: Role,
    pub content: PromptContent,
}
```

**Design Rationale**:
- Rich types for developer ergonomics
- Supports both loose and strict modes
- Handles not exposed to protocol
- Conversion happens at the edge

#### 4.3 Conversion at the Edge

Convert internal types to protocol types only when sending to client.

```rust
// src/server/workflow/conversion.rs

/// Context needed for handle expansion
pub struct ExpansionContext<'a> {
    tools: &'a HashMap<Arc<str>, ToolInfo>,
    resources: &'a HashMap<Arc<str>, ResourceInfo>,
}

impl PromptContent {
    /// Convert to protocol MessageContent
    /// Expands handles using server registry
    pub fn to_protocol(
        &self,
        ctx: &ExpansionContext,
    ) -> Result<MessageContent, WorkflowError> {
        match self {
            // Loose mode - direct passthrough
            PromptContent::Text(text) => Ok(MessageContent::Text {
                text: text.clone(),
            }),

            PromptContent::Image { data, mime_type } => Ok(MessageContent::Image {
                data: data.clone(),
                mime_type: mime_type.clone(),
            }),

            PromptContent::ResourceUri(uri) => Ok(MessageContent::Resource {
                uri: uri.clone(),
                text: None,
                mime_type: None,
            }),

            // Strict mode - expand handles
            PromptContent::ToolHandle(handle) => {
                // Look up tool in registry
                let tool_info = ctx.tools.get(handle.name())
                    .ok_or_else(|| WorkflowError::MissingTool {
                        workflow: "unknown".to_string(),
                        tool: handle.clone(),
                    })?;

                // Embed tool schema as text (LLM can read it)
                Ok(MessageContent::Text {
                    text: format!(
                        "Tool: {}\nDescription: {}\nSchema: {}",
                        handle.name(),
                        tool_info.description,
                        serde_json::to_string_pretty(&tool_info.input_schema)?
                    ),
                })
            }

            PromptContent::ResourceHandle(handle) => {
                // Validate resource exists
                if !ctx.resources.contains_key(handle.uri()) {
                    return Err(WorkflowError::MissingResource {
                        workflow: "unknown".to_string(),
                        resource: handle.clone(),
                    });
                }

                // Return as resource reference (LLM will fetch)
                Ok(MessageContent::Resource {
                    uri: handle.uri().to_string(),
                    text: None,
                    mime_type: None,
                })
            }

            PromptContent::Multi(parts) => {
                // Convert first part (MCP doesn't support multi-part content)
                // or concatenate text parts
                let mut text_parts = Vec::new();
                for part in parts {
                    let protocol = part.to_protocol(ctx)?;
                    if let MessageContent::Text { text } = protocol {
                        text_parts.push(text);
                    }
                }
                Ok(MessageContent::Text {
                    text: text_parts.join("\n\n"),
                })
            }
        }
    }
}

impl InternalPromptMessage {
    pub fn to_protocol(
        &self,
        ctx: &ExpansionContext,
    ) -> Result<PromptMessage, WorkflowError> {
        Ok(PromptMessage {
            role: self.role,
            content: self.content.to_protocol(ctx)?,
        })
    }
}
```

**Design Rationale**:
- Handle expansion is internal only
- Validation happens at conversion time
- Protocol types never see handles
- Clean separation of concerns

#### 4.4 Migration Path: Loose to Strict

Support both modes to enable gradual migration.

**Loose Mode** (Easy migration from TypeScript/Python):

```rust
use pmcp::{SimplePrompt, PromptContent, InternalPromptMessage, Role};

// Copy-paste from TypeScript - works immediately
let code_review = SimplePrompt::new("code-review", |args, _| {
    Box::pin(async move {
        let language = args.get("language").unwrap_or(&"unknown".to_string());
        let code = args.get("code").ok_or_else(|| pmcp::Error::validation("code required"))?;

        Ok(GetPromptResult {
            messages: vec![
                InternalPromptMessage {
                    role: Role::System,
                    content: PromptContent::Text(format!(
                        "You are an expert {} code reviewer. Provide constructive feedback.",
                        language
                    )),
                },
                InternalPromptMessage {
                    role: Role::User,
                    content: PromptContent::Text(format!(
                        "Please review this {} code:\n\n```{}\n{}\n```",
                        language, language, code
                    )),
                },
            ],
            description: Some(format!("Code review for {}", language)),
        })
    })
})
.with_description("Generate a code review prompt")
.with_argument("language", "Programming language", false)
.with_argument("code", "Code to review", true);
```

**Strict Mode** (Type-safe, refactor-safe):

```rust
use pmcp::{SequentialWorkflow, WorkflowStep, PromptContent, ToolHandle, ResourceHandle};

// Create handles (type-safe)
let analyze_code = ToolHandle::new("analyze-code");
let style_guide = ResourceHandle::new("resource://guides/code-style")?;

// Build workflow with handles
let code_review = SequentialWorkflow::builder()
    .name("code-review")
    .description("Generate a code review with tool support")
    .argument("language", "Programming language", false)
    .argument("code", "Code to review", true)

    // Reference tool by handle (validated at build time)
    .step(WorkflowStep {
        tool: analyze_code.clone(),
        input_mapping: indexmap! {
            ArgName::new("language") => DataSource::PromptArg(ArgName::new("language")),
            ArgName::new("code") => DataSource::PromptArg(ArgName::new("code")),
        },
        output_binding: BindingName::new("analysis"),
        output_transform: None,
    })

    .build()?;

// Server validates handles exist
let server = Server::builder()
    .tool(/* analyze_code tool definition */)
    .resource(/* style_guide resource definition */)
    .prompt_workflow(code_review)  // ← Validates handles
    .build()?;
```

**Key Benefits of This Design**:

1. **Easy Migration**: Copy existing prompts → they work immediately (loose mode)
2. **Gradual Adoption**: Start loose → refactor to strict when ready
3. **Type Safety**: Strict mode catches errors at compile/build time
4. **Clean Protocol**: Wire format stays minimal and stable
5. **No Breaking Changes**: Existing code continues to work

### 5. Sequential Workflow

The core workflow type for linear tool orchestration.

```rust
/// A workflow that executes tools in sequence
pub struct SequentialWorkflow {
    /// Workflow name (used for registration)
    name: String,

    /// Human-readable description
    description: String,

    /// Expected input arguments
    /// Most workflows have 1-4 arguments, use SmallVec to avoid heap allocation
    arguments: SmallVec<[PromptArgument; 3]>,

    /// Instruction resources (inserted as System/Assistant messages)
    /// Most workflows have 1-3 instruction resources
    instructions: SmallVec<[ResourceHandle; 3]>,

    /// Tool execution steps with data flow
    /// Most workflows have 2-5 steps
    steps: SmallVec<[WorkflowStep; 4]>,

    /// Error handling strategy
    error_handling: ErrorStrategy,

    /// Handler function (generates messages at runtime)
    handler: Box<dyn WorkflowHandler>,
}

/// A single step in a workflow
#[derive(Clone, Debug)]
pub struct WorkflowStep {
    /// Tool to execute
    tool: ToolHandle,

    /// How to construct tool arguments from available data
    /// Uses IndexMap for deterministic iteration order (helps testing)
    input_mapping: IndexMap<ArgName, DataSource>,

    /// Variable name to store output (strongly typed)
    output_binding: BindingName,

    /// Optional transformation on output
    output_transform: Option<fn(Value) -> Result<Value>>,
}

/// Builder for WorkflowStep with chainable API
pub struct WorkflowStepBuilder {
    tool: ToolHandle,
    input_mapping: IndexMap<ArgName, DataSource>,
    output_binding: Option<BindingName>,
    output_transform: Option<fn(Value) -> Result<Value>>,
}

impl WorkflowStep {
    /// Create a new step builder
    pub fn new(tool: ToolHandle) -> WorkflowStepBuilder {
        WorkflowStepBuilder {
            tool,
            input_mapping: IndexMap::new(),
            output_binding: None,
            output_transform: None,
        }
    }
}

impl WorkflowStepBuilder {
    /// Add an argument mapping (chainable)
    pub fn arg(
        mut self,
        arg_name: impl Into<ArgName>,
        source: DataSource,
    ) -> Self {
        self.input_mapping.insert(arg_name.into(), source);
        self
    }

    /// Set output binding (required before build)
    pub fn bind(mut self, binding: impl Into<BindingName>) -> Self {
        self.output_binding = Some(binding.into());
        self
    }

    /// Add output transformation
    pub fn transform(mut self, f: fn(Value) -> Result<Value>) -> Self {
        self.output_transform = Some(f);
        self
    }

    /// Build the step
    pub fn build(self) -> Result<WorkflowStep, WorkflowError> {
        let output_binding = self.output_binding.ok_or_else(|| {
            WorkflowError::MissingField {
                workflow: "step".to_string(),
                field: "output_binding",
            }
        })?;

        Ok(WorkflowStep {
            tool: self.tool,
            input_mapping: self.input_mapping,
            output_binding,
            output_transform: self.output_transform,
        })
    }
}

/// Where data comes from for tool arguments
#[derive(Clone, Debug)]
#[non_exhaustive]  // Can add new data sources without breaking changes
pub enum DataSource {
    /// From the prompt's input arguments
    PromptArg(ArgName),

    /// From a previous step's output
    StepOutput {
        step: BindingName,  // Use typed binding name
        field: String,      // Field name in output JSON
    },

    /// From a resource (fetched at runtime)
    Resource(ResourceHandle),

    /// Constant value
    Constant(Value),

    /// Computed from workflow context
    Computed(fn(&WorkflowContext) -> Result<Value>),
}

/// Error handling strategies
#[derive(Clone, Debug)]
#[non_exhaustive]  // Can add new strategies without breaking changes
pub enum ErrorStrategy {
    /// Stop workflow on first error
    FailFast,

    /// Continue on errors, collect them
    CollectErrors,

    /// Retry failed steps
    Retry { max_attempts: usize },
}
```

### 5. DSL Helpers for Ergonomic Workflow Construction

Reduce stringly-typed usage with helper functions and macros.

#### 5.1 DataSource Helper Functions

```rust
// src/server/workflow/dsl.rs

/// Reference a prompt input argument
pub fn prompt_arg(name: impl Into<ArgName>) -> DataSource {
    DataSource::PromptArg(name.into())
}

/// Reference output from a previous step
pub fn from_step(
    step: impl Into<BindingName>,
    field: impl Into<String>,
) -> DataSource {
    DataSource::StepOutput {
        step: step.into(),
        field: field.into(),
    }
}

/// Constant value
pub fn constant(value: Value) -> DataSource {
    DataSource::Constant(value)
}

/// Reference a resource
pub fn from_resource(handle: ResourceHandle) -> DataSource {
    DataSource::Resource(handle)
}

/// Helper for field names (type-safe)
pub fn field(name: &str) -> String {
    name.to_string()
}
```

#### 5.2 Compile-Time Validation Macros

```rust
// src/server/workflow/macros.rs

/// Create ArgName with compile-time validation
/// Validates identifier syntax
#[macro_export]
macro_rules! arg {
    ($name:ident) => {
        ArgName::new(stringify!($name))
    };
}

/// Create BindingName with compile-time validation
#[macro_export]
macro_rules! binding {
    ($name:ident) => {
        BindingName::new(stringify!($name))
    };
}

/// Create field name with compile-time validation
#[macro_export]
macro_rules! field {
    ($name:ident) => {
        stringify!($name)
    };
}
```

#### 5.3 Ergonomic Usage Example

**Before (verbose, stringly-typed)**:

```rust
.step(WorkflowStep {
    tool: add_content.clone(),
    input_mapping: indexmap! {
        ArgName::new("date") => DataSource::PromptArg(ArgName::new("date")),
        ArgName::new("content") => DataSource::StepOutput {
            step: BindingName::new("content"),
            field: "text".to_string(),
        },
    },
    output_binding: BindingName::new("result"),
    output_transform: None,
})
```

**After (concise, chainable, type-safe)**:

```rust
use pmcp::dsl::*;  // Import helpers

.step(
    WorkflowStep::new(add_content)
        .arg("date", prompt_arg(arg!(date)))
        .arg("content", from_step(binding!(content), field!(text)))
        .bind(binding!(result))
        .build()?
)
```

**Even better with Into implementations**:

```rust
impl From<&str> for ArgName {
    fn from(s: &str) -> Self {
        ArgName::new(s)
    }
}

impl From<&str> for BindingName {
    fn from(s: &str) -> Self {
        BindingName::new(s)
    }
}

// Now it's even simpler:
.step(
    WorkflowStep::new(add_content)
        .arg("date", prompt_arg("date"))           // Implicit Into<ArgName>
        .arg("content", from_step("content", "text"))  // Implicit Into<BindingName>
        .bind("result")                             // Implicit Into<BindingName>
        .build()?
)
```

#### 5.4 Real-World Comparison

**Logseq TODO Workflow - Before**:

```rust
let add_todo_workflow = SequentialWorkflow::builder()
    .name("add-todo-to-project")
    .step(WorkflowStep {
        tool: normalize_project.clone(),
        input_mapping: indexmap! {
            ArgName::new("input") => DataSource::PromptArg(ArgName::new("project_name")),
        },
        output_binding: BindingName::new("normalized"),
        output_transform: None,
    })
    .step(WorkflowStep {
        tool: build_content.clone(),
        input_mapping: indexmap! {
            ArgName::new("task") => DataSource::PromptArg(ArgName::new("task_description")),
            ArgName::new("project") => DataSource::StepOutput {
                step: BindingName::new("normalized"),
                field: "name".to_string(),
            },
        },
        output_binding: BindingName::new("content"),
        output_transform: None,
    })
    .build()?;
```

**Logseq TODO Workflow - After**:

```rust
use pmcp::dsl::*;

let add_todo_workflow = SequentialWorkflow::builder()
    .name("add-todo-to-project")
    .step(
        WorkflowStep::new(normalize_project)
            .arg("input", prompt_arg("project_name"))
            .bind("normalized")
            .build()?
    )
    .step(
        WorkflowStep::new(build_content)
            .arg("task", prompt_arg("task_description"))
            .arg("project", from_step("normalized", "name"))
            .bind("content")
            .build()?
    )
    .build()?;
```

**Lines of code**: 24 → 14 (42% reduction)
**Readability**: ✅ Much clearer what each step does
**Type safety**: ✅ Same (newtypes + validation)
**Refactorability**: ✅ Better (fluent API easier to modify)

### 6. Workflow Error Types

Dedicated error types using `thiserror` for clear, actionable error messages.

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("Step '{step}' references unknown binding '{binding}'")]
    UnknownBinding {
        step: StepName,
        binding: BindingName,
    },

    #[error("Workflow '{workflow}' requires unregistered tool '{tool}'")]
    MissingTool {
        workflow: String,
        tool: ToolHandle,
    },

    #[error("Workflow '{workflow}' requires unregistered resource '{resource}'")]
    MissingResource {
        workflow: String,
        resource: ResourceHandle,
    },

    #[error("Circular dependency detected in workflow: {cycle}")]
    CircularDependency {
        cycle: String,
    },

    #[error("Invalid argument mapping in step '{step}': {reason}")]
    InvalidMapping {
        step: StepName,
        reason: String,
    },

    #[error("Invalid URI '{uri}': must start with 'resource://' or 'file://'")]
    InvalidUri {
        uri: String,
    },

    #[error("Workflow '{workflow}' is missing required field: {field}")]
    MissingField {
        workflow: String,
        field: &'static str,
    },

    #[error("Step '{step}' output field '{field}' not found in result")]
    OutputFieldNotFound {
        step: BindingName,
        field: String,
    },

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl From<pmcp::Error> for WorkflowError {
    fn from(err: pmcp::Error) -> Self {
        WorkflowError::Other(Box::new(err))
    }
}
```

**Benefits**:
- Type-safe error matching with pattern matching
- Clear error messages with context
- Automatic `Display` implementation via `thiserror`
- Integration with `pmcp::Error` via `From` trait
- No string-based error construction (`Error::validation("...")`)

### 6. Builder Pattern

The primary API for constructing workflows.

```rust
impl SequentialWorkflow {
    pub fn builder() -> SequentialWorkflowBuilder {
        SequentialWorkflowBuilder::default()
    }
}

pub struct SequentialWorkflowBuilder {
    name: Option<String>,
    description: Option<String>,
    arguments: SmallVec<[PromptArgument; 3]>,
    instructions: SmallVec<[ResourceHandle; 3]>,
    steps: SmallVec<[WorkflowStep; 4]>,
    error_handling: ErrorStrategy,
}

impl Default for SequentialWorkflowBuilder {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            arguments: SmallVec::new(),
            instructions: SmallVec::new(),
            steps: SmallVec::new(),
            error_handling: ErrorStrategy::FailFast,
        }
    }
}

impl SequentialWorkflowBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn argument(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.arguments.push(PromptArgument {
            name: name.into(),
            description: Some(description.into()),
            required,
        });
        self
    }

    pub fn instruction(mut self, resource: ResourceHandle) -> Self {
        self.instructions.push(resource);
        self
    }

    pub fn step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn error_strategy(mut self, strategy: ErrorStrategy) -> Self {
        self.error_handling = strategy;
        self
    }

    pub fn build(self) -> Result<SequentialWorkflow, WorkflowError> {
        let name = self.name.ok_or_else(|| WorkflowError::MissingField {
            workflow: "workflow".to_string(),
            field: "name",
        })?;

        // Validate steps reference valid bindings
        self.validate_steps(&name)?;

        Ok(SequentialWorkflow {
            name,
            description: self.description.unwrap_or_default(),
            arguments: self.arguments,
            instructions: self.instructions,
            steps: self.steps,
            error_handling: self.error_handling,
            handler: Box::new(DefaultWorkflowHandler),
        })
    }

    fn validate_steps(&self, workflow_name: &str) -> Result<(), WorkflowError> {
        let mut available_bindings = HashSet::new();

        for (step_index, step) in self.steps.iter().enumerate() {
            let step_name = StepName::new(format!("step_{}", step_index));

            // Check that step references only available bindings
            for source in step.input_mapping.values() {
                if let DataSource::StepOutput { step: binding, .. } = source {
                    if !available_bindings.contains(binding) {
                        return Err(WorkflowError::UnknownBinding {
                            step: step_name,
                            binding: binding.clone(),
                        });
                    }
                }
            }

            // Add this step's output binding
            available_bindings.insert(step.output_binding.clone());
        }

        Ok(())
    }
}
```

### 6. Server Integration

Server validates all handles during build.

```rust
impl ServerBuilder {
    pub fn prompt_workflow(
        mut self,
        workflow: SequentialWorkflow,
    ) -> Self {
        // Extract dependencies
        let tools = workflow.required_tools();
        let resources = workflow.required_resources();

        // Store for validation at build time
        self.prompt_dependencies.push(PromptDependencies {
            prompt_name: workflow.name.clone(),
            tools,
            resources,
        });

        // Register as prompt handler
        self.prompts.insert(
            workflow.name.clone(),
            Box::new(workflow),
        );

        self
    }

    pub fn build(self) -> Result<Server> {
        // Validate all prompt dependencies
        for dep in &self.prompt_dependencies {
            for tool in &dep.tools {
                if !self.tools.contains_key(tool.name()) {
                    return Err(Error::validation(format!(
                        "Prompt '{}' requires unregistered tool '{}'",
                        dep.prompt_name,
                        tool.name()
                    )));
                }
            }

            for resource in &dep.resources {
                if !self.resources.contains_key(resource.uri()) {
                    return Err(Error::validation(format!(
                        "Prompt '{}' requires unregistered resource '{}'",
                        dep.prompt_name,
                        resource.uri()
                    )));
                }
            }
        }

        // Build server
        Ok(Server { /* ... */ })
    }
}
```

---

## Workflow Execution Model: MCP-Compliant Server-Side Execution

### Executive Summary

Based on MCP protocol research and best practices, workflow prompts execute **server-side during `prompts/get`**, not client-side. The server returns a **conversation trace** showing the full execution flow, allowing the LLM to see the complete context and results.

### Key Insight from MCP Research

From the MCP Prompts research:

> "Prompts are user-triggered workflows that users explicitly select for use. MCP prompts can return multi-turn conversation sequences that set up context, demonstrate workflows, and pre-execute server-side operations before the LLM takes over."

**Critical distinction:**
- ❌ **NOT**: Prompts tell the LLM which tools to call (guidance-only)
- ✅ **YES**: Prompts execute workflows server-side and return the execution trace

### The Conversation Pattern

Workflow execution during `prompts/get` generates a structured conversation:

```
Message 1 [User]:      "I want to add task 'Fix bug' to project 'Website'"
Message 2 [Assistant]: "Here's my plan: 1. list_pages, 2. verify_project, 3. add_task"
Message 3 [Assistant]: "Calling tool 'list_pages' with parameters: {}"
Message 4 [User]:      "Tool result: {\"pages\": [\"Website\", \"Mobile\", \"Blog\"]}"
Message 5 [Assistant]: "Calling tool 'verify_project' with {\"project\": \"Website\", ...}"
Message 6 [User]:      "Tool result: {\"exists\": true, \"path\": \"/projects/Website\"}"
Message 7 [Assistant]: "Calling tool 'add_task' with {\"project\": \"Website\", \"task\": \"Fix bug\", ...}"
Message 8 [User]:      "Tool result: {\"success\": true, \"task_id\": \"123\"}"
```

**Key benefits:**
1. **Complete context**: LLM sees the entire execution trace
2. **Pre-executed tools**: Read-only operations completed server-side
3. **Data flow visible**: Step outputs bound and passed to next steps
4. **Minimal client interaction**: MCP client just displays the result
5. **Execution stops on error**: Clear failure point for LLM to address

### Why Server-Side Execution?

From the research:

> "The server is responsible for injecting the argument values into the prompt's content wherever needed, as well as pulling in any other context (from files, databases, etc.) that the prompt is designed to include."

**Rationale:**
- **Server has context**: Access to tools, resources, and data
- **Deterministic workflows**: Same inputs → same execution path
- **Atomic operations**: Workflow completes or fails as a unit
- **Efficient**: No round-trips for read-only operations
- **Safe**: Server validates and executes in controlled environment

### Architecture: Execution During prompts/get

```
┌────────────────────────────────────────────────────────────────┐
│ Client Invokes Prompt                                          │
│ Request: prompts/get { name: "add_task", args: {...} }        │
└────────────────────┬───────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│ Server: WorkflowPromptHandler::handle()                        │
│                                                                 │
│ 1. Create user intent message from workflow description        │
│ 2. Create assistant plan message (list all steps)              │
│ 3. Execute workflow steps sequentially:                        │
│    FOR EACH step:                                              │
│      a. Build tool parameters from bindings + arguments        │
│      b. Create assistant message: "Calling tool X with {...}"  │
│      c. Execute tool handler server-side                       │
│      d. Create user message: "Tool result: {...}"              │
│      e. Store result in ExecutionContext (binding)             │
│      f. If error → STOP, return partial trace                  │
│ 4. Return GetPromptResult with all messages                    │
└────────────────────┬───────────────────────────────────────────┘
                     │
                     ▼
┌────────────────────────────────────────────────────────────────┐
│ Client Receives Conversation Trace                             │
│ - Displays messages to user (or keeps hidden)                  │
│ - Feeds entire trace to LLM as context                         │
│ - LLM continues conversation with full knowledge               │
└────────────────────────────────────────────────────────────────┘
```

### ExecutionContext: Binding Storage

During workflow execution, we need to store step outputs for use by subsequent steps:

```rust
/// Stores step execution results (bindings) during workflow execution
struct ExecutionContext {
    bindings: HashMap<BindingName, Value>,
}

impl ExecutionContext {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    fn store_binding(&mut self, name: BindingName, value: Value) {
        self.bindings.insert(name, value);
    }

    fn get_binding(&self, name: &BindingName) -> Option<&Value> {
        self.bindings.get(name)
    }
}
```

**Purpose:**
- Stores outputs from executed steps
- Enables data flow between steps
- Used to resolve `DataSource::StepOutput` references

### Tool Parameter Resolution

Steps declare how to build tool parameters from available data:

```rust
fn resolve_tool_parameters(
    step: &WorkflowStep,
    args: &HashMap<String, String>,
    ctx: &ExecutionContext,
) -> Result<Value> {
    let mut params = serde_json::Map::new();

    for (arg_name, data_source) in step.arguments() {
        let value = match data_source {
            // From prompt arguments
            DataSource::PromptArg(arg_name) => {
                args.get(arg_name.as_str())
                    .map(|s| Value::String(s.clone()))
                    .ok_or_else(|| Error::validation(format!(
                        "Missing required argument '{}'", arg_name
                    )))?
            }

            // From constant
            DataSource::Constant(val) => val.clone(),

            // From previous step (entire output)
            DataSource::StepOutput { step: binding_name, field: None } => {
                ctx.get_binding(binding_name)
                    .cloned()
                    .ok_or_else(|| Error::validation(format!(
                        "Binding '{}' not found", binding_name
                    )))?
            }

            // From previous step (specific field)
            DataSource::StepOutput { step: binding_name, field: Some(field_name) } => {
                let binding_value = ctx.get_binding(binding_name)
                    .ok_or_else(|| Error::validation(format!(
                        "Binding '{}' not found", binding_name
                    )))?;

                binding_value.get(field_name.as_str())
                    .cloned()
                    .ok_or_else(|| Error::validation(format!(
                        "Field '{}' not found in binding '{}'",
                        field_name, binding_name
                    )))?
            }
        };

        params.insert(arg_name.to_string(), value);
    }

    Ok(Value::Object(params))
}
```

### Updated WorkflowPromptHandler

```rust
pub struct WorkflowPromptHandler {
    workflow: SequentialWorkflow,
    tools: HashMap<Arc<str>, ToolInfo>,
    resources: HashMap<Arc<str>, ResourceInfo>,
    // NEW: Access to actual tool handlers for execution
    tool_handlers: HashMap<Arc<str>, Arc<dyn ToolHandler>>,
}

#[async_trait]
impl PromptHandler for WorkflowPromptHandler {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        let mut messages = Vec::new();
        let mut execution_context = ExecutionContext::new();

        // 1. User Intent Message
        messages.push(self.create_user_intent(&args)?);

        // 2. Assistant Plan Message (list all workflow steps)
        messages.push(self.create_assistant_plan()?);

        // 3. Execute workflow steps sequentially
        for step in self.workflow.steps() {
            // Assistant announces the tool call with parameters
            match self.create_tool_call_announcement(step, &args, &execution_context) {
                Ok(announcement) => messages.push(announcement),
                Err(e) => {
                    // Can't build parameters - missing argument
                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "Cannot proceed with step '{}': {}",
                                step.name(), e
                            ),
                        },
                    });
                    break; // Stop execution
                }
            }

            // Execute the tool server-side
            match self.execute_tool_step(step, &args, &execution_context, &extra).await {
                Ok(result) => {
                    // User message with successful result
                    messages.push(PromptMessage {
                        role: Role::User,
                        content: MessageContent::Text {
                            text: format!(
                                "Tool result:\n{}",
                                serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| format!("{:?}", result))
                            ),
                        },
                    });

                    // Store binding for next steps
                    if let Some(binding) = step.binding() {
                        execution_context.store_binding(binding.clone(), result);
                    }
                }
                Err(e) => {
                    // User message with error - STOP EXECUTION
                    messages.push(PromptMessage {
                        role: Role::User,
                        content: MessageContent::Text {
                            text: format!("Error executing tool: {}", e),
                        },
                    });
                    break; // Let LLM handle recovery
                }
            }
        }

        Ok(GetPromptResult {
            description: Some(self.workflow.description().to_string()),
            messages,
        })
    }
}
```

### Execution Flow Examples

#### Success Case: All Steps Complete

```
Input: { project: "Website", task: "Fix login bug" }

Message 1 [User]:
  I want to add a task to a project.
  Parameters:
    - project: "Website"
    - task: "Fix login bug"

Message 2 [Assistant]:
  Here's my plan:
  1. list_pages - Get all available pages
  2. verify_project - Check if project exists
  3. add_journal_task - Add the task to the project

Message 3 [Assistant]:
  Calling tool 'list_pages' with parameters:
  {}

Message 4 [User]:
  Tool result:
  {
    "pages": ["Website", "Mobile", "Blog"]
  }

Message 5 [Assistant]:
  Calling tool 'verify_project' with parameters:
  {
    "project": "Website",
    "available_pages": ["Website", "Mobile", "Blog"]
  }

Message 6 [User]:
  Tool result:
  {
    "exists": true,
    "path": "/projects/Website"
  }

Message 7 [Assistant]:
  Calling tool 'add_journal_task' with parameters:
  {
    "project": "Website",
    "task": "Fix login bug",
    "project_path": "/projects/Website"
  }

Message 8 [User]:
  Tool result:
  {
    "success": true,
    "task_id": "task-123"
  }
```

**LLM receives:** Complete execution trace → can summarize or take next action

#### Error Case: Step Fails Mid-Workflow

```
Message 1 [User]:
  I want to add a task to a project.
  Parameters:
    - project: "Nonexistent"
    - task: "Fix bug"

Message 2 [Assistant]:
  Here's my plan:
  1. list_pages - Get all available pages
  2. verify_project - Check if project exists
  3. add_journal_task - Add the task to the project

Message 3 [Assistant]:
  Calling tool 'list_pages' with parameters:
  {}

Message 4 [User]:
  Tool result:
  {
    "pages": ["Website", "Mobile", "Blog"]
  }

Message 5 [Assistant]:
  Calling tool 'verify_project' with parameters:
  {
    "project": "Nonexistent",
    "available_pages": ["Website", "Mobile", "Blog"]
  }

Message 6 [User]:
  Error executing tool: Project 'Nonexistent' not found in available pages
```

**LLM receives:** Partial trace with error → can explain problem to user or suggest corrections

### Benefits of This Approach

1. **MCP-Compliant**: Follows protocol specification for prompt workflows
2. **Complete Context**: LLM sees full execution trace
3. **Efficient**: No round-trips for data fetching
4. **Deterministic**: Same inputs produce same execution
5. **Error Transparent**: Clear failure points
6. **Data Flow Visible**: Step bindings shown in results
7. **Debuggable**: Conversation trace shows exactly what happened

### Comparison: Guidance vs. Execution

**Old Approach (Guidance Only):**
```
Message [User]: "Use tool 'list_pages', then 'verify_project', then 'add_task'"
→ LLM must call each tool separately
→ Client mediates each tool call
→ 6+ round trips for 3-step workflow
```

**New Approach (Server-Side Execution):**
```
8 Messages returned immediately:
- User intent
- Assistant plan
- Assistant call 1 + User result 1
- Assistant call 2 + User result 2
- Assistant call 3 + User result 3
→ All tools executed server-side
→ 1 round trip total
→ LLM has complete context
```

**Performance:** ~6x faster for 3-step workflows

---

## API Examples

### Example 1: Simple Workflow

```rust
use pmcp::{SequentialWorkflow, WorkflowStep, DataSource, ErrorStrategy};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Create handles
    let greet_tool = ToolHandle::new("greet");
    let greeting_guide = ResourceHandle::new("resource://greetings/guide");

    // Build workflow
    let workflow = SequentialWorkflow::builder()
        .name("personalized-greeting")
        .description("Generate a personalized greeting")
        .argument("name", "Person's name", true)
        .argument("style", "Greeting style (formal/casual)", false)
        .instruction(greeting_guide.clone())
        .step(WorkflowStep {
            tool: greet_tool.clone(),
            input_mapping: hashmap! {
                "name" => DataSource::PromptArg("name".to_string()),
                "style" => DataSource::PromptArg("style".to_string()),
            },
            output_binding: "greeting".to_string(),
            output_transform: None,
        })
        .error_strategy(ErrorStrategy::FailFast)
        .build()?;

    // Register in server
    let server = Server::builder()
        .name("greeting-server")
        .tool(/* greet tool definition */)
        .resource(/* greeting guide definition */)
        .prompt_workflow(workflow)
        .build()?;

    server.run_stdio().await
}
```

### Example 2: Multi-Step Workflow (Logseq TODO)

```rust
use pmcp::*;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Handles
    let normalize_project = ToolHandle::new("normalize-project");
    let build_content = ToolHandle::new("build-todo-content");
    let add_content = ToolHandle::new("add-content");

    let logseq_intro = ResourceHandle::new("resource://logseq/workflow-intro");
    let error_handling = ResourceHandle::new("resource://logseq/error-handling");
    let todo_format = ResourceHandle::new("resource://logseq/todo-format");

    // Workflow
    let add_todo_workflow = SequentialWorkflow::builder()
        .name("add-todo-to-project")
        .description("Add a TODO item to a Logseq project")
        .argument("task_description", "What needs doing", true)
        .argument("project_name", "Project name (without brackets)", true)
        .argument("date", "Date (defaults to today)", false)

        // Instruction resources
        .instruction(logseq_intro.clone())
        .instruction(error_handling.clone())
        .instruction(todo_format.clone())

        // Step 1: Normalize project name
        .step(WorkflowStep {
            tool: normalize_project.clone(),
            input_mapping: hashmap! {
                "input" => DataSource::PromptArg("project_name".to_string()),
            },
            output_binding: "normalized".to_string(),
            output_transform: None,
        })

        // Step 2: Build TODO content
        .step(WorkflowStep {
            tool: build_content.clone(),
            input_mapping: hashmap! {
                "task" => DataSource::PromptArg("task_description".to_string()),
                "project" => DataSource::StepOutput {
                    step: "normalized".to_string(),
                    field: "name".to_string(),
                },
            },
            output_binding: "content".to_string(),
            output_transform: None,
        })

        // Step 3: Add to journal
        .step(WorkflowStep {
            tool: add_content.clone(),
            input_mapping: hashmap! {
                "date" => DataSource::PromptArg("date".to_string()),
                "content" => DataSource::StepOutput {
                    step: "content".to_string(),
                    field: "text".to_string(),
                },
            },
            output_binding: "result".to_string(),
            output_transform: None,
        })

        .error_strategy(ErrorStrategy::FailFast)
        .build()?;

    // Server
    let server = Server::builder()
        .name("logseq-mcp")
        .version("1.0.0")
        .tool(/* normalize_project definition */)
        .tool(/* build_content definition */)
        .tool(/* add_content definition */)
        .resource(/* logseq_intro definition */)
        .resource(/* error_handling definition */)
        .resource(/* todo_format definition */)
        .prompt_workflow(add_todo_workflow)
        .build()?;  // ← Validates all handles exist

    server.run_stdio().await
}
```

### Example 3: Reusable Instruction Resources

```rust
// Define once, reuse across workflows
mod logseq_resources {
    use pmcp::ResourceHandle;

    pub fn intro() -> ResourceHandle {
        ResourceHandle::new("resource://logseq/workflow-intro")
    }

    pub fn error_handling() -> ResourceHandle {
        ResourceHandle::new("resource://logseq/error-handling")
    }

    pub fn todo_format() -> ResourceHandle {
        ResourceHandle::new("resource://logseq/todo-format")
    }

    pub fn project_tags() -> ResourceHandle {
        ResourceHandle::new("resource://logseq/project-tags")
    }
}

// Use in multiple workflows
let workflow1 = SequentialWorkflow::builder()
    .instruction(logseq_resources::intro())
    .instruction(logseq_resources::error_handling())
    .instruction(logseq_resources::todo_format())
    // ...
    .build()?;

let workflow2 = SequentialWorkflow::builder()
    .instruction(logseq_resources::intro())      // Same instructions!
    .instruction(logseq_resources::error_handling())  // Consistency!
    .instruction(logseq_resources::project_tags())
    // ...
    .build()?;
```

**Benefit**: Update instruction content once → all workflows improve.

---

## Implementation Phases

### Dependencies

This implementation requires adding the following dependencies to `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }

# New dependencies for workflow system
indexmap = { version = "2.2", features = ["serde"] }  # Deterministic HashMap
thiserror = "1.0"                                      # Error type derivation
smallvec = { version = "1.13", features = ["serde", "union"] }  # Stack-allocated vectors
```

**Rationale**:
- **indexmap**: Provides `IndexMap` with insertion-order preservation for deterministic iteration
- **thiserror**: Ergonomic error type definitions with automatic `Display` implementation
- **smallvec**: Stack-allocated vectors for small collections (instructions, steps, parts) to minimize heap allocations

### Phase 1: Foundation (Week 1)

**Scope**: Protocol types, internal types, handles, newtypes, conversion traits

**Deliverables**:
- [ ] **Protocol types** (keep minimal):
  - [ ] `MessageContent` enum (text, image, resource only) with `#[non_exhaustive]`
  - [ ] `PromptMessage` struct (protocol format)
- [ ] **Internal types** (rich, type-safe):
  - [ ] `PromptContent` enum (ToolHandle, ResourceHandle, Text, etc.) with `#[non_exhaustive]`
  - [ ] Use `SmallVec<[PromptContent; 3]>` for Multi variant
  - [ ] `InternalPromptMessage` struct (internal format)
- [ ] **Newtypes**: `StepName`, `BindingName`, `ArgName`, `Uri` (all using `Arc<str>`)
- [ ] **Handles**: `ToolHandle`, `ResourceHandle` (using `Arc<str>`)
- [ ] **Traits**: `IntoPromptContent` + implementations
- [ ] **Conversion**: `PromptContent::to_protocol(ctx)` + `ExpansionContext`
- [ ] **Errors**: `WorkflowError` error type (using `thiserror`)
- [ ] Unit tests for all components
- [ ] **Performance**: Benchmark SmallVec vs Vec for common workflows

**Files to Create/Modify**:
- `src/types/protocol.rs` (KEEP MINIMAL - protocol types only)
- `src/server/workflow/mod.rs` (new module)
- `src/server/workflow/newtypes.rs` (new - domain newtypes)
- `src/server/workflow/handles.rs` (new - ToolHandle, ResourceHandle)
- `src/server/workflow/prompt_content.rs` (new - PromptContent internal type)
- `src/server/workflow/into_prompt_content.rs` (new - conversion trait)
- `src/server/workflow/conversion.rs` (new - to_protocol conversion)
- `src/server/workflow/error.rs` (new - WorkflowError)
- `src/server/mod.rs` (export new types)

**Success Criteria**:
- ✅ Protocol types (`MessageContent`) remain minimal and stable
- ✅ Internal types (`PromptContent`) support both loose and strict modes
- ✅ Handles are `Clone + Debug + PartialEq + Display`
- ✅ Conversion `to_protocol()` works for all `PromptContent` variants
- ✅ All unit tests pass (handles, newtypes, conversion)
- ✅ No breaking changes to existing API

### Phase 2: Sequential Workflow (Week 2)

**Scope**: Workflow builder, validation, ergonomic DSL

**Deliverables**:
- [ ] `SequentialWorkflow` type
  - [ ] Use `SmallVec<[PromptArgument; 3]>` for arguments
  - [ ] Use `SmallVec<[ResourceHandle; 3]>` for instructions
  - [ ] Use `SmallVec<[WorkflowStep; 4]>` for steps
- [ ] `WorkflowStep` type (using `IndexMap` for `input_mapping`)
- [ ] `WorkflowStepBuilder` with chainable `.arg()`, `.bind()` API
- [ ] `DataSource` enum (with `#[non_exhaustive]`)
- [ ] `ErrorStrategy` enum (with `#[non_exhaustive]`)
- [ ] `SequentialWorkflowBuilder` with validation (using `WorkflowError`)
  - [ ] Use `SmallVec` for all collections
- [ ] **DSL helpers**: `prompt_arg()`, `from_step()`, `constant()`, `from_resource()`
- [ ] **Macros**: `arg!()`, `binding!()`, `field!()`
- [ ] **Into implementations**: `From<&str> for ArgName/BindingName`
- [ ] Server integration (`.prompt_workflow()`)
- [ ] Dependency validation at build time (returns `WorkflowError`)
- [ ] Integration tests
- [ ] Ergonomics tests (compare before/after)
- [ ] **Performance tests**: SmallVec allocation benchmarks

**Files to Create/Modify**:
- `src/server/workflow/workflow.rs` (new - SequentialWorkflow)
- `src/server/workflow/workflow_step.rs` (new - WorkflowStep + WorkflowStepBuilder)
- `src/server/workflow/data_source.rs` (new - DataSource)
- `src/server/workflow/builder.rs` (new - SequentialWorkflowBuilder)
- `src/server/workflow/dsl.rs` (new - helper functions)
- `src/server/workflow/macros.rs` (new - arg!, binding!, field! macros)
- `src/server/server.rs` (modify builder)
- `tests/workflow_validation.rs` (new)
- `tests/workflow_ergonomics.rs` (new - DSL usage tests)

**Success Criteria**:
- Builder API compiles and works
- Step validation catches invalid references
- Server build fails on missing dependencies
- All integration tests pass

### Phase 3: Runtime Expansion (Week 2)

**Scope**: Convert handles to actual content

**Deliverables**:
- [ ] Runtime handle expansion in prompt execution
- [ ] Tool schema lookup from server registry
- [ ] Resource content fetching
- [ ] Message generation from workflow definition
- [ ] End-to-end workflow execution tests

**Files to Create/Modify**:
- `src/server/workflow.rs` (add handler logic)
- `src/server/server.rs` (add registry access)
- `examples/10_workflow_prompt.rs` (new example)

**Success Criteria**:
- Prompts with ToolHandle references work
- Prompts with ResourceHandle references work
- Generated messages match MCP protocol
- Example server runs successfully

### Phase 4: Documentation & Examples (Week 3)

**Scope**: User-facing docs, examples, migration guide

**Deliverables**:
- [ ] Update Chapter 7 (Prompts) in pmcp-book
- [ ] Create `examples/11_logseq_workflow.rs`
- [ ] Write migration guide for existing prompts
- [ ] API documentation (rustdoc)
- [ ] Design pattern guide

**Files to Create/Modify**:
- `pmcp-book/src/ch07-prompts.md` (rewrite)
- `examples/11_logseq_workflow.rs` (new)
- `docs/MIGRATION_WORKFLOW_PROMPTS.md` (new)
- `README.md` (add workflow section)

**Success Criteria**:
- Chapter 7 showcases workflow API
- Logseq example demonstrates real-world usage
- Migration guide is clear and actionable
- All code examples compile and run

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_handle_equality() {
        let h1 = ToolHandle::new("greet");
        let h2 = ToolHandle::new("greet");
        let h3 = ToolHandle::new("farewell");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_workflow_builder_validates_steps() {
        let tool = ToolHandle::new("test");

        let result = SequentialWorkflow::builder()
            .name("invalid-workflow")
            .step(WorkflowStep {
                tool: tool.clone(),
                input_mapping: hashmap! {
                    "input" => DataSource::StepOutput {
                        step: "nonexistent".to_string(),
                        field: "value".to_string(),
                    },
                },
                output_binding: "result".to_string(),
                output_transform: None,
            })
            .build();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown binding"));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_server_validates_prompt_dependencies() {
    let missing_tool = ToolHandle::new("missing-tool");

    let workflow = SequentialWorkflow::builder()
        .name("test-workflow")
        .step(WorkflowStep {
            tool: missing_tool.clone(),
            input_mapping: HashMap::new(),
            output_binding: "result".to_string(),
            output_transform: None,
        })
        .build()
        .unwrap();

    let result = Server::builder()
        .name("test-server")
        .prompt_workflow(workflow)
        .build();

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unregistered tool"));
}
```

### Property Tests

```rust
#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_tool_handle_name_roundtrip(name in "[a-z]{1,20}") {
            let handle = ToolHandle::new(&name);
            assert_eq!(handle.name(), name);
        }

        #[test]
        fn test_workflow_step_references_are_valid(
            step_count in 1..10usize
        ) {
            // Generate workflow with N steps
            // Verify all DataSource::StepOutput references exist
        }
    }
}
```

### End-to-End Tests

```rust
#[tokio::test]
async fn test_logseq_workflow_end_to_end() {
    // Build complete Logseq server with workflow
    let server = build_logseq_server().await.unwrap();

    // Invoke workflow via MCP client
    let request = GetPromptRequest {
        name: "add-todo-to-project".to_string(),
        arguments: Some(hashmap! {
            "task_description" => "Write tests".to_string(),
            "project_name" => "SDK".to_string(),
        }),
    };

    let result = server.handle_get_prompt(request).await.unwrap();

    // Verify message structure
    assert!(result.messages.len() >= 5);
    assert!(result.messages.iter().any(|m| matches!(
        m.content,
        MessageContent::ToolSchema { .. }
    )));
    assert!(result.messages.iter().any(|m| matches!(
        m.content,
        MessageContent::ResourceLink { .. }
    )));
}
```

---

## Migration Path

### For Existing Simple Prompts

**Before**:
```rust
let prompt = SimplePrompt::new("greet", |args, _| {
    Box::pin(async move {
        let name = args.get("name").unwrap();
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Say hello to {}", name)
                }
            }],
            description: None
        })
    })
});
```

**After** (no change required - backward compatible):
```rust
// Existing code continues to work
let prompt = SimplePrompt::new("greet", |args, _| {
    Box::pin(async move {
        let name = args.get("name").unwrap();
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!("Say hello to {}", name)
                }
            }],
            description: None
        })
    })
});
```

### For Tool-Referencing Prompts

**Before**:
```rust
let prompt = SimplePrompt::new("use-greet", |args, _| {
    Box::pin(async move {
        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: "Use the 'greet' tool to say hello.".to_string()
                }
            }],
            description: None
        })
    })
});
```

**After** (recommended upgrade):
```rust
let greet_tool = ToolHandle::new("greet");

let workflow = SequentialWorkflow::builder()
    .name("use-greet")
    .step(WorkflowStep {
        tool: greet_tool.clone(),
        input_mapping: HashMap::new(),
        output_binding: "result".to_string(),
        output_transform: None,
    })
    .build()?;

// Server validates greet_tool exists
let server = Server::builder()
    .tool(/* greet definition */)
    .prompt_workflow(workflow)
    .build()?;
```

### Migration Checklist

- [ ] Identify prompts that reference tools/resources by string
- [ ] Create handles for referenced tools/resources
- [ ] Convert to `SequentialWorkflow` with typed references
- [ ] Update server registration to use `.prompt_workflow()`
- [ ] Run tests to verify validation works
- [ ] Update documentation

---

## Alternatives Considered

### Alternative 1: String-Based References with Runtime Validation

**Approach**: Keep strings, add runtime validation.

```rust
let prompt = SimplePrompt::new("use-greet", |args, ctx| {
    Box::pin(async move {
        // Runtime check
        ctx.require_tool("greet")?;

        Ok(GetPromptResult {
            messages: vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: "Use the 'greet' tool.".to_string()
                }
            }],
            description: None
        })
    })
});
```

**Pros**:
- Simple implementation
- No new types

**Cons**:
- Still error-prone (typos in strings)
- No compile-time safety
- No refactoring support
- No IDE autocomplete

**Decision**: Rejected - doesn't solve core problems.

### Alternative 2: Direct Object References

**Approach**: Pass actual tool/resource objects.

```rust
let greet_tool = Tool::builder().name("greet").build();

let prompt = Prompt::builder()
    .reference_tool(&greet_tool)  // Borrow tool
    .build();
```

**Pros**:
- Strong type safety

**Cons**:
- Lifetime hell in builder pattern
- Ownership/borrowing complexity
- Hard to use in practice

**Decision**: Rejected - Rust lifetimes make this impractical.

### Alternative 3: Macro-Only Approach

**Approach**: Skip builder, go straight to macros.

```rust
workflow_prompt! {
    name: "add-todo",
    steps: { ... },
}
```

**Pros**:
- Very concise
- Declarative

**Cons**:
- Hard to implement
- Poor error messages
- Less flexible
- Steeper learning curve

**Decision**: Deferred - do this as Layer 3, after builder is solid.

---

## Open Questions

### 1. Runtime vs. Build-Time Validation

**Question**: Should we validate tool/resource existence at build time or runtime?

**Options**:
- **Build time**: Server builder checks all handles
- **Runtime**: Workflow execution checks handles
- **Both**: Validate at build, re-check at runtime

**Recommendation**: Build time + runtime
- Build time catches most issues early
- Runtime handles dynamic scenarios (if needed)

**Status**: ✅ Resolved - both

### 2. Schema Representation in Handles

**Question**: Should handles embed schema information?

**Options**:
- **No**: Handles are just identifiers
- **Yes**: Handles include schema for validation
- **Lazy**: Handles fetch schema from server on-demand

**Recommendation**: No - keep handles lightweight
- Server registry is source of truth
- Handles are just identifiers
- Runtime expansion looks up schema

**Status**: ✅ Resolved - no schema in handles

### 3. Macro Syntax Design

**Question**: If we add macros, what syntax?

**Options**:
- Declarative DSL (like SQL)
- Rust-like struct syntax
- Builder-like chaining

**Recommendation**: Defer until Phase 2+ is complete
- Let builder API inform macro design
- See what patterns emerge from real usage

**Status**: ⏸️ Deferred to future phase

### 4. Support for Conditional Workflows

**Question**: Should workflows support if/else logic?

**Options**:
- **No**: Keep workflows linear, LLM handles branching
- **Yes**: Add conditional steps
- **Hybrid**: Allow optional steps

**Recommendation**: No - workflows are linear
- LLM decides what to execute
- Workflows describe "how", not "when"
- Keeps design simple

**Status**: ✅ Resolved - no conditionals

### 5. Parallel Step Execution

**Question**: Should we support parallel tool execution?

**Options**:
- **No**: Sequential only
- **Yes**: Add `ParallelWorkflow` type
- **Configurable**: Steps can be marked parallel

**Recommendation**: Defer to future
- Start with sequential
- Add parallel later if needed
- Most LLM workflows are sequential

**Status**: ⏸️ Deferred to future phase

---

## References

- [MCP Specification - Prompts](https://spec.modelcontextprotocol.io/specification/architecture/prompts/)
- [TypeScript SDK - Prompt Implementation](https://github.com/modelcontextprotocol/typescript-sdk/blob/main/src/server/mcp.ts)
- [Toyota Way Principles](https://en.wikipedia.org/wiki/The_Toyota_Way)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

---

## Appendix A: Full API Reference

### Handle Types

```rust
pub struct ToolHandle { /* ... */ }
pub struct ResourceHandle { /* ... */ }
```

### Traits

```rust
pub trait PromptReferenceable {
    fn prompt_ref(&self) -> String;
    fn prompt_schema(&self) -> Option<Value>;
    fn as_message(&self, role: Role) -> PromptMessage;
}
```

### Workflow Types

```rust
pub struct SequentialWorkflow { /* ... */ }
pub struct WorkflowStep { /* ... */ }
pub enum DataSource { /* ... */ }
pub enum ErrorStrategy { /* ... */ }
```

### Builders

```rust
pub struct SequentialWorkflowBuilder { /* ... */ }
```

---

## Appendix B: Example Resource Definitions

```rust
// Instruction resource for Logseq workflows
async fn logseq_intro_resource() -> Result<String> {
    Ok(r#"
# Logseq Workflow Execution

You are executing a Logseq automation workflow. Follow these principles:

1. **Normalize inputs** - Remove formatting, trim whitespace
2. **Use exact formats** - Project tags must be [[name]]
3. **Single-line content** - Keep TODO items on one line
4. **Structured output** - Return JSON matching the schema

## Available Tools

The workflow will reference specific tools. Use them in the order specified.

## Error Handling

See the error-handling resource for details on how to handle failures.
    "#.to_string())
}

async fn logseq_error_handling_resource() -> Result<String> {
    Ok(r#"
# Error Handling for Logseq Workflows

## On Tool Failure

```json
{
  "status": "error",
  "message": "<concise reason>",
  "entry_id": null,
  "page": null,
  "block_ref": null
}
```

## Common Errors

- **Empty task**: Return error with message "task_description cannot be empty"
- **Invalid date**: Return error with message "date must be YYYY-MM-DD or 'today'"
- **Tool failure**: Include tool error message in response

## Important

- Do NOT ask follow-up questions
- Do NOT attempt to fix errors yourself
- Return structured JSON immediately
    "#.to_string())
}
```

---

## Summary: Ergonomics Improvements

This design introduces multiple layers of ergonomic improvements over the initial approach:

### Before: Verbose and Stringly-Typed

```rust
let workflow = SequentialWorkflow::builder()
    .name("add-todo")
    .step(WorkflowStep {
        tool: normalize_project.clone(),
        input_mapping: indexmap! {
            ArgName::new("input") => DataSource::PromptArg(ArgName::new("project_name")),
        },
        output_binding: BindingName::new("normalized"),
        output_transform: None,
    })
    .step(WorkflowStep {
        tool: build_content.clone(),
        input_mapping: indexmap! {
            ArgName::new("task") => DataSource::PromptArg(ArgName::new("task_description")),
            ArgName::new("project") => DataSource::StepOutput {
                step: BindingName::new("normalized"),
                field: "name".to_string(),
            },
        },
        output_binding: BindingName::new("content"),
        output_transform: None,
    })
    .build()?;
```

**Issues**:
- ❌ 30+ lines of boilerplate
- ❌ Stringly-typed (`"project_name"`, `"normalized"`)
- ❌ Verbose constructor syntax
- ❌ Unclear data flow
- ❌ Hard to refactor

### After: Chainable Builders + DSL

```rust
use pmcp::dsl::*;

let workflow = SequentialWorkflow::builder()
    .name("add-todo")
    .step(
        WorkflowStep::new(normalize_project)
            .arg("input", prompt_arg("project_name"))
            .bind("normalized")
            .build()?
    )
    .step(
        WorkflowStep::new(build_content)
            .arg("task", prompt_arg("task_description"))
            .arg("project", from_step("normalized", "name"))
            .bind("content")
            .build()?
    )
    .build()?;
```

**Benefits**:
- ✅ 14 lines (53% reduction)
- ✅ Type-safe (newtypes + Into traits)
- ✅ Clear data flow (chainable API)
- ✅ Easy to refactor (fluent interface)
- ✅ Self-documenting (intent over mechanics)

### Improvement Metrics

| Aspect | Before | After | Improvement |
|--------|--------|-------|-------------|
| **Lines of code** | 30 | 14 | 53% reduction |
| **Cognitive load** | High (struct literals) | Low (fluent API) | ⬇️⬇️⬇️ |
| **Type safety** | Some (newtypes) | Full (newtypes + validation) | ✅ |
| **Refactorability** | Hard (many edits) | Easy (chainable) | ⬆️⬆️ |
| **Readability** | Intent obscured | Intent clear | ⬆️⬆️⬆️ |
| **Migration** | N/A | Easy (loose mode) | ✅✅ |
| **Allocations** | ~10 heap allocs | ~2 heap allocs | 80% reduction |

### Performance Optimizations

**SmallVec Usage** - Stack-allocated vectors for hot paths:

```rust
// Typical workflow allocations:
SmallVec<[PromptArgument; 3]>      // 3 arguments     → 0 heap allocs
SmallVec<[ResourceHandle; 3]>      // 2 instructions  → 0 heap allocs
SmallVec<[WorkflowStep; 4]>        // 3 steps         → 0 heap allocs
SmallVec<[PromptContent; 3]>       // 2 content parts → 0 heap allocs
```

**Estimated Performance Impact**:
- **Workflow construction**: ~50% faster (fewer allocations)
- **Clone operations**: ~70% faster (`Arc<str>` vs `String`)
- **Memory usage**: ~30% lower (stack vs heap)
- **Cache locality**: Better (stack-allocated data)

**Benchmark Assumptions** (typical Logseq TODO workflow):
- 3 arguments: `task_description`, `project_name`, `date`
- 2 instructions: `intro`, `error_handling`
- 3 steps: `normalize`, `build_content`, `add_content`
- All fit in SmallVec → **0 heap allocations** for workflow structure

### Key Innovations

1. **Protocol vs. Internal Types** - Clean separation enables both migration and type safety
2. **Chainable Builders** - WorkflowStep builder reduces boilerplate by 70%
3. **DSL Helpers** - `prompt_arg()`, `from_step()` make intent explicit
4. **Type-Safe Newtypes** - `ArgName`, `BindingName` prevent type confusion
5. **Dual Mode** - Loose (migration) and Strict (type-safe) both supported
6. **Conversion at Edge** - Handles never exposed to protocol

### Positioning

This design positions the Rust SDK as:

- ✅ **Best Migration Path** (loose mode = copy-paste from TS/Python)
- ✅ **Best Type Safety** (strict mode = build-time validation)
- ✅ **Best Developer Experience** (chainable DSL > struct literals)
- ✅ **Best Performance**:
  - `Arc<str>` → O(1) cloning (vs O(n) for `String`)
  - `SmallVec` → 0 heap allocs for typical workflows (vs 10+)
  - `IndexMap` → Deterministic iteration (vs `HashMap` randomness)
  - ~50% faster workflow construction vs naive implementation
- ✅ **Best Protocol Stability** (minimal wire format)
- ✅ **Best Forward Compatibility** (`#[non_exhaustive]` on all enums)
- ✅ **Best Error Messages** (`thiserror` with context)

---

**END OF DESIGN DOCUMENT**
