//! HTTP-09 tripwire — the mechanical half of "every peer-controlled read on the
//! v2 transport path is memory-bounded".
//!
//! # What the requirement says
//!
//! > **HTTP-09**: Every peer-controlled read on the v2 transport path is
//! > memory-bounded. Closure is **enumerable, not narrative**: a tripwire test
//! > asserts that no unbounded whole-body read (`.collect()`, `read_to_end`) and
//! > no unbounded accumulation over peer-supplied bytes exists in `src/shared/`,
//! > `src/client/subscriptions.rs`, or `src/server/streamable_http_server.rs`
//! > outside an explicit reviewed allowlist, and that no scan over peer-chosen
//! > input is worse than O(n).
//!
//! This file is the *enumeration* half. The O(n) clause is a separate artifact.
//!
//! # Why it exists
//!
//! "Memory-bounded" started life as a derived success criterion with no closure
//! condition, and it reopened three times. Each round bounded exactly the sites
//! that round's review happened to name; the next review found one it had not.
//! The fixes were real, but "a reviewer must notice" is not a closure condition.
//! This file replaces it with "the suite fails by name".
//!
//! # What each check actually proves — read this before trusting it
//!
//! The two checks have deliberately different strengths, and this file does not
//! claim more than it verifies:
//!
//! * The **whole-body-read check is a structural property check**. It asserts
//!   that every whole-body read in scope is bounded *in its own statement*. It
//!   therefore fails both when a new unbounded read appears AND when an existing
//!   read loses its bound — a site count that never moves cannot hide it.
//! * The **accumulation check is a change detector**, not a proof of
//!   boundedness. Whether appending to a growable buffer is bounded depends on
//!   the drain downstream of it, which no lexical scan can see. So each site is
//!   enumerated with a written justification naming the mechanism that bounds
//!   it, and the check fails when the population changes in either direction.
//!
//! Both checks scan **stripped** source: comments and string/char literal
//! contents are removed before matching, so a doc comment that merely mentions
//! `collect()` in prose is not counted as a site. That stripping is itself unit
//! tested below, because a scanner that over-strips would make every check pass
//! vacuously.
//!
//! # When this file fails
//!
//! Bound the new site, or justify it in the allowlist and have that
//! justification reviewed. Raising a number to match reality is the failure mode
//! this file exists to prevent.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Scope discovery
// ---------------------------------------------------------------------------

/// The directory walked at runtime, so a NEW file cannot escape the scan by
/// nobody remembering to add it here. Losing coverage by omission is exactly
/// how this requirement reopened three times.
const SHARED_DIR: &str = "src/shared";

/// The two individually-named files HTTP-09 puts in scope beyond `src/shared/`.
const EXTRA_SCOPE: &[&str] = &[
    "src/client/subscriptions.rs",
    "src/server/streamable_http_server.rs",
];

/// Files whose absence from the discovered scope means discovery is broken.
///
/// Without this, a `read_dir` that silently returned nothing would make every
/// check in this file pass over an empty set.
const REQUIRED_FILES: &[&str] = &[
    "http.rs",
    "sse_parser.rs",
    "streamable_http.rs",
    "streamable_http_server.rs",
    "subscriptions.rs",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every file HTTP-09 puts in scope, discovered at runtime.
fn scope_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    collect_rs_files(&root.join(SHARED_DIR), &mut files);
    for extra in EXTRA_SCOPE {
        let path = root.join(extra);
        assert!(path.is_file(), "scope file {extra} no longer exists");
        files.push(path);
    }
    files.sort();
    files.dedup();
    assert!(
        !files.is_empty(),
        "scope discovery returned nothing — every check in this file would pass vacuously"
    );
    for required in REQUIRED_FILES {
        assert!(
            files
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == *required)),
            "scope discovery lost {required}; discovered: {:?}",
            files.iter().map(|p| rel(p)).collect::<Vec<_>>()
        );
    }
    files
}

// ---------------------------------------------------------------------------
// Source stripping (comments and literal contents removed, line map retained)
// ---------------------------------------------------------------------------

/// Source rendered with whitespace collapsed away, comments removed and every
/// string/char literal's CONTENT removed (delimiters kept, so a call still
/// reads as a call).
///
/// `lines[i]` is the 1-based source line of `text`'s i-th byte, so every match
/// can be reported as `path:line`.
#[derive(Default)]
struct Stripped {
    text: String,
    lines: Vec<u32>,
}

impl Stripped {
    fn push_char(&mut self, ch: char, line: u32) {
        self.text.push(ch);
        for _ in 0..ch.len_utf8() {
            self.lines.push(line);
        }
    }

