---
phase: 03-handler-middleware-and-server-integration
plan: 01
subsystem: api
tags: [mcp-tasks, protocol, serde, async-trait, builder-pattern]

# Dependency graph
requires:
  - phase: 01-core-types-and-traits
    provides: "TaskRecord, TaskStore trait, TaskParams, task wire types"
  - phase: 02-in-memory-backend-and-owner-security
    provides: "InMemoryTaskStore, TaskContext, owner-based security model"
provides:
  - "task field on CallToolRequest for task-augmented tool calls"
  - "execution field on ToolInfo for task support metadata"
  - "TasksGet/TasksResult/TasksList/TasksCancel ClientRequest variants"
  - "TaskRouter trait as integration contract between pmcp and pmcp-tasks"
  - "with_task_store() builder method with auto-configured experimental.tasks capability"
  - "task_router field on ServerCore"
affects: [03-02, 03-03, 04-dynamodb-backend, 05-workflow-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: ["TaskRouter trait object pattern for crate boundary", "Value params to avoid circular dependency"]

key-files:
  created:
    - "src/server/tasks.rs"
  modified:
    - "src/types/protocol.rs"
    - "src/server/builder.rs"
    - "src/server/core.rs"
    - "src/server/mod.rs"
    - "src/client/mod.rs"
    - "src/server/typed_tool.rs"
    - "src/shared/protocol_helpers.rs"

key-decisions:
  - "Use serde_json::Value for all TaskRouter params/returns to avoid circular dependency (pmcp-tasks depends on pmcp)"
  - "TaskRouter trait defined in pmcp (not pmcp-tasks) so builder can accept it without reverse dependency"
  - "Task ClientRequest variants return METHOD_NOT_FOUND error when no task router configured"
  - "with_task_store() method name kept per CONTEXT.md locked decision even though parameter is Arc<dyn TaskRouter>"

patterns-established:
  - "TaskRouter trait object: Arc<dyn TaskRouter> crossing crate boundaries via Value params"
  - "Auto-capability pattern: builder methods that configure experimental capabilities"

requirements-completed: [INTG-01, INTG-02, INTG-10]

# Metrics
duration: 15min
completed: 2026-02-22
---

# Phase 03 Plan 01: Protocol Types, TaskRouter Trait, and Builder Integration Summary

**Protocol-level task support via CallToolRequest.task field, ToolInfo.execution field, four ClientRequest task variants, TaskRouter trait, and with_task_store() builder method with auto-configured experimental.tasks capability**

## Performance

- **Duration:** 15 min
- **Started:** 2026-02-22T03:39:13Z
- **Completed:** 2026-02-22T03:55:05Z
- **Tasks:** 3
- **Files modified:** 18

## Accomplishments
- Extended CallToolRequest with optional task field and ToolInfo with optional execution field for MCP Tasks
- Added four task endpoint variants (TasksGet, TasksResult, TasksList, TasksCancel) to ClientRequest enum with serde round-trip support
- Defined TaskRouter trait in pmcp as the integration contract for pmcp-tasks, avoiding circular dependency
- Wired TaskRouter into ServerCoreBuilder and ServerCore with auto-configured experimental.tasks capability
- Updated all 30+ CallToolRequest struct initializers across the codebase (src, tests, examples, benches)

## Task Commits

Each task was committed atomically:

1. **Task 1: Protocol types and ClientRequest variants** - `b268784` (feat)
2. **Task 2a: TaskRouter trait definition** - `ed5eced` (feat)
3. **Task 2b: Builder and core plumbing** - `3ef1092` (feat)

## Files Created/Modified
- `src/server/tasks.rs` - TaskRouter trait definition with 8 methods for task lifecycle routing
- `src/types/protocol.rs` - Added task/execution fields and 4 ClientRequest variants with tests
- `src/server/builder.rs` - Added with_task_store() method, task_router field, 2 new tests
- `src/server/core.rs` - Added task_router field to ServerCore struct and new() parameter
- `src/server/mod.rs` - Registered tasks module, handled new ClientRequest variants in match
- `src/shared/protocol_helpers.rs` - Added JSON-RPC conversion for task ClientRequest variants
- `src/client/mod.rs` - Added task: None to call_tool() method
- `src/server/typed_tool.rs` - Added execution: None to 3 ToolInfo initializers
- `src/server/adapters.rs` - Added task_router: None to test ServerCore::new() call
- `examples/11_progress_countdown.rs` - Added task: None to CallToolRequest initializers
- `examples/58_oauth_transport_to_tools.rs` - Added task: None to 3 CallToolParams initializers
- `examples/wasm-client/src/lib.rs` - Added task: None to CallToolRequest initializer
- `tests/protocol_invariants.rs` - Added task: None to CallToolParams initializer
- `tests/auth_context_integration_test.rs` - Added task: None to 2 CallToolParams initializers
- `benches/protocol_serialization.rs` - Added task: None to CallToolParams initializer
- `benches/transport_performance.rs` - Added task: None to 3 CallToolParams initializers
- `benches/client_server_operations.rs` - Added task: None to 3 CallToolParams initializers
- `src/server/core_tests.rs` - Added task: None to 4 CallToolParams initializers
- `src/server/adapter_tests.rs` - Added task: None to 2 CallToolParams initializers

## Decisions Made
- Used `serde_json::Value` for all TaskRouter method params/returns to avoid circular crate dependency (pmcp-tasks depends on pmcp, so pmcp cannot use pmcp-tasks types)
- Defined TaskRouter in pmcp rather than pmcp-tasks so the builder can reference it without reverse dependency
- New ClientRequest task variants return METHOD_NOT_FOUND (-32601) when no task router is configured, matching JSON-RPC spec for unsupported methods
- Kept `with_task_store()` as the method name per CONTEXT.md locked decision, even though the parameter is `Arc<dyn TaskRouter>` -- from the developer's perspective they pass a "task store" that pmcp-tasks wraps in a router
- Used `#[allow(dead_code)]` on task_router field in ServerCore since Plan 02 will wire the routing logic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed all CallToolRequest/CallToolParams struct initializers across codebase**
- **Found during:** Task 1 (protocol type changes)
- **Issue:** Adding `task` field to `CallToolRequest` broke ~30 struct literal initializers across src, tests, examples, and benches that did not include the new field
- **Fix:** Added `task: None` to every `CallToolRequest`/`CallToolParams` struct literal in the codebase
- **Files modified:** 15 files (client/mod.rs, server/mod.rs, typed_tool.rs, core_tests.rs, adapter_tests.rs, protocol_helpers.rs, 4 examples, 2 tests, 3 benches)
- **Verification:** `cargo test --package pmcp --lib` passes (666 tests)
- **Committed in:** b268784 (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed ToolInfo struct initializers missing execution field**
- **Found during:** Task 1 (protocol type changes)
- **Issue:** Adding `execution` field to `ToolInfo` broke 3 struct literal initializers in typed_tool.rs that returned `ToolInfo` from `metadata()` methods
- **Fix:** Added `execution: None` to all 3 ToolInfo initializers in typed_tool.rs
- **Files modified:** src/server/typed_tool.rs
- **Verification:** `cargo check --package pmcp` passes
- **Committed in:** b268784 (Task 1 commit)

**3. [Rule 3 - Blocking] Fixed non-exhaustive match for new ClientRequest variants**
- **Found during:** Task 1 (protocol type changes)
- **Issue:** Two match statements on `ClientRequest` in server/mod.rs and shared/protocol_helpers.rs did not handle the 4 new task variants
- **Fix:** Added match arms: server returns METHOD_NOT_FOUND error, protocol_helpers converts to JSON-RPC
- **Files modified:** src/server/mod.rs, src/shared/protocol_helpers.rs
- **Verification:** `cargo check --package pmcp` passes
- **Committed in:** b268784 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary consequences of the planned type changes. The scope of struct initializer updates was larger than anticipated (~30 call sites vs ~5 mentioned in the plan) but all were mechanical additions of `task: None` or `execution: None`.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Protocol types, TaskRouter trait, and builder integration are complete
- Plan 02 can now wire task routing in ServerCore::handle_request using the task_router field
- Plan 02 can implement TaskRouter in pmcp-tasks using the trait defined here
- Plan 03 can add integration tests using the builder's with_task_store() method

## Self-Check: PASSED

All created files exist, all commits verified, all key code artifacts confirmed present.

---
*Phase: 03-handler-middleware-and-server-integration*
*Completed: 2026-02-22*
