# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- ✅ **v1.1 Task-Prompt Bridge** — Phases 4-8 (shipped 2026-02-23)
- ✅ **v1.2 Pluggable Storage Backends** — Phases 9-13 (shipped 2026-02-24)
- ✅ **v1.3 MCP Apps Developer Experience** — Phases 14-19 (shipped 2026-02-26)
- ✅ **v1.4 Book & Course Update** — Phases 20-24 (shipped 2026-02-28)
- ✅ **v1.5 Cloud Load Testing Upload** — Phases 25-26 (shipped 2026-03-01)
- **v1.6 CLI DX Overhaul** — Phases 27-32 (in progress)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.1 Task-Prompt Bridge (Phases 4-8) — SHIPPED 2026-02-23</summary>

- [x] Phase 4: Foundation Types and Contracts (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Partial Execution Engine (2/2 plans) — completed 2026-02-23
- [x] Phase 6: Structured Handoff and Client Continuation (2/2 plans) — completed 2026-02-23
- [x] Phase 7: Integration and End-to-End Validation (2/2 plans) — completed 2026-02-23
- [x] Phase 8: Quality Polish and Test Coverage (2/2 plans) — completed 2026-02-23

See: `.planning/milestones/v1.1-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.2 Pluggable Storage Backends (Phases 9-13) — SHIPPED 2026-02-24</summary>

- [x] Phase 9: Storage Abstraction Layer (2/2 plans) — completed 2026-02-24
- [x] Phase 10: InMemory Backend Refactor (2/2 plans) — completed 2026-02-24
- [x] Phase 11: DynamoDB Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 12: Redis Backend (2/2 plans) — completed 2026-02-24
- [x] Phase 13: Feature Flag Verification (1/1 plans) — completed 2026-02-24

See: `.planning/milestones/v1.2-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.3 MCP Apps Developer Experience (Phases 14-19) — SHIPPED 2026-02-26</summary>

- [x] Phase 14: Preview Bridge Infrastructure (2/2 plans) — completed 2026-02-24
- [x] Phase 15: WASM Widget Bridge (2/2 plans) — completed 2026-02-25
- [x] Phase 16: Shared Bridge Library (2/2 plans) — completed 2026-02-26
- [x] Phase 17: Widget Authoring DX and Scaffolding (2/2 plans) — completed 2026-02-26
- [x] Phase 18: Publishing Pipeline (2/2 plans) — completed 2026-02-26
- [x] Phase 19: Ship Examples and Playwright E2E (2/2 plans) — completed 2026-02-26

See: `.planning/milestones/v1.3-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.4 Book & Course Update (Phases 20-24) — SHIPPED 2026-02-28</summary>

- [x] Phase 20: Book Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 21: Book MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 22: Course Load Testing (2/2 plans) — completed 2026-02-28
- [x] Phase 23: Course MCP Apps Refresh (2/2 plans) — completed 2026-02-28
- [x] Phase 24: Course Quizzes & Exercises (2/2 plans) — completed 2026-02-28

See: `.planning/milestones/v1.4-ROADMAP.md` for full phase details

</details>

<details>
<summary>v1.5 Cloud Load Testing Upload (Phases 25-26) — SHIPPED 2026-03-01</summary>

- [x] Phase 25: Loadtest Config Upload (2/2 plans) — completed 2026-02-28
- [x] Phase 26: Add OAuth Support to Load Testing (4/4 plans) — completed 2026-03-01

See phase details in `.planning/phases/25-*` and `.planning/phases/26-*`

</details>

### v1.6 CLI DX Overhaul (In Progress)

**Milestone Goal:** Normalize the cargo pmcp CLI for consistency and developer experience ahead of course recording -- fix flag inconsistencies, propagate auth to all server-facing commands, surface mcp-tester via `cargo pmcp test`, and add doctor/completions commands.

- [x] **Phase 27: Global Flag Infrastructure** - Add --no-color and --quiet as global flags available on all commands (completed 2026-03-04)
- [ ] **Phase 28: Flag Normalization** - Rename and normalize all per-command flags for consistency (positional URL, --server, --verbose, --yes, -o, --format, #[arg()])
- [ ] **Phase 29: Auth Flag Propagation** - Add shared OAuth and API-key flag structs to all server-facing commands
- [ ] **Phase 30: Tester CLI Integration** - Surface mcp-tester subcommands through cargo pmcp test with aligned flags
- [ ] **Phase 31: New Commands** - Add cargo pmcp doctor and cargo pmcp completions commands
- [ ] **Phase 32: Help Text Polish** - Consistent help text format with descriptions and usage examples across all commands

## Phase Details

### Phase 27: Global Flag Infrastructure
**Goal**: Every cargo pmcp invocation supports --no-color and --quiet for scripting and CI use
**Depends on**: Phase 26 (v1.5 complete)
**Requirements**: FLAG-08, FLAG-09
**Success Criteria** (what must be TRUE):
  1. User can pass `--no-color` to any cargo pmcp command and all terminal output is plain text (no ANSI escape codes)
  2. User can pass `--quiet` to any cargo pmcp command and only errors and explicit requested output appear
  3. Both flags work when placed before or after the subcommand (global position)
**Plans**: 2 plans
Plans:
- [ ] 27-01-PLAN.md — GlobalFlags struct, --no-color/--quiet CLI args, wire through all command dispatch, global color suppression
- [ ] 27-02-PLAN.md — Quiet mode output filtering across all commands, verbose-wins-over-quiet precedence

### Phase 28: Flag Normalization
**Goal**: Every existing cargo pmcp command uses the same conventions for URLs, server references, verbosity, confirmations, output, and format values
**Depends on**: Phase 27
**Requirements**: FLAG-01, FLAG-02, FLAG-03, FLAG-04, FLAG-05, FLAG-06, FLAG-07
**Success Criteria** (what must be TRUE):
  1. User can pass a server URL as a positional argument to any command that connects to a server (no more `--url` or `--endpoint`)
  2. User can use `--server` consistently for pmcp.run server references (no more `--server-id`)
  3. User can use `--verbose` / `-v` for detailed output on any command (no more `--detailed`)
  4. User can use `--yes` to skip confirmations and `-o` as shorthand for `--output` on any command that supports them
  5. All `--format` flags accept `text` and `json` as values (no other human-readable format names)
**Plans**: TBD

### Phase 29: Auth Flag Propagation
**Goal**: Every command that connects to an MCP server accepts OAuth and API-key authentication flags
**Depends on**: Phase 28
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, AUTH-05, AUTH-06
**Success Criteria** (what must be TRUE):
  1. User can pass `--api-key <key>` to test check/run/generate, preview, schema export, and connect commands
  2. User can pass OAuth flags (--oauth-issuer, --oauth-client-id, --oauth-scopes, --oauth-no-cache, --oauth-redirect-port) to any of those same commands
  3. Auth flags are defined in a shared struct (AuthFlags or similar) flattened into each command, not duplicated per command
  4. Commands that already had auth support (e.g., loadtest) continue to work unchanged
**Plans**: TBD

### Phase 30: Tester CLI Integration
**Goal**: Users can run all mcp-tester capabilities through cargo pmcp test subcommands with consistent flag conventions
**Depends on**: Phase 29
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-07, TEST-08
**Success Criteria** (what must be TRUE):
  1. User can run `cargo pmcp test compliance <url>`, `cargo pmcp test diagnose <url>`, and `cargo pmcp test compare <url1> <url2>` to validate MCP servers
  2. User can run `cargo pmcp test tools <url>`, `cargo pmcp test resources <url>`, `cargo pmcp test prompts <url>`, and `cargo pmcp test health <url>` to inspect server capabilities
  3. All `cargo pmcp test` subcommands accept the same auth flags (--api-key, OAuth) and global flags (--verbose, --no-color, --quiet) established in prior phases
  4. The standalone `mcp-tester` binary uses the same flag conventions as `cargo pmcp test` (positional URL, --verbose/-v, --yes)
**Plans**: TBD

### Phase 31: New Commands
**Goal**: Users have workspace diagnostics and shell completion generation built into the CLI
**Depends on**: Phase 28
**Requirements**: CMD-01, CMD-02
**Success Criteria** (what must be TRUE):
  1. User can run `cargo pmcp doctor` and see validation results for workspace structure, Rust toolchain, config files, and optionally server connectivity
  2. User can run `cargo pmcp completions bash` (or zsh/fish/powershell) and pipe the output to the appropriate shell config file
  3. Both commands follow all established flag conventions (global flags, --format, help text patterns)
**Plans**: TBD

### Phase 32: Help Text Polish
**Goal**: Every cargo pmcp command has professional, consistent help output ready for course recording
**Depends on**: Phase 31
**Requirements**: HELP-01, HELP-02
**Success Criteria** (what must be TRUE):
  1. Every command's `--help` output includes a description, grouped options (by category: connection, auth, output, etc.), and a usage examples section via `after_help`
  2. All help text follows the same structural pattern: synopsis line, categorized options, examples section
  3. Running `cargo pmcp --help` shows a clean top-level overview with all subcommands and their one-line descriptions
**Plans**: TBD

## Progress

**Execution Order:** Phase 27 next

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Handoff + Continuation | v1.1 | 2/2 | Complete | 2026-02-23 |
| 7. Integration | v1.1 | 2/2 | Complete | 2026-02-23 |
| 8. Quality Polish | v1.1 | 2/2 | Complete | 2026-02-23 |
| 9. Storage Abstraction | v1.2 | 2/2 | Complete | 2026-02-24 |
| 10. InMemory Refactor | v1.2 | 2/2 | Complete | 2026-02-24 |
| 11. DynamoDB Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 12. Redis Backend | v1.2 | 2/2 | Complete | 2026-02-24 |
| 13. Feature Flags | v1.2 | 1/1 | Complete | 2026-02-24 |
| 14. Preview Bridge | v1.3 | 2/2 | Complete | 2026-02-24 |
| 15. WASM Bridge | v1.3 | 2/2 | Complete | 2026-02-25 |
| 16. Shared Bridge Lib | v1.3 | 2/2 | Complete | 2026-02-26 |
| 17. Authoring DX | v1.3 | 2/2 | Complete | 2026-02-26 |
| 18. Publishing | v1.3 | 2/2 | Complete | 2026-02-26 |
| 19. Ship + E2E | v1.3 | 2/2 | Complete | 2026-02-26 |
| 20. Book Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 21. Book MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 22. Course Load Testing | v1.4 | 2/2 | Complete | 2026-02-28 |
| 23. Course MCP Apps | v1.4 | 2/2 | Complete | 2026-02-28 |
| 24. Course Quizzes | v1.4 | 2/2 | Complete | 2026-02-28 |
| 25. Loadtest Upload | v1.5 | 2/2 | Complete | 2026-02-28 |
| 26. OAuth Load Testing | v1.5 | 4/4 | Complete | 2026-03-01 |
| 27. Global Flag Infrastructure | 3/3 | Complete   | 2026-03-04 | - |
| 28. Flag Normalization | v1.6 | 0/? | Not started | - |
| 29. Auth Flag Propagation | v1.6 | 0/? | Not started | - |
| 30. Tester CLI Integration | v1.6 | 0/? | Not started | - |
| 31. New Commands | v1.6 | 0/? | Not started | - |
| 32. Help Text Polish | v1.6 | 0/? | Not started | - |

### Phase 33: Fix mcp-tester failure with v1.12.0

**Goal:** Bump mcp-tester to 0.2.2 and cargo-pmcp to 0.3.4, publish both to crates.io so `cargo install cargo-pmcp` works without `--locked`
**Requirements**: None (hotfix)
**Depends on:** Phase 32
**Plans:** 1 plan

Plans:
- [ ] 33-01-PLAN.md — Version bumps and crates.io publish

### Phase 34: Fix MCP Apps ChatGPT compatibility

**Goal:** Fix SDK metadata format, MIME types, and mcp-preview routes to be compatible with ChatGPT's MCP Apps implementation
**Requirements**: CHATGPT-01, CHATGPT-02, CHATGPT-03, CHATGPT-04, CHATGPT-05, CHATGPT-06
**Depends on:** Phase 33
**Plans:** 2/2 plans complete

Plans:
- [x] 34-01-PLAN.md — Fix tool _meta format (nested ui.resourceUri + openai/outputTemplate), add MIME type variant, dual-emit WidgetMeta
- [ ] 34-02-PLAN.md — Fix mcp-preview axum 0.8 wildcard route panic

### Phase 35: Add meta key constants module for UI/MCP Apps strings

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 34
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 35 to break down)

### Phase 36: Unify UIMimeType and ExtendedUIMimeType with From bridge

**Goal:** Add From/TryFrom conversion traits between UIMimeType and ExtendedUIMimeType so code can seamlessly convert across the feature-gate boundary
**Requirements**: MIME-BRIDGE-01
**Depends on:** Phase 35
**Plans:** 1 plan

Plans:
- [ ] 36-01-PLAN.md — TDD: From<UIMimeType> for ExtendedUIMimeType and TryFrom<ExtendedUIMimeType> for UIMimeType

### Phase 37: Add with_ui support to TypedSyncTool

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 36
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 37 to break down)

### Phase 38: Cache ToolInfo at registration to avoid per-request cloning

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 37
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 38 to break down)

### Phase 39: Add deep-merge for ui meta key to prevent collision

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 38
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 39 to break down)
