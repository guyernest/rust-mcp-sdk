---
phase: 105
reviewers: [codex]
reviewed_at: 2026-07-05T16:45:00Z
plans_reviewed: []
artifacts_reviewed: [105-CONTEXT.md, ROADMAP.md phase entry, src/client/mod.rs wait_for_task, src/types/tasks.rs]
review_stage: pre-planning (design/context review — no PLAN.md files exist yet)
---

# Cross-AI Design Review — Phase 105

> Pre-planning review: Codex reviewed the phase scope, CONTEXT.md decisions, and the
> API design against the actual 2.12.0 code — before plans are written. Feed into
> `/gsd:plan-phase 105 --reviews`.

## Codex Review

## Summary

The phase design is sound and appropriately scoped. The core split is correct: `Task::poll_decision()` handles pure task-state classification, while `resolve_poll_interval()` handles stateless interval policy, and `wait_for_task` keeps loop-only concerns like timeout budgets. This should give durable/replay consumers the reusable primitive they need without inventing protocol behavior around `input_required`.

## Strengths

- `Task::poll_decision(&self)` is a good API home: discoverable, pure, and naturally replay-safe.
- Dropping `Unpollable` is correct for a method on `Task`; client initialization/capability failures are not task properties.
- Keeping terminal result retrieval out of the classifier is the right separation. `tasks/result` is a separate operation and belongs to the consumer.
- `InputRequired` as a unit variant is clean; callers already have the `Task`.
- `resolve_poll_interval()` is a useful second extraction and avoids mixing task classification with polling policy.
- Leaving budget clamping inside `wait_for_task` is correct because remaining budget is loop state, not task state.
- The scope fences are disciplined: no wire changes, no fake `tasks/provide_input`, no new `TaskStatus`, no behavior change to `wait_for_task`.

## Concerns

- **MEDIUM:** The “single decision point” guarantee cannot be fully enforced by API shape alone. `wait_for_task` must structurally `match task.poll_decision()` so future edits are less likely to reintroduce parallel status logic.
- **MEDIUM:** `#[non_exhaustive]` on `TaskPollDecision` is good, but `TaskStatus` itself appears exhaustive today. Future status variants would still be semver-sensitive for the SDK. This is not a blocker, but docs should avoid implying unknown statuses can be handled gracefully today.
- **MEDIUM:** Unknown task statuses being rejected during serde means durable consumers may fail a memoized `tasks/get` step before classification. That is acceptable, but the docs should state that the classifier only runs after successful typed deserialization.
- **LOW:** `resolve_poll_interval()` returning `u64` milliseconds is simple, but less type-safe than returning `Duration`. Keeping `u64` may match existing public option shapes, so this is mostly an ergonomics tradeoff.
- **LOW:** A single light runnable example is probably sufficient for the API, but the durable/replay pattern is the real user value. The prose snippets need to be concrete enough to prevent people from wrapping `wait_for_task` inside durable workflows.

## Suggestions

- Implement `wait_for_task` as an explicit match:

```rust
match task.poll_decision() {
    TaskPollDecision::Terminal { .. } => break,
    TaskPollDecision::InputRequired => return Err(...),
    TaskPollDecision::InProgress { poll_hint } => {
        let mut interval = resolve_poll_interval(opts.poll_interval, poll_hint);
        ...
    }
}
```

- Make `resolve_poll_interval()` public wherever consumers naturally import task APIs. If it lives outside `types`, consider re-exporting it near `TaskPollDecision`.
- Add focused tests for:
  - every current `TaskStatus` maps to the expected `TaskPollDecision`;
  - caller override beats server hint;
  - server hint beats default;
  - zero/low intervals floor to 50 ms;
  - `wait_for_task` preserves current `input_required` error behavior;
  - budget clamping still prevents oversleep and remains outside the helper.
- In docs, explicitly say: `poll_decision()` is replay-deterministic only as a pure function over an already-deserialized `Task`; the network call and serde decode must be inside the durable runtime’s memoized step.
- In rustdoc for `TaskPollDecision::Terminal`, state that callers still need `tasks/result` to retrieve the final `CallToolResult`.
- In the durable docs, include a clear “do not use `wait_for_task` inside replay workflows” note because it sleeps, loops, and owns the polling lifecycle.

## Risk Assessment

**Overall risk: LOW to MEDIUM.** The API is additive, wire-neutral, and mostly a refactor of existing behavior into public helpers. The main risk is not semantic correctness but drift: future maintainers could accidentally add status logic back into `wait_for_task`. That risk is manageable if the implementation structurally matches on `poll_decision()` and tests pin both the classifier and `wait_for_task` behavior.

---

## Consensus Summary

Single reviewer (Codex only, per --codex flag). Its verdict: design sound and
appropriately scoped; overall risk LOW–MEDIUM; the main risk is future DRIFT,
not semantic correctness.

### Agreed Strengths
- The three-way split is correct: pure task-state classification (`poll_decision`)
  / stateless interval policy (`resolve_poll_interval`) / loop-only state (budget
  clamp stays in `wait_for_task`)
- Dropping `Unpollable` and the unit `InputRequired` variant are both endorsed
- Scope fences called "disciplined"

### Priority Concerns (feed into planning)
1. **MEDIUM — structural no-drift enforcement:** `wait_for_task` must literally
   `match task.poll_decision()` (not merely call it somewhere) so status logic
   cannot be reintroduced inline; pin both with tests.
2. **MEDIUM — docs must scope the replay-determinism claim:** classifier is
   deterministic only over an already-deserialized `Task`; the network call +
   serde decode belong INSIDE the durable runtime's memoized step; unknown
   future statuses fail at deserialization BEFORE classification.
3. **MEDIUM — semver honesty:** `TaskStatus` is exhaustive today; docs must not
   imply unknown statuses are handled gracefully.
4. **LOW — ergonomics:** consider `Duration` vs `u64` ms for the resolver return
   (u64 matches existing option shapes); re-export `resolve_poll_interval` near
   `TaskPollDecision`.
5. **LOW — docs sharpness:** include an explicit "do NOT wrap `wait_for_task`
   inside replay workflows" warning; `Terminal` rustdoc must say `tasks/result`
   is still a separate fetch.

### Test matrix Codex wants (carry into plan `<verification>`)
- Every current `TaskStatus` → expected `TaskPollDecision` (exhaustive map)
- Caller override beats server hint; hint beats default; zero/low floors to 50ms
- `wait_for_task` `input_required` error behavior byte-identical after refactor
- Budget clamp still prevents oversleep and remains outside the helper

### Divergent Views
None (single reviewer).
