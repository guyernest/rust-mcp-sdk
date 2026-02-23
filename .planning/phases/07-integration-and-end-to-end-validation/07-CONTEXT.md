# Phase 7: Integration and End-to-End Validation - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

The task-prompt bridge is wired into `ServerCoreBuilder` with a clean API, validated end-to-end, and demonstrated with a working example. This phase covers builder API validation (INTG-01), backward compatibility tests (INTG-02), a full lifecycle example (INTG-03), and integration tests (INTG-04). It does NOT add new workflow capabilities — it validates what Phases 4-6 built.

</domain>

<decisions>
## Implementation Decisions

### Example lifecycle design
- Scenario: multi-step data processing (fetch-data + transform + store), NOT the deploy pipeline from the existing example
- Pause trigger: missing dependency — a step depends on output from a step that requires client action, showing the data-dependency handoff
- Client continuation demonstrated via direct `ServerCore` calls — construct JSON requests with `_task_id` in `_meta`, no transport layer
- Print the full message list from the handoff so the reader sees: user intent, assistant plan, tool call/result pairs, and the handoff narrative
- Replace the existing `62_task_workflow_opt_in.rs` with the full lifecycle example

### Builder API completeness
- The existing API is sufficient: `with_task_support(true)` on the workflow + `with_task_store()` + `prompt_workflow()` — no new builder methods needed
- No convenience methods — keep the 3-piece API clean and explicit
- Add explicit backward compatibility integration tests: register both task and non-task workflows on the same server, verify non-task workflows are unaffected

### Integration test scenarios
- Happy path: create-execute-handoff-continue-complete full lifecycle
- Error cases: step failure with handoff, retry with `_task_id` (overwrite behavior), cancel-with-result for explicit completion
- Tests call handler methods directly (handler-level), not through ServerCore JSON-RPC
- Tests live in `crates/pmcp-tasks/tests/` alongside existing pmcp-tasks tests

### Example naming and structure
- Filename: `62_task_workflow_lifecycle.rs` (replaces `62_task_workflow_opt_in.rs`)
- Synchronous: keep `fn main()` (no `#[tokio::main]`), consistent with current example style
- Heavy inline comments: comment each lifecycle stage ("// Stage 1: Invoke workflow prompt", "// Stage 2: Inspect handoff", etc.) — this is a teaching example
- Module-level `//!` doc explaining the full lifecycle

### Claude's Discretion
- Exact data processing scenario (what tools and data shapes)
- Number of workflow steps (enough to show the lifecycle, not more)
- Integration test file name and organization within `crates/pmcp-tasks/tests/`
- How to handle the sync-vs-async tension if handler calls require async (may need `tokio::runtime::Runtime::new()` block)

</decisions>

<specifics>
## Specific Ideas

- The example should be self-contained and runnable with `cargo run --example 62_task_workflow_lifecycle`
- Full message list printing makes the example work as documentation — readers can see exactly what an LLM client would receive
- The data-dependency pause trigger is more instructive than a simple unresolvable parameter because it shows the step-output-binding system in action

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-integration-and-end-to-end-validation*
*Context gathered: 2026-02-23*
