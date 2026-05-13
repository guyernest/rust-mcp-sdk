//! SEP-2640 Agent Skills — `ResourceHandler`-served skill resources with
//! a parallel prompt-surface fallback for SEP-2640-blind hosts.
//!
//! This module is intentionally empty in Phase 80 Plan 80-01.
//! Plan 80-02 populates it with:
//!
//! - `Skill` — a single Agent Skill (SKILL.md + zero-or-more references).
//! - `SkillReference` — a supporting file within a skill's directory.
//! - `Skills` — a registry that flattens into a
//!   [`crate::server::ResourceHandler`].
//! - `SkillsHandler` (internal) — the synthesized `ResourceHandler` impl.
//! - `SkillPromptHandler` (internal) — the parallel `PromptHandler` impl
//!   carrying `Skill::as_prompt_text()` as its single message body.
//! - `ComposedResources` (internal) — URI-prefix routing between skills and
//!   any pre-existing `.resources(...)` handler (built once at `.build()`
//!   time per 80-REVIEWS.md Fix 1).
//!
//! Builder integration (`.skill(...)`, `.skills(...)`, `.try_skills(...)`,
//! `.bootstrap_skill_and_prompt(...)`) lands alongside the type definitions
//! in Plan 80-02 on BOTH `ServerBuilder` (`src/server/mod.rs`) AND
//! `ServerCoreBuilder` (`src/server/builder.rs`) per 80-REVIEWS.md Fix 2.
//!
//! See `.planning/phases/80-sep-2640-skills-support/80-CONTEXT.md` for the
//! full implementation contract and the spike-findings skill at
//! `.claude/skills/spike-findings-rust-mcp-sdk/` for the design reference.

// Plan 80-02 populates this module with Skill / SkillReference / Skills types.
// Intentionally left blank.
