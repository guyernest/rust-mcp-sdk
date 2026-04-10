# Requirements: PMCP SDK rmcp Upgrades

**Defined:** 2026-04-10
**Core Value:** Close credibility and DX gaps where rmcp outshines PMCP — documentation accuracy, feature gate presentation, macro documentation, example index, and repo hygiene.

## v2.1 Requirements

Requirements for rmcp Upgrades milestone. Each maps to roadmap phases.

### Examples Cleanup

- [ ] **EXMP-01**: Examples README replaced with accurate PMCP example index organized by category with required features and run commands
- [ ] **EXMP-02**: All example .rs files in examples/ are registered in Cargo.toml with correct required-features (17 orphans resolved)
- [ ] **EXMP-03**: No duplicate example number prefixes — each numbered prefix maps to exactly one file (08, 11, 12, 32 resolved)

### Protocol Accuracy

- [ ] **PROT-01**: README MCP-Compatible badge and compatibility table show 2025-11-25, matching LATEST_PROTOCOL_VERSION in code

### Macros Documentation

- [ ] **MACR-01**: pmcp-macros README rewritten to document #[mcp_tool], #[mcp_server], #[mcp_prompt], #[mcp_resource] as primary APIs with working examples
- [ ] **MACR-02**: Migration section guiding users from deprecated #[tool]/#[tool_router] to #[mcp_tool]/#[mcp_server]
- [ ] **MACR-03**: pmcp-macros lib.rs uses include_str!("../README.md") so docs.rs shows the rewritten README

### docs.rs Pipeline

- [ ] **DRSD-01**: lib.rs contains cfg_attr(docsrs, feature(doc_auto_cfg)) enabling automatic feature badges on all feature-gated items
- [ ] **DRSD-02**: Cargo.toml [package.metadata.docs.rs] uses explicit feature list (~13 user-facing features) instead of all-features = true
- [ ] **DRSD-03**: Feature flag table added to lib.rs doc comments documenting all user-facing features with descriptions
- [ ] **DRSD-04**: Zero rustdoc warnings — all broken intra-doc links and unclosed HTML tags resolved, CI gate added

### General Polish

- [ ] **PLSH-01**: lib.rs crate-level doctests updated to show TypedToolWithOutput and current builder patterns (not legacy Server::builder())
- [ ] **PLSH-02**: CI enforcement: example file count matches Cargo.toml [[example]] count, cargo semver-checks on PRs
- [ ] **PLSH-03**: Transport matrix table in lib.rs docs linking to actual transport types

## Previous Requirements

<details>
<summary>v2.0 Protocol Type Construction DX (Complete)</summary>

| ID | Phase | Status |
|----|-------|--------|
| PROTO-TYPE-DX | Phase 54.1 | Complete |

</details>

<details>
<summary>v1.6 CLI DX Overhaul (27/27 Complete)</summary>

- [x] FLAG-01..09 (Phase 27-28)
- [x] AUTH-01..06 (Phase 29)
- [x] TEST-01..08 (Phase 30)
- [x] CMD-01..02 (Phase 31)
- [x] HELP-01..02 (Phase 32)

</details>

<details>
<summary>v1.5 Cloud Load Testing Upload (6/6 Complete)</summary>

- [x] CLI-01..04 (Phase 25-26)
- [x] UPLD-01..03 (Phase 25-26)
- [x] VALD-01..02 (Phase 25-26)

</details>

## Future Requirements

Deferred to later milestone. Tracked but not in current roadmap.

### Documentation Depth

- **DOCD-01**: Per-capability code examples in README (book/course fill this role today)
- **DOCD-02**: Separate crate-level README distinct from repo README for docs.rs
- **DOCD-03**: Community showcase ("Built with PMCP") section when real projects exist

### CLI Enhancements

- **CLIH-01**: `cargo pmcp init` interactive project setup wizard
- **CLIH-02**: `cargo pmcp config` command for managing .pmcp/config.toml
- **CLIH-03**: `cargo pmcp update` self-update mechanism

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Copying rmcp's trait-based architecture docs | Different SDK architecture; would be misleading |
| Per-capability inline README sections | Would make README 2000+ lines; book/course serve this role |
| Example subdirectory reorganization | High churn for low gain; flat numbering works |
| document-features crate | Adds build dep for something a manual table does equally well |
| Removing book/course/ecosystem from README | These are genuine PMCP differentiators rmcp lacks |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| EXMP-01 | Phase 65 | Pending |
| EXMP-02 | Phase 65 | Pending |
| EXMP-03 | Phase 65 | Pending |
| PROT-01 | Phase 65 | Pending |
| MACR-01 | Phase 66 | Pending |
| MACR-02 | Phase 66 | Pending |
| MACR-03 | Phase 66 | Pending |
| DRSD-01 | Phase 67 | Pending |
| DRSD-02 | Phase 67 | Pending |
| DRSD-03 | Phase 67 | Pending |
| DRSD-04 | Phase 67 | Pending |
| PLSH-01 | Phase 68 | Pending |
| PLSH-02 | Phase 68 | Pending |
| PLSH-03 | Phase 68 | Pending |

**Coverage:**
- v2.1 requirements: 14 total
- Mapped to phases: 14
- Unmapped: 0

---
*Requirements defined: 2026-04-10*
*Last updated: 2026-04-10 after roadmap creation*
