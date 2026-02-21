# Roadmap: MCP Tasks for PMCP SDK

## Overview

Build `pmcp-tasks`, a separate crate implementing the MCP Tasks specification (2025-11-25, experimental) for the PMCP SDK. The work follows the strict dependency chain imposed by the architecture: protocol types and store contract first (everything depends on them), then the in-memory backend with security enforcement (unblocks all testing), then handler/middleware/server integration (the highest-risk phase touching core `pmcp`), then the DynamoDB production backend, and finally workflow integration. Each phase delivers a testable, verifiable capability. The crate ships at `0.x` semver to accommodate spec drift.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation Types and Store Contract** - Spec-compliant protocol types, TaskStore trait, state machine, error types, and serialization tests (completed 2026-02-21)
- [ ] **Phase 2: In-Memory Backend and Owner Security** - Working in-memory storage with atomic transitions, owner isolation, security config, TaskContext, and property tests
- [ ] **Phase 3: Handler, Middleware, and Server Integration** - TaskHandler routing, TaskMiddleware for tools/call interception, capability negotiation, end-to-end lifecycle with basic example
- [ ] **Phase 4: DynamoDB Backend and Deployment** - Production DynamoDB storage with conditional writes, TTL, GSI pagination, CloudFormation template, and cargo-pmcp integration
- [ ] **Phase 5: Workflow Integration and Examples** - SequentialWorkflow backed by tasks, task variables in workflow steps, and remaining examples (workflow, code mode, DynamoDB)

## Phase Details

### Phase 1: Foundation Types and Store Contract
**Goal**: Developers can depend on `pmcp-tasks` and use correct, spec-compliant types that serialize to match the MCP 2025-11-25 schema exactly
**Depends on**: Nothing (first phase)
**Requirements**: TYPE-01, TYPE-02, TYPE-03, TYPE-04, TYPE-05, TYPE-06, TYPE-07, TYPE-08, TYPE-09, TYPE-10, STOR-01, STOR-02, STOR-03, STOR-04, HNDL-01, TEST-01, TEST-02
**Success Criteria** (what must be TRUE):
  1. `pmcp-tasks` crate compiles as a workspace member with `cargo check --package pmcp-tasks`
  2. All protocol types (Task, TaskStatus, CreateTaskResult, TaskParams, capability types, error types) round-trip through serde and match the MCP 2025-11-25 JSON schema
  3. TaskStatus state machine rejects invalid transitions (e.g., completed -> working) and accepts all valid transitions
  4. TaskStore trait is defined with all methods including atomic `complete_with_result`, and TaskRecord struct is usable as the store's internal record type
  5. TaskWithVariables extends Task with a variable store, and variables serialize into `_meta` (not flattened at top level)
**Plans**: 3 plans

Plans:
- [ ] 01-01-PLAN.md -- Crate scaffold, all wire types (Task, TaskStatus, params, capabilities, notification, execution), error enum, constants
- [ ] 01-02-PLAN.md -- Domain types (TaskRecord, TaskWithVariables) and TaskStore async trait with pagination
- [ ] 01-03-PLAN.md -- Serialization round-trip tests (TEST-01) and state machine transition tests (TEST-02)

### Phase 2: In-Memory Backend and Owner Security
**Goal**: Developers can create, poll, update, and cancel tasks using an in-memory store with enforced owner isolation and security limits
**Depends on**: Phase 1
**Requirements**: STOR-05, STOR-06, STOR-07, HNDL-02, HNDL-03, HNDL-04, HNDL-05, HNDL-06, SEC-01, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, TEST-03, TEST-04, TEST-06, TEST-07
**Success Criteria** (what must be TRUE):
  1. InMemoryTaskStore passes all TaskStore trait operations (create, get, update_status, set_variables, set_result, list, cancel, cleanup_expired) with correct results
  2. Owner isolation is enforced: a task created by owner A cannot be read, updated, listed, or cancelled by owner B
  3. TaskSecurityConfig limits are enforced: max tasks per owner rejects creation beyond limit, TTL defaults and maximums are applied, anonymous access is rejected when allow_anonymous is false
  4. TaskContext provides ergonomic variable access (get/set/delete via null) and status transition convenience methods (require_input, fail, complete) that work correctly against the in-memory store
  5. Property tests verify state machine transition invariants, variable merge semantics (including null-deletion), task ID uniqueness, and owner isolation under concurrent access
