# Design: Dynamic Resource URI Interpolation in Workflows

**Status**: Implementation In Progress
**Author**: Claude Code
**Date**: 2025-11-02
**Related Issue**: Team feedback - Interactive Fiction MCP Server

## Problem Statement

The workflow DSL currently provides data source functions (`prompt_arg`, `from_step`, `field`, `constant`) for binding tool arguments, but **there is no way to construct dynamic resource URIs** that incorporate values from previous workflow steps or prompt arguments.

### Real-World Use Case

In the Interactive Fiction MCP server, a `get_hint` prompt workflow needs to:

1. Call `get_my_progress` tool → get current game info
2. Read resource at `if://walkthrough/{game_id}` where `{game_id}` comes from step 1's result
3. Return both progress and walkthrough to LLM

**Current workaround**: Abandon workflows entirely and implement custom `PromptHandler` with 100+ lines of boilerplate.

## Current Architecture Analysis

### Existing Components (that support this feature)

1. **Template Substitution Infrastructure** ✅
   - Location: `src/server/workflow/prompt_handler.rs:170-188`
   - Function: `substitute_arguments(template: &str, args: &HashMap<String, String>)`
   - Syntax: `{arg_name}` (curly braces)
   - Currently used for: guidance messages and constant string values

2. **DataSource Enum** ✅
   - Location: `src/server/workflow/data_source.rs`
   - Variants: `PromptArg`, `StepOutput { step, field }`, `Constant`
   - Already used for tool argument binding

3. **Flexible Type System** ✅
   - Uses `serde_json::Value` throughout
   - Can represent any JSON type
   - Already supports dynamic resolution at execution time

### Gap Identified

**Resource URIs are static strings** ❌
- `WorkflowStep::with_resource(uri: &str)` accepts only literal strings
- No mechanism to bind template variables
- No resolution of interpolation patterns during execution

## Proposed Solution

### Design Philosophy

**Extend existing patterns rather than introducing new concepts:**
- Use same `{var}` template syntax as guidance messages
- Leverage existing `DataSource` types for bindings
- Apply template substitution during resource resolution
- Maintain backwards compatibility with static URIs

### API Design

#### Option 1: Template Binding Method (Recommended)

```rust
use pmcp::server::workflow::dsl::*;

SequentialWorkflow::new("get_hint", "Get contextual hint for current game")
    // Step 1: Get user's current game progress
    .step(
        WorkflowStep::new("get_progress", ToolHandle::new("get_my_progress"))
            .bind("user_progress")
    )
    // Step 2: Dynamically fetch walkthrough resource based on game_id
    .step(
        WorkflowStep::new("read_walkthrough", ResourceHandle::new("if://walkthrough/{game_id}"))
            .with_template_binding("game_id", field("user_progress", "game_id"))
            .bind("walkthrough")
    )
    .finish()
```

**Key characteristics:**
- Resource URI contains template variables: `{game_id}`
- `.with_template_binding()` maps template variables to `DataSource`
- Resolution happens at execution time
- Backwards compatible: static URIs without bindings work unchanged

#### Usage Patterns

**Pattern 1: Field from Previous Step**
```rust
.step(get_user_info().bind("user"))
.step(
    read_resource("profile://user/{user_id}")
        .with_template_binding("user_id", field("user", "id"))
)
```

**Pattern 2: Prompt Argument**
```rust
.step(
    read_resource("dataset://samples/{dataset_id}")
        .with_template_binding("dataset_id", prompt_arg("dataset_id"))
)
```

**Pattern 3: Multiple Variables**
```rust
.step(
    read_resource("project://{org}/{repo}/config")
        .with_template_binding("org", field("project", "organization"))
        .with_template_binding("repo", field("project", "repository"))
)
```

**Pattern 4: Nested Field Access**
```rust
.step(
    read_resource("api://v1/{endpoint}")
        .with_template_binding("endpoint", field("config", "api.endpoint_name"))
)
```

## Implementation Plan

### 1. Extend WorkflowStep Structure

**File**: `src/server/workflow/workflow_step.rs`

