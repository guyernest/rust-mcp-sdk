//! Workflow validation error messages example
//!
//! This example demonstrates:
//! - Common workflow validation errors
//! - Actionable error messages
//! - How to diagnose and fix workflow issues
//!
//! Error cases covered:
//! 1. Unknown binding - referencing a binding that doesn't exist
//! 2. Undefined prompt argument - using an argument that wasn't declared
//! 3. Step without .bind() - trying to reference output of unbound step

use pmcp::server::workflow::{
    dsl::{from_step, prompt_arg},
    SequentialWorkflow, ToolHandle, WorkflowStep,
};

fn main() {
    println!("=== Workflow Validation Error Messages ===\n");
    println!("This example demonstrates common workflow errors and their messages.\n");

    // Error Case 1: Unknown Binding
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("❌ Error Case 1: Unknown Binding");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let workflow_unknown_binding = SequentialWorkflow::new(
        "broken_workflow_1",
        "Workflow with unknown binding reference",
    )
    .argument("topic", "The topic to write about", true)
    .step(
        WorkflowStep::new("create", ToolHandle::new("create_content"))
            .arg("topic", prompt_arg("topic"))
            .bind("content"), // ← Binding is "content"
    )
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_content"))
            .arg("text", from_step("draft")), // ← ERROR: References "draft" but binding is "content"
    );

    println!("Problem:");
    println!("  Step 'review' references binding 'draft' which doesn't exist.");
    println!("  The previous step binds to 'content', not 'draft'.\n");

    println!("Code:");
    println!("  .step(");
    println!("      WorkflowStep::new(\"create\", ToolHandle::new(\"create_content\"))");
    println!("          .bind(\"content\")  // ← Binds as 'content'");
    println!("  )");
    println!("  .step(");
    println!("      WorkflowStep::new(\"review\", ToolHandle::new(\"review_content\"))");
    println!("          .arg(\"text\", from_step(\"draft\"))  // ← ERROR: 'draft' doesn't exist");
    println!("  )\n");

    match workflow_unknown_binding.validate() {
        Ok(_) => println!("  ✓ Validation passed (unexpected!)"),
        Err(e) => {
            println!("Error Message:");
            println!("  {}\n", e);

            println!("Fix:");
            println!("  Change from_step(\"draft\") to from_step(\"content\")");
            println!("  OR change .bind(\"content\") to .bind(\"draft\")\n");
        },
    }

    // Error Case 2: Undefined Prompt Argument
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("❌ Error Case 2: Undefined Prompt Argument");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let workflow_undefined_arg = SequentialWorkflow::new(
        "broken_workflow_2",
        "Workflow with undefined prompt argument",
    )
    .argument("topic", "The topic to write about", true)
    // Note: We define "topic" but not "style"
    .step(
        WorkflowStep::new("create", ToolHandle::new("create_content"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", prompt_arg("writing_style")) // ← ERROR: "writing_style" not defined
            .bind("content"),
    );

    println!("Problem:");
    println!("  Step 'create' uses prompt_arg(\"writing_style\") but the workflow");
    println!("  only defines argument 'topic'. 'writing_style' was never declared.\n");

    println!("Code:");
    println!("  SequentialWorkflow::new(...)");
    println!("      .argument(\"topic\", \"The topic to write about\", true)");
    println!("      // ← Missing: .argument(\"writing_style\", ..., ...)");
    println!("      .step(");
    println!("          WorkflowStep::new(\"create\", ...)");
    println!("              .arg(\"style\", prompt_arg(\"writing_style\"))  // ← ERROR");
    println!("      )\n");

    match workflow_undefined_arg.validate() {
        Ok(_) => println!("  ✓ Validation passed (unexpected!)"),
        Err(e) => {
            println!("Error Message:");
            println!("  {}\n", e);

            println!("Fix:");
            println!("  Add .argument(\"writing_style\", \"Writing style\", false)");
            println!("  before the .step() call.\n");
        },
    }

    // Error Case 3: Step Without .bind() Cannot Be Referenced
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("❌ Error Case 3: Step Without .bind() Cannot Be Referenced");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let workflow_no_binding = SequentialWorkflow::new(
        "broken_workflow_3",
        "Workflow with step that has no binding",
    )
    .argument("topic", "The topic", true)
    .step(
        WorkflowStep::new("create", ToolHandle::new("create_content"))
            .arg("topic", prompt_arg("topic")),
        // ← ERROR: No .bind() call - output cannot be referenced!
    )
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_content"))
            .arg("text", from_step("create")), // ← ERROR: "create" has no binding
    );

    println!("Problem:");
    println!("  Step 'create' doesn't have a .bind() call, so its output");
    println!("  cannot be referenced by later steps. Step 'review' tries to");
    println!("  reference it anyway.\n");

    println!("Code:");
    println!("  .step(");
    println!("      WorkflowStep::new(\"create\", ToolHandle::new(\"create_content\"))");
    println!("          .arg(\"topic\", prompt_arg(\"topic\"))");
    println!("          // ← Missing: .bind(\"content\")");
    println!("  )");
    println!("  .step(");
    println!("      WorkflowStep::new(\"review\", ToolHandle::new(\"review_content\"))");
    println!("          .arg(\"text\", from_step(\"create\"))  // ← ERROR");
    println!("  )\n");

    match workflow_no_binding.validate() {
        Ok(_) => println!("  ✓ Validation passed (unexpected!)"),
        Err(e) => {
            println!("Error Message:");
            println!("  {}\n", e);

            println!("Fix:");
            println!("  Add .bind(\"content\") to the first step:");
            println!("    WorkflowStep::new(\"create\", ...)");
            println!("        .arg(\"topic\", prompt_arg(\"topic\"))");
            println!("        .bind(\"content\")  // ← Add this");
            println!();
            println!("  Then update the reference:");
            println!("    .arg(\"text\", from_step(\"content\"))  // ← Use binding name\n");
        },
    }

    // Bonus: Multiple Errors
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("❌ Bonus: Workflow with Multiple Errors");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let workflow_multiple_errors = SequentialWorkflow::new(
        "broken_workflow_4",
        "Workflow with multiple validation errors",
    )
    .argument("topic", "The topic", true)
    // Missing: .argument("style", ...)
    .step(
        WorkflowStep::new("create", ToolHandle::new("create_content"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", prompt_arg("writing_style")) // ← ERROR 1: undefined arg
            .bind("content"),
    )
    .step(
        WorkflowStep::new("enhance", ToolHandle::new("enhance_content"))
            .arg("text", from_step("content")),
        // ← ERROR 2: No .bind() - can't be referenced later
    )
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_content"))
            .arg("text", from_step("enhanced")), // ← ERROR 3: wrong binding name
    );

    println!("Problem:");
    println!("  This workflow has THREE errors:");
    println!("  1. References undefined prompt argument 'writing_style'");
    println!("  2. Step 'enhance' has no .bind() but is referenced later");
    println!("  3. Step 'review' references 'enhanced' instead of 'content'\n");

    match workflow_multiple_errors.validate() {
        Ok(_) => println!("  ✓ Validation passed (unexpected!)"),
        Err(e) => {
            println!("Error Message:");
            println!("  {}\n", e);
            println!("Note: Validation stops at the first error encountered.");
            println!("Fix errors one at a time and re-validate.\n");
        },
    }

    // Success Case: Properly Validated Workflow
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Success Case: Properly Validated Workflow");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let workflow_correct = SequentialWorkflow::new(
        "correct_workflow",
        "A properly constructed workflow",
    )
    .argument("topic", "The topic to write about", true)
    .argument("style", "Writing style", false) // ✓ All args defined
    .step(
        WorkflowStep::new("create", ToolHandle::new("create_content"))
            .arg("topic", prompt_arg("topic"))
            .arg("style", prompt_arg("style"))
            .bind("content"), // ✓ Output is bound
    )
    .step(
        WorkflowStep::new("review", ToolHandle::new("review_content"))
            .arg("text", from_step("content")) // ✓ References existing binding
            .bind("review_result"), // ✓ Output is bound
    )
    .step(
        WorkflowStep::new("publish", ToolHandle::new("publish_content"))
            .arg("content", from_step("content")) // ✓ References existing binding
            .arg("review", from_step("review_result")), // ✓ References existing binding
                                                         // No .bind() is OK if output not needed
    );

    println!("Code:");
    println!("  SequentialWorkflow::new(...)");
    println!("      .argument(\"topic\", \"The topic to write about\", true)");
    println!("      .argument(\"style\", \"Writing style\", false)");
    println!("      .step(");
    println!("          WorkflowStep::new(\"create\", ToolHandle::new(\"create_content\"))");
    println!("              .arg(\"topic\", prompt_arg(\"topic\"))");
    println!("              .arg(\"style\", prompt_arg(\"style\"))");
    println!("              .bind(\"content\")");
    println!("      )");
    println!("      .step(");
    println!("          WorkflowStep::new(\"review\", ToolHandle::new(\"review_content\"))");
    println!("              .arg(\"text\", from_step(\"content\"))");
    println!("              .bind(\"review_result\")");
    println!("      )");
    println!("      .step(");
    println!("          WorkflowStep::new(\"publish\", ToolHandle::new(\"publish_content\"))");
    println!("              .arg(\"content\", from_step(\"content\"))");
    println!("              .arg(\"review\", from_step(\"review_result\"))");
    println!("      )\n");

    match workflow_correct.validate() {
        Ok(_) => {
            println!("✅ Validation passed!\n");
            println!(
                "Output bindings: {:?}\n",
                workflow_correct.output_bindings()
            );
        },
        Err(e) => println!("  ✗ Validation failed: {}\n", e),
    }

    // Summary
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Summary: Common Workflow Validation Errors");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("1. UnknownBinding:");
    println!("   - Cause: from_step(\"X\") where no step has .bind(\"X\")");
    println!("   - Fix: Ensure binding name matches .bind() call\n");

    println!("2. InvalidMapping (undefined prompt arg):");
    println!("   - Cause: prompt_arg(\"X\") where no .argument(\"X\", ...) exists");
    println!("   - Fix: Add .argument(\"X\", description, required) to workflow\n");

    println!("3. UnknownBinding (step without .bind()):");
    println!("   - Cause: Referencing a step that has no .bind() call");
    println!("   - Fix: Add .bind(\"name\") to the step being referenced\n");

    println!("💡 Best Practices:");
    println!("   - Always call .bind() on steps whose output will be used");
    println!("   - Use descriptive binding names (not step names)");
    println!("   - Declare all prompt arguments before using them");
    println!("   - Call .validate() early to catch errors before runtime");
    println!("   - Fix errors one at a time - validation stops at first error\n");

    println!("✨ Error messages are designed to be actionable:");
    println!("   - They tell you exactly which step/binding/argument is the problem");
    println!("   - They include context to help you locate the issue");
    println!("   - They suggest the type of fix needed\n");
}
