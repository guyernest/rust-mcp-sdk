# Phase 39: Add Deep Merge for UI Meta Key to Prevent Collision - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Prevent `_meta` data loss when multiple systems contribute to the same `_meta` map on `ToolInfo`. Currently, `build_meta_map()` creates a fresh map with only UI keys, and `metadata()` returns it as the entire `_meta`. If anything else needs to add keys to `_meta` (execution config, custom metadata), it overwrites UI keys or vice versa. Fix: make each `with_*()` builder method merge into `_meta` rather than replacing it.

</domain>

<decisions>
## Implementation Decisions

### Merge strategy
- Deep merge: recursively merge nested objects so `with_ui()` adds `{ui: {resourceUri: ...}}`, `with_execution()` adds `{execution: ...}`, and both coexist
- If two contributors set the same nested key, last-in wins at the leaf level

### Collision rules
- Last-in wins — simple and predictable
- Builder methods are called in order; the last call sets the final value at that key
- Matches Rust's `HashMap::insert` semantics — user controls priority by call order

### API surface
- Builder chain pattern: each `with_*()` method (`with_ui`, `with_execution`, etc.) inserts its own keys into `_meta` directly
- No separate merge function needed — each builder method knows its own key namespace and inserts directly
- Existing `with_ui()` already knows it needs `ui` and `openai/outputTemplate` — it just inserts those into the existing map rather than replacing the whole map

### Scope
- Fix `ToolInfo._meta` only — this is where the collision actually happens (with_ui + execution + annotations all compete)
- `CallToolResult._meta` and `GetPromptResult._meta` are set by single sources and don't have the collision problem today

### Claude's Discretion
- Internal helper function design for the deep-merge utility
- Whether to add a public `deep_merge_meta()` or keep merge logic inside each `with_*()` method
- Test structure and naming conventions

</decisions>

<specifics>
## Specific Ideas

- Phase 37 established `ui_resource_uri: Option<String>` with `_meta` built via `build_meta_map()` in `metadata()`
- Phase 38 caches `_meta` at registration time — merge only needs to happen once at builder time, not per-request
- `TypedToolWithOutput::metadata()` currently sets `_meta: None`, losing any UI metadata if both `with_ui()` and output schema are used
- The `with_meta()` method on `CallToolResult` already replaces entirely — but that's a separate concern (result-time, not registration-time)

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ToolUIMetadata::build_meta_map()` at `src/types/ui.rs:276` — builds `{ui: {resourceUri}, openai/outputTemplate}` map
- `ToolInfo::with_ui()` at `src/types/protocol.rs:327` — static constructor that sets `_meta` from `build_meta_map()`
- `TypedTool::with_ui()` at `src/server/typed_tool.rs:205` — stores `ui_resource_uri`, builds `_meta` in `metadata()`
- `TypedSyncTool::with_ui()` at `src/server/typed_tool.rs:374` — same pattern
- `WasmTypedTool::with_ui()` at `src/server/wasm_typed_tool.rs:86` — same pattern

### Established Patterns
- `_meta` is `Option<serde_json::Map<String, Value>>` on `ToolInfo`
- Builder methods take `mut self` and return `Self`
- `metadata()` implementations construct `ToolInfo` with `_meta` set from `ui_resource_uri` if present, or `None`
- `TypedToolWithOutput::metadata()` at `src/server/typed_tool.rs:683` always sets `_meta: None` — collision point

### Integration Points
- `TypedTool::metadata()` at line 229 — needs to merge `ui_resource_uri` meta with any other `_meta` entries
- `TypedSyncTool::metadata()` at line 393 — same
- `TypedToolWithOutput::metadata()` at line 683 — currently ignores UI, needs to merge
- `WasmTypedTool::info()` at line 95 — same pattern for WASM
- `ServerCoreBuilder::tool()/tool_arc()` at `src/server/builder.rs` — where cached `_meta` is stored

</code_context>

<deferred>
## Deferred Ideas

- Apply deep-merge to `CallToolResult._meta` and `GetPromptResult._meta` — if middleware collision becomes a problem
- Meta key constants module (phase 35) — extract string literals to named constants
- Public `with_meta_entry(key, value)` API for arbitrary `_meta` contributions — if users need custom metadata

</deferred>

---

*Phase: 39-add-deep-merge-for-ui-meta-key-to-prevent-collision*
*Context gathered: 2026-03-06*