**Plans**: TBD

Plans:
- [ ] 02-01: TBD
- [ ] 02-02: TBD

### Phase 3: Handler, Middleware, and Server Integration
**Goal**: A PMCP server can advertise task support, intercept task-augmented tool calls, route all four task endpoints, and run a complete create-poll-complete lifecycle end to end
**Depends on**: Phase 2
**Requirements**: INTG-01, INTG-02, INTG-03, INTG-04, INTG-05, INTG-06, INTG-07, INTG-08, INTG-09, INTG-10, INTG-11, INTG-12, TEST-08, EXMP-01
**Success Criteria** (what must be TRUE):
  1. Server advertises task capabilities via `experimental.tasks` in the initialize response, and tools declare task support via `execution.taskSupport` in tools/list
  2. A tools/call request containing a `task` field returns a CreateTaskResult immediately and the tool executes in the background
  3. tasks/get returns current task state, tasks/result returns the operation result for terminal tasks, tasks/list returns paginated owner-scoped results, and tasks/cancel transitions non-terminal tasks to cancelled
  4. A full lifecycle integration test passes: create task via tools/call -> poll via tasks/get until terminal -> retrieve result via tasks/result
  5. The basic tasks example (`60_tasks_basic.rs`) compiles and runs successfully, demonstrating the complete task lifecycle
**Plans**: TBD

Plans:
- [ ] 03-01: TBD
- [ ] 03-02: TBD
- [ ] 03-03: TBD

### Phase 4: DynamoDB Backend and Deployment
**Goal**: Developers can use a production-ready DynamoDB storage backend for tasks in serverless environments, provisioned via cargo-pmcp deployment
**Depends on**: Phase 1 (store trait), Phase 3 (full integration for meaningful tests)
**Requirements**: STOR-08, STOR-09, STOR-10, STOR-11, STOR-12, STOR-13, TEST-05, EXMP-04
**Success Criteria** (what must be TRUE):
  1. DynamoDbTaskStore implements all TaskStore trait methods behind the `dynamodb` feature flag
  2. State transitions use conditional writes that reject invalid transitions atomically, with retries disabled on ConditionalCheckFailedException
  3. Expired tasks are filtered at read time (not relying solely on DynamoDB's delayed TTL deletion), and tasks/list uses GSI with cursor-based pagination scoped to owner
  4. Integration tests pass against a real DynamoDB table in CI (not mocked), covering CRUD, conditional writes, TTL expiry, and GSI pagination
  5. CloudFormation template for the DynamoDB table is provided and integrates with cargo-pmcp deployment plugin system
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD

### Phase 5: Workflow Integration and Examples
**Goal**: Existing SequentialWorkflow users can optionally back workflows with tasks for durable state, and all task usage patterns have working examples
**Depends on**: Phase 3 (task system), Phase 4 (DynamoDB for DynamoDB example)
**Requirements**: WKFL-01, WKFL-02, WKFL-03, WKFL-04, EXMP-02, EXMP-03
**Success Criteria** (what must be TRUE):
  1. A SequentialWorkflow can be configured with `.with_task_support()` and its steps read/write task variables via TaskContext
  2. DataSource::StepOutput resolves from task variables, enabling step outputs to persist across tool calls
  3. A workflow step that needs client input transitions the backing task to input_required, and the workflow resumes when the task returns to working
  4. Three additional examples compile and run: tasks with workflow (61_tasks_workflow.rs), tasks with code mode (62_tasks_code_mode.rs), and tasks with DynamoDB (63_tasks_dynamodb.rs)
**Plans**: TBD

Plans:
- [ ] 05-01: TBD
- [ ] 05-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Types and Store Contract | 0/3 | Complete    | 2026-02-21 |
| 2. In-Memory Backend and Owner Security | 0/? | Not started | - |
| 3. Handler, Middleware, and Server Integration | 0/? | Not started | - |
| 4. DynamoDB Backend and Deployment | 0/? | Not started | - |
| 5. Workflow Integration and Examples | 0/? | Not started | - |
