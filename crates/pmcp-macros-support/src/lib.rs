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
    /// Harvest `#[doc = "..."]` attributes into a normalized description
    /// string.
    ///
    /// Applies the rmcp-parity normalization:
    /// - trim each doc literal (leading/trailing whitespace stripped);
    /// - drop empty post-trim lines;
    /// - join remaining lines with `"\n"`.
    ///
    /// Skips non-`NameValue` doc attrs (e.g. `#[doc(hidden)]`, `#[doc(alias = "...")]`)
    /// and skips `NameValue` forms whose value is not a string literal — including
    /// `#[doc = include_str!("...")]` and `#[cfg_attr(..., doc = "...")]`.
    ///
    /// Returns `None` if no non-empty rustdoc is present.
    ///
    /// # Unsupported forms
    ///
    /// - `#[doc = include_str!("...")]` — silently skipped (macro expansion not evaluated).
    /// - `#[cfg_attr(..., doc = "...")]` — silently skipped (attr shape does not match).
    /// - Indented code fences inside doc blocks — indentation stripped along with all
    ///   other whitespace per the trim-each-line rule. Tool descriptions render as plain
    ///   text in MCP clients, not as rendered rustdoc HTML, so this is acceptable.
    #[must_use]
    pub fn extract_doc_description(attrs: &[syn::Attribute]) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        for attr in attrs {
            if !attr.path().is_ident("doc") {
                continue;
            }
            let syn::Meta::NameValue(nv) = &attr.meta else {
                continue;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) = &nv.value
            else {
                continue;
            };
            let trimmed = lit_str.value().trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            lines.push(trimmed);
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Reference implementation of the normalization algorithm over raw
    /// line strings.
    ///
    /// Used as the property-test oracle AND as a convenience for the fuzz
    /// target (Plan 03) to avoid the `syn::parse_str` round-trip. This is
    /// the authoritative plain-Rust spec of the normalization semantics.
    #[must_use]
    pub fn reference_normalize(lines: &[String]) -> Option<String> {
        let filtered: Vec<String> = lines
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if filtered.is_empty() {
            None
        } else {
            Some(filtered.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rustdoc::{extract_doc_description, reference_normalize};

    fn doc_attrs(lines: &[&str]) -> Vec<syn::Attribute> {
        lines
            .iter()
            .map(|line| {
                let lit = syn::LitStr::new(line, proc_macro2::Span::call_site());
                syn::parse_quote! { #[doc = #lit] }
            })
            .collect()
    }

    // ==== 10 normalization vectors from 71-RESEARCH.md §"Test vectors" ====

    #[test]
    fn vec1_single_line() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&[" Add two numbers."])),
            Some("Add two numbers.".to_string())
        );
    }

    #[test]
    fn vec2_two_lines_join_newline() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&[" Add two numbers.", " Returns their sum."])),
            Some("Add two numbers.\nReturns their sum.".to_string())
        );
    }

    #[test]
    fn vec3_blank_middle_line_dropped() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&[" Line 1.", "", " Line 2."])),
            Some("Line 1.\nLine 2.".to_string())
        );
    }

    #[test]
    fn vec4_leading_whitespace_trimmed() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&["   Indented body."])),
            Some("Indented body.".to_string())
        );
    }

    #[test]
    fn vec5_trailing_whitespace_trimmed() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&[" Line 1.  "])),
            Some("Line 1.".to_string())
        );
    }

    #[test]
    fn vec6_no_doc_attrs_returns_none() {
        assert_eq!(extract_doc_description(&[]), None);
    }

    #[test]
    fn vec7_only_empty_lines_returns_none() {
        assert_eq!(extract_doc_description(&doc_attrs(&["", "   ", ""])), None);
    }

    #[test]
    fn vec8_doc_hidden_skipped() {
        let mut attrs = doc_attrs(&[" Line 1."]);
        attrs.push(syn::parse_quote! { #[doc(hidden)] });
        attrs.extend(doc_attrs(&[" Line 2."]));
        assert_eq!(
            extract_doc_description(&attrs),
            Some("Line 1.\nLine 2.".to_string())
        );
    }

    #[test]
    fn vec9_embedded_quotes_preserved() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&[" Line with \"quotes\""])),
            Some("Line with \"quotes\"".to_string())
        );
    }

    #[test]
    fn vec10_whitespace_only_lines_dropped() {
        assert_eq!(
            extract_doc_description(&doc_attrs(&["   ", " Real content.", "   "])),
            Some("Real content.".to_string())
        );
    }

    // ==== Unsupported rustdoc forms (MEDIUM-3 from 71-REVIEWS.md) ====

    #[test]
    fn unsupported_include_str_skipped() {
        // `#[doc = include_str!("readme.md")]` has Meta::NameValue with
        // Expr::Macro, not Expr::Lit. The helper skips it silently.
        let attr: syn::Attribute = syn::parse_quote! { #[doc = include_str!("nonexistent.md")] };
        assert_eq!(extract_doc_description(&[attr]), None);
    }

    #[test]
    fn unsupported_cfg_attr_doc_skipped() {
        // `#[cfg_attr(docsrs, doc = "...")]` — outer path is `cfg_attr`,
        // not `doc` — skipped by the `is_ident("doc")` guard.
        let attr: syn::Attribute = syn::parse_quote! { #[cfg_attr(docsrs, doc = "conditional")] };
        assert_eq!(extract_doc_description(&[attr]), None);
    }

    #[test]
    fn unsupported_forms_mixed_with_real_docs() {
        // A real doc line + an unsupported form → real doc wins.
        let mut attrs = doc_attrs(&[" Real line."]);
        attrs.push(syn::parse_quote! { #[doc = include_str!("nonexistent.md")] });
        assert_eq!(
            extract_doc_description(&attrs),
            Some("Real line.".to_string())
        );
    }

    // ==== Reference oracle sanity checks ====

    #[test]
    fn ref_empty_input_returns_none() {
        assert_eq!(reference_normalize(&[]), None);
    }

    #[test]
    fn ref_matches_extract_for_simple_case() {
        let lines = vec!["Line 1.".to_string(), " Line 2.".to_string()];
        let via_attrs = extract_doc_description(&doc_attrs(&["Line 1.", " Line 2."]));
        let via_ref = reference_normalize(&lines);
        assert_eq!(via_attrs, via_ref);
    }

    #[test]
    fn ref_idempotent_on_normalized_output() {
        let once = reference_normalize(&[
            " A ".to_string(),
            String::new(),
            "B".to_string(),
        ]);
        let s = once.as_ref().unwrap();
        let twice = reference_normalize(&s.split('\n').map(String::from).collect::<Vec<_>>());
        assert_eq!(twice.as_deref(), Some(s.as_str()));
    }
}
