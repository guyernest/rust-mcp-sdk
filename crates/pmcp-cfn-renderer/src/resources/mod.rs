//! Allowlist-scoped CFN resource builders — one module per resource family.
//!
//! This is the landing zone for the renderer's per-family resource-building
//! logic. The v1 resource surface is EXACTLY seven families, matching the
//! design spec's §4 table: `lambda`, `iam`, `logs`, `http_api`, `cognito`,
//! `dynamodb`, `outputs`. A descriptor requesting anything outside this
//! surface must fail loudly via [`crate::RenderError::UnsupportedSection`] —
//! never a silent skip, and this module must never grow toward
//! CDK-completeness.
//!
//! Empty in this task (crate skeleton): [`crate::render`] currently returns
//! an empty-resource template with no resource-module wiring. The
//! `lambda`/`logs`/`outputs` modules land in a later task, followed by
//! `iam`, then `http_api`, then `cognito`/`dynamodb`.
