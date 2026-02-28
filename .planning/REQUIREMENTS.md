# Requirements: PMCP SDK Extensions

**Defined:** 2026-02-27
**Core Value:** Tool handlers can manage long-running operations through a durable task lifecycle with shared variable state, plus developers can build rich UI widgets and upload loadtest configs for cloud execution.

## v1.5 Requirements

Requirements for cloud load testing upload. Each maps to roadmap phases.

### CLI Command

- [ ] **CLI-01**: User can run `cargo pmcp loadtest upload` with `--server-id` and path to TOML config
- [ ] **CLI-02**: User receives clear error if TOML config is invalid or has no scenarios
- [ ] **CLI-03**: User sees upload success with config identifier and version from pmcp.run
- [ ] **CLI-04**: User sees next steps guidance (view on pmcp.run dashboard, trigger remote run)

### Upload

- [ ] **UPLD-01**: Loadtest TOML config content is uploaded via GraphQL mutation to pmcp.run
- [ ] **UPLD-02**: Upload reuses existing pmcp.run auth (OAuth, client credentials, access token)
- [ ] **UPLD-03**: Upload sends config content, format, name, and server association

### Validation

- [ ] **VALD-01**: Config file is parsed and validated before upload (valid TOML, has scenarios)
- [ ] **VALD-02**: User receives actionable error messages for invalid configs

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Provider Abstraction

- **PROV-01**: LoadtestProvider trait for pluggable cloud backends
- **PROV-02**: Provider registry with target selection (`--target` flag)

### Remote Execution

- **REXE-01**: Trigger remote load test execution from CLI
- **REXE-02**: Poll remote execution status from CLI
- **REXE-03**: Download remote execution results to local JSON report

### Bulk Upload

- **BULK-01**: Upload multiple TOML configs from directory
- **BULK-02**: Per-file error handling with accumulated statistics

## Out of Scope

| Feature | Reason |
|---------|--------|
| Provider trait abstraction | Wait for second provider to appear — avoid premature abstraction |
| Remote execution trigger from CLI | pmcp.run UI handles triggering for now |
| Result download/polling from CLI | Results viewed on pmcp.run dashboard |
| Multi-region configuration from CLI | Cloud service manages region distribution |
| Bulk/directory upload | Single file sufficient for initial release |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLI-01 | — | Pending |
| CLI-02 | — | Pending |
| CLI-03 | — | Pending |
| CLI-04 | — | Pending |
| UPLD-01 | — | Pending |
| UPLD-02 | — | Pending |
| UPLD-03 | — | Pending |
| VALD-01 | — | Pending |
| VALD-02 | — | Pending |

**Coverage:**
- v1.5 requirements: 9 total
- Mapped to phases: 0
- Unmapped: 9 ⚠️

---
*Requirements defined: 2026-02-27*
*Last updated: 2026-02-27 after initial definition*
