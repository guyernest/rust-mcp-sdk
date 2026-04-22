# pmcp-code-mode: Deferred Improvements

**Captured:** 2026-04-12
**Source:** /simplify code review after Phase 67.1 Wave 1 (crate move from pmcp-run)
**Context:** These issues were discovered during the initial move into rust-mcp-sdk. They were not fixed during the move phase to preserve source fidelity with pmcp-run and avoid breaking subsequent Phase 67.1 plans (02-06). Each is a real issue worth addressing in a future cleanup phase.

---

## Performance (Hot Path)

### P-01: HashMap clone per array iteration
**File:** `src/eval.rs:487,501,516,531,546,559,579,647`
**Impact:** HIGH — every array method (`.map()`, `.filter()`, `.find()`, `.some()`, `.every()`, `.reduce()`, `.flatMap()`, `.sort()`) clones the entire `local_vars` HashMap per element. For `.map()` over 100 items with 10 scope variables, that's 100 full HashMap clones.
**Fix:** Replace with a scope-chain/stack design — push one binding, evaluate, pop — instead of cloning the whole map. This is the single largest performance improvement available in the execution engine.

### P-02: Double SWC parse (validation then execution)
**File:** `src/javascript.rs:452-477` (validation parse), `src/executor.rs:708-727` (compilation parse)
**Impact:** HIGH — SWC parsing is the heaviest operation per request. The same JS code is parsed twice: once during `validate_code` and again during `execute_code`. The parsed `Module` AST from validation should be passed through to the compiler.
**Fix:** Return the parsed AST from validation alongside the validation result. The `execute_code` path receives the pre-parsed AST and skips re-parsing. Requires a new type (e.g., `ValidatedCode { ast: Module, validation: ValidationResult }`).

### P-03: Double GraphQL parse on async fallback
**File:** `src/validation.rs:268` (first parse), `src/validation.rs:304` → `src/validation.rs:187` (second parse)
**Impact:** MEDIUM — when no policy evaluator is configured, `validate_graphql_query_async` parses the query, then calls the sync `validate_graphql_query` which parses it again.
**Fix:** Extract mutation authorization checks (lines 190-238) into a helper method `check_mutation_authorization(&self, query_info: &GraphQLQueryInfo) -> Result<(), ValidationResult>`, then call it from both the sync and async paths with the already-parsed `query_info`.

### P-04: Double code hash per token operation
**File:** `src/token.rs:69-83,173,207,222-226`
**Impact:** MEDIUM — `hash_code()` calls `canonicalize_code()` (allocates a new String) and `payload_bytes()` builds a format string → Vec<u8>. Both run during `generate()` AND `verify_code()`, and `payload_bytes` runs in both `sign()` and `verify_signature()`.
**Fix:** Cache the canonical form and code hash. Compute `payload_bytes` once per token operation.

### P-05: Per-field to_lowercase in GraphQL analysis
**File:** `src/graphql.rs:286-290`
**Impact:** LOW (FIXED in Wave 1 simplify) — sensitive fields are now pre-lowercased at construction time.
**Status:** RESOLVED

---

## Code Quality

### Q-01: Duplicate json_to_string / value_to_string
**File:** `src/eval.rs:445` (`json_to_string`), `src/eval.rs:1049` (`value_to_string`)
**Impact:** MEDIUM — both convert JSON values to strings with subtle behavioral differences (`json_to_string` renders Object as `"[object Object]"`, `value_to_string` uses `serde_json::to_string`). Both are `pub`.
**Fix:** Unify into one function with a parameter or enum controlling object rendering behavior. Remove the duplicate.

### Q-02: Parallel RiskLevel enums
**File:** `src/types.rs:10` (`RiskLevel { Low, Medium, High, Critical }`), `src/schema_exposure.rs:111` (`OperationRiskLevel { Safe, Low, Medium, High, Critical }`)
**Impact:** MEDIUM — two public enums representing the same concept (risk assessment) with overlapping variant sets but no conversion between them.
**Fix:** Consolidate into one enum (add `Safe` variant to `RiskLevel`) or add `From<OperationRiskLevel> for RiskLevel` impl.