Add template bindings support:

```rust
pub struct WorkflowStep {
    // ... existing fields ...

    /// Template variable bindings for resource URI interpolation
    template_bindings: HashMap<String, DataSource>,
}

impl WorkflowStep {
    pub fn new(name: impl Into<String>, handle: impl Into<Handle>) -> Self {
        Self {
            // ... existing initializations ...
            template_bindings: HashMap::new(),
        }
    }

    /// Bind a template variable to a data source for URI interpolation
    ///
    /// # Example
    /// ```
    /// WorkflowStep::new("read", ResourceHandle::new("docs://{doc_id}"))
    ///     .with_template_binding("doc_id", field("query_result", "document_id"))
    /// ```
    pub fn with_template_binding(
        mut self,
        var_name: impl Into<String>,
        source: DataSource,
    ) -> Self {
        self.template_bindings.insert(var_name.into(), source);
        self
    }
}
```

### 2. Add Template Resolution Helper

**File**: `src/server/workflow/prompt_handler.rs`

Add new method to resolve template bindings:

```rust
impl WorkflowPromptHandler {
    /// Resolve template bindings from execution context
    ///
    /// Takes template variables and their DataSource definitions,
    /// resolves them to actual values from the execution context.
    fn resolve_template_bindings(
        &self,
        bindings: &HashMap<String, DataSource>,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, String>> {
        let mut resolved = HashMap::new();

        for (var_name, data_source) in bindings {
            let value = self.resolve_data_source_to_string(data_source, args, ctx)?;
            resolved.insert(var_name.clone(), value);
        }

        Ok(resolved)
    }

    /// Resolve a DataSource to a string value
    fn resolve_data_source_to_string(
        &self,
        source: &DataSource,
        args: &HashMap<String, String>,
        ctx: &ExecutionContext,
    ) -> Result<String> {
        match source {
            DataSource::PromptArg(arg_name) => {
                args.get(arg_name.as_str())
                    .cloned()
                    .ok_or_else(|| Error::workflow(format!("Missing prompt argument: {}", arg_name)))
            },
            DataSource::StepOutput { step, field } => {
                let step_result = ctx.get_binding(step)
                    .ok_or_else(|| Error::workflow(format!("Step binding not found: {}", step)))?;

                if let Some(field_name) = field {
                    // Extract field from step result
                    self.extract_field_as_string(step_result, field_name)
                } else {
                    // Use entire step result
                    Ok(step_result.to_string())
                }
            },
            DataSource::Constant(value) => {
                Ok(value.to_string())
            },
        }
    }

    /// Extract a field from a JSON value and convert to string
    fn extract_field_as_string(&self, value: &Value, field_path: &str) -> Result<String> {
        // Support dot notation for nested fields: "user.profile.id"
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)
                .ok_or_else(|| Error::workflow(format!("Field not found: {}", field_path)))?;
        }

        // Convert to string based on type
        match current {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            _ => Ok(serde_json::to_string(current)?),
        }
    }

    /// Substitute template variables in a string
    /// Supports: {var_name} syntax
    fn substitute_template(
        template: &str,
        vars: &HashMap<String, String>,
    ) -> String {
        let mut result = template.to_string();
        for (key, value) in vars {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }
}
```

### 3. Modify Resource Fetching Logic

**File**: `src/server/workflow/prompt_handler.rs` (around lines 555-584)

Update resource fetching to support interpolation:

```rust
/// Fetch resources for a workflow step with template interpolation support
async fn fetch_resources_for_step(
    &self,
    step: &WorkflowStep,
    args: &HashMap<String, String>,
    ctx: &ExecutionContext,
) -> Result<Vec<UserMessage>> {
    let mut messages = Vec::new();

    // Resolve template bindings if any
    let template_vars = if !step.template_bindings.is_empty() {
        self.resolve_template_bindings(&step.template_bindings, args, ctx)?
    } else {
        HashMap::new()
    };

    // Fetch resources with interpolated URIs
    for resource_handle in &step.resources {
        let uri = resource_handle.uri();

        // Apply template substitution if needed
        let interpolated_uri = if !template_vars.is_empty() {
            Self::substitute_template(&uri, &template_vars)
        } else {
            uri.to_string()
        };

        // Fetch resource with interpolated URI
        let resource_result = self.server
            .read_resource(&interpolated_uri)
            .await?;

        // Convert resource to user message
        for content in resource_result.contents {
            messages.push(UserMessage {
                content: content.into(),
                role: Role::User,
            });
        }
    }

    Ok(messages)
}
```

### 4. Update WorkflowStep Serialization

**File**: `src/server/workflow/workflow_step.rs`

Ensure `template_bindings` is properly serialized/deserialized:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    name: String,
    handle: Handle,
    arguments: IndexMap<ArgName, DataSource>,
    resources: Vec<ResourceHandle>,
    guidance: Option<String>,
    binding: Option<BindingName>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    template_bindings: HashMap<String, DataSource>,
}
```