    fn push_delims(&mut self, delims: &str, line: u32) {
        for ch in delims.chars() {
            self.push_char(ch, line);
        }
    }
}

/// Source line of the byte at `index` in the stripped text.
fn line_of(stripped: &Stripped, index: usize) -> u32 {
    stripped.lines.get(index).copied().unwrap_or(0)
}

/// A lexical construct whose interior must not be scanned.
struct Construct {
    /// Index one past the construct's last character.
    end: usize,
    /// What to emit in its place (delimiters only, or nothing for comments).
    delims: &'static str,
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn line_numbers(chars: &[char]) -> Vec<u32> {
    let mut lines = Vec::with_capacity(chars.len());
    let mut line: u32 = 1;
    for &ch in chars {
        lines.push(line);
        if ch == '\n' {
            line += 1;
        }
    }
    lines
}

fn end_of_line(chars: &[char], from: usize) -> usize {
    let mut j = from;
    while j < chars.len() && chars[j] != '\n' {
        j += 1;
    }
    j
}

/// End of a block comment, honouring Rust's comment nesting.
fn end_of_block_comment(chars: &[char], from: usize) -> usize {
    let mut depth: usize = 0;
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
            depth += 1;
            j += 2;
        } else if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return j;
            }
        } else {
            j += 1;
        }
    }
    chars.len()
}

/// End of a double-quoted literal, honouring backslash escapes.
fn end_of_string(chars: &[char], from: usize) -> usize {
    let mut j = from + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '"' => return j + 1,
            _ => j += 1,
        }
    }
    chars.len()
}

/// End of an `r"..."` / `r#"..."#` raw string starting at `from` (the `r`).
///
/// `None` when this `r` begins an identifier or a raw identifier (`r#type`)
/// rather than a raw string.
fn raw_string_end(chars: &[char], from: usize) -> Option<usize> {
    let mut hashes: usize = 0;
    let mut j = from + 1;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if chars.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < chars.len() {
        if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(chars.len())
}

/// End of a char literal starting at `from`.
///
/// `None` when the tick opens a LIFETIME (`'a`, `'static`, `'_`) rather than a
/// literal — getting this wrong would swallow the rest of the file.
fn end_of_char_literal(chars: &[char], from: usize) -> Option<usize> {
    let c1 = *chars.get(from + 1)?;
    if c1 == '\\' {
        // The escaped character occupies at least one position, so the closing
        // tick cannot be earlier than `from + 3` (an escaped tick is the tight
        // case).
        let mut j = from + 3;
        while j < chars.len() && chars[j] != '\'' {
            j += 1;
        }
        return Some((j + 1).min(chars.len()));
    }
    if chars.get(from + 2) == Some(&'\'') {
        return Some(from + 3);
    }
    None
}

/// Classify the construct beginning at `i`, if any.
fn skip_construct(chars: &[char], i: usize, prev_ident: bool) -> Option<Construct> {
    let next = chars.get(i + 1).copied();
    match chars[i] {
        '/' if next == Some('/') => Some(Construct {
            end: end_of_line(chars, i),
            delims: "",
        }),
        '/' if next == Some('*') => Some(Construct {
            end: end_of_block_comment(chars, i),
            delims: "",
        }),
        '"' => Some(Construct {
            end: end_of_string(chars, i),
            delims: "\"\"",
        }),
        '\'' => end_of_char_literal(chars, i).map(|end| Construct { end, delims: "''" }),
        'r' if !prev_ident => raw_string_end(chars, i).map(|end| Construct {
            end,
            delims: "\"\"",
        }),
        // `br#"..."#` — the byte-raw-string prefix, whose `r` is preceded by an
        // identifier character and so would otherwise be missed.
        'b' if !prev_ident && next == Some('r') => {
            raw_string_end(chars, i + 1).map(|end| Construct {
                end,
                delims: "\"\"",
            })
        },
        _ => None,
    }
}

/// Strip `source` to scannable text plus a byte-to-line map.
///
/// One pass: comments vanish, literal contents vanish (delimiters survive),
/// whitespace vanishes. Whitespace removal is what lets a rustfmt-broken method
/// chain be matched as a single needle.
fn strip(source: &str) -> Stripped {
    let chars: Vec<char> = source.chars().collect();
    let lines = line_numbers(&chars);
    let mut out = Stripped::default();
    let mut i: usize = 0;
    let mut prev_ident = false;
    while i < chars.len() {
        if let Some(construct) = skip_construct(&chars, i, prev_ident) {
            out.push_delims(construct.delims, lines[i]);
            i = construct.end.max(i + 1);
            prev_ident = false;
            continue;
        }
        let ch = chars[i];
        if ch.is_whitespace() {
            prev_ident = false;
        } else {
            out.push_char(ch, lines[i]);
            prev_ident = is_ident_char(ch);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// `cfg`-test region exclusion
// ---------------------------------------------------------------------------

/// Index of the delimiter closing the group opened at `open`.
fn balanced_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (opener, closer) = match bytes.get(open)? {
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'{' => (b'{', b'}'),
        _ => return None,
    };
    let mut depth: usize = 0;
    for (offset, byte) in bytes.iter().enumerate().skip(open) {
        if *byte == opener {
            depth += 1;
        } else if *byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

/// Split a `cfg` predicate list on top-level commas.
fn split_top_level(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start: usize = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..idx].trim());
                start = idx + 1;
            },
            _ => {},
        }
    }
    parts.push(inner[start..].trim());
    parts
}

