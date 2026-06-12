//! Manifest model RE-EXPORTS — never a local re-declaration.
//!
//! The logical [`Manifest`] (and its `CellRole`/`Role`/`Dtype`/`InputTier`/
//! `AnnotationDecl` companions) lives in [`pmcp_workbook_runtime`] (Phase 91, Plan
//! 05) so the served binary can deserialize the manifest projection WITHOUT
//! linking this offline compiler. The synthesis side EMITS that exact struct, so
//! it re-exports the types here — a second local `Manifest` would make the served
//! loader and the compiler read a DIFFERENT definition (the milestone's #1 trap).
//!
//! Every hand-built `Manifest { … }` literal in [`super::synth`] therefore MUST
//! supply the in-repo `annotations` field (`vec![]` when no Guide annotations) and
//! the D-04 `ratified`/`ratified_by`/`ratified_at` sign-off fields — the lighthouse
//! struct lacked `annotations` and would not compile against the in-repo struct.

// Re-export the runtime manifest model. NEVER re-declare any of these here.
pub use pmcp_workbook_runtime::{
    AnnotationDecl, CapabilityDecl, CellRole, ChangelogEntry, Dtype, GovernedDatum, InputTier,
    LoopDecl, Manifest, Role,
};
