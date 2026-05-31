---
created: 2026-03-04T05:10:57.357Z
title: Create README docs for cargo-pmcp CLI
area: docs
files:
  - cargo-pmcp/README.md
  - cargo-pmcp/src/commands/
---

## Problem

The `cargo-pmcp` CLI crate lacks comprehensive README documentation. Users and contributors need clear documentation covering:
- What `cargo pmcp` does and how to install it
- Available subcommands (auth, deploy, init, loadtest, preview, validate, etc.)
- Usage examples for common workflows
- Configuration options and flags (e.g., --quiet, --json)

## Solution

Create or update `cargo-pmcp/README.md` with:
- Installation instructions (cargo install)
- Command reference table with descriptions
- Usage examples for key workflows (deploy, loadtest, preview)
- Global flags documentation (--quiet, --json)
- Links to further docs or the main pmcp README

## Resolution (2026-05-30) — DONE (already satisfied)

`cargo-pmcp/README.md` already exists (555 lines) and is current: Overview,
Features, a full **Commands** reference table, global flags, plus dedicated
`Config-Driven SQL Server (new --kind sql-server)` and `Config-Driven OpenAPI
Server (new --kind openapi-server)` walkthroughs, deploy, secrets, and OAuth.
It covers everything this todo asked for; no further work needed. Verified and
closed during the v2.9.0 release prep.
