# Requirements: PMCP SDK — Milestone v2.4 Agents & Teams

**Defined:** 2026-07-17
**Core Value:** The PMCP SDK is the reference implementation for agents-as-MCP-clients and agent teams — one open agent loop and one portable package format that run identically on a laptop, any deploy target, and pmcp.run (contracts + reference implementations in the SDK; operation + scale on the platform).
**Design doc:** `docs/design/agents-teams-sdk-extraction-plan.md` (approved 2026-07-17 incl. §6 recommendations)

## v1 Requirements

### Client Host Surface (HOST)

- [ ] **HOST-01**: A pmcp `Client` can register a client-side `SamplingHandler` that answers incoming spec-direction `sampling/createMessage` requests from servers (tools/tool_choice included), instead of erroring "Unexpected message type"
- [ ] **HOST-02**: A pmcp `Client` can register an `ElicitationHandler` that answers incoming `elicitation/create` requests
- [ ] **HOST-03**: A pmcp `Client` answers `roots/list` from a registered roots provider
- [ ] **HOST-04**: The client sampling path exposes a human-in-the-loop approval hook (async callback slot, default allow) per the spec SHOULD
- [ ] **HOST-05**: `ClientCapabilities` advertised on initialize reflect which host handlers are registered (sampling/elicitation/roots)
- [ ] **HOST-06**: The legacy inverted sampling path (`Client::create_message` → server `SamplingHandler`) is documented as the distinct "LLM-server pattern" with no breaking changes, disambiguated from spec sampling in rustdoc and book

### Contracts & Package Format (PKG)

- [ ] **PKG-01**: `pmcp-package` lives in this repo as its canonical home (standalone workspace-excluded crate) with publish-ready metadata: public-facing description, README, license files, docs.rs-verified rustdoc
- [ ] **PKG-02**: `pmcp-package` 0.1.0 is published to crates.io with the wire-freeze policy documented (0.1.x = digest/serialization-stable enforced by golden fixtures; serialized-shape changes bump 0.2.0)
- [ ] **PKG-03**: Team-server tool contracts (`fs__*`, `mem__*`, `team_mcp__<member>` dispatch semantics, `resolve_approval`/`get_approval` + dynamic `team_approval__ask_*`) are captured as versioned provable-contracts YAML with shared conformance fixtures, marked as namespaced provisional PMCP extensions

### Agent Runtime (AGNT)

- [ ] **AGNT-01**: `crates/pmcp-agent` (0.x experimental) defines object-safe async effect-seam traits: `CompletionSource`, `ToolInvoker`, `ConversationStore` — reusing SDK sampling types verbatim for `CompletionSource`
- [ ] **AGNT-02**: The agent iteration loop (LLM call → tool-call decision → parallel tool dispatch → result digestion → end-turn detection → iteration/budget limits) runs pure between effect seams, with retry classification exposed as data (no retry/backoff policy inside the loop)
- [ ] **AGNT-03**: Replay-safety invariant is property-tested: identical effect results ⇒ identical loop decisions (proptest over recorded effect traces)
- [ ] **AGNT-04**: `SamplingSource` implements `CompletionSource` over spec sampling via the agent's server-side peer with zero additional dependencies
- [ ] **AGNT-05**: `OpenAiCompatSource` (feature-gated) implements `CompletionSource` against any OpenAI-compatible endpoint (Ollama/vLLM/OpenRouter/xAI/DeepSeek)
- [ ] **AGNT-06**: `AnthropicSource` (feature-gated) implements `CompletionSource` against the Anthropic Messages API
- [ ] **AGNT-07**: An agent can be exposed as an MCP server (agent-as-server adapter on `ServerCore`), deployable through existing target adapters (Lambda/Docker/WASM)
- [ ] **AGNT-08**: The `ToolInvoker` over `pmcp::Client` honors task-augmented tool results via `poll_decision` (SEP-1686) — long tool calls surface as pollable state
- [ ] **AGNT-09**: An agent is configured from an `AgentPackage` (pmcp-package) plus resolved config slots — the same definition drives laptop, deploy targets, and platform

### Team Reference Servers (TEAM)

