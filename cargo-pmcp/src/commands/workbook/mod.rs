//! `cargo pmcp workbook <subcommand>` — governed Excel workbook bundle tooling.
//!
//! This module shells over the `pmcp-workbook-compiler` library verbs. Plan
//! 94-01 introduces only the project-config parser ([`config`]); the command
//! group enum and its `compile`/`lint`/`emit` handlers are added in Plan 94-02.

pub mod config;