### Q-03: compile_call() is ~400 lines of repetitive match arms
**File:** `src/executor.rs:1386-1786`
**Impact:** MEDIUM — callback-taking array methods (`map`, `filter`, `find`, `some`, `every`, `flatMap`) each follow an identical 8-line pattern. A helper like `compile_callback_method(call, array, ArrayMethodCall::Map)` would eliminate ~60 lines. The `ExtractedCall -> ValueExpr/PlanStep` conversion is also repeated 3 times (lines 1036, 1289, 1390).
**Fix:** Extract helper functions for the common patterns.

### Q-04: Error-as-control-flow for loop break/continue
**File:** `src/types.rs:489-494`
**Impact:** MEDIUM — `ExecutionError::LoopContinue` and `ExecutionError::LoopBreak` are control flow signals abusing the `Result` error channel. Every `execute_step` caller must pattern-match on "errors" that aren't errors.
**Fix:** Introduce `enum StepOutcome { Value(Value), None, Return(Value), Break, Continue }` returned from `execute_step` instead of overloading `Err`. This is a fundamental change to the execution model — scope carefully.

### Q-05: Redundant iteration limit check (dead code)
**File:** `src/executor.rs:2894-2898`
**Impact:** LOW (FIXED in Wave 1 simplify) — `.take(limit)` already bounds the iterator; the explicit `if i >= limit { break; }` was unreachable.
**Status:** RESOLVED

---

## Code Reuse / Architecture

### R-01: ValidationResult vs ValidationResponse duplication
**File:** `src/types.rs:159` (`ValidationResult`), `src/handler.rs:17` (`ValidationResponse`)
**Impact:** MEDIUM — near-identical structs (both have `is_valid`, `explanation`, `risk_level`, `approval_token`, `violations`, `metadata`, `warnings`). `ValidationResponse` adds `auto_approved`, `action`, `validated_code_hash`.
**Fix:** `ValidationResponse` should wrap or extend `ValidationResult` with the additional fields, not duplicate all of them.

### R-02: Workspace dependencies should use `[workspace.dependencies]`
**Impact:** LOW — the root `Cargo.toml` does not have a `[workspace.dependencies]` section yet. When one is added (likely during a workspace-wide cleanup), `pmcp-code-mode` deps like `serde`, `serde_json`, `tokio`, `async-trait`, `thiserror`, `tracing`, `sha2`, `base64`, `uuid`, `chrono` should use `workspace = true` to prevent version divergence.
**Blocked on:** Adding `[workspace.dependencies]` to root Cargo.toml — affects all workspace members.

### R-03: `hex` crate could be replaced by `base16ct`
**File:** `Cargo.toml:28`
**Impact:** LOW — `hex` is a new dependency in the workspace. `base16ct` from the RustCrypto ecosystem may already be pulled transitively and could replace it. Minor dep reduction.

### R-04: ValidationError naming collision
**File:** `src/types.rs:422` vs root `src/server/validation.rs:11`
**Impact:** LOW — both define `ValidationError` but with different shapes (code-mode is domain-specific, pmcp is elicitation-oriented). Not a functional issue, but confusing for code that imports both crates.
**Fix:** Consider renaming to `CodeModeValidationError` or using a module-qualified import convention.

---

## Priority Recommendation

For a future cleanup phase, address in this order:
1. **P-01** (HashMap clone) — largest single performance win
2. **P-02** (double SWC parse) — second largest performance win
3. **Q-04** (error-as-control-flow) — cleanest architectural improvement
4. **R-01** (ValidationResult/Response) — reduces confusion for derive macro consumers
5. **Q-03** (compile_call refactor) — reduces maintenance burden in the largest file