/// Whether a `cfg` predicate can only hold when `test` is enabled.
///
/// True for `test` itself and for any `all(...)` carrying `test` as a conjunct —
/// which is what makes the streamable HTTP transport's
/// `all(test, not(target_arch = "wasm32"), feature = "streamable-http")` module
/// an excluded region even though a bare `#[cfg(test)]` grep finds nothing in
/// that 108 KB file.
///
/// False when `test` appears only inside an `any(...)`: such an item COMPILES
/// WITHOUT `test` (the `any(feature = "fuzzing", test)` fuzz seam is on in every
/// build that enables that feature), so it ships and must stay in scope.
///
/// Matching is by whole-conjunct equality, so `latest`, `testing` and
/// `test_utils` do not trigger it.
fn cfg_requires_test(predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate == "test" {
        return true;
    }
    let Some(inner) = predicate
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    split_top_level(inner).into_iter().any(cfg_requires_test)
}

/// Span of the item that follows an attribute ending at `from`.
///
/// Skips any further attributes, jumps over balanced groups so a comma inside a
/// parameter list is not mistaken for the end of the item, and ends at either
/// the item's brace-matched body or the `;` / `,` terminating a body-less item.
///
/// Brace matching runs over the ALREADY-STRIPPED text, so a brace inside a
/// string or a comment cannot unbalance it.
fn item_span(text: &str, from: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => i = balanced_end(text, i)? + 1,
            b';' | b',' => return Some(from..i + 1),
            b'{' => return balanced_end(text, i).map(|end| from..end + 1),
            _ => i += 1,
        }
    }
    None
}

/// Every region of `stripped` that only compiles under `cfg(test)`.
///
/// Deliberately per-item rather than "truncate at the first marker": the
/// streamable HTTP server has two `cfg(test)` FUNCTIONS ahead of its test
/// module, and truncating at the first would drop thousands of lines of
/// production code from the scan — the miss-by-omission failure this file
/// exists to prevent.
fn cfg_test_spans(stripped: &Stripped) -> Vec<Range<usize>> {
    let text = &stripped.text;
    let mut spans = Vec::new();
    let mut search: usize = 0;
    while let Some(found) = text[search..].find("#[cfg(") {
        let paren = search + found + "#[cfg".len();
        let Some(close) = balanced_end(text, paren) else {
            break;
        };
        let predicate = &text[paren + 1..close];
        search = close + 1;
        if !cfg_requires_test(predicate) {
            continue;
        }
        if let Some(span) = item_span(text, search) {
            search = span.end.max(search);
            spans.push(span);
        }
    }
    spans
}

fn is_excluded(spans: &[Range<usize>], index: usize) -> bool {
    spans.iter().any(|span| span.contains(&index))
}

// ---------------------------------------------------------------------------
// Match enumeration
// ---------------------------------------------------------------------------

