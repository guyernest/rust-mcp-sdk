# Phase 8: Quality Polish and Test Coverage - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Close all tech debt and integration findings from the v1.1 milestone audit. This phase fixes diagnostic accuracy (SchemaMismatch field names), execution path coverage (silent breaks), compiler warnings (clippy), property test regression (TTL overflow), and E2E test gaps (continuation with succeeding tool). No new features — strictly quality polish on Phases 4-7 deliverables.

</domain>

<decisions>
## Implementation Decisions

### SchemaMismatch fix approach
- Change `params_satisfy_tool_schema` return type from `Result<bool>` to `Result<Vec<String>>` — returns all missing required field names (empty vec = satisfied)
- Collect ALL missing fields, not just the first one — full diagnostic in one shot
- Missing field names go in `_meta` JSON only (the task layer) — the prompt narrative remains unchanged
- Tasks and prompts are independent mechanisms: do not tie prompt narrative content to task _meta presence. Developers choose tasks, developers choose prompts — no coupling between them

### Silent break handling
- Both silent-break paths (resolve_tool_parameters at line 574, params_satisfy_tool_schema at line 578) should produce `PauseReason::UnresolvableParams`
- Add `tracing::warn!` logging on both paths for observability — these are "should not happen" conditions that indicate unexpected state

### Claude's Discretion
- Whether the PauseReason is set directly at the break site or routed through classify_resolution_failure — pick based on code clarity
- How to adapt existing callers of params_satisfy_tool_schema to the new Vec<String> return type — pick the most idiomatic Rust approach

### E2E continuation test design
- Same tool, different arguments: workflow invokes fetch_data with an argument that causes failure (e.g., "non_existent_key"), client continuation calls with args that succeed (e.g., "existing_key")
- Uncomment and fix the existing test_full_lifecycle_happy_path Stage 2 — it was always meant to include this stage
- After successful continuation, verify full progress: both `_workflow.result.fetch` exists with tool output AND `_workflow.progress` shows the fetch step as completed

### Property test fix strategy
- Both: saturating arithmetic in production code (defensive — overflow means "never expires") AND constrained proptest range (realistic inputs)
- Max TTL for proptest: 30 days
- Clean up the proptest regression file after fix — it's no longer needed once the fix makes the case pass

</decisions>

<specifics>
## Specific Ideas

- The tasks/prompts independence principle: "We should expect prompts without tasks and tasks without prompts. We should not tie them together in any way." — this guides the SchemaMismatch narrative decision
- The same-tool-different-args pattern for the E2E test is realistic: a tool failing with one input and succeeding with another mirrors real-world retry scenarios

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 08-quality-polish-and-test-coverage*
*Context gathered: 2026-02-23*
