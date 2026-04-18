#![no_main]

use libfuzzer_sys::fuzz_target;
use syn::parse::Parser;

fn parse_attr(src: &str) -> Option<syn::Attribute> {
    syn::Attribute::parse_outer
        .parse_str(src)
        .ok()
        .and_then(|mut v| v.pop())
}

// Fuzz the rustdoc-harvest normalizer. The pure normalization function
// (trim each line → drop empties → join with "\n") must never panic on
// any `Vec<syn::Attribute>` input, including:
//   - plain `#[doc = "..."]` string-literal attrs
//   - `#[doc(hidden)]` / `#[doc(alias = "...")]` (Meta::List / Meta::Path)
//   - non-doc attrs mixed in (`#[allow(dead_code)]`, etc.)
//   - adversarial UTF-8 sequences inside doc literals
//
// Output invariant: either None or Some(non_empty_string). No panic.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Split input by newline. Each chunk's first byte selects one of four
    // attribute shapes, producing mixed-shape attribute arrays rather than
    // pure `#[doc = "..."]` runs.
    let mut attrs: Vec<syn::Attribute> = Vec::new();
    for chunk in s.split('\n') {
        let selector = chunk.as_bytes().first().copied().unwrap_or(0);
        match selector % 4 {
            0 => {
                let escaped = chunk.replace('\\', "\\\\").replace('"', "\\\"");
                let src = format!("#[doc = \"{}\"]", escaped);
                if let Some(attr) = parse_attr(&src) {
                    attrs.push(attr);
                }
            },
            1 => {
                if let Some(attr) = parse_attr("#[doc(hidden)]") {
                    attrs.push(attr);
                }
            },
            2 => {
                if let Some(attr) = parse_attr("#[doc(alias = \"foo\")]") {
                    attrs.push(attr);
                }
            },
            _ => {
                if let Some(attr) = parse_attr("#[allow(dead_code)]") {
                    attrs.push(attr);
                }
            },
        }
    }

    let out = pmcp_macros_support::rustdoc::extract_doc_description(&attrs);
    if let Some(ref out_s) = out {
        assert!(!out_s.is_empty(), "non-None result must be non-empty");
    }
});
