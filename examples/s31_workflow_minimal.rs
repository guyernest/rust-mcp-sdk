//! Minimal workflow example: Quadratic formula solver
//!
//! This example demonstrates the workflow system without server integration:
//! - Building a SequentialWorkflow
//! - Using WorkflowStep with .bind() for output bindings
//! - DSL helpers: prompt_arg, from_step, field, constant
//! - Validating the workflow
//! - Inspecting output bindings
//!
//! The workflow solves ax² + bx + c = 0 using the quadratic formula:
//! x = (-b ± √(b² - 4ac)) / 2a
//!
//! Steps:
//! 1. calculate_discriminant: Compute b² - 4ac → binds to "discriminant"
//! 2. calculate_root1: Compute (-b + √discriminant) / 2a → binds to "root1"
//! 3. calculate_root2: Compute (-b - √discriminant) / 2a → binds to "root2"
//! 4. format_solution: Format the results → binds to "formatted_solution"

use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use serde_json::json;

fn main() {
    println!("=== Workflow System Demo: Quadratic Formula Solver ===\n");

    // Build a workflow that solves quadratic equations
    let workflow = SequentialWorkflow::new(
        "quadratic_solver",
        "Solve quadratic equations using the quadratic formula",
    )
    // Define required prompt arguments
    .argument("a", "Coefficient a (x² term)", true)
    .argument("b", "Coefficient b (x term)", true)
    .argument("c", "Coefficient c (constant term)", true)
    // Add instruction messages
    .instruction(InternalPromptMessage::system(
        "Solve the quadratic equation ax² + bx + c = 0",
    ))
    // Step 1: Calculate discriminant (b² - 4ac)
    .step(
        WorkflowStep::new("calc_discriminant", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("discriminant")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("c", prompt_arg("c"))
            .bind("discriminant"), // ← Bind output as "discriminant"
    )
    // Step 2: Calculate first root using discriminant
    .step(
        WorkflowStep::new("calc_root1", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value")) // ← Reference binding
            .arg("sign", constant(json!("+")))
            .bind("root1"), // ← Bind output as "root1"
    )
    // Step 3: Calculate second root using discriminant
    .step(
        WorkflowStep::new("calc_root2", ToolHandle::new("calculator"))
            .arg("operation", constant(json!("quadratic_root")))
            .arg("a", prompt_arg("a"))
            .arg("b", prompt_arg("b"))
            .arg("discriminant_value", field("discriminant", "value")) // ← Reference binding
            .arg("sign", constant(json!("-")))
            .bind("root2"), // ← Bind output as "root2"
    )
    // Step 4: Format the solution using entire step outputs
    .step(
        WorkflowStep::new("format_solution", ToolHandle::new("formatter"))
            .arg("discriminant_result", from_step("discriminant")) // ← Reference entire output
            .arg("root1_result", from_step("root1")) // ← Reference entire output
            .arg("root2_result", from_step("root2")) // ← Reference entire output
            .arg("format_template", constant(json!("Solution: x = {root1} or x = {root2}")))
            .bind("formatted_solution"),
    );

    // Validate the workflow
    println!("📋 Workflow: {}", workflow.name());
    println!("📝 Description: {}", workflow.description());
    println!("\n🔍 Validating workflow...");

    match workflow.validate() {
        Ok(()) => {
            println!("✅ Workflow is valid!\n");

            // Inspect the workflow structure
            println!("📥 Required Arguments:");
            for (name, spec) in workflow.arguments() {
                let required = if spec.required {
                    "required"
                } else {
                    "optional"
                };
                println!("  - {} ({}): {}", name, required, spec.description);
            }

            println!("\n🔗 Workflow Steps:");
            for (i, step) in workflow.steps().iter().enumerate() {
                let tool_name = step.tool().map(|t| t.name()).unwrap_or("[resource-only]");
                println!("  {}. {} → tool: {}", i + 1, step.name(), tool_name);

                // Show arguments
                for (arg_name, source) in step.arguments() {
                    println!("     - arg '{}' from: {:?}", arg_name, source);
                }

                // Show binding
                if let Some(binding) = step.binding() {
                    println!("     → binds output to: '{}'", binding);
                }
            }

            println!("\n📤 Output Bindings:");
            let bindings = workflow.output_bindings();
            for binding in &bindings {
                println!("  - {}", binding);
            }
            println!(
                "\n💡 Later steps can reference these {} bindings using from_step() or field()",
                bindings.len()
            );

            println!("\n📊 Instructions:");
            for (i, instruction) in workflow.instructions().iter().enumerate() {
                println!("  {}. {:?}", i + 1, instruction);
            }
        },
        Err(e) => {
            eprintln!("❌ Validation failed: {}", e);
            std::process::exit(1);
        },
    }

    println!("\n✨ Example demonstrates:");
    println!("  1. SequentialWorkflow::new() - create workflow");
    println!("  2. .argument() - define prompt arguments");
    println!("  3. .instruction() - add system instructions");
    println!("  4. WorkflowStep::new() - create steps");
    println!("  5. .bind() - create output bindings");
    println!("  6. DSL helpers:");
    println!("     - prompt_arg(\"name\") - reference prompt arguments");
    println!("     - from_step(\"binding\") - reference entire step output");
    println!("     - field(\"binding\", \"field\") - reference specific output field");
    println!("     - constant(value) - provide constant values");
    println!("  7. .validate() - verify workflow correctness");
    println!("  8. .output_bindings() - inspect available outputs");

    println!("\n🎯 Key Concept: Binding Names vs Step Names");
    println!("  - Step name (first arg to WorkflowStep::new): identifies the step");
    println!("  - Binding name (set via .bind()): how other steps reference the output");
    println!("  - Only steps with .bind() can have their outputs referenced!");
    println!("  - Use binding names with from_step() and field(), not step names");

    println!("\n📖 Next Steps:");
    println!("  - See examples/51_workflow_error_messages.rs for validation error examples");
    println!("  - See examples/52_workflow_dsl_cookbook.rs for DSL patterns and recipes");
    println!("  - See examples/53_typed_tools_workflow_integration.rs for server-side execution");
}
