# Migration Guide: pmcp v1.x to v2.0

## Overview

pmcp v2.0 upgrades to MCP protocol version 2025-11-25 with a major type system
cleanup. This is a semver major release with breaking changes. The `protocol.rs`
monolith (2326 lines, 58 types) has been split into domain sub-modules, all
deprecated type aliases have been removed, and the elicitation API has been
replaced with the spec-compliant version.

**Protocol versions supported:** 2025-11-25, 2025-06-18, 2025-03-26

**Protocol versions dropped:** 2024-11-05, 2024-10-07

## Import Path Changes

All types previously accessed via `pmcp::types::protocol::` are now in domain
sub-modules with re-exports from `pmcp::types::`. The recommended import
pattern is:

```rust
// v2.0 (recommended)
use pmcp::types::{ToolInfo, Content, ResourceInfo};

// v1.x (no longer works)
use pmcp::types::protocol::{ToolInfo, Content, ResourceInfo};
```

### Find and Replace

| Search | Replace |
|--------|---------|
| `pmcp::types::protocol::` | `pmcp::types::` |
| `crate::types::protocol::` | `crate::types::` |

These are safe mechanical replacements. All types that were previously in
`protocol::` are re-exported from the flat `types::` path.

## Removed Type Aliases

These aliases have been removed. Use the canonical name instead.

| Removed Alias | Use Instead |
|---------------|-------------|
| `InitializeParams` | `InitializeRequest` |
| `ListToolsParams` | `ListToolsRequest` |
| `CallToolParams` | `CallToolRequest` |
| `ListPromptsParams` | `ListPromptsRequest` |
| `GetPromptParams` | `GetPromptRequest` |
| `ListResourcesParams` | `ListResourcesRequest` |
| `ReadResourceParams` | `ReadResourceRequest` |
| `CancelledParams` | `CancelledNotification` |
| `Progress` | `ProgressNotification` |
| `MessageContent` | `Content` |
| `CreateMessageRequest` | `CreateMessageParams` |

### Find and Replace

```
InitializeParams     -> InitializeRequest
ListToolsParams      -> ListToolsRequest
CallToolParams       -> CallToolRequest
ListPromptsParams    -> ListPromptsRequest
GetPromptParams      -> GetPromptRequest
ListResourcesParams  -> ListResourcesRequest
ReadResourceParams   -> ReadResourceRequest
CancelledParams      -> CancelledNotification
Progress             -> ProgressNotification  (careful: common word)
MessageContent       -> Content
CreateMessageRequest -> CreateMessageParams
```

## Renamed Enum Variants

| Type | Old Variant | New Variant | Wire Format |
|------|-------------|-------------|-------------|
| `IncludeContext` | `All` | `AllServers` | `"allServers"` |
| `IncludeContext` | `ThisServerOnly` | `ThisServer` | `"thisServer"` |

## Changed Field Types

| Struct | Field | Old Type | New Type |
|--------|-------|----------|----------|
| `ToolInfo` | `execution` | `Option<Value>` | `Option<ToolExecution>` |
| `LogMessageParams` | `level` | `LogLevel` | `LoggingLevel` |
| `SamplingMessage` | `content` | `Content` | `SamplingMessageContent` |

### LogLevel to LoggingLevel

`LogLevel` is available as a deprecated type alias during v2.0 transition.
`LoggingLevel` is the canonical enum with 8 syslog-level values:

```rust
// v1.x (4 values)
LogLevel::Debug | LogLevel::Info | LogLevel::Warning | LogLevel::Error

// v2.0 (8 values)
LoggingLevel::Debug | LoggingLevel::Info | LoggingLevel::Notice
| LoggingLevel::Warning | LoggingLevel::Error | LoggingLevel::Critical
| LoggingLevel::Alert | LoggingLevel::Emergency
```

If you pattern-match on `LogLevel`/`LoggingLevel`, add a wildcard arm or
handle the 4 new variants (`Notice`, `Critical`, `Alert`, `Emergency`).

### SamplingMessage Content

`SamplingMessage.content` changed from `Content` to `SamplingMessageContent`,
which is a separate enum supporting text, image, audio, tool_use, and
tool_result variants for multi-turn sampling interactions.