/// Byte offsets of every occurrence of `needle` in `text`.
fn occurrences(text: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from: usize = 0;
    while let Some(found) = text[from..].find(needle) {
        let at = from + found;
        out.push(at);
        from = at + 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Tests for the scanner itself — without these, every check built on top of it
// could pass vacuously and nobody would know.
// ---------------------------------------------------------------------------

mod scanner {
    use super::{
        cfg_requires_test, cfg_test_spans, is_excluded, line_of, occurrences, scope_files, strip,
    };

    /// Line of the first stripped occurrence of `needle`, if any.
    fn find_one(source: &str, needle: &str) -> Option<u32> {
        let stripped = strip(source);
        let at = stripped.text.find(needle)?;
        Some(line_of(&stripped, at))
    }

    #[test]
    fn line_comments_doc_comments_and_block_comments_are_not_scanned() {
        let source = "fn f() {\n    // let x = body.collect().await;\n    let y = 1;\n}\n";
        assert!(find_one(source, ".collect().await").is_none());

        let doc = "/// See `body.collect().await` for the unbounded shape.\nfn f() {}\n";
        assert!(find_one(doc, ".collect().await").is_none());

        let inner = "//! `body.collect().await` is the unbounded form.\nfn f() {}\n";
        assert!(find_one(inner, ".collect().await").is_none());

        let block =
            "fn f() {\n/* body.collect().await /* nested */ still comment */\nlet y = 1;\n}\n";
        assert!(find_one(block, ".collect().await").is_none());
        assert!(
            find_one(block, "lety=1").is_some(),
            "a nested block comment must end where Rust says it ends"
        );
    }

    #[test]
    fn string_and_raw_string_contents_are_not_scanned() {
        let source = "fn f() {\n    let msg = \"body.collect().await\";\n}\n";
        assert!(find_one(source, ".collect().await").is_none());
        assert!(
            find_one(source, "letmsg=\"\";").is_some(),
            "delimiters must survive so a call still reads as a call"
        );

        let raw = "fn f() {\n    let msg = r#\"body.collect().await\"#;\n}\n";
        assert!(find_one(raw, ".collect().await").is_none());

        let byte_raw = "fn f() {\n    let msg = br#\"body.collect().await\"#;\n}\n";
        assert!(find_one(byte_raw, ".collect().await").is_none());
    }

    #[test]
    fn lifetimes_and_char_literals_do_not_derail_the_scan() {
        let source = "fn f<'a>(s: &'a str) -> char {\n    let c = '\\'';\n    let d = 'x';\n    \
                      let _ = (c, d, s);\n    body.collect().await\n}\n";
        assert_eq!(
            find_one(source, ".collect().await"),
            Some(5),
            "tick handling swallowed the rest of the file"
        );
    }

    #[test]
    fn a_rustfmt_broken_chain_is_matched_and_reports_its_first_line() {
        let source = "fn f() {\n    let b = body\n        .collect()\n        .await;\n}\n";
        assert_eq!(
            find_one(source, ".collect().await"),
            Some(3),
            "a chain broken across lines must match, at the line of its FIRST character"
        );
    }

    #[test]
    fn cfg_requires_test_classifies_the_documented_predicate_shapes() {
        assert!(cfg_requires_test("test"));
        assert!(cfg_requires_test(
            "all(test, not(target_arch = \"wasm32\"), feature = \"streamable-http\")"
        ));
        assert!(
            !cfg_requires_test("any(feature = \"fuzzing\", test)"),
            "an any(...) item compiles WITHOUT test, so it ships and stays in scope"
        );
        assert!(!cfg_requires_test("feature = \"testing\""));
        assert!(!cfg_requires_test("all(test_utils, latest)"));
    }

    #[test]
    fn a_cfg_test_fn_excludes_only_its_own_body() {
        let source = "#[cfg(test)]\nfn helper(a: u8, b: u8) -> u8 {\n    \
                      let _ = body.collect().await;\n    a + b\n}\n\n\
                      fn shipped() {\n    body.collect().await\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = occurrences(&stripped.text, ".collect().await");
        assert_eq!(
            hits.len(),
            2,
            "both occurrences should be lexically present"
        );
        assert!(
            is_excluded(&spans, hits[0]),
            "the cfg(test) helper body must be excluded"
        );
        assert!(
            !is_excluded(&spans, hits[1]),
            "production code AFTER a cfg(test) item must still be scanned — truncating at the \
             first marker is the under-scan this file exists to prevent"
        );
    }

    #[test]
    fn a_body_less_cfg_test_item_does_not_swallow_the_file() {
        let source = "#[cfg(test)]\nuse super::*;\n\nfn shipped() {\n    body.collect().await\n}\n";
        let stripped = strip(source);
        let spans = cfg_test_spans(&stripped);
        let hits = occurrences(&stripped.text, ".collect().await");
        assert_eq!(hits.len(), 1);
        assert!(!is_excluded(&spans, hits[0]));
    }

    #[test]
    fn scope_discovery_finds_the_named_files_at_runtime() {
        let files = scope_files();
        let names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        for required in [
            "http.rs",
            "sse_parser.rs",
            "streamable_http.rs",
            "streamable_http_server.rs",
            "subscriptions.rs",
        ] {
            assert!(names.contains(&required.to_string()), "missing {required}");
        }
        assert!(
            files.len() > 20,
            "src/shared/ carries about thirty files; discovering {} suggests the walk is broken",
            files.len()
        );
    }
}
