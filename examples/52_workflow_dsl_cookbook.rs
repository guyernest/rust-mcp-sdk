//! Workflow DSL Cookbook
//!
//! Quick recipes you can copy and adapt for your workflows.
//!
//! Topics:
//! - DSL mapping variants (prompt_arg, from_step, field, constant)
//! - Deterministic argument order (IndexMap preservation)
//! - BindingName vs StepName (consistent usage)
//! - Common patterns and best practices

use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    InternalPromptMessage, SequentialWorkflow, ToolHandle, WorkflowStep,
};
use serde_json::json;

fn main() {
    println!("=== Workflow DSL Cookbook ===\n");
    println!("Copy-paste ready recipes for common workflow patterns.\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 1: DSL Mapping Variants
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 1: DSL Mapping Variants");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe1 = SequentialWorkflow::new("dsl_variants", "Demonstrates all DSL mapping variants")
        .argument("user_input", "Input from user", true)
        .argument("optional_style", "Optional style parameter", false)
        .step(
            WorkflowStep::new("step1", ToolHandle::new("processor"))
                // 1. prompt_arg() - Reference a prompt argument
                .arg("input", prompt_arg("user_input"))
                // 2. constant() - Provide a constant value
                .arg("mode", constant(json!("auto")))
                .arg("max_length", constant(json!(100)))
                .bind("result1"),
        )
        .step(
            WorkflowStep::new("step2", ToolHandle::new("enhancer"))
                // 3. from_step() - Reference entire output from previous step
                .arg("data", from_step("result1"))
                // 4. field() - Reference specific field from previous step output
                .arg("style", field("result1", "recommended_style"))
                .bind("result2"),
        );

    println!("Pattern: Using all four DSL helpers\n");
    println!("```rust");
    println!("WorkflowStep::new(\"step_name\", ToolHandle::new(\"tool\"))");
    println!("    // 1. prompt_arg(\"arg_name\") - Get value from workflow arguments");
    println!("    .arg(\"input\", prompt_arg(\"user_input\"))");
    println!();
    println!("    // 2. constant(json!(...)) - Provide a constant value");
    println!("    .arg(\"mode\", constant(json!(\"auto\")))");
    println!("    .arg(\"count\", constant(json!(42)))");
    println!();
    println!("    // 3. from_step(\"binding\") - Get entire output from previous step");
    println!("    .arg(\"data\", from_step(\"result1\"))");
    println!();
    println!("    // 4. field(\"binding\", \"field\") - Get specific field from output");
    println!("    .arg(\"style\", field(\"result1\", \"recommended_style\"))");
    println!("    .bind(\"result2\")");
    println!("```\n");

    assert!(recipe1.validate().is_ok());
    println!("✅ Recipe 1 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 2: Chaining Steps with Bindings
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 2: Chaining Steps with Bindings");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe2 = SequentialWorkflow::new("step_chaining", "Chain multiple steps together")
        .argument("topic", "Topic to write about", true)
        .step(
            // Step 1: Create draft
            WorkflowStep::new("create_draft", ToolHandle::new("writer"))
                .arg("topic", prompt_arg("topic"))
                .arg("format", constant(json!("markdown")))
                .bind("draft"), // ← Bind output as "draft"
        )
        .step(
            // Step 2: Review draft (uses output from step 1)
            WorkflowStep::new("review_draft", ToolHandle::new("reviewer"))
                .arg("content", from_step("draft")) // ← Reference "draft" binding
                .arg("criteria", constant(json!(["grammar", "clarity"])))
                .bind("review"), // ← Bind output as "review"
        )
        .step(
            // Step 3: Revise based on review (uses outputs from steps 1 & 2)
            WorkflowStep::new("revise_draft", ToolHandle::new("editor"))
                .arg("original", from_step("draft")) // ← Reference "draft"
                .arg("feedback", field("review", "suggestions")) // ← Extract field from "review"
                .bind("final"), // ← Bind output as "final"
        );

    println!("Pattern: Linear step chaining\n");
    println!("```rust");
    println!("SequentialWorkflow::new(...)");
    println!("    .step(");
    println!("        WorkflowStep::new(\"step1\", ToolHandle::new(\"tool1\"))");
    println!("            .arg(\"input\", prompt_arg(\"user_input\"))");
    println!("            .bind(\"output1\")  // ← First binding");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"step2\", ToolHandle::new(\"tool2\"))");
    println!("            .arg(\"data\", from_step(\"output1\"))  // ← Use first binding");
    println!("            .bind(\"output2\")  // ← Second binding");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"step3\", ToolHandle::new(\"tool3\"))");
    println!("            .arg(\"result\", from_step(\"output2\"))  // ← Use second binding");
    println!("            .bind(\"final\")");
    println!("    )");
    println!("```\n");

    assert!(recipe2.validate().is_ok());
    println!("✅ Recipe 2 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 3: Binding Names vs Step Names
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 3: Binding Names vs Step Names");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe3 = SequentialWorkflow::new(
        "binding_vs_step",
        "Demonstrates the difference between step names and binding names",
    )
    .argument("query", "Search query", true)
    .step(
        WorkflowStep::new("search_database", ToolHandle::new("search"))
            //                ^^^^^^^^^^^^^^^ Step name (identifies the step)
            .arg("query", prompt_arg("query"))
            .bind("search_results"), // ← Binding name (how others reference output)
    )
    .step(
        WorkflowStep::new("format_results", ToolHandle::new("formatter"))
            //                ^^^^^^^^^^^^^^ Different step name
            .arg(
                "data",
                from_step("search_results"), // ← Use BINDING name, not step name!
            )
            .bind("formatted_output"), // ← New binding name
    );

    println!("⚠️  IMPORTANT: Step Name ≠ Binding Name\n");
    println!("```rust");
    println!("WorkflowStep::new(\"search_database\", ToolHandle::new(\"search\"))");
    println!("//                 ^^^^^^^^^^^^^^^ Step name - internal identifier");
    println!("    .arg(\"query\", prompt_arg(\"query\"))");
    println!("    .bind(\"search_results\")");
    println!("//         ^^^^^^^^^^^^^^ Binding name - how others reference this");
    println!();
    println!("WorkflowStep::new(\"format_results\", ToolHandle::new(\"formatter\"))");
    println!("    .arg(\"data\", from_step(\"search_results\"))  // ← Use binding name!");
    println!("//                         ^^^^^^^^^^^^^^");
    println!("```\n");

    println!("❌ Common Mistake:");
    println!("  .arg(\"data\", from_step(\"search_database\"))  // WRONG - step name");
    println!();
    println!("✅ Correct:");
    println!("  .arg(\"data\", from_step(\"search_results\"))  // RIGHT - binding name\n");

    assert!(recipe3.validate().is_ok());
    println!("✅ Recipe 3 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 4: Deterministic Argument Order (IndexMap)
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 4: Deterministic Argument Order");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe4 =
        SequentialWorkflow::new("arg_order", "Arguments maintain insertion order (IndexMap)")
            .argument("first", "First arg", true)
            .argument("second", "Second arg", true)
            .argument("third", "Third arg", true)
            .step(
                WorkflowStep::new("test_order", ToolHandle::new("processor"))
            .arg("z_param", prompt_arg("first")) // Added third
            .arg("a_param", prompt_arg("second")) // Added first
            .arg("m_param", prompt_arg("third")) // Added second
            .bind("result"),
            );

    println!("🔑 Key Feature: Arguments preserve insertion order\n");
    println!("```rust");
    println!("WorkflowStep::new(\"step\", ToolHandle::new(\"tool\"))");
    println!("    .arg(\"z_param\", prompt_arg(\"first\"))   // Position 1");
    println!("    .arg(\"a_param\", prompt_arg(\"second\"))  // Position 2");
    println!("    .arg(\"m_param\", prompt_arg(\"third\"))   // Position 3");
    println!("```\n");

    println!("Order is preserved (not alphabetical):");
    for (i, (arg_name, _source)) in recipe4.steps()[0].arguments().iter().enumerate() {
        println!("  {}. {}", i + 1, arg_name);
    }
    println!();

    println!("💡 Why this matters:");
    println!("  - Predictable serialization order");
    println!("  - Easier debugging and testing");
    println!("  - Consistent across runs");
    println!("  - No HashMap randomness\n");

    assert!(recipe4.validate().is_ok());
    println!("✅ Recipe 4 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 5: Extracting Nested Fields
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 5: Extracting Nested Fields");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe5 = SequentialWorkflow::new("nested_fields", "Extract specific fields from outputs")
        .argument("input", "Input data", true)
        .step(
            WorkflowStep::new("analyze", ToolHandle::new("analyzer"))
                .arg("data", prompt_arg("input"))
                .bind("analysis"), // Output: { "summary": {...}, "metadata": {...}, "scores": {...} }
        )
        .step(
            WorkflowStep::new("generate_report", ToolHandle::new("reporter"))
                // Extract specific fields from the analysis
                .arg("summary", field("analysis", "summary"))
                .arg("confidence", field("analysis", "scores"))
                .arg("timestamp", field("analysis", "metadata"))
                .bind("report"),
        );

    println!("Pattern: Extracting fields from complex output\n");
    println!("Assume 'analysis' step returns:");
    println!("```json");
    println!("{{");
    println!("  \"summary\": {{ \"text\": \"...\", \"length\": 42 }},");
    println!("  \"metadata\": {{ \"timestamp\": \"2024-01-01\", \"version\": 1 }},");
    println!("  \"scores\": {{ \"confidence\": 0.95, \"accuracy\": 0.88 }}");
    println!("}}");
    println!("```\n");

    println!("Extract specific fields:");
    println!("```rust");
    println!("WorkflowStep::new(\"use_fields\", ToolHandle::new(\"tool\"))");
    println!("    .arg(\"summary\", field(\"analysis\", \"summary\"))");
    println!("    .arg(\"confidence\", field(\"analysis\", \"scores\"))");
    println!("    .arg(\"timestamp\", field(\"analysis\", \"metadata\"))");
    println!("```\n");

    println!("💡 Benefits:");
    println!("  - Extract only what you need");
    println!("  - Type-safe field references");
    println!("  - Self-documenting data flow\n");

    assert!(recipe5.validate().is_ok());
    println!("✅ Recipe 5 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 6: Optional Steps (No Binding)
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 6: Optional Steps (No Binding)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe6 = SequentialWorkflow::new(
        "optional_bindings",
        "Not all steps need bindings if output isn't used",
    )
    .argument("data", "Data to process", true)
    .step(
        WorkflowStep::new("process", ToolHandle::new("processor"))
            .arg("input", prompt_arg("data"))
            .bind("result"), // ← Binding needed - used by next step
    )
    .step(
        WorkflowStep::new("log_result", ToolHandle::new("logger"))
            .arg("message", from_step("result")),
        // ← NO .bind() - output not used by other steps
    )
    .step(
        WorkflowStep::new("notify", ToolHandle::new("notifier"))
            .arg("status", constant(json!("complete"))),
        // ← NO .bind() - side-effect only (send notification)
    );

    println!("Pattern: Steps without .bind() are OK if output isn't used\n");
    println!("```rust");
    println!("SequentialWorkflow::new(...)");
    println!("    .step(");
    println!("        WorkflowStep::new(\"process\", ToolHandle::new(\"processor\"))");
    println!("            .arg(\"input\", prompt_arg(\"data\"))");
    println!("            .bind(\"result\")  // ← Bind because used later");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"log\", ToolHandle::new(\"logger\"))");
    println!("            .arg(\"message\", from_step(\"result\"))");
    println!("            // NO .bind() - just logs, doesn't produce reusable output");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"notify\", ToolHandle::new(\"notifier\"))");
    println!("            .arg(\"status\", constant(json!(\"done\")))");
    println!("            // NO .bind() - side-effect only (sends notification)");
    println!("    )");
    println!("```\n");

    println!("💡 When to skip .bind():");
    println!("  - Side-effects only (logging, notifications, metrics)");
    println!("  - Terminal steps (no steps follow)");
    println!("  - Output not needed by subsequent steps\n");

    assert!(recipe6.validate().is_ok());
    println!("✅ Recipe 6 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 7: Multiple Steps Using Same Output
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 7: Multiple Steps Using Same Output");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe7 =
        SequentialWorkflow::new("fan_out", "One output used by multiple subsequent steps")
            .argument("source", "Source data", true)
            .step(
                WorkflowStep::new("fetch_data", ToolHandle::new("fetcher"))
                    .arg("url", prompt_arg("source"))
                    .bind("data"), // ← This binding used by multiple steps below
            )
            .step(
                WorkflowStep::new("analyze", ToolHandle::new("analyzer"))
            .arg("input", from_step("data")) // ← Uses "data"
            .bind("analysis"),
            )
            .step(
                WorkflowStep::new("summarize", ToolHandle::new("summarizer"))
            .arg("input", from_step("data")) // ← Also uses "data"
            .bind("summary"),
            )
            .step(
                WorkflowStep::new("validate", ToolHandle::new("validator"))
            .arg("input", from_step("data")) // ← Also uses "data"
            .bind("validation"),
            );

    println!("Pattern: Fan-out - one output feeds multiple steps\n");
    println!("```rust");
    println!("SequentialWorkflow::new(...)");
    println!("    .step(");
    println!("        WorkflowStep::new(\"fetch\", ToolHandle::new(\"fetcher\"))");
    println!("            .arg(\"url\", prompt_arg(\"source\"))");
    println!("            .bind(\"data\")  // ← One binding");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"analyze\", ToolHandle::new(\"analyzer\"))");
    println!("            .arg(\"input\", from_step(\"data\"))  // ← Used here");
    println!("            .bind(\"analysis\")");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"summarize\", ToolHandle::new(\"summarizer\"))");
    println!("            .arg(\"input\", from_step(\"data\"))  // ← And here");
    println!("            .bind(\"summary\")");
    println!("    )");
    println!("    .step(");
    println!("        WorkflowStep::new(\"validate\", ToolHandle::new(\"validator\"))");
    println!("            .arg(\"input\", from_step(\"data\"))  // ← And here");
    println!("            .bind(\"validation\")");
    println!("    )");
    println!("```\n");

    println!("💡 Benefits:");
    println!("  - Reuse expensive operations (fetch once, use many times)");
    println!("  - Clear data dependencies");
    println!("  - Easy to parallelize in future (all depend on same input)\n");

    assert!(recipe7.validate().is_ok());
    println!("✅ Recipe 7 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Recipe 8: Workflow with Instructions
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 Recipe 8: Adding Workflow Instructions");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let recipe8 = SequentialWorkflow::new(
        "with_instructions",
        "Workflow with system instructions for LLM guidance",
    )
    .argument("topic", "Topic to research", true)
    .instruction(InternalPromptMessage::system(
        "You are a research assistant. Be thorough and cite sources.",
    ))
    .instruction(InternalPromptMessage::system(
        "Format all responses in markdown with clear sections.",
    ))
    .step(
        WorkflowStep::new("research", ToolHandle::new("researcher"))
            .arg("query", prompt_arg("topic"))
            .bind("findings"),
    )
    .step(
        WorkflowStep::new("synthesize", ToolHandle::new("synthesizer"))
            .arg("data", from_step("findings"))
            .bind("report"),
    );

    println!("Pattern: Add system instructions for LLM context\n");
    println!("```rust");
    println!("SequentialWorkflow::new(\"workflow\", \"description\")");
    println!("    .argument(\"topic\", \"Topic to research\", true)");
    println!("    .instruction(InternalPromptMessage::system(");
    println!("        \"You are a research assistant. Be thorough.\"");
    println!("    ))");
    println!("    .instruction(InternalPromptMessage::system(");
    println!("        \"Format responses in markdown.\"");
    println!("    ))");
    println!("    .step(...)");
    println!("```\n");

    println!("💡 Instructions are converted to system messages in prompts");
    println!("  - Guide LLM behavior across the workflow");
    println!("  - Set tone, style, and constraints");
    println!("  - Reused in prompt generation\n");

    assert!(recipe8.validate().is_ok());
    println!("✅ Recipe 8 validated successfully\n");

    // ═══════════════════════════════════════════════════════════════
    // Summary
    // ═══════════════════════════════════════════════════════════════

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📚 Quick Reference Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("DSL Helpers:");
    println!("  prompt_arg(\"name\")          → Get value from workflow arguments");
    println!("  from_step(\"binding\")        → Get entire output from step");
    println!("  field(\"binding\", \"field\")  → Get specific field from output");
    println!("  constant(json!(...))        → Provide constant value\n");

    println!("Key Concepts:");
    println!("  • Step Name: Identifies the step (first arg to WorkflowStep::new)");
    println!("  • Binding Name: How others reference output (.bind(\"name\"))");
    println!("  • Use BINDING names in from_step() and field(), not step names!");
    println!("  • Arguments preserve insertion order (IndexMap)");
    println!("  • .bind() is optional if output isn't used\n");

    println!("Best Practices:");
    println!("  ✓ Use descriptive binding names");
    println!("  ✓ Declare all arguments before using them");
    println!("  ✓ Call .validate() early");
    println!("  ✓ Add .bind() to steps whose output will be referenced");
    println!("  ✓ Use field() to extract only needed data\n");

    println!("✨ All {} recipes validated successfully!", 8);
}
