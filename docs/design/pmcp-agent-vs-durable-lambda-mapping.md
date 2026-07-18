# `pmcp-agent` ↔ durable-agent-lambda shape-compatibility mapping (D-09)

**Status:** design-validation artifact (Phase 108). **No private-repo code is
copied here** — this document names identifiers and describes *shapes* only, to
demonstrate that the private `durable-agent-lambda` iteration loop could adopt
the open-source `pmcp-agent` crate and delete code. The actual migration is
explicitly **out of scope** for Phase 108 (DEFER-04); this is the boundary razor
that justified extracting the loop as a reusable crate rather than forking it.

## Why this document exists

`pmcp-agent` was extracted as the deploy-anywhere decision loop. The design
claim it validates: the pieces of the private durable Lambda's
`handler/iteration.rs`, `mcp/client.rs`, and `llm/` modules each have a
counterpart in `pmcp-agent`, so a durable host can keep ONLY its
platform-specific durability plumbing (`ctx.step`/`ctx.map`/`ctx.wait`,
checkpoint storage, Lambda suspend/resume) and delegate the AGENT LOGIC to the
crate. Everything that is pure decision-making or seam-shaped effect dispatch
moves behind `pmcp-agent`'s three seams.

Reference material was read **read-only** from
`~/Development/mcp/sdk/pmcp-run/amplify/functions/durable-agent-lambda/src/`
(`handler/iteration.rs`, `mcp/client.rs`, `types.rs`, `llm/`). None of it is
reproduced.

## The boundary razor

| Layer | Owner after adoption | Rationale |
|-------|----------------------|-----------|
| Pure decisions (end-turn test, tool extraction, submit-result validation, retry classification, limit checks) | `pmcp-agent::iteration::decide` | Deterministic, replay-safe, platform-agnostic — the crate's core value |
| Effect dispatch (completion, tool calls, state load/save) | `pmcp-agent` seams (`CompletionSource` / `ToolInvoker` / `ConversationStore`) | One object-safe interface per effect; the host supplies impls |
| Durability (checkpoint storage, suspend/resume, retry backoff timing) | **Stays platform-side** (Lambda `ctx.step`/`ctx.wait`/child-context checkpointing) | Durability is a platform capability; the crate returns retry classification as DATA and never sleeps |

The razor: **anything above the seam boundary is agent logic (crate); anything
that suspends compute or persists durably is platform (Lambda).**

## Shape-compatibility table