```rust
// v1.x
match &msg.content {
    Content::Text { text } => ...,
    Content::Image { .. } => ...,
    Content::Resource { .. } => ...,
}

// v2.0
match &msg.content {
    SamplingMessageContent::Text { text, .. } => ...,
    SamplingMessageContent::Image { .. } => ...,
    SamplingMessageContent::Audio { .. } => ...,
    SamplingMessageContent::ToolUse { .. } => ...,
    SamplingMessageContent::ToolResult { .. } => ...,
}
```

## New Required Fields (on existing structs)

These structs gained new `Option<T>` fields. If you construct them with struct
literal syntax, add the new fields:

### `Implementation`

```rust
// v1.x
Implementation { name: "my-server".into(), version: "1.0".into() }

// v2.0 (option A: use constructor)
Implementation::new("my-server", "1.0")

// v2.0 (option B: struct literal with new fields)
Implementation {
    name: "my-server".into(),
    title: None,
    version: "1.0".into(),
    website_url: None,
    description: None,
    icons: None,
}
```

**Recommendation:** Use `Implementation::new(name, version)` -- it defaults
the new fields to `None`.

### `ResourceInfo`

```rust
// v2.0 -- add these fields
ResourceInfo {
    uri: "...".into(),
    name: "...".into(),
    title: None,         // NEW
    description: Some("...".into()),
    mime_type: Some("text/plain".into()),
    icons: None,         // NEW
    annotations: None,   // NEW
    meta: None,
}
```

### `ResourceTemplate`

```rust
// v2.0 -- add these fields
ResourceTemplate {
    uri_template: "...".into(),
    name: "...".into(),
    title: None,         // NEW
    description: Some("...".into()),
    mime_type: Some("...".into()),
    icons: None,         // NEW
    annotations: None,   // NEW
    meta: None,          // NEW
}
```

### `PromptInfo`

```rust
// v2.0 -- add these fields
PromptInfo {
    name: "...".into(),
    title: None,         // NEW
    description: Some("...".into()),
    arguments: None,
    icons: None,         // NEW
    meta: None,          // NEW
}
```

### `ToolInfo`

```rust
// v2.0 -- add these fields to struct literals
// title: None,  // NEW
// icons: None,  // NEW
```

### `ClientCapabilities`

```rust
// v2.0 -- add tasks field
ClientCapabilities {
    sampling: ...,
    elicitation: ...,
    roots: ...,
    tasks: None,          // NEW
    experimental: None,
}
```

## Content Enum: New Variants

The `Content` enum gained two new variants. If you pattern-match on `Content`,
add arms for:

```rust
match content {
    Content::Text { text } => ...,
    Content::Image { data, mime_type } => ...,
    Content::Resource { uri, text, mime_type, meta } => ...,
    // NEW in v2.0:
    Content::Audio { data, mime_type, annotations, meta } => ...,
    Content::ResourceLink { name, uri, title, description, mime_type, icons, annotations, meta } => ...,
}
```

Or add a wildcard: `_ => { /* handle unknown content */ }`

## Elicitation API (Complete Replacement)

The elicitation API has been completely rewritten to match MCP spec 2025-11-25.

| v1.x (PMCP Proprietary) | v2.0 (MCP Spec) |
|--------------------------|-----------------|
| `ElicitInputRequest` | `ElicitRequestParams` (form or url mode) |
| `ElicitInputResponse` | `ElicitResult` (accept/decline/cancel) |
| `InputType` (16 variants) | JSON Schema `requested_schema` |
| `InputValidation` | Standard JSON Schema validation |
| `SelectOption` | JSON Schema enum type |
| `ElicitInputBuilder` | Construct `ElicitRequestParams` directly |
| `elicit_text()`, `elicit_boolean()`, etc. | JSON Schema properties |
| `ServerRequest::ElicitInput` | `ServerRequest::ElicitationCreate` |

### Method Name Change

```
// v1.x wire format
{"method": "elicitation/elicitInput", ...}

// v2.0 wire format
{"method": "elicitation/create", ...}
```

### Example Migration

```rust
// v1.x
use pmcp::types::elicitation::{elicit_text, elicit_boolean, SelectOption};
let response = ctx.elicit_input(
    elicit_text("Project name?").required().build()
).await?;
if response.cancelled { return ...; }
let name = response.value.unwrap();

// v2.0
use pmcp::types::elicitation::{ElicitRequestParams, ElicitResult, ElicitAction};
let response = ctx.elicit_input(ElicitRequestParams::Form {
    message: "Configure your project".into(),
    requested_schema: json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Project name" }
        },
        "required": ["name"]
    }),
}).await?;
match response.action {
    ElicitAction::Accept => {
        let name = response.content.unwrap()["name"].as_str().unwrap();
    },
    ElicitAction::Decline | ElicitAction::Cancel => return ...,
}
```

