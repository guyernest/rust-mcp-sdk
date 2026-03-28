# Codebase Concerns

**Analysis Date:** 2026-02-26

## Tech Debt

**Panicking Error Handling in Core Functions:**
- Issue: 168+ unwrap() calls throughout the codebase - particularly in publishing, manifest generation, and landing page generation
- Files: `src/publishing/landing.rs` (28 unwrap calls), `src/publishing/manifest.rs` (30+ unwrap calls)
- Impact: CLI tool crashes on unexpected input rather than providing user-friendly error messages; production deployments fail ungracefully
- Fix approach: Replace unwrap()/expect() with Result types and proper error propagation using anyhow::Context for clarity

**Excessive clone() Usage:**
- Issue: 635+ clone()/to_string()/String::from calls throughout codebase creating unnecessary allocations
- Files: `src/commands/deploy/init.rs` (common in builder patterns), `src/deployment/targets/pmcp_run/auth.rs`
- Impact: Increased memory pressure, slower performance, especially in hot paths like OAuth flows and GraphQL requests
- Fix approach: Use references where possible, redesign builder patterns to avoid intermediate clones, use Cow<str> for conditional ownership

**Unimplemented Test Doubles in Production Code:**
- Issue: 9 unimplemented!() macros in MockTarget struct used in tests
- Files: `src/deployment/registry.rs` lines 125, 133, 141, 148, 157, 165, 173, 181, 189
- Impact: Test artifacts in codebase that fail at runtime if accidentally called; indicates incomplete test infrastructure
- Fix approach: Move mock implementations to test-only modules using #[cfg(test)], use mockito or similar mocking crate instead

**Incomplete Feature Implementations:**
- Issue: 9 TODO comments indicating missing functionality
- Files:
  - `src/secrets/providers/aws.rs` lines 95, 103, 116, 124 - AWS Secrets Manager operations not implemented (list, get, set, delete)
  - `src/deployment/targets/cloudflare/init.rs` lines 594, 645 - Cloudflare adapter placeholders
  - `src/commands/landing/mod.rs` line 87 - Landing page implementation incomplete
  - `src/commands/add.rs` lines 261, 277 - Tool and workflow scaffolding not implemented
- Impact: Features advertised as available but non-functional; users will encounter NotImplementedError at runtime
- Fix approach: Either implement the features completely or remove/disable them until ready

**Stubbed Validation Tests:**
- Issue: 6 TODO items in validation command with hardcoded "TODO:" println statements
- Files: `src/commands/validate.rs` lines 353, 361, 370, 378, 385, 393, 412
- Impact: Validation command appears to work but performs no actual validation
- Fix approach: Implement workflow validation logic properly before shipping validate command

## Security Considerations

**AWS Credentials Handling in Auth:**
- Risk: Environment variable checks for AWS credentials without validation; OAuth callback localhost server on hardcoded port 8787
- Files: `src/deployment/targets/pmcp_run/auth.rs` line 133, line 48 (CALLBACK_PORT constant)
- Current mitigation: Standard AWS credential chain used, Cognito-provided tokens processed
- Recommendations:
  - Validate AWS credentials are actually valid before returning healthy status
  - Make callback port configurable to avoid port conflicts
  - Add PKCE flow validation (already using PkceCodeChallenge, ensure validation is complete)
  - Store tokens in secure storage (not plaintext if written to disk)

**Secrets Manager Not Fully Hardened:**
- Risk: AWS Secrets Manager provider has placeholder implementation that returns errors; local filesystem provider stores secrets in plaintext JSON
- Files: `src/secrets/providers/aws.rs`, `src/secrets/providers/local.rs`
- Current mitigation: Feature flag guards AWS provider, local provider only for development
- Recommendations:
  - Implement AWS Secrets Manager integration with proper retry and timeout handling
  - Document that local provider is development-only with security warnings
  - Add encryption option for local secrets storage
  - Implement secret rotation support

**Secret Name Validation Missing Depth:**
- Risk: Secret name format validation only checks characters, not max length (AWS limit is 512 chars)
- Files: `src/secrets/providers/aws.rs` lines 67-92
- Current mitigation: AWS SDK would catch on actual API calls
- Recommendations: Add length validation, add reserved name checks

## Performance Bottlenecks

**String Allocations in OAuth Flow:**
- Problem: Every OAuth configuration discovery creates new String allocations
- Files: `src/deployment/targets/pmcp_run/auth.rs` lines 80-92 (get_api_base_url, multiple unwrap_or chains creating Strings)
- Cause: Configuration discovery repeatedly allocates strings for environment variable lookups
- Improvement path: Cache discovered configuration (already implemented with CachedConfig), use static strings for defaults instead of unwrap_or creating new Strings

**GraphQL Request Serialization Overhead:**
- Problem: Every deployment upload generates a new GraphQL request with full object serialization
- Files: `src/deployment/targets/pmcp_run/graphql.rs` (GraphQLRequest structure)
- Cause: No request batching or connection pooling; reqwest client created fresh for each request
- Improvement path: Implement request pooling, batch multiple mutations, reuse HTTP client instance

