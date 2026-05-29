//! OpenAPI schema parsing seam (Phase 90 Plan 03).
//!
//! This module is an intentional forward stub for Plan 01 so the `http` module
//! tree compiles. Plan 03 (OAPI-02) fills it with the `openapiv3`-backed parser
//! that builds [`crate::http::Operation`] values from an OpenAPI document. The
//! `Operation` / `Parameter` / `ParameterLocation` types the parser populates
//! are defined in [`crate::http`] (mod.rs) so the [`crate::http::HttpConnector`]
//! trait signature can reference them in Wave 1; Plan 03 may extend `Operation`
//! additively from the `openapiv3` parse.