## Sampling API Changes

### SamplingMessage Content Expansion

`SamplingMessage.content` changed from `Content` to `SamplingMessageContent`,
which is an enum supporting text, image, audio, tool_use, and tool_result
variants.

### New Type: CreateMessageResultWithTools

`CreateMessageResultWithTools` extends `CreateMessageResult` with array content
that can include tool use and tool result items for multi-turn interactions.

### CreateMessageParams Expansion

`CreateMessageParams` gained two new optional fields:
- `tools: Option<Vec<ToolInfo>>` -- tool definitions for the model
- `tool_choice: Option<ToolChoice>` -- tool selection mode

## Version Negotiation

| Constant | v1.x | v2.0 |
|----------|------|------|
| `LATEST_PROTOCOL_VERSION` | `"2025-06-18"` | `"2025-11-25"` |
| `SUPPORTED_PROTOCOL_VERSIONS` | 4 versions (incl. 2024) | 3 versions (2025 only) |

Unsupported versions now return `LATEST_PROTOCOL_VERSION` instead of
`DEFAULT_PROTOCOL_VERSION`. Clients sending 2024 protocol versions will receive
2025-11-25 as the negotiated version (they should reject or upgrade).

## New Types (2025-11-25)

### Content
- `Content::Audio` -- audio content with base64 data and MIME type
- `Content::ResourceLink` -- resource link with metadata (name, uri, title, icons)
- `Annotations` -- content annotation metadata (audience, priority, lastModified)

### Tasks
- `Task`, `TaskStatus`, `TaskCreationParams`, `RelatedTaskMetadata`
- `CreateTaskResult`, `GetTaskRequest`, `GetTaskResult`
- `GetTaskPayloadRequest`, `ListTasksRequest`, `ListTasksResult`
- `CancelTaskRequest`, `CancelTaskResult`, `TaskStatusNotification`
- `RELATED_TASK_META_KEY` constant

### Tools
- `ToolExecution` -- typed execution metadata (replaces `Option<Value>`)
- `TaskSupport` -- enum: `Required`, `Optional`, `Forbidden`

### Sampling
- `ToolChoice`, `ToolChoiceMode` -- tool selection for sampling
- `ToolUseContent`, `ToolResultContent` -- tool use in sampling messages
- `SamplingMessageContent` -- expanded content union for sampling
- `CreateMessageResultWithTools` -- sampling result with tool use array content

### Protocol
- `IconInfo`, `IconTheme` -- icon metadata for entities
- `ProtocolErrorCode` -- MCP-specific JSON-RPC error codes

### Elicitation
- `ElicitRequestParams` (form/url modes)
- `ElicitResult`, `ElicitAction`
- `ElicitationCompleteNotification`

### Capabilities
- `ServerTasksCapability`, `ClientTasksCapability`
- `FormElicitationCapability`
- Expanded `ElicitationCapabilities` (form + url)
- Expanded `SamplingCapabilities` (context + tools)

### Logging
- `LoggingLevel` expanded: added `Notice`, `Alert`, `Emergency`

## Module Structure (v2.0)

Types are organized by MCP domain, all re-exported from `pmcp::types::`:

```
src/types/
  mod.rs              # Re-exports everything for flat access
  protocol/
    mod.rs            # Init, Implementation, ProtocolVersion, Request enums
    version.rs        # Version constants, negotiation
  tools.rs            # ToolInfo, CallToolRequest/Result, ToolExecution
  resources.rs        # ResourceInfo, ResourceTemplate, ReadResourceResult
  prompts.rs          # PromptInfo, PromptArgument, GetPromptResult
  tasks.rs            # Task, TaskStatus, Get/List/Cancel requests
  content.rs          # Content enum, Annotations, Role
  sampling.rs         # SamplingMessage, CreateMessageParams/Result
  notifications.rs    # ServerNotification, ClientNotification, LoggingLevel
  capabilities.rs     # Server/ClientCapabilities, all sub-capabilities
  elicitation.rs      # ElicitRequestParams, ElicitResult (spec-compliant)
  completable.rs      # CompletionReference, CompleteRequest/Result
```

