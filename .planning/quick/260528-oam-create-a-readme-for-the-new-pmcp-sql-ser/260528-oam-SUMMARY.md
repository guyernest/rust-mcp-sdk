---
phase: quick-260528-oam
plan: 01
subsystem: pmcp-sql-server
tags: [docs, readme, sql-server, config-driven, code-mode]
requires: []
provides:
  - "crates/pmcp-sql-server/README.md (crate README grounded in real CLI/config/backends)"
affects:
  - crates/pmcp-sql-server
tech-stack:
  added: []
  patterns:
    - "README voice/structure mirrors sibling crates/pmcp-server-toolkit/README.md"
    - "Doctest-safe fences only (toml/bash/text); no bare ```rust"
key-files:
  created:
    - crates/pmcp-sql-server/README.md
  modified: []
decisions:
  - "Excluded any Rust code from the README — crate is consumed as binary + config, not a library API; avoids doctest compilation"
  - "Backend table + required fields traced verbatim to src/dispatch.rs (sqlite file_path|database, postgres/mysql url, athena workgroup)"
  - "Minimal config.toml copied from examples/sql_server_min.rs CONFIG (real, compiling source)"
metrics:
  duration: ~6m
  completed: 2026-05-28
  tasks: 2
  files: 1
---

# Phase quick-260528-oam Plan 01: pmcp-sql-server README Summary

Authored `crates/pmcp-sql-server/README.md` (112 lines) for the Shape A pure-config SQL MCP server binary, grounded entirely in the real crate source (cli.rs, dispatch.rs, Cargo.toml, examples/sql_server_min.rs, tests/fixtures/reference-config.toml) and matching the voice of the sibling pmcp-server-toolkit README.

## What was built

- **Title + one-liner + Status line** — `pmcp-sql-server`, `0.1.0 — early access`, explicitly noting the pipeline is fully implemented (not WIP).
- **The improvement section** — contrasts hand-writing a Rust MCP server (ServerBuilder, per-query handlers, connector management, recompile-per-change) against the two-input (`config.toml` + schema file) one-binary model; references the Pareto ~20% curated tools / ~80% Code Mode split and points at the `pmcp-server-toolkit` library it builds on.
- **What this crate is NOT** — binary not library, SQL-only (not DynamoDB/NoSQL), no invented SQL dialect (you supply DDL + backend URL/path).
- **Supported backends table** — sqlite/postgres/mysql/athena with required `[database]` fields traced verbatim to `dispatch.rs`; default all-backends build plus `--no-default-features --features sqlite` lean build and the `rebuild with --features <name>` / `supported: sqlite, postgres, mysql, athena` error behavior.
- **Quickstart** — build/install (`cargo build -p pmcp-sql-server --release`, `cargo install --path ...`), a minimal real `config.toml` (from the example CONFIG), CLI run with `--config`/`--schema`/`--http` (default `127.0.0.1:8080`) plus `RUST_LOG`, and backend selection via `[database] type`.
- **Design context** — links the spike-findings SKILL.md and the `.planning/phases/85-*` design log.

## Verification

| Task | Verify | Result |
| ---- | ------ | ------ |
| 1 | README exists, contains `pmcp-sql-server`, `--config`, `--schema`, all four backends; no bare ```rust fence | OK (112 lines) |
| 2 | No bare ```rust fence + `cargo build -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite` | PASS — example compiled (`Finished dev profile`) |

Ground-truth cross-checks performed against the real source before writing:
- `--config` / `--schema` required, `--http` default `127.0.0.1:8080` — confirmed in `src/cli.rs` (`Args`).
- Backends + required fields — confirmed in `src/dispatch.rs` (sqlite `file_path` or `database`; postgres/mysql `url`; athena `workgroup` + region env + optional `output_location`/`database`).
- `default = ["sqlite","postgres","mysql","athena"]`, version `0.1.0` — confirmed in `Cargo.toml`.
- Minimal config shape — copied from `examples/sql_server_min.rs` CONFIG (`[server]`/`[database]`/`[code_mode]` with `${CODE_MODE_SECRET}`/`[[tools]]`+`[[tools.parameters]]`).
- `RUST_LOG`/tracing EnvFilter + `AllowedOrigins::localhost()` loopback default — confirmed in `src/lib.rs` (`run`/`serve`).

## Deviations from Plan

None — plan executed exactly as written. No deviation rules triggered; the example compiled on the first verify run.

## Known Stubs

None.

## Commits

- `751bb05a` docs(quick-260528-oam-01): add pmcp-sql-server crate README

## Self-Check: PASSED

- FOUND: crates/pmcp-sql-server/README.md
- FOUND: commit 751bb05a
