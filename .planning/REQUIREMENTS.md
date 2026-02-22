# Requirements: MCP Tasks for PMCP SDK

**Defined:** 2026-02-21
**Core Value:** Tool handlers can manage long-running operations through a durable task lifecycle with shared variable state that persists across tool calls.

## v1 Requirements

Requirements for initial release (`pmcp-tasks` v0.1.0). Each maps to roadmap phases.

### Core Types

- [x] **TYPE-01**: Protocol types (Task, TaskStatus, CreateTaskResult, TaskParams) serialize to match MCP 2025-11-25 schema exactly
- [x] **TYPE-02**: TaskStatus enum supports all 5 states (working, input_required, completed, failed, cancelled) with serde snake_case serialization
- [x] **TYPE-03**: Task status state machine validates transitions: working -> {input_required, completed, failed, cancelled}, input_required -> {working, completed, failed, cancelled}, terminal states reject all transitions
- [x] **TYPE-04**: Related-task metadata helper produces correct `io.modelcontextprotocol/related-task` JSON
- [x] **TYPE-05**: Task capability types (ServerTaskCapabilities, ClientTaskCapabilities) with convenience constructors (full, tools_only)
- [x] **TYPE-06**: TaskGetParams, TaskResultParams, TaskListParams, TaskCancelParams request types match spec schema
- [x] **TYPE-07**: TaskStatusNotification type matches spec notification structure
- [x] **TYPE-08**: TaskSupport enum (forbidden/optional/required) with ToolExecution metadata for tools/list
- [x] **TYPE-09**: TaskError variants map to spec-compliant JSON-RPC error codes (-32602, -32603)
- [x] **TYPE-10**: ModelImmediateResponse meta key constant defined for `io.modelcontextprotocol/model-immediate-response`

### Storage

- [x] **STOR-01**: TaskStore async trait with create, get, update_status, set_variables, set_result, get_result, list, cancel, cleanup_expired methods
- [x] **STOR-02**: TaskStore trait includes atomic `complete_with_result` method (single operation for status + result)
- [x] **STOR-03**: TaskStore trait enforces configurable variable size limits across all backends
- [x] **STOR-04**: TaskRecord includes protocol task fields, owner_id, variables, result, and request_method
- [ ] **STOR-05**: In-memory backend implements TaskStore with HashMap + synchronization
- [ ] **STOR-06**: In-memory backend validates state machine transitions atomically
- [ ] **STOR-07**: In-memory backend supports configurable poll interval and max TTL
- [ ] **STOR-08**: DynamoDB backend implements TaskStore behind `dynamodb` feature flag
- [ ] **STOR-09**: DynamoDB backend uses conditional writes for atomic state transitions
- [ ] **STOR-10**: DynamoDB backend uses native DynamoDB TTL + read-time expiry filtering
- [ ] **STOR-11**: DynamoDB backend uses GSI for owner-scoped listing with cursor-based pagination
- [ ] **STOR-12**: DynamoDB backend disables conditional write retries and uses ReturnValuesOnConditionCheckFailure
- [ ] **STOR-13**: CloudFormation template for DynamoDB table integrates with cargo-pmcp deployment plugin system

### Handler Integration

- [x] **HNDL-01**: TaskWithVariables type extends Task with shared variable store (HashMap<String, Value>)
- [ ] **HNDL-02**: Task variables surfaced to client via `_meta` in task responses
- [ ] **HNDL-03**: Variable merge semantics: new keys added, existing keys overwritten, null deletes
- [x] **HNDL-04**: TaskContext provides get_variable, set_variable, set_variables, variables methods
- [x] **HNDL-05**: TaskContext provides require_input, fail, complete convenience methods for status transitions
- [x] **HNDL-06**: TaskContext is Clone and wraps Arc<dyn TaskStore> for sharing across async boundaries

### Server Integration

