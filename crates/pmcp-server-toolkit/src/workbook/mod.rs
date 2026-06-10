//! Governed-Excel workbook served-tool module (Phase 92,
//! `bundlesource-served-tool-toolkit-module`).
//!
//! This is the toolkit-side home for the served `calculate` / `explain` /
//! `get_manifest` / `diff_version` / `render_workbook` tools that operate on a
//! verified [`pmcp_workbook_runtime::WorkbookBundle`] (loaded fail-closed via
//! the runtime's `BundleSource` + `BundleLoader`).
//!
//! # Wiring discipline (Codex HIGH #1 / HIGH #2)
//!
//! The feature + this gated module skeleton land EARLY (this plan, wave 2) so
//! the wave-3/4 plans can `cargo test --features workbook` before the handlers
//! exist. The skeleton intentionally exposes NOTHING yet — the submodule
//! declarations below stay COMMENTED until the plan that creates each file
//! uncomments the matching `pub mod` line (never declare a `pub mod` before its
//! file exists — Codex HIGH #2):
//!
//! - Plan 03 Task 1 creates `error`/`schema`/`input`/`handler` and uncomments
//!   their declarations as each file lands.
//! - Plan 05 adds the builder-ext wiring + crate-root re-exports.
//!
//! Until then the `workbook` / `workbook-embedded` builds compile against this
//! empty skeleton, proving the feature gate is wired correctly.

// pub mod error;
// pub mod schema;
// pub mod input;
// pub mod handler;