- [ ] **TEAM-01**: `crates/pmcp-team-servers` exists with per-server feature flags and runnable dev binaries for all four servers
- [ ] **TEAM-02**: team-fs reference serves the `fs__*` contract over a `TeamFsBackend` trait with a local-directory dev backend
- [ ] **TEAM-03**: approval-mcp reference serves the approval contract over an in-memory `TaskStore` with console (dev) and webhook (CI) approval channels
- [ ] **TEAM-04**: mem-mcp reference serves the `mem__*` contract over a `TeamMemoryBackend` trait with a keyword/BM25 in-memory dev backend (no embedder dependency)
- [ ] **TEAM-05**: team-mcp reference composes agent-as-server members as per-member tools returning `ToolOutput::Result` with top-level `related_task` `_meta` (the migration template replacing the platform's raw-JSON-RPC bypass)
- [ ] **TEAM-06**: Conformance tests prove each reference server's tool surface matches the PKG-03 contracts (same fixtures the platform servers can run)

### CLI (CLI)

- [ ] **CLI-01**: `cargo pmcp agent new` scaffolds an agent project (AgentPackage manifest + standalone runner) with a version-pin tripwire test against `pmcp-agent`
- [ ] **CLI-02**: `cargo pmcp agent dev` runs an agent locally against an OpenAI-compat endpoint or as a sampling-hosted server
- [ ] **CLI-03**: `cargo pmcp team dev` runs an in-process small team (member agents + all four reference team servers with dev backends) wired from a `TeamPackage`
- [ ] **CLI-04**: `cargo pmcp package capture|show` subcommands work as thin clients to the platform capture API with `pmcp-package = "0.1"` (caret) and a pin tripwire test

### Documentation & Examples (DOCS)

- [ ] **DOCS-01**: pmcp-book chapters: "Agents as MCP Clients", "Agent Teams", "Sampling & Hosting" (incl. LLM-server pattern disambiguation)
- [ ] **DOCS-02**: Runnable examples: sampling host, standalone-vs-hosted agent (same loop, two sources), small team end-to-end
- [ ] **DOCS-03**: README + pmcp-course updated per the three-shapes rule, leading with the `cargo pmcp` workflow and the deploy-anywhere/preferred-pmcp.run positioning

## v2 Requirements

### Deferred (design doc follow-ons)

- **DEFER-01**: AgentCore deploy adapter (`cargo pmcp deploy` target)
- **DEFER-02**: Additional `CompletionSource` implementations beyond the three shipped
- **DEFER-03**: Scaled team-memory backends (embeddings/vector stores) in the open SDK
- **DEFER-04**: Platform-side migrations (pmcp.run adopting the loop/traits) — coordinated via the §8 companion note, not SDK work

## Out of Scope

| Feature | Reason |
|---------|--------|
| LLM provider matrix in the SDK | `CompletionSource` trait is the extension point; 3 sources max (design §7) |
| Open-sourcing mem0-rust / UnifiedLLMService / durable Lambda / capture service | Operation and scale are the platform's product (boundary razor) |
| Distributed-team portability | Deploy-anywhere teams = "small team, one process"; scale-out is pmcp.run |
| New wire methods in MCP WG territory | Extensions stay namespaced/provisional (matches tasks Ask-B posture) |
| Folding new crates into pmcp core | 0.x experimental isolation until contracts stabilize (pmcp-tasks precedent) |
| `=0.1.0` exact pin for pmcp-package in cargo-pmcp | Caret `"0.1"` + wire-freeze contract; exact pin adds lockstep churn for no guarantee |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| HOST-01 | Phase 106 | Pending |
| HOST-02 | Phase 106 | Pending |
| HOST-03 | Phase 106 | Pending |
| HOST-04 | Phase 106 | Pending |
| HOST-05 | Phase 106 | Pending |
| HOST-06 | Phase 106 | Pending |
| PKG-01 | Phase 107 | Pending |
| PKG-02 | Phase 107 | Pending |
| PKG-03 | Phase 107 | Pending |
| AGNT-01 | Phase 108 | Pending |
| AGNT-02 | Phase 108 | Pending |
| AGNT-03 | Phase 108 | Pending |
| AGNT-04 | Phase 108 | Pending |
| AGNT-05 | Phase 108 | Pending |
| AGNT-06 | Phase 108 | Pending |
| AGNT-07 | Phase 108 | Pending |
| AGNT-08 | Phase 108 | Pending |
| AGNT-09 | Phase 108 | Pending |
| TEAM-01 | Phase 109 | Pending |
| TEAM-02 | Phase 109 | Pending |
| TEAM-03 | Phase 109 | Pending |
| TEAM-04 | Phase 109 | Pending |
| TEAM-05 | Phase 109 | Pending |
| TEAM-06 | Phase 109 | Pending |
| CLI-01 | Phase 110 | Pending |
| CLI-02 | Phase 110 | Pending |
| CLI-03 | Phase 110 | Pending |
| CLI-04 | Phase 110 | Pending |
| DOCS-01 | Phase 111 | Pending |
| DOCS-02 | Phase 111 | Pending |
| DOCS-03 | Phase 111 | Pending |

**Coverage:**

- v1 requirements: 31 total
- Mapped to phases: 31 ✓
- Unmapped: 0 ✓

Phase distribution: Phase 106 (HOST, 6) · Phase 107 (PKG, 3) · Phase 108 (AGNT, 9) · Phase 109 (TEAM, 6) · Phase 110 (CLI, 4) · Phase 111 (DOCS, 3)

---
*Requirements defined: 2026-07-17*
*Last updated: 2026-07-17 — roadmap created, all 31 v1 requirements mapped to Phases 106-111 (100% coverage)*