- [x] **INTG-01**: Server task capabilities advertised via `experimental.tasks` field during initialization
- [x] **INTG-02**: Tool-level task support declared via `execution.taskSupport` in tools/list response
- [x] **INTG-03**: TaskMiddleware intercepts tools/call requests containing `task` field and creates task
- [x] **INTG-04**: TaskMiddleware returns CreateTaskResult immediately and spawns background tool execution
- [x] **INTG-05**: tasks/get endpoint returns current task state for polling
- [x] **INTG-06**: tasks/result endpoint returns the operation result for terminal tasks
- [x] **INTG-07**: tasks/list endpoint returns paginated tasks scoped to owner's authorization context
- [x] **INTG-08**: tasks/cancel endpoint transitions non-terminal tasks to cancelled status
- [x] **INTG-09**: TTL enforcement: receivers respect requested TTL, can override with max, clean up expired tasks
- [x] **INTG-10**: JSON-RPC routing handles tasks/get, tasks/result, tasks/list, tasks/cancel methods
- [x] **INTG-11**: progressToken from original request threaded through to background task execution
- [x] **INTG-12**: Model immediate response supported via optional `_meta` field in CreateTaskResult

### Security

- [ ] **SEC-01**: Owner ID resolved from OAuth sub claim, client ID, or session ID (priority order)
- [ ] **SEC-02**: Every task operation enforces owner matching (get, update, cancel, set_variables, set_result)
- [ ] **SEC-03**: tasks/list scoped to requesting owner only
- [ ] **SEC-04**: TaskSecurityConfig with configurable max_tasks_per_owner (default: 100)
- [ ] **SEC-05**: TaskSecurityConfig with configurable max_ttl_ms (default: 24 hours)
- [ ] **SEC-06**: TaskSecurityConfig with configurable default_ttl_ms (default: 1 hour)
- [ ] **SEC-07**: TaskSecurityConfig with allow_anonymous toggle (default: false)
- [ ] **SEC-08**: Task IDs use UUIDv4 (122 bits of entropy) to prevent guessing

### Workflow Integration

- [ ] **WKFL-01**: SequentialWorkflow can be optionally backed by a task via `.with_task_support()`
- [ ] **WKFL-02**: Workflow steps read/write task variables via TaskContext
- [ ] **WKFL-03**: DataSource::StepOutput resolves from task variables
- [ ] **WKFL-04**: Workflow step needing client input transitions task to input_required

### Testing

- [x] **TEST-01**: Protocol type serialization tests (all types round-trip correctly)
- [x] **TEST-02**: State machine transition tests (valid and invalid transitions, terminal state enforcement)
- [x] **TEST-03**: TaskContext behavior tests (variable CRUD, status transitions, complete with result)
- [x] **TEST-04**: In-memory store tests (CRUD, pagination, TTL, concurrent access)
- [ ] **TEST-05**: DynamoDB store integration tests (real cloud table in CI, conditional writes, GSI pagination)
- [x] **TEST-06**: Security tests (owner isolation, anonymous rejection, max tasks enforcement, UUID entropy)
- [x] **TEST-07**: Property tests (status transitions, variable merge, task ID uniqueness, owner isolation)
- [x] **TEST-08**: Full lifecycle integration tests (create -> poll -> complete -> get_result end-to-end)

### Examples

- [x] **EXMP-01**: Basic task-augmented tool call example (60_tasks_basic.rs)
- [ ] **EXMP-02**: Tasks with SequentialWorkflow example (61_tasks_workflow.rs)
- [ ] **EXMP-03**: Tasks with code mode example (62_tasks_code_mode.rs)
- [ ] **EXMP-04**: Tasks with DynamoDB backend example (63_tasks_dynamodb.rs)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Notifications

- **NOTF-01**: Task status notifications sent on status transitions (where transport supports)
- **NOTF-02**: Notifications are supplementary -- polling remains authoritative

### Advanced Storage

- **ADVS-01**: Redis storage backend
- **ADVS-02**: Task progress streaming via SSE integration

### Spec Stabilization

- **SPEC-01**: Move task types into core pmcp crate when spec drops experimental status
- **SPEC-02**: Add `tasks` field directly to ServerCapabilities/ClientCapabilities
- **SPEC-03**: Add `task` field directly to CallToolRequest

## Out of Scope

