# Milestones

## v1.0 MCP Load Testing (Shipped: 2026-02-27)

**Phases:** 1-4 | **Plans:** 14 | **Commits:** 55 | **LOC:** 5,749 Rust
**Timeline:** 1 day (2026-02-26) | **Execution time:** 1.28 hours
**Git range:** `df67c09` (feat(01-01)) → `cdabfdb` (test(04))
**Requirements:** 16/16 satisfied | **Audit:** PASSED

**Delivered:** A complete MCP load testing CLI (`cargo pmcp loadtest`) that lets developers stress-test deployed MCP servers with concurrent virtual users, live terminal progress, and detailed performance reports.

**Key accomplishments:**
1. Typed TOML config with weighted MCP operation scenarios and HdrHistogram metrics with coordinated omission correction
2. Concurrent VU engine with tokio task tracking, dual-recorder metrics aggregation, and graceful Ctrl+C shutdown
3. k6-style live terminal display with per-second updates and colorized terminal summary report
4. `cargo pmcp loadtest` CLI with config discovery, schema-aware init, and JSON report output
5. Stage-driven load shaping with ramp-up/hold/ramp-down and per-VU cancellation
6. Breaking point auto-detection with rolling window self-calibrating algorithm and per-tool metrics breakdown

**Tech debt (3 info-level items):**
- SUMMARY 02-01 claims 'Added Engine variant to LoadTestError' but variant does not exist (doc inaccuracy)
- `LiveDisplay` struct is `pub` but only used internally by `display_loop()`
- `step_to_operation_type()` is `pub` but only used in tests within `vu.rs`

---

