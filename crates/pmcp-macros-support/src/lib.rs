//! Pure helpers for `pmcp-macros`.
//!
//! This crate exists because `pmcp-macros` has `proc-macro = true`, which
//! per the Rust Reference restricts its public API to only the
//! procedural macros defined via `#[proc_macro]`. Property tests and fuzz
//! targets cannot import internal helpers from a proc-macro crate. This
//! crate holds the pure normalization logic so it is importable by any
//! downstream consumer: `pmcp-macros` itself (for the macro expansion
//! path), property tests, and fuzz harnesses.
//!
//! This crate has no stability guarantees — it is a workspace-internal
//! implementation detail published alongside `pmcp-macros`. External
//! users should never depend on it directly.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

/// Rustdoc-harvest helpers (`extract_doc_description`, `reference_normalize`).
pub mod rustdoc {
    /// Placeholder — Task 2 replaces with real implementation.
    #[must_use]
    pub fn extract_doc_description(_attrs: &[syn::Attribute]) -> Option<String> {
        None
    }

    /// Placeholder — Task 2 replaces with real implementation.
    #[must_use]
    pub fn reference_normalize(_lines: &[String]) -> Option<String> {
        None
    }
}