| Feature | Reason |
|---------|--------|
| Built-in task scheduler / job queue | SDK is not a runtime. Task execution is the server's responsibility. |
| Task-to-task dependencies / DAG execution | MCP tasks are independent state machines. Use SequentialWorkflow for ordering. |
| Task history / event log | MCP tasks are state machines, not event-sourced workflows. Use tracing/logging. |
| Automatic retry with backoff | SDK cannot know if operations are idempotent. Application concern. |
| Bounded blocking on tasks/result | Spec says MUST block until terminal. Polling-only pattern works for Lambda. |
| Namespaced variable keys | Flat keys with convention recommendation. Complexity without clear benefit. |
| Task status notifications (v1) | Skip for now, rely on polling. Add in v2 where transport allows. |
| Variable size limit per-backend | Use trait-level configurable limit for consistency across backends. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TYPE-01 | Phase 1 | Complete |
| TYPE-02 | Phase 1 | Complete |
| TYPE-03 | Phase 1 | Complete |
| TYPE-04 | Phase 1 | Complete |
| TYPE-05 | Phase 1 | Complete |
| TYPE-06 | Phase 1 | Complete |
| TYPE-07 | Phase 1 | Complete |
| TYPE-08 | Phase 1 | Complete |
| TYPE-09 | Phase 1 | Complete |
| TYPE-10 | Phase 1 | Complete |
| STOR-01 | Phase 1 | Complete |
| STOR-02 | Phase 1 | Complete |
| STOR-03 | Phase 1 | Complete |
| STOR-04 | Phase 1 | Complete |
| STOR-05 | Phase 2 | Pending |
| STOR-06 | Phase 2 | Pending |
| STOR-07 | Phase 2 | Pending |
| STOR-08 | Phase 4 | Pending |
| STOR-09 | Phase 4 | Pending |
| STOR-10 | Phase 4 | Pending |
| STOR-11 | Phase 4 | Pending |
| STOR-12 | Phase 4 | Pending |
| STOR-13 | Phase 4 | Pending |
| HNDL-01 | Phase 1 | Complete |
| HNDL-02 | Phase 2 | Pending |
| HNDL-03 | Phase 2 | Pending |
| HNDL-04 | Phase 2 | Complete |
| HNDL-05 | Phase 2 | Complete |
| HNDL-06 | Phase 2 | Complete |
| INTG-01 | Phase 3 | Complete |
| INTG-02 | Phase 3 | Complete |
| INTG-03 | Phase 3 | Complete |
| INTG-04 | Phase 3 | Complete |
| INTG-05 | Phase 3 | Complete |
| INTG-06 | Phase 3 | Complete |
| INTG-07 | Phase 3 | Complete |
| INTG-08 | Phase 3 | Complete |
| INTG-09 | Phase 3 | Complete |
| INTG-10 | Phase 3 | Complete |
| INTG-11 | Phase 3 | Complete |
| INTG-12 | Phase 3 | Complete |
| SEC-01 | Phase 2 | Pending |
| SEC-02 | Phase 2 | Pending |
| SEC-03 | Phase 2 | Pending |
| SEC-04 | Phase 2 | Pending |
| SEC-05 | Phase 2 | Pending |
| SEC-06 | Phase 2 | Pending |
| SEC-07 | Phase 2 | Pending |
| SEC-08 | Phase 2 | Pending |
| WKFL-01 | Phase 5 | Pending |
| WKFL-02 | Phase 5 | Pending |
| WKFL-03 | Phase 5 | Pending |
| WKFL-04 | Phase 5 | Pending |
| TEST-01 | Phase 1 | Complete |
| TEST-02 | Phase 1 | Complete |
| TEST-03 | Phase 2 | Complete |
| TEST-04 | Phase 2 | Complete |
| TEST-05 | Phase 4 | Pending |
| TEST-06 | Phase 2 | Complete |
| TEST-07 | Phase 2 | Complete |
| TEST-08 | Phase 3 | Complete |
| EXMP-01 | Phase 3 | Complete |
| EXMP-02 | Phase 5 | Pending |
| EXMP-03 | Phase 5 | Pending |
| EXMP-04 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 65 total
- Mapped to phases: 65
- Unmapped: 0

---
*Requirements defined: 2026-02-21*
*Last updated: 2026-02-21 after roadmap creation*