## Quick Migration Checklist

- [ ] Replace `pmcp::types::protocol::` with `pmcp::types::` everywhere
- [ ] Replace `crate::types::protocol::` with `crate::types::` in library code
- [ ] Replace removed type aliases with canonical names (see table above)
- [ ] Add `title: None, icons: None, annotations: None` to `ResourceInfo` struct literals
- [ ] Add `title: None, icons: None, meta: None` to `PromptInfo` struct literals
- [ ] Add `title: None, icons: None, annotations: None, meta: None` to `ResourceTemplate` struct literals
- [ ] Use `Implementation::new(name, version)` instead of struct literals
- [ ] Add `tasks: None` to `ClientCapabilities` struct literals
- [ ] Add wildcard or new arms to `Content` pattern matches (Audio, ResourceLink)
- [ ] Add wildcard or new arms to `LoggingLevel` pattern matches (Notice, Alert, Emergency, Critical)
- [ ] Update `SamplingMessage.content` usage from `Content` to `SamplingMessageContent`
- [ ] Rewrite elicitation code using `ElicitRequestParams` / `ElicitResult`
- [ ] Update protocol version references from 2024 to 2025

## Construction DX (v2.0)

All protocol structs now use the uniform construction pattern:
`#[non_exhaustive]` + `Default` + `::new(required_fields)` + `.with_*(optional_field)`

This means:
- **Struct literal syntax from outside the crate no longer works** (due to `#[non_exhaustive]`)
- **New optional fields in future versions won't break your code** (constructor defaults them to `None`)
- **Upgrades are painless** -- just bump the version, constructors handle new fields

### Before (v1.x)

```rust
// Painful -- every field must be specified, breaks on every SDK update
let resource = ResourceInfo {
    uri: "file://test.txt".to_string(),
    name: "test.txt".to_string(),
    title: None,
    description: Some("A test file".to_string()),
    mime_type: Some("text/plain".to_string()),
    icons: None,
    annotations: None,
    meta: None,
};

let prompt = PromptInfo {
    name: "greet".to_string(),
    title: None,
    description: Some("A greeting prompt".to_string()),
    arguments: Some(vec![PromptArgument {
        name: "name".to_string(),
        description: Some("Name to greet".to_string()),
        required: true,
        completion: None,
        arg_type: None,
    }]),
    icons: None,
    meta: None,
};

let content = Content::Text { text: "Hello".to_string() };
```

### After (v2.0)

```rust
// Clean -- only specify what matters, future-proof
let resource = ResourceInfo::new("file://test.txt", "test.txt")
    .with_description("A test file")
    .with_mime_type("text/plain");

let prompt = PromptInfo::new("greet")
    .with_description("A greeting prompt")
    .with_arguments(vec![
        PromptArgument::new("name")
            .with_description("Name to greet")
            .required(),
    ]);

let content = Content::text("Hello");
```

### Content Enum Helpers

| Helper | Creates |
|--------|---------|
| `Content::text("hello")` | `Content::Text { text: "hello" }` |
| `Content::image(data, mime)` | `Content::Image { data, mime_type }` |
| `Content::resource(uri)` | `Content::Resource { uri, text: None, mime_type: None, meta: None }` |
| `Content::resource_with_text(uri, text, mime)` | `Content::Resource { uri, text, mime_type, meta: None }` |
| `Content::audio(data, mime)` | `Content::Audio { data, mime_type, annotations: None, meta: None }` |
| `Content::resource_link(name, uri)` | `Content::ResourceLink(Box::new(ResourceLinkContent::new(name, uri)))` |

### Task Type Helpers (for Phase 55+)

```rust
let task = Task::new("t-123", TaskStatus::Working)
    .with_timestamps("2025-01-01T00:00:00Z", "2025-01-01T00:01:00Z")
    .with_ttl(60000)
    .with_poll_interval(5000)
    .with_status_message("Processing...");
```

### Types Affected

Every protocol struct now has this pattern. Key types:
`ResourceInfo`, `ResourceTemplate`, `PromptInfo`, `PromptArgument`, `PromptMessage`,
`Implementation`, `IconInfo`, `InitializeResult`, `Task`, `CreateMessageParams`,
`ProgressNotification`, `CancelledNotification`, `LogMessageParams`, `CallToolResult`,
`ToolExecution`, `CallToolRequest`, `Annotations`, `ResourceLinkContent`
