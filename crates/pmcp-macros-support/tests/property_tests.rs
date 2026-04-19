//! Property-based tests for pmcp-macros-support rustdoc-harvest normalization.
//!
//! Invariants:
//! - reference-equivalence: real helper matches the plain-Rust oracle
//! - determinism: same input → same output
//! - no-panic: never panics on arbitrary UTF-8
//! - mixed-attr-shape robustness: `#[doc(hidden)]`, non-doc attrs, and
//!   regular doc attrs in any order produce well-defined output.

use pmcp_macros_support::rustdoc::{extract_doc_description, reference_normalize};
use proptest::prelude::*;

fn make_doc_attrs(lines: &[String]) -> Vec<syn::Attribute> {
    lines
        .iter()
        .map(|line| {
            let lit = syn::LitStr::new(line, proc_macro2::Span::call_site());
            syn::parse_quote! { #[doc = #lit] }
        })
        .collect()
}

#[derive(Debug, Clone)]
enum AttrKind {
    Doc(String),
    DocHidden,
    NonDoc,
}

/// Strategy that builds a mixed vector of doc / doc-hidden / non-doc attrs.
/// Returns both the `Vec<syn::Attribute>` AND the subset of plain doc-line
/// strings (for comparing against `reference_normalize`).
fn mixed_attrs_strategy() -> impl Strategy<Value = (Vec<syn::Attribute>, Vec<String>)> {
    prop::collection::vec(
        prop_oneof![
            ".*".prop_map(AttrKind::Doc),
            Just(AttrKind::DocHidden),
            Just(AttrKind::NonDoc),
        ],
        0..30,
    )
    .prop_map(|kinds| {
        let mut attrs: Vec<syn::Attribute> = Vec::new();
        let mut doc_lines: Vec<String> = Vec::new();
        for k in kinds {
            match k {
                AttrKind::Doc(line) => {
                    let lit = syn::LitStr::new(&line, proc_macro2::Span::call_site());
                    attrs.push(syn::parse_quote! { #[doc = #lit] });
                    doc_lines.push(line);
                },
                AttrKind::DocHidden => {
                    attrs.push(syn::parse_quote! { #[doc(hidden)] });
                },
                AttrKind::NonDoc => {
                    attrs.push(syn::parse_quote! { #[allow(dead_code)] });
                },
            }
        }
        (attrs, doc_lines)
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    /// Invariant 1: `extract_doc_description` matches `reference_normalize`
    /// for any `Vec<String>` of simulated doc-line inputs.
    #[test]
    fn prop_normalize_matches_reference(
        lines in prop::collection::vec(".*", 0..20)
    ) {
        let attrs = make_doc_attrs(&lines);
        let got = extract_doc_description(&attrs);
        let want = reference_normalize(&lines);
        prop_assert_eq!(got, want);
    }

    /// Invariant 2: `extract_doc_description` is deterministic —
    /// two sequential calls return identical output.
    #[test]
    fn prop_normalize_deterministic(
        lines in prop::collection::vec(".*", 0..20)
    ) {
        let attrs = make_doc_attrs(&lines);
        let a = extract_doc_description(&attrs);
        let b = extract_doc_description(&attrs);
        prop_assert_eq!(a, b);
    }

    /// Invariant 3: `extract_doc_description` never panics on arbitrary
    /// UTF-8 byte strings; non-None outputs are non-empty.
    #[test]
    fn prop_no_panic_on_arbitrary_utf8(
        lines in prop::collection::vec("\\PC*", 0..30)
    ) {
        let attrs = make_doc_attrs(&lines);
        let result = extract_doc_description(&attrs);
        if let Some(ref s) = result {
            prop_assert!(!s.is_empty(), "non-None result must be non-empty");
        }
    }

    /// Invariant 4: mixed-attr-shape robustness. For arbitrary mixes of
    /// `#[doc = "..."]`, `#[doc(hidden)]`, and non-doc attrs, the helper
    /// terminates AND its output equals
    /// `reference_normalize(extracted_plain_doc_lines)`.
    #[test]
    fn prop_mixed_attr_shapes_robust(
        (attrs, doc_lines) in mixed_attrs_strategy()
    ) {
        let got = extract_doc_description(&attrs);
        let want = reference_normalize(&doc_lines);
        prop_assert_eq!(got, want);
    }
}
