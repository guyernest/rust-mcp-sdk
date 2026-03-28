---
status: complete
phase: 04-load-shaping-and-tool-metrics
source: [04-01-SUMMARY.md, 04-02-SUMMARY.md, 04-03-SUMMARY.md, 04-04-SUMMARY.md]
started: 2026-02-27T06:30:00Z
updated: 2026-02-27T06:50:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Stage Config in TOML
expected: Adding `[[stage]]` blocks to `.pmcp/loadtest.toml` with `target_vus` and `duration_secs` fields is accepted by the config parser. Running `cargo pmcp loadtest run` with stages defined uses the staged scheduler (no parse errors).
result: pass

### 2. Stage Label in Live Display
expected: During a staged load test, the live terminal progress line shows a `[stage N/M]` prefix that updates as the test transitions through ramp-up, hold, and ramp-down stages.
result: pass

### 3. Backwards Compatibility Without Stages
expected: Running a load test with a TOML config that has NO `[[stage]]` blocks works exactly as before (flat load, all VUs start immediately, no stage labels in output).
result: pass

### 4. VUs Flag Warning With Stages
expected: Running with both `[[stage]]` blocks in config AND `--vus` CLI flag shows a warning that `--vus` is ignored when stages are present.
result: pass

### 5. Per-Tool Terminal Table
expected: After a load test completes against a server with multiple tools, the terminal summary includes a "Per Tool" section with a table showing each tool name, request count, rate, error%, P50, P95, P99 — one row per tool called.
result: pass

### 6. Per-Tool JSON Report
expected: The JSON report file contains a `per_tool` object with per-tool metrics including latency stats (min, max, mean, percentiles) and error breakdown by type. The `schema_version` field is "1.1".
result: pass

### 7. Breaking Point Live Warning
expected: When running a load test that pushes a server past its capacity (e.g. high VU count causing errors or latency spikes), a WARNING line appears in the terminal output indicating the breaking point was detected, including the VU count at detection time (non-zero).
result: pass

### 8. Breaking Point in JSON Report
expected: After a test where breaking point was detected, the JSON report contains a `breaking_point` object with `vus`, `error_rate`, `p99_ms`, and `reason` fields. The `vus` field reflects the actual VU count (not 0).
result: pass

## Summary

total: 8
passed: 8
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
