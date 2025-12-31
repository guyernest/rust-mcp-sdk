::: exercise
id: ch06-02-workflow-validation
difficulty: intermediate
time: 30 minutes
:::

Your team is building a code review workflow that automates the analysis, review,
and formatting pipeline. The workflow needs to execute deterministically on the
server side, binding data between steps automatically.

Your task is to build a hard workflow using `SequentialWorkflow`, validate it
with tests, and verify it using `cargo pmcp validate`.

::: objectives
thinking:
  - Understand the difference between soft prompts and hard workflows
  - Learn how binding names connect steps together
  - Recognize validation errors and their causes
doing:
  - Build a SequentialWorkflow with multiple steps
  - Write validation tests that catch structural errors
  - Use `cargo pmcp validate workflows` for project-wide validation
  - Fix binding errors discovered through validation
:::

::: discussion
- Why are hard workflows preferable when steps are deterministic?
- What's the difference between a step name and a binding name?
- When would you add `.with_guidance()` to create a hybrid workflow?
:::

::: starter file="workflow.rs"
```rust
//! Code Review Workflow Exercise
//!
//! Build a hard workflow that:
//! 1. Analyzes code for complexity and issues
//! 2. Reviews the analysis and produces recommendations
//! 3. Formats the results with inline annotations

use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use pmcp::server::workflow::dsl::*;
use serde_json::json;

/// Create the code review workflow
///
/// Requirements:
/// - Accept `code` (required) and `language` (optional) arguments
/// - Step 1: analyze_code -> bind to "analysis_result"
/// - Step 2: review_code using analysis_result -> bind to "review_result"
/// - Step 3: format_results using code + review_result -> bind to "formatted_output"
pub fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review",
        "Comprehensive code review with analysis and formatting"
    )
    // TODO: Add arguments
    // .argument("code", "Source code to review", true)
    // .argument("language", "Programming language", false)

    // TODO: Add Step 1 - analyze_code
    // Tip: Use prompt_arg("code") and prompt_arg("language")
    // Don't forget to .bind("analysis_result")

    // TODO: Add Step 2 - review_code
    // Tip: Use field("analysis_result", "summary") to extract a specific field
    // Or use from_step("analysis_result") for the entire output

    // TODO: Add Step 3 - format_results
    // Tip: You can reference multiple previous bindings
    // Use prompt_arg("code") and from_step("review_result")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_validates() {
        let workflow = create_code_review_workflow();

        // This should NOT panic if your workflow is valid
        workflow.validate().expect("Workflow should be valid");
    }

    #[test]
    fn test_workflow_has_expected_structure() {
        let workflow = create_code_review_workflow();

        assert_eq!(workflow.name(), "code_review");
        assert_eq!(workflow.steps().len(), 3);
    }

    #[test]
    fn test_workflow_bindings() {
        let workflow = create_code_review_workflow();

        let bindings = workflow.output_bindings();

        // Verify all expected bindings exist
        assert!(bindings.contains(&"analysis_result".into()),
            "Missing analysis_result binding");
        assert!(bindings.contains(&"review_result".into()),
            "Missing review_result binding");
        assert!(bindings.contains(&"formatted_output".into()),
            "Missing formatted_output binding");
    }

    #[test]
    fn test_workflow_detects_unknown_binding() {
        // This test demonstrates what happens with an invalid binding
        let bad_workflow = SequentialWorkflow::new("bad", "Invalid workflow")
            .argument("input", "Some input", true)
            .step(
                WorkflowStep::new("step1", ToolHandle::new("tool1"))
                    .arg("x", prompt_arg("input"))
                    .bind("result")
            )
            .step(
                WorkflowStep::new("step2", ToolHandle::new("tool2"))
                    // ERROR: "wrong_name" doesn't exist!
                    .arg("y", from_step("wrong_name"))
                    .bind("final")
            );

        let result = bad_workflow.validate();
        assert!(result.is_err(), "Should fail validation");

        let err = result.unwrap_err();
        assert!(err.to_string().contains("wrong_name") ||
                err.to_string().contains("UnknownBinding"),
            "Error should mention the bad binding name");
    }
}
```
:::

::: hint level=1 title="Workflow structure template"
Start with the basic structure:
```rust
SequentialWorkflow::new("code_review", "Description")
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language", false)
    .step(
        WorkflowStep::new("step_name", ToolHandle::new("tool_name"))
            .arg("param", /* source */)
            .bind("binding_name")
    )
```

Remember: you reference BINDING names in `from_step()`, not step names!
:::

::: hint level=2 title="DSL helper functions"
Four ways to source argument values:
```rust
// From workflow arguments (user provides)
.arg("code", prompt_arg("code"))

// From previous step's entire output
.arg("data", from_step("analysis_result"))

// From specific field of previous step
.arg("summary", field("analysis_result", "summary"))

// Constant value
.arg("format", constant(json!("markdown")))
```
:::

::: hint level=3 title="Common validation error"
If you see "UnknownBinding" error, check:

1. **Binding name mismatch**: `.bind("analysis_result")` but `from_step("analysis")`
2. **Step vs binding confusion**: Step is `"analyze"`, binding is `"analysis_result"`
3. **Typos**: `"analysis_result"` vs `"analyis_result"`

The workflow validator shows available bindings in error messages.
:::

