# Phase 11: DynamoDB Backend - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement `DynamoDbBackend` that implements the `StorageBackend` trait (from Phase 9-10) behind a `dynamodb` feature flag, using the `aws-sdk-dynamodb` crate. Developers can persist tasks in DynamoDB for production AWS/Lambda deployments. The backend assumes the table already exists with the correct schema. Table provisioning tooling is deferred to a later phase.

</domain>

<decisions>
## Implementation Decisions

### Table design & key schema
- Owner-scoped partition key: `PK = OWNER#<owner_id>`, `SK = TASK#<task_id>` -- natural owner isolation, efficient list-by-owner queries
- Single-table design as specified in success criteria
- Human-readable attribute names: `owner_id`, `task_id`, `status`, `version`, `expires_at`, `data` -- aligns with Phase 9's JSON debuggability decision
- TTL attribute (`expires_at`) only set on tasks that have an explicit expiry configured -- no dummy far-future values for tasks without expiry
- DynamoDB native TTL enabled on the `expires_at` attribute for automatic cleanup

### Configuration & construction
- Two construction paths: `DynamoDbBackend::new(client, table_name)` accepts a pre-built `aws_sdk_dynamodb::Client` for full flexibility; `from_env()` or similar auto-discovers configuration from the standard AWS SDK config chain
- Default table name: `pmcp_tasks`, overridable via config
- No special endpoint URL support in the backend -- developers configure DynamoDB Local/LocalStack via standard AWS SDK environment variables (`AWS_ENDPOINT_URL`) or by constructing the client themselves
- Backend assumes table exists with correct schema; fails fast with clear error if table is missing or misconfigured

### Integration testing strategy
- Tests run against real DynamoDB in AWS (not DynamoDB Local)
- Gated behind `dynamodb-tests` feature flag: `cargo test --features dynamodb-tests`
- Test isolation via shared table with unique owner prefix per test run (random UUID prefix, no table creation/deletion overhead)
- TTL testing: verify the `expires_at` attribute is correctly set on items, do not wait for DynamoDB's actual TTL deletion (up to 48 hours)

### Error mapping & retries
- Use the AWS SDK's built-in retry with exponential backoff for transient failures (throttling, network). Phase 9's "no auto-retry" applies to our code layer, not the SDK's transport layer
- DynamoDB `ConditionalCheckFailedException` maps to `StorageError::ConcurrentModification`
- All unexpected DynamoDB errors map to generic `StorageError::Backend(Box<dyn Error>)` -- keep the error model backend-agnostic, no DynamoDB-specific variants
- Size limit (350KB) enforced in GenericTaskStore via StoreConfig, not in DynamoDbBackend -- backend never sees oversized items

### Claude's Discretion
- CAS error detail: whether to include actual_version by doing an extra read after ConditionalCheckFailedException, or report expected_version only
- Internal DynamoDB attribute mapping details (how TaskRecord fields map to DynamoDB attributes)
- Construction ergonomics (`from_env` naming, builder pattern, etc.)
- GSI design if needed for any access patterns beyond PK/SK queries

</decisions>

<specifics>
## Specific Ideas

- Table provisioning should eventually be a CLI command: `cargo pmcp tasks init --backend dynamodb` -- but that's a separate phase
- The `InMemoryBackend` from Phase 10 serves as the reference implementation for the `StorageBackend` trait contract
- Phase 9 context established: JSON serialization, TaskRecord-based trait, domain-aware errors, no content sanitization
- Per the Phase 9 security posture: owner binding enforced structurally via composite PK, NotFound on owner mismatch

</specifics>

<deferred>
## Deferred Ideas

- `cargo pmcp tasks init --backend dynamodb` CLI command for table provisioning -- future phase
- DynamoDB Local support as a first-class testing option -- currently handled via AWS SDK config

</deferred>

---

*Phase: 11-dynamodb-backend*
*Context gathered: 2026-02-23*