| durable-agent-lambda piece (private, read-only) | Shape | `pmcp-agent` counterpart | Notes |
|---|---|---|---|
| `types::IterationResult { llm_response, assistant_message, tool_results_message, is_final }` | per-iteration checkpoint struct | `pmcp_agent::iteration::IterationResult { assistant_message, tool_results_message, is_final }` | Same field intent; the crate drops the provider-specific `llm_response` (the raw completion) because the loop consumes only the decision-relevant projection. `Serialize + Deserialize` for checkpoint round-trip in both. |
| `handler::iteration::execute_iteration(...) -> DurableResult<IterationResult>` | one async iteration: llm-call → end-turn check → parallel tool dispatch | `pmcp_agent::iteration::AgentEngine` step + `pmcp_agent::iteration::decide` | The engine's `step` awaits ONLY the seam calls; every between-await decision is a pure `decide::*` fn. The durable version's `ctx.step("llm-call-{attempt}")` wrapping becomes the platform's `CompletionSource` impl. |
| `is_end_turn` match on stop reason (inside `execute_iteration`) | stop-reason → terminal? | `pmcp_agent::iteration::decide::is_end_turn(Option<&str>)` | Pure boolean over the stop reason. |
| `evaluate_submit_result(...)` | validate the assistant turn against the agent's output schema to decide finality | `pmcp_agent::iteration::decide::evaluate_submit_result(&turn, output_schema)` | Both use JSON-schema validation to decide whether a structured submission ends the run. |
| two-class retry split in `call_llm_with_retry` (`Class-1` infra 5xx/timeout, `Class-2` capacity 429/529) | error → retry policy | `pmcp_agent::seams::RetryClass { Fatal, Transient{attempt_hint}, Capacity{attempt_hint} }` surfaced via `pmcp_agent::iteration::RunOutcome::RetryRequired { class }` | The Lambda's Class-1 → `Transient`, Class-2 → `Capacity`, non-retryable → `Fatal`. Crucially the crate returns the class as DATA and does NOT sleep/`ctx.wait`; the platform keeps its `ctx.wait`-based backoff (durable suspend). Classification is centralized in `decide::classify_retry`. |
| `ctx.map(tool_calls, ...)` parallel tool dispatch | dispatch a batch of tool calls, one result per call | `pmcp_agent::seams::ToolInvoker::invoke_batch(Vec<ToolCall>) -> Vec<ToolCallResult>` | Same "one result per input, input order, id-matched" contract. The crate's `ClientToolInvoker` overrides the default with bounded-concurrency `buffered(N)`; the platform can instead map the single seam call onto durable `ctx.map`. |
| digest of tool results into the next turn | fold `ToolCallResult`s into a `tool_result` turn | `pmcp_agent::iteration::decide::digest_tool_results(...)` + `extract_tool_calls(...)` | Pure folding; tool errors travel as DATA (`ToolCallResult.is_error`), not as loop failures, so the model can react. |
| `mcp/client.rs::classify(&Task) -> PollDecision { Terminal, InputRequired, InProgress }` + `poll_task_durably(...)` | poll a task-augmented tool result to terminal | `pmcp_agent::invoker::ClientToolInvoker` driving `ConnectorClient::wait_for_related_task(meta, WaitForTaskOptions{ max_poll_duration_secs })` | The crate reuses the SDK's typed `related_task()` accessor + `wait_for_related_task` poll primitive under a HARD host cap (never polls forever), rather than a hand-rolled `PollDecision` loop. `InputRequired` handling stays a host concern. |
| `detect_task_response` via typed `related_task()` | recognize a task-augmented result | `CallToolResult::related_task()` (SDK) consumed inside `ClientToolInvoker::dispatch` | Same typed accessor; no manual `_meta` string-poking. |
| checkpoint/resume via `ctx.step` child-context + serialized `IterationResult` | crash-safe mid-iteration resume | `pmcp_agent::seams::ConversationStore::load`/`save` + `RunPhase { ReadyForCompletion, PendingTools, ToolsCompleted }` | The engine saves `PendingTools` BEFORE dispatch and the final state before returning, so a resumed run dispatches already-saved calls instead of re-running completion — the same crash-safety invariant the durable child-context checkpoint provides, expressed as a portable seam. |
| durable `ctx.step` / `ctx.wait` / Lambda suspend | durability + backoff timing | **the seam boundary itself** (no crate counterpart — intentionally) | Durability stays platform-side. The crate's purity (no wall-clock, no RNG, retry-as-data) is precisely what lets the platform own suspend/resume without the loop fighting it. |

## What the durable Lambda could DELETE after adoption

Adopting `pmcp-agent` would let `durable-agent-lambda` remove the code whose
behavior the crate now owns, keeping only thin seam adapters + durability glue:

- The hand-written end-turn / submit-result / tool-extraction / retry-class
  decision logic in `handler/iteration.rs` → replaced by `iteration::decide::*`
  (the engine calls them; the Lambda calls the engine).
- The `IterationResult` type and its serde plumbing → replaced by
  `pmcp_agent::iteration::IterationResult`.
- The bespoke `PollDecision` classification + poll loop in `mcp/client.rs` →
  replaced by `ClientToolInvoker` + the SDK `wait_for_related_task` primitive.
- The per-provider completion-shaping code in `llm/` → replaced by a
  `CompletionSource` impl (or the crate's `OpenAiCompatSource`/`AnthropicSource`
  when the provider matches), all behind one seam.

What it KEEPS: the `ctx.step`/`ctx.map`/`ctx.wait` durability wrappers, its
checkpoint store (as a `ConversationStore` impl), and its Class-2 `ctx.wait`
backoff — now driven by the `RetryClass` the crate returns as data.

## Non-goals (DEFER-04)

- No code is migrated in Phase 108. This is a design-validation artifact only.
- The forward composition target (`team_mcp__<member>` team servers) is covered
  by `contracts/team-servers-v1.yaml` (Phase 109 conformance), not here.
- `pmcp-agent` remains an experimental 0.x crate; the mapping is a compatibility
  argument, not an API-stability promise.