**Large File Scanning Without Streaming:**
- Problem: Mock data and schema discovery load entire files into memory
- Files: `src/publishing/landing.rs` (load_mock_data function), `src/commands/schema.rs`
- Cause: Simple walkdir iteration with fs::read_to_string for every file
- Improvement path: Implement streaming JSON parser for large datasets, add file size limits

## Known Issues

**Landing Page HTML String Search Using unwrap():**
- Symptoms: CLI crashes if HTML template structure changes (missing expected tags)
- Files: `src/publishing/landing.rs` lines 341, 342, 355, 356 (looking for `<script type="module">`, `</head>`, `<body>` tags)
- Trigger: If template generation changes tag locations or format, HTML manipulation fails
- Workaround: None - code must be fixed before template changes can be safely made
- Fix: Use HTML parser (html5ever) instead of string searching, add comprehensive test coverage for template structure

**Manifest Generation Assumes JSON Structure:**
- Symptoms: Panic on unexpected Cargo.toml format or missing expected fields
- Files: `src/publishing/manifest.rs` lines 114-163 (multiple unwrap() on parsed JSON)
- Trigger: Non-standard Cargo.toml files or missing required metadata fields
- Workaround: Ensure Cargo.toml follows standard format
- Fix: Add defensive parsing with clear error messages for missing fields

**OAuth Configuration Cache May Become Stale:**
- Symptoms: Updated Cognito configuration not reflected after 1 hour cache duration
- Files: `src/deployment/targets/pmcp_run/auth.rs` line 55 (CONFIG_CACHE_DURATION_SECS = 3600)
- Trigger: Administrator updates Cognito configuration while CLI is in use
- Workaround: Delete .pmcp/pmcp-run-config.json cache file to force refresh
- Fix: Add cache invalidation endpoint or shorter default TTL with bypass option

## Fragile Areas

**Landing Page Generation Module:**
- Files: `src/publishing/landing.rs` (510 lines)
- Why fragile: Heavy reliance on HTML string manipulation with unwrap() calls; no structured HTML parsing; 28+ unwrap() calls on find() operations
- Safe modification: Add comprehensive HTML structure tests; consider switching to typed HTML builder library; add validation that all expected elements exist before string manipulation
- Test coverage: Zero existing tests for HTML generation logic

**Deployment Configuration State Machine:**
- Files: `src/commands/deploy/init.rs` (1705 lines - largest file), `src/deployment/config.rs` (716 lines)
- Why fragile: Complex initialization flow with multiple optional OAuth configurations; state transitions not formally validated; Cognito config creation has 6 builder methods that can conflict
- Safe modification: Add integration tests for all OAuth provider combinations; add invariant checks at configuration creation time
- Test coverage: Minimal - no tests for initialization flow

**GraphQL Integration to pmcp.run:**
- Files: `src/deployment/targets/pmcp_run/graphql.rs` (1189 lines)
- Why fragile: Hardcoded GraphQL queries; no retry logic; assumes specific response structure; 15-minute presigned URL expiry with no refresh mechanism
- Safe modification: Move GraphQL queries to external files; add comprehensive error handling for API failures; implement request retry with exponential backoff
- Test coverage: No tests for GraphQL mutation success/failure cases

**AWS CDK Template Generation:**
- Files: `src/commands/deploy/init.rs` (embedded TypeScript string starting line ~1500)
- Why fragile: CDK infrastructure defined as generated string literal (3000+ lines); no type checking of generated code; if AWS CDK API changes, generated code becomes invalid
- Safe modification: Use aws-cdk-lib TypeScript directly with template variables; generate from structured data representation
- Test coverage: No validation that generated CDK code is syntactically correct

## Test Coverage Gaps

**Publishing Module Untested:**
- What's not tested: Landing page HTML generation, manifest creation, project detection
- Files: `src/publishing/landing.rs` (0 tests), `src/publishing/manifest.rs` (extensive unwrap but minimal test coverage)
- Risk: Changes to landing page or manifest structure silently break production deployments
- Priority: High - landing page is user-facing feature

**Deployment Init Command No Tests:**
- What's not tested: Configuration creation, OAuth provider setup, Cargo.toml parsing
- Files: `src/commands/deploy/init.rs` (34 async functions, 0 tests)
- Risk: Deployment initialization logic works unexpectedly in corner cases (missing fields, workspace structures, OAuth combinations)
- Priority: High - critical deployment path

**Secrets Provider Plugin System Untested:**
- What's not tested: Provider registration, credential validation, secret storage isolation
- Files: `src/secrets/providers/` (only aws.rs has 3 basic unit tests)
- Risk: Adding new providers or switching providers may silently fail
- Priority: Medium - feature not yet critical

