# Ticket: `package capture` — slug-friendly `--team-id` guard + fix wrong "UUID" `long_help`

**Status:** open / not yet filed as a GitHub issue (stub — file against `paiml/rust-mcp-sdk`)
**Spun off from:** package-capture contract seam (design spec §9, 2026-07-20)
**Priority:** DX polish — orthogonal to the contract seam, not release-blocking

## Problem

`cargo pmcp package capture`'s positional argument is an **AgentTeam id that is a
slug** (e.g. `day-trip-planner-team`), **not a UUID**. Two defects follow from the
current code assuming a UUID:

1. **Wrong help text.** `cargo-pmcp/src/commands/package/capture.rs` describes the
   argument (both the `///` doc and the `long_help`) as
   `AgentTeam ID (UUID) — the team's id, not its display name.` and claims
   `submitPackageCapture` does a `GetItem` by primary key so "a display name will
   not resolve." The "(UUID)" characterization is wrong — the id is a slug.
2. **No guard, so display names fail opaquely.** If a user passes a display name
   (e.g. `"Day Trip Planner"`) it is sent as-is and the server returns a bare
   not-found, with nothing steering the user toward the slug id.

## Why a naive UUID guard is WRONG

A "reject anything that isn't a UUID" validation would reject every valid slug id.
Client-side code **cannot reliably distinguish a valid slug-id from a display
name** — both are arbitrary strings. So the guard must be conservative.

## Fix

1. ✅ **DONE (cargo-pmcp 0.19.0)** — **Fixed the help text** (`capture.rs`): the `///`
   doc comment and `long_help` now say the argument is an **AgentTeam id (slug)** —
   e.g. `day-trip-planner-team` — **not the display name (and not a UUID)**. The
   remaining items below (the conservative display-name guard) are still open.
2. **Reject only OBVIOUS display names** with a clear, actionable message: input that
   **contains a space** or is **mixed-case** is almost certainly a display name, not a
   slug — reject those up front pointing the user at the slug form. Everything else
   passes through.
3. **Lean on a clean server-side "AgentTeam not found"** for anything that passes the
   client guard but doesn't resolve. Do NOT gate on UUID format.

## Out of scope

Server-side name→id lookup (a documented deferral). This ticket only fixes the
client help text + the conservative display-name guard.