::: solution
```rust
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use pmcp::server::workflow::dsl::*;
use serde_json::json;

pub fn create_code_review_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "code_review",
        "Comprehensive code review with analysis and formatting"
    )
    // Declare workflow arguments
    .argument("code", "Source code to review", true)
    .argument("language", "Programming language (default: rust)", false)

    // Step 1: Analyze the code
    .step(
        WorkflowStep::new("analyze", ToolHandle::new("analyze_code"))
            .arg("code", prompt_arg("code"))
            .arg("language", prompt_arg("language"))
            .bind("analysis_result")  // Other steps reference this binding name
    )

    // Step 2: Review based on analysis
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_code"))
            // Use field() to extract specific part of previous output
            .arg("analysis", field("analysis_result", "summary"))
            // Use constant() for fixed values
            .arg("focus", constant(json!(["security", "performance"])))
            .bind("review_result")
    )

    // Step 3: Format results with annotations
    .step(
        WorkflowStep::new("format", ToolHandle::new("format_results"))
            // Can reference workflow args AND previous steps
            .arg("code", prompt_arg("code"))
            // Use from_step() for entire previous output
            .arg("recommendations", from_step("review_result"))
            .bind("formatted_output")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_validates() {
        let workflow = create_code_review_workflow();
        workflow.validate().expect("Workflow should be valid");
    }

    #[test]
    fn test_workflow_has_expected_structure() {
        let workflow = create_code_review_workflow();

        assert_eq!(workflow.name(), "code_review");
        assert_eq!(workflow.steps().len(), 3);

        // Check step order
        let steps = workflow.steps();
        assert_eq!(steps[0].name(), "analyze");
        assert_eq!(steps[1].name(), "review");
        assert_eq!(steps[2].name(), "format");
    }

    #[test]
    fn test_workflow_bindings() {
        let workflow = create_code_review_workflow();
        let bindings = workflow.output_bindings();

        assert!(bindings.contains(&"analysis_result".into()));
        assert!(bindings.contains(&"review_result".into()));
        assert!(bindings.contains(&"formatted_output".into()));
    }

    #[test]
    fn test_workflow_arguments() {
        let workflow = create_code_review_workflow();
        let args = workflow.arguments();

        // code is required
        let code_arg = args.iter().find(|a| a.name == "code").unwrap();
        assert!(code_arg.required);

        // language is optional
        let lang_arg = args.iter().find(|a| a.name == "language").unwrap();
        assert!(!lang_arg.required);
    }
}
```

**Running validation:**

```bash
# Run tests to validate workflow structure
cargo test workflow

# Use cargo pmcp for project-wide validation
cargo pmcp validate workflows

# Generate test scaffolding if starting fresh
cargo pmcp validate workflows --generate
```

**Example output:**

```
🔍 PMCP Workflow Validation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 1: Checking compilation...
  ✓ Compilation successful

Step 2: Looking for workflow validation tests...
  ✓ Found 1 workflow test pattern(s)

Step 3: Running workflow validation tests...
  ✓ Pattern 'workflow': 4 passed

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ All 4 workflow validation tests passed!

  Your workflows are structurally valid and ready for use.
```

**Key takeaways:**

1. **Bindings connect steps** - Use descriptive binding names like `analysis_result`
2. **Reference bindings, not step names** - `from_step("analysis_result")` not `from_step("analyze")`
3. **Validation is automatic** - `.prompt_workflow()` validates at registration
4. **Tests catch errors early** - Write unit tests with `workflow.validate()`
5. **CLI validates projects** - `cargo pmcp validate workflows` for CI/pre-commit
:::

::: tests mode=local
```rust
#[cfg(test)]
mod exercise_tests {
    use super::*;

    #[test]
    fn workflow_compiles_and_validates() {
        let workflow = create_code_review_workflow();
        assert!(workflow.validate().is_ok());
    }

    #[test]
    fn workflow_has_three_steps() {
        let workflow = create_code_review_workflow();
        assert_eq!(workflow.steps().len(), 3);
    }

    #[test]
    fn workflow_has_required_code_argument() {
        let workflow = create_code_review_workflow();
        let code_arg = workflow.arguments().iter()
            .find(|a| a.name == "code")
            .expect("Should have code argument");
        assert!(code_arg.required);
    }

    #[test]
    fn workflow_has_all_bindings() {
        let workflow = create_code_review_workflow();
        let bindings = workflow.output_bindings();

        assert!(bindings.contains(&"analysis_result".into()));
        assert!(bindings.contains(&"review_result".into()));
        assert!(bindings.contains(&"formatted_output".into()));
    }
}
```
:::

::: reflection
- How would you extend this workflow with error handling steps?
- When would you convert this to a hybrid workflow with `.with_guidance()`?
- How does `cargo pmcp validate` fit into your CI/CD pipeline?
- What other MCP components could benefit from similar validation patterns?
:::

## Related Examples

For more workflow patterns and variations, explore these SDK examples:

- **[50_workflow_minimal.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/50_workflow_minimal.rs)** - Quadratic formula solver workflow demonstrating DSL helpers
- **[51_workflow_error_messages.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/51_workflow_error_messages.rs)** - Workflow error handling patterns
- **[52_workflow_dsl_cookbook.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/52_workflow_dsl_cookbook.rs)** - Comprehensive DSL cookbook with many patterns
- **[54_hybrid_workflow_execution.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/54_hybrid_workflow_execution.rs)** - Hybrid workflows with AI guidance

Run locally with:
```bash
cargo run --example 50_workflow_minimal
cargo run --example 52_workflow_dsl_cookbook
```