## Testing Strategy

### Unit Tests

**File**: `src/server/workflow/tests/dynamic_resources.rs` (new file)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::dsl::*;

    #[tokio::test]
    async fn test_template_binding_from_step_field() {
        // Test: Resource URI interpolated with field from previous step
        let workflow = SequentialWorkflow::new("test", "Test dynamic resource")
            .step(
                WorkflowStep::new("get_game", tool_handle)
                    .bind("game")
            )
            .step(
                WorkflowStep::new("read_guide", resource_handle)
                    .with_resource("guide://walkthrough/{game_id}")
                    .with_template_binding("game_id", field("game", "id"))
            );

        // Execute and verify interpolated URI is used
        // Assert that resource is fetched with "guide://walkthrough/123"
    }

    #[tokio::test]
    async fn test_template_binding_from_prompt_arg() {
        // Test: Resource URI interpolated with prompt argument
    }

    #[tokio::test]
    async fn test_multiple_template_variables() {
        // Test: "resource://{type}/{id}/data"
    }

    #[tokio::test]
    async fn test_nested_field_access() {
        // Test: field("result", "user.profile.id")
    }

    #[tokio::test]
    async fn test_missing_template_binding_error() {
        // Test: Error handling when template var not bound
    }

    #[tokio::test]
    async fn test_missing_step_binding_error() {
        // Test: Error handling when step binding not found
    }
}
```

### Integration Tests

**File**: `tests/workflow_dynamic_resources_integration.rs` (new file)

```rust
#[tokio::test]
async fn test_interactive_fiction_use_case() {
    // Full end-to-end test of the Interactive Fiction use case:
    // 1. Call get_my_progress tool
    // 2. Extract game_id
    // 3. Fetch if://walkthrough/{game_id}
    // 4. Return combined prompt
}
```

### Property Tests

```rust
#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_template_substitution_idempotent(
            template in "resource://[a-z]+/\\{var\\}",
            value in "[a-zA-Z0-9]+"
        ) {
            // Template substitution should be idempotent
            let vars = HashMap::from([("var".to_string(), value.clone())]);
            let result1 = substitute_template(&template, &vars);
            let result2 = substitute_template(&result1, &vars);
            assert_eq!(result1, result2);
        }
    }
}
```

## Example: Interactive Fiction Server

**File**: `examples/59_dynamic_resource_workflow.rs` (new file)

```rust
use pmcp::prelude::*;
use pmcp::server::workflow::dsl::*;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("interactive-fiction")
        .version("1.0.0")
        .capabilities(ServerCapabilities::prompts_only())

        // Add the get_hint workflow prompt
        .workflow_prompt(
            SequentialWorkflow::new("get_hint", "Get contextual hint for current game")
                // Step 1: Get user's current game progress
                .step(
                    WorkflowStep::new("get_progress", ToolHandle::new("get_my_progress"))
                        .with_guidance("I'll check your current game progress first...")
                        .bind("user_progress")
                )
                // Step 2: Fetch walkthrough resource dynamically
                .step(
                    WorkflowStep::new(
                        "read_walkthrough",
                        ResourceHandle::new("if://walkthrough/{game_id}")
                    )
                    .with_template_binding("game_id", field("user_progress", "game_id"))
                    .with_guidance("Now I'll fetch the walkthrough for your current game...")
                    .bind("walkthrough")
                )
                // Final: LLM receives both progress and walkthrough
                .finish()
        )

        .build()?;

    server.run_stdio().await
}
```

## Migration Path

### Backwards Compatibility

**Static resource URIs continue to work unchanged:**

```rust
// This still works exactly as before
.step(
    WorkflowStep::new("read", ResourceHandle::new("docs://static-page"))
)
```

**Opt-in to dynamic behavior:**

```rust
// Add template bindings only when you need dynamic URIs
.step(
    WorkflowStep::new("read", ResourceHandle::new("docs://{page_id}"))
        .with_template_binding("page_id", prompt_arg("page"))
)
```

### Existing Code Impact

- No changes required to existing workflows
- New field `template_bindings` defaults to empty HashMap
- Serialization skips empty template_bindings (no schema bloat)
- All existing tests continue to pass

## Benefits

1. **✅ Solves real-world use case** - Enables Interactive Fiction and AI Evals servers to use workflows
2. **✅ Consistent with existing patterns** - Uses same `{var}` syntax as guidance messages
3. **✅ Type-safe** - Compiler enforces correct DataSource usage
4. **✅ Composable** - Works with all DataSource variants
5. **✅ Educational value** - Clean, readable syntax for tutorials
6. **✅ Minimal API surface** - Single new method: `.with_template_binding()`
7. **✅ Performance** - No overhead for static URIs
8. **✅ Backwards compatible** - Zero impact on existing code

## Alternatives Considered

### Alternative 1: New DSL Function `format_binding()`

```rust
.arg("uri", format_binding(
    "if://walkthrough/{game_id}",
    vec![("game_id", field("user_progress", "game_id"))]
))
```

**Rejected because:**
- Requires new DataSource variant
- More complex API
- Doesn't reuse existing template substitution

### Alternative 2: Template String Syntax

```rust
.arg_template("uri", "if://walkthrough/${user_progress.game_id}")
```

**Rejected because:**
- Different syntax from guidance messages (`${}` vs `{}`)
- Harder to parse and validate
- Less explicit about bindings

### Alternative 3: Let LLM Make Resource Call

**Rejected because:**
- Extra round-trip latency
- Wastes LLM tokens
- Breaks educational flow
- Inconsistent with workflow philosophy

## Future Extensions

### Support for Conditional Resources

```rust
.step(
    read_resource("guide://beginner/{topic}")
        .with_template_binding("topic", field("user", "topic"))
        .when(field("user", "skill_level"), equals("beginner"))
)
```

### Support for Array Iteration

```rust
.step(
    read_resources("docs://{item_id}")
        .with_template_binding("item_id", from_step("items"))
        .for_each()  // Iterate over array
)
```

### Support for Transformations

```rust
.step(
    read_resource("api://v1/{endpoint}")
        .with_template_binding("endpoint",
            field("config", "endpoint").to_lowercase()
        )
)
```

## Implementation Checklist

- [ ] Add `template_bindings` field to `WorkflowStep`
- [ ] Implement `.with_template_binding()` method
- [ ] Add template resolution helpers in `prompt_handler.rs`
- [ ] Modify resource fetching to support interpolation
- [ ] Add unit tests for template binding
- [ ] Add integration tests for full workflow
- [ ] Add property tests for edge cases
- [ ] Create example: `59_dynamic_resource_workflow.rs`
- [ ] Update workflow documentation
- [ ] Add tutorial section on dynamic resources

## References

- **Issue Source**: Team feedback from Interactive Fiction MCP Server development
- **Related Code**:
  - `src/server/workflow/workflow_step.rs` - WorkflowStep structure
  - `src/server/workflow/prompt_handler.rs` - Workflow execution
  - `src/server/workflow/data_source.rs` - DataSource types
  - `examples/54_hybrid_workflow_execution.rs` - Similar pattern with guidance