**Validation Command Tests Are Stubs:**
- What's not tested: Actual workflow validation
- Files: `src/commands/validate.rs` (has test structure but all test functions print "TODO:")
- Risk: Command appears successful when validation should fail
- Priority: High - validation command is unreliable

## Missing Critical Features

**AWS Secrets Manager Implementation:**
- Problem: AWS Secrets Manager provider is advertised but returns "not yet implemented" error for all operations
- Blocks: Users cannot store secrets in AWS Secrets Manager for production deployments
- Current behavior: list(), get(), set(), delete() all return error
- Migration path: Implement using aws-sdk-secretsmanager crate (dependency present but unused)

**Cloudflare Deployment Adapter:**
- Problem: Cloudflare Workers deployment shows TODO placeholders instead of actual implementation
- Blocks: Users cannot deploy to Cloudflare Workers
- Current behavior: init.rs lines 594, 645 have TODO comments with placeholder text
- Migration path: Implement using cloudflare-workers crate or wasm runtime adapter

**Tool and Workflow Scaffolding:**
- Problem: `cargo pmcp add tool` and `cargo pmcp add workflow` commands are stubbed
- Blocks: Users cannot easily add new MCP tools or workflows to their server
- Current behavior: Commands print "TODO: Implement tool/workflow scaffolding" and exit
- Migration path: Create template generation for tool and workflow boilerplate

**Watch Mode in Landing Development:**
- Problem: Landing page dev command has watch mode flag but no implementation
- Blocks: Developers must restart landing dev server manually on file changes
- Current behavior: _watch parameter accepted but ignored
- Migration path: Implement file watcher with hot reload using notify crate

## Dependencies at Risk

**No Version Constraints in Cargo.toml:**
- Risk: Minor version bumps in tokio, serde, reqwest could break functionality without notice
- Current: No MSRV (Minimum Supported Rust Version) declared
- Impact: CI/CD might fail on newer Rust versions due to API changes
- Migration plan: Add explicit version constraints, test against stable/beta/nightly, declare MSRV in Cargo.toml

**AWS SDK Features Highly Conditional:**
- Risk: aws-secrets feature flag has significant code paths, inconsistent testing across feature combinations
- Current: Features guarded by #[cfg(feature = "aws-secrets")] but tests don't run with feature enabled
- Impact: AWS provider code could have runtime errors not caught by CI
- Migration plan: Run full test suite with and without optional features; add feature-specific integration tests

## Scaling Limits

**Config Cache File Blocking:**
- Current capacity: Single file-based cache (pmcp-run-config.json) shared across all CLI invocations
- Limit: Concurrent CLI commands may have race condition on cache read/write
- Scaling path: Implement in-memory cache with file persistence, add file locking

**GraphQL Request Timeout Not Configurable:**
- Current: Fixed 30-second timeout for deployment uploads
- Limit: Large template files (>5MB) may timeout before upload completes
- Scaling path: Make timeout configurable via env var, implement resumable uploads

**Landing Page Mock Data Limited:**
- Current: Scans mock-data/ directory for *.json files, loads all into memory
- Limit: Large mock datasets (>100MB) not practical; no pagination in UI
- Scaling path: Implement streaming JSON responses, add data size limits and warnings, implement client-side pagination

## Logging and Observability Gaps

**CLI Uses println! Instead of Structured Logging:**
- Issue: 1215+ println!/eprintln! calls throughout codebase
- Files: Every command and module uses println! directly
- Impact: Hard to parse logs programmatically, no log levels, no timestamps, no structured fields
- Fix approach: Implement tracing or log crate usage; add log level support; consider using indicatif for progress bars

**No Metrics for Deployment Success/Failure:**
- Issue: No instrumentation of deployment paths
- Files: `src/commands/deploy/`, `src/deployment/targets/`
- Impact: No visibility into why deployments fail at scale
- Fix approach: Add span/event instrumentation to key operations, export metrics in structured format

## Code Quality Issues

**Excessive use of #[allow(dead_code)]:**
- Issue: 30+ functions marked #[allow(dead_code)] suggesting dead code paths
- Files: `src/secrets/`, `src/deployment/registry.rs`, throughout
- Impact: Unclear which code is actually used; increases maintenance burden
- Fix approach: Remove unused functions or actually use them; if for future use, document why

**File Size Complexity:**
- Issue: `src/commands/deploy/init.rs` is 1705 lines, `src/deployment/targets/pmcp_run/graphql.rs` is 1189 lines
- Impact: Hard to understand control flow, difficult to test in isolation
- Fix approach: Break into smaller modules; extract template generation to separate functions/files; move GraphQL logic to dedicated query module

**Placeholder Tests in validate.rs:**
- Issue: Test functions exist but contain println!("TODO: ...") instead of actual assertions
- Files: `src/commands/validate.rs` lines 370, 385, 412
- Impact: Tests pass when they should fail; false sense of coverage
- Fix approach: Either implement validation tests properly or remove placeholder tests

---

*Concerns audit: 2026-02-26*
