//! Phase 115 tripwires — the invariants configuration alone cannot keep.
//!
//! # What this file fences
//!
//! * **SEP-2106 (SCHM-01)** — output-schema validation must never fetch an
//!   external `$ref`. That property is currently STRUCTURAL: `jsonschema` is
//!   declared `default-features = false` everywhere, so its `resolve-http` /
//!   `resolve-file` retrievers are not compiled in and an external `$ref`
//!   compiles down to a hard `Err`. Cargo feature unification is decided by the
//!   WHOLE dependency graph, though, so one workspace member, example or
//!   dev-dependency declaring `jsonschema` with default features turns the
//!   refusal into a live outbound fetch — and a behavioural test would still
//!   pass, indeed it would pass *better*, because the fetch would succeed. Only
//!   a dependency-graph check catches that.
//! * **D-12 (SCHM-03)** — the caching hints are written at exactly ONE shared
//!   projection point, in the `cfg`-free `src/types/caching.rs`.
//! * **The wasm dispatcher's strip call** — `src/server/wasm_server.rs` is
//!   `cfg(target_arch = "wasm32")`, so no native gate compiles it and no gate at
//!   all executes it. 115-06 MEASURED that deleting its `project_caching_hints`
//!   call leaves `make wasm-build` at exit 0, which makes the source assertion
//!   here the only automated gate that can catch the removal.
//! * **The projection-versus-middleware ordering** — pinned by measurement, so a
//!   silent reorder fails a named test.
//!
//! # Manifests are NEVER read as text
//!
//! The pre-review shape of this file scanned `Cargo.toml` dependency LINES with
//! string matching. That misses a table-style declaration, a multiline
//! declaration, a dependency renamed via `package = "jsonschema"`, and any
//! future `[workspace.dependencies]` inheritance. This file parses cargo's own
//! output instead, in two layers:
//!
//! 1. `cargo metadata --no-deps` → every workspace package's DECLARED
//!    dependency, with `rename`, `optional`, `uses_default_features` and
//!    `features` as structured fields;
//! 2. `cargo metadata --features validation` → the RESOLVED graph's
//!    `resolve.nodes[].features`, which is the definitive unification answer and
//!    the only layer that sees a dev-dependency or an example turning a feature
//!    on.
//!
//! This needs no new dependency: `std::process::Command` plus `serde_json`.
//!
//! # The scanner primitives are DELIBERATELY duplicated
//!
//! A Rust integration test is its own crate, so this file cannot import
//! `tests/v2_tasks_tripwires.rs`'s scanner and that file cannot import this one.
//! The primitives below are therefore RESTATED rather than shared, and the idiom
//! is kept identical on purpose so the repository has ONE source-scanning shape
//! rather than three divergent ones. The cross-AI review flagged the duplication
//! as surface cost; it is declined as a trim for exactly that reason.
//!
//! # Every test name carries the file stem
//!
//! Every test function here begins with `v2_schema_tripwires_`, so BOTH
//! `binary(v2_schema_tripwires)` and `test(/v2_schema_tripwires/)` select this
//! suite. The nextest `test(...)` selector matches the test NAME, not the binary
//! name, and silently selects zero tests when the two differ — which is not a
//! failure, it is a green run over nothing.
//!
//! # When a check here fails
//!
//! Restore the invariant, or move the allowlist entry and write down why.
//! Deleting the check, or widening it until it passes, is the failure mode it
//! exists to prevent.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// A justification shorter than this is a label, not a decision.
const MIN_JUSTIFICATION_CHARS: usize = 40;

/// The crate whose feature set decides whether output validation can fetch.
const JSONSCHEMA: &str = "jsonschema";

/// The module that owns the era-keyed validator compilation.
const OUTPUT_VALIDATION: &str = "src/server/output_validation.rs";

/// `jsonschema`'s DEFAULT features, verbatim from `cargo info jsonschema`, plus
/// the two other resolver/TLS features a manifest could reach for.
///
/// `resolve-http` pulls in `reqwest` + `rustls`; `resolve-async` pulls in
/// `referencing/retrieve-async` and `reqwest`. Any of them turns an external
/// `$ref` inside output validation into an outbound request.
const RESOLVER_FEATURES: &[&str] = &[
    "resolve-http",
    "resolve-file",
    "resolve-async",
    "tls-aws-lc-rs",
    "tls-ring",
];

/// Identifiers that install a `$ref` retriever, in any of its spellings.
///
/// `with_retriever` / `with_http_options` are the builder entry points;
/// `Retrieve` / `AsyncRetrieve` / `Retriever` catch a hand-written
/// implementation of the trait, which is the same capability arrived at from the
/// other direction.
const RETRIEVER_NEEDLES: &[&str] = &[
    "with_retriever",
    "with_http_options",
    "Retrieve",
    "AsyncRetrieve",
    "Retriever",
];

/// The two identifiers that construct a `jsonschema` validator.
const VALIDATOR_NEEDLES: &[&str] = &["validator_for", "draft202012"];

/// The remedy every SEP-2106 failure message points at.
const SEP_2106_WHY: &str = "\
SEP-2106: output-schema validation MUST NOT fetch an external `$ref`.\n\
  `jsonschema`'s DEFAULT features are [\"resolve-http\", \"resolve-file\", \"tls-aws-lc-rs\"], and \
cargo feature unification is GRAPH-WIDE: one workspace member, example or dev-dependency that \
enables any of them enables it for every crate in the graph, including the MCP output-validation \
path.\n  The refusal this phase measured (~60 microseconds, no socket) is STRUCTURAL — the \
retriever is not compiled in, so an external `$ref` is a hard `Err`. Enabling a resolver feature \
converts that into a live outbound fetch performed by the SERVER on a schema an untrusted tool \
author supplied: server-side request forgery from inside a validation path.\n  A behavioural test \
would NOT catch this. It would pass better, because the fetch would succeed.";

// ===========================================================================
// 1. Scanner primitives — restated, not shared. See the module docs.
// ===========================================================================

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

fn read(path: &str) -> String {
    let full = repo_root().join(path);
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("cannot read {}: {e}", full.display()))
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

/// Every `.rs` file under `src/`, discovered at RUNTIME with `read_dir` so a NEW
/// file cannot escape the scan by nobody remembering to add it.
fn src_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(&repo_root().join("src"), &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "src/ carries well over fifty files; discovering {} means the walk is broken and every \
         check in this file would pass vacuously",
        files.len()
    );
    files
}

// --- stripping (comments removed; literals removed or kept, line map preserved) ---

/// Source with whitespace collapsed and comments removed.
///
/// `lines[i]` is the 1-based source line of `text`'s i-th byte.
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

fn line_of(stripped: &Stripped, index: usize) -> u32 {
    stripped.lines.get(index).copied().unwrap_or(0)
}

struct Construct {
    end: usize,
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

/// End of an `r"..."` / `r#"..."#` raw string starting at `from`.
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

/// End of a char literal, or `None` when the tick opens a LIFETIME.
fn end_of_char_literal(chars: &[char], from: usize) -> Option<usize> {
    let c1 = *chars.get(from + 1)?;
    if c1 == '\\' {
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
/// Comments always vanish. String and char literal CONTENTS vanish too unless
/// `keep_literals`, in which case the literal is copied through verbatim.
///
/// Whitespace collapses to a single space rather than vanishing, because this
/// scanner matches IDENTIFIERS, which need word boundaries: removing whitespace
/// entirely turns `pub const FOO` into `pubconstFOO`, whose preceding character
/// is an identifier character, so a whole-token filter would reject the
/// DEFINITION site and silently lose coverage of the file being scanned.
fn strip_with(source: &str, keep_literals: bool) -> Stripped {
    let chars: Vec<char> = source.chars().collect();
    let lines = line_numbers(&chars);
    let mut out = Stripped::default();
    let mut i: usize = 0;
    let mut prev_ident = false;
    let mut pending_space = false;
    while i < chars.len() {
        if let Some(construct) = skip_construct(&chars, i, prev_ident) {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            if keep_literals && !construct.delims.is_empty() {
                for (j, ch) in chars.iter().enumerate().take(construct.end).skip(i) {
                    out.push_char(*ch, lines[j]);
                }
            } else {
                out.push_delims(construct.delims, lines[i]);
            }
            i = construct.end.max(i + 1);
            prev_ident = false;
            continue;
        }
        let ch = chars[i];
        if ch.is_whitespace() {
            prev_ident = false;
            pending_space = true;
        } else {
            if pending_space {
                out.push_char(' ', lines[i]);
                pending_space = false;
            }
            out.push_char(ch, lines[i]);
            prev_ident = is_ident_char(ch);
        }
        i += 1;
    }
    out
}

/// Comments and literal CONTENTS removed — the mode for identifier scans.
fn strip(source: &str) -> Stripped {
    strip_with(source, false)
}

// --- `cfg(test)` region exclusion ---

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
/// Per-item brace matching, NOT truncation at the first marker: truncating would
/// drop thousands of production lines from the larger server modules.
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

/// A whole-token match: `Retrieve` must not match `Retriever`.
fn token_hits(text: &str, needle: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    occurrences(text, needle)
        .into_iter()
        .filter(|at| {
            let before_ok = *at == 0 || !is_ident_char(char::from(bytes[at - 1]));
            let after = at + needle.len();
            let after_ok = after >= bytes.len() || !is_ident_char(char::from(bytes[after]));
            before_ok && after_ok
        })
        .collect()
}

// --- function spans, so a site is attributed to the function that owns it ---

/// The body span of every named `fn` in `stripped`, innermost-resolvable.
///
/// A declaration with no body (`fn f(&self) -> bool;` in a trait) contributes
/// nothing, so the next function's body is never mistaken for it.
fn fn_spans(stripped: &Stripped) -> Vec<(String, Range<usize>)> {
    let text = &stripped.text;
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    for at in token_hits(text, "fn") {
        let rest = text[at + 2..].trim_start();
        let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
        if name.is_empty() {
            continue;
        }
        let after_name = at + 2 + (text.len() - at - 2 - rest.len()) + name.len();
        let Some(paren) = text[after_name..].find('(').map(|o| o + after_name) else {
            continue;
        };
        let Some(params_end) = balanced_end(text, paren) else {
            continue;
        };
        let mut i = params_end + 1;
        while i < bytes.len() && bytes[i] != b'{' && bytes[i] != b';' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b';' {
            continue;
        }
        if let Some(end) = balanced_end(text, i) {
            spans.push((name, i..end + 1));
        }
    }
    spans
}

/// The INNERMOST function whose body contains `index`.
fn enclosing_fn(spans: &[(String, Range<usize>)], index: usize) -> Option<&str> {
    spans
        .iter()
        .filter(|(_, span)| span.contains(&index))
        .min_by_key(|(_, span)| span.end - span.start)
        .map(|(name, _)| name.as_str())
}

// ===========================================================================
// 2. Cargo's own dependency graph — declared AND resolved.
// ===========================================================================

/// Run `cargo metadata` with `args` and parse its stdout.
///
/// Fails LOUDLY on a non-zero exit, naming the command, its status and its
/// stderr, rather than returning an empty document. A broken invocation that
/// returned "no dependencies" would make every check keyed on it pass over
/// nothing, which is the exact failure mode these tripwires exist to prevent.
fn cargo_metadata(args: &[&str]) -> Value {
    let cargo = env!("CARGO");
    let rendered = args.join(" ");
    let output = Command::new(cargo)
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("cannot run `{cargo} {rendered}`: {e}"));
    assert!(
        output.status.success(),
        "`{cargo} {rendered}` exited with {}; a broken invocation must fail loudly rather than \
         yield an empty dependency graph every check below would pass over.\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!("`{cargo} {rendered}` produced output that is not JSON metadata: {e}")
    })
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// One DECLARED dependency on [`JSONSCHEMA`], as cargo itself reports it.
///
/// `rename` is the field that catches a `package = "jsonschema"` alias: a
/// renamed dependency still reports `"name": "jsonschema"` with a non-null
/// `rename`, which a text scan of the manifest KEY would miss entirely.
#[derive(Debug)]
struct DeclaredDep {
    package: String,
    rename: Option<String>,
    optional: bool,
    uses_default_features: bool,
    features: Vec<String>,
    req: String,
    kind: Option<String>,
}

impl DeclaredDep {
    /// How this declaration should be named in a failure message.
    fn describe(&self) -> String {
        format!(
            "package `{}` (rename: {:?}, kind: {:?}, req: {}, optional: {})",
            self.package, self.rename, self.kind, self.req, self.optional
        )
    }
}

fn declared_dep(package: &str, dep: &Value) -> DeclaredDep {
    DeclaredDep {
        package: package.to_string(),
        rename: dep["rename"].as_str().map(ToString::to_string),
        optional: dep["optional"].as_bool().unwrap_or(false),
        uses_default_features: dep["uses_default_features"].as_bool().unwrap_or(true),
        features: string_array(&dep["features"]),
        req: dep["req"].as_str().unwrap_or("<unknown>").to_string(),
        kind: dep["kind"].as_str().map(ToString::to_string),
    }
}

/// Every workspace package's DECLARED dependency on [`JSONSCHEMA`].
///
/// Matching on the dependency's `name` rather than on the manifest key is what
/// catches a `package = "jsonschema"` alias, a table-style declaration, a
/// multiline declaration and a future `[workspace.dependencies]` inheritance —
/// every case a text scan of `Cargo.toml` misses.
fn declared_jsonschema_deps() -> Vec<DeclaredDep> {
    let meta = cargo_metadata(&["metadata", "--format-version", "1", "--no-deps"]);
    let packages = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array");
    assert!(
        !packages.is_empty(),
        "`cargo metadata --no-deps` reported ZERO workspace packages; the manifest scan would \
         pass over nothing"
    );
    let mut out = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or("<unnamed>");
        let Some(deps) = package["dependencies"].as_array() else {
            continue;
        };
        for dep in deps {
            if dep["name"].as_str() == Some(JSONSCHEMA) {
                out.push(declared_dep(name, dep));
            }
        }
    }
    out.sort_by(|a, b| a.package.cmp(&b.package));
    out
}

/// One RESOLVED [`JSONSCHEMA`] node in the unified dependency graph.
#[derive(Debug)]
struct ResolvedNode {
    version: String,
    features: Vec<String>,
    id: String,
}

/// Every resolved [`JSONSCHEMA`] node, with the features unification settled on.
///
/// This is the definitive answer: `.resolve.nodes[].features` is what will
/// actually be compiled, after every workspace member, example, dev-dependency
/// and transitive dependency has had its say.
fn resolved_jsonschema_nodes() -> Vec<ResolvedNode> {
    let meta = cargo_metadata(&[
        "metadata",
        "--format-version",
        "1",
        "--features",
        "validation",
    ]);
    let packages = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array");
    let by_id: BTreeMap<&str, &Value> = packages
        .iter()
        .filter_map(|package| package["id"].as_str().map(|id| (id, package)))
        .collect();
    let nodes = meta["resolve"]["nodes"]
        .as_array()
        .expect("a resolving `cargo metadata` run always reports `resolve.nodes`");
    assert!(
        !nodes.is_empty(),
        "`cargo metadata --features validation` resolved ZERO nodes; the unification check would \
         pass over nothing"
    );
    let mut out = Vec::new();
    for node in nodes {
        let Some(id) = node["id"].as_str() else {
            continue;
        };
        let Some(package) = by_id.get(id) else {
            continue;
        };
        if package["name"].as_str() != Some(JSONSCHEMA) {
            continue;
        }
        out.push(ResolvedNode {
            version: package["version"]
                .as_str()
                .unwrap_or("<unknown>")
                .to_string(),
            features: string_array(&node["features"]),
            id: id.to_string(),
        });
    }
    out
}

/// Every workspace package's `src/` directory, discovered from cargo metadata.
///
/// Runtime discovery rather than a hardcoded list: a NEW workspace member that
/// declared `jsonschema` or installed a retriever would otherwise escape both
/// scans by nobody remembering to add it here.
fn workspace_src_dirs() -> Vec<PathBuf> {
    let meta = cargo_metadata(&["metadata", "--format-version", "1", "--no-deps"]);
    let mut out: Vec<PathBuf> = meta["packages"]
        .as_array()
        .expect("`cargo metadata` always reports a `packages` array")
        .iter()
        .filter_map(|package| package["manifest_path"].as_str())
        .filter_map(|manifest| Path::new(manifest).parent().map(|dir| dir.join("src")))
        .filter(|dir| dir.is_dir())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every `.rs` file under every workspace package's `src/`.
fn workspace_rs_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in workspace_src_dirs() {
        collect_rs_files(&dir, &mut files);
    }
    files.sort();
    files.dedup();
    assert!(
        files.len() > 300,
        "the workspace carries well over three hundred source files; discovering {} means the \
         walk is broken and every workspace-wide check here would pass vacuously",
        files.len()
    );
    files
}

// ===========================================================================
// 3. Shared allowlist discipline.
// ===========================================================================

/// Every entry in a justified allowlist carries a real, distinct reason.
///
/// Length alone is trivially defeated by padding; pairwise distinctness alone is
/// defeated by five one-word labels. Both together mean a copy-pasted or empty
/// justification fails.
fn assert_justifications(label: &str, entries: &[(&str, &str)]) {
    let mut seen: Vec<&str> = Vec::new();
    for (name, why) in entries {
        let why = why.trim();
        assert!(
            why.len() >= MIN_JUSTIFICATION_CHARS,
            "{label} entry `{name}` needs a real justification, not {why:?} ({} chars, minimum \
             {MIN_JUSTIFICATION_CHARS})",
            why.len()
        );
        assert!(
            !seen.contains(&why),
            "{label} entry `{name}` reuses another entry's justification verbatim; a copy-pasted \
             reason is not a reason"
        );
        seen.push(why);
    }
    assert!(
        !entries.is_empty(),
        "{label} is EMPTY, so every check keyed on it passes over nothing"
    );
}

// ===========================================================================
// 4. TASK 1 — SEP-2106, over cargo's DECLARED graph.
// ===========================================================================

/// No workspace manifest may declare [`JSONSCHEMA`] with a resolver enabled.
#[test]
fn v2_schema_tripwires_no_manifest_declares_jsonschema_with_default_features() {
    let declared = declared_jsonschema_deps();
    let mut failures = String::new();

    for dep in &declared {
        if dep.uses_default_features {
            let _ = writeln!(
                failures,
                "\n  DEFAULT FEATURES ON: {} declares `{JSONSCHEMA}` WITHOUT \
                 `default-features = false`.\n    Add `default-features = false` to that \
                 declaration.",
                dep.describe()
            );
        }
        for feature in &dep.features {
            if RESOLVER_FEATURES.contains(&feature.as_str()) {
                let _ = writeln!(
                    failures,
                    "\n  RESOLVER FEATURE ON: {} enables `{JSONSCHEMA}/{feature}`.\n    Remove it \
                     from that declaration's `features` list.",
                    dep.describe()
                );
            }
        }
    }

    assert!(failures.is_empty(), "{SEP_2106_WHY}\n{failures}");
}

/// The RESOLVED graph — the definitive unification answer.
#[test]
fn v2_schema_tripwires_the_resolved_graph_enables_no_jsonschema_resolver_feature() {
    let nodes = resolved_jsonschema_nodes();

    assert_eq!(
        nodes.len(),
        1,
        "expected EXACTLY ONE resolved `{JSONSCHEMA}` node and found {}: {nodes:#?}.\n  Two nodes \
         means two copies are compiled into the same graph, which is the state 115-03's \
         workspace-wide bump to a single `0.49` requirement exists to prevent: one validator \
         could then be pinned and the other not.",
        nodes.len()
    );

    for node in &nodes {
        assert!(
            node.features.is_empty(),
            "{SEP_2106_WHY}\n  RESOLVED node `{}` compiles with features {:?}, not [].\n  This is \
             the ONLY check that sees the effect of a DEV-dependency, an example, or a sibling \
             workspace crate turning a feature on: unification is graph-wide, so the declared \
             dependency check above would still pass while the retriever is compiled in.",
            node.id,
            node.features
        );
        assert!(
            node.version.starts_with("0.49"),
            "the resolved `{JSONSCHEMA}` version is {}, not 0.49.x.\n  115-03 pinned the whole \
             workspace to `0.49` and MEASURED the Draft 2020-12 divergence case \
             (`contentEncoding`) against 0.49.2. A different major/minor may restore \
             `resolve-http` to a different default set or change the dialect behaviour, so the \
             measurement has to be redone before this pin moves.",
            node.version
        );
    }
}

/// ANTI-VACUITY for the two `cargo metadata` scans.
#[test]
fn v2_schema_tripwires_the_manifest_scan_is_not_vacuous() {
    let declared = declared_jsonschema_deps();
    assert!(
        declared.len() >= 3,
        "expected at least three DECLARED `{JSONSCHEMA}` dependencies and found {}: {declared:#?}\
         \n  Without this the two checks above would pass over an empty set, which is a green run \
         over nothing rather than a clean bill of health.",
        declared.len()
    );

    let packages: BTreeSet<&str> = declared.iter().map(|dep| dep.package.as_str()).collect();
    for required in ["pmcp", "pmcp-agent", "pmcp-server-toolkit"] {
        assert!(
            packages.contains(required),
            "`{required}` declares `{JSONSCHEMA}` (measured 2026-08-01) yet the metadata scan did \
             not find it. Observed: {packages:?}.\n  Either the declaration moved — in which case \
             update this list deliberately — or the scan is broken."
        );
    }

    let resolved = resolved_jsonschema_nodes();
    assert!(
        !resolved.is_empty(),
        "the RESOLVED scan found no `{JSONSCHEMA}` node at all; the unification check above would \
         then iterate over an empty vector and pass"
    );
}

/// No source file may install a `$ref` retriever, in any spelling.
#[test]
fn v2_schema_tripwires_no_source_installs_a_ref_retriever() {
    let mut failures = String::new();

    for path in workspace_rs_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !RETRIEVER_NEEDLES.iter().any(|needle| raw.contains(needle)) {
            continue;
        }
        let stripped = strip(&raw);
        for needle in RETRIEVER_NEEDLES {
            for at in token_hits(&stripped.text, needle) {
                let _ = writeln!(
                    failures,
                    "\n  RETRIEVER: `{needle}` at {}:{}",
                    rel(&path),
                    line_of(&stripped, at)
                );
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{SEP_2106_WHY}\n  A retriever was installed in source:{failures}\n  Today the refusal is \
         STRUCTURAL — with `default-features = false` the retriever compiles down to a hard \
         `Err`, so no code path can fetch. Installing one converts that structural guarantee into \
         a POLICY we hope holds, reachable from any schema an untrusted tool author supplies."
    );
}

/// Why a `jsonschema` validator construction site is accounted for.
enum ValidatorDisposition {
    /// The v2 arm: explicitly pinned to Draft 2020-12 (D-02 / SCHM-01).
    PinnedByPolicy,
    /// The v1 arm: frozen at today's auto-detecting behaviour (D-01).
    EraFrozenV1,
    /// Not the MCP `outputSchema` seam at all — a recorded, deferred exception.
    OutOfScopeAllowlisted,
}

/// One allowlisted validator construction site.
struct ValidatorSite {
    file: &'static str,
    function: &'static str,
    hits: usize,
    disposition: ValidatorDisposition,
    why: &'static str,
}

/// Every `validator_for(` / `draft202012::` construction site in the workspace.
///
/// The `hits` count is part of the entry on purpose: a SECOND construction added
/// inside an already-allowlisted function is exactly the shape a regression
/// takes, and a file-level or function-level presence check cannot see it.
const VALIDATOR_SITES: &[ValidatorSite] = &[
    ValidatorSite {
        file: "src/server/output_validation.rs",
        function: "compile_2020_12",
        hits: 1,
        disposition: ValidatorDisposition::PinnedByPolicy,
        why: "The v2 arm. MCP 2026-07-28 pins outputSchema to JSON Schema Draft 2020-12, so this \
              site must construct through `draft202012::new` on a document whose `$schema` has \
              been normalized first — never through the auto-detecting `validator_for`, which \
              would silently honour a tool author's declared dialect instead of the spec's.",
    },
    ValidatorSite {
        file: "src/server/output_validation.rs",
        function: "compile_for_era",
        hits: 1,
        disposition: ValidatorDisposition::EraFrozenV1,
        why: "The v1 arm, deliberately frozen by D-01 at today's behaviour: the dialect is \
              auto-detected from the document's own `$schema` declaration. Changing this site \
              would alter validation outcomes for every existing 2025-11-25 server, which is a \
              breaking change this phase explicitly declined to make.",
    },
    ValidatorSite {
        file: "crates/pmcp-agent/src/iteration/decide.rs",
        function: "evaluate_submit_result",
        hits: 1,
        disposition: ValidatorDisposition::OutOfScopeAllowlisted,
        why: "Out of scope, recorded rather than fixed. This validates an AGENT's submit-result \
              payload against a caller-supplied schema; it is not the MCP outputSchema seam. \
              SCHM-01 scopes to the server output-validation path, and pinning the draft here \
              would be a behaviour change to a different surface with its own users, so it is \
              booked as a deferred item instead of being changed inside a schema-pinning phase.",
    },
];

/// Every validator construction site, grouped by file and enclosing function.
fn validator_sites() -> BTreeMap<(String, String), Vec<u32>> {
    let mut out: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    for path in workspace_rs_files() {
        let raw = fs::read_to_string(&path).expect("readable source");
        if !VALIDATOR_NEEDLES.iter().any(|needle| raw.contains(needle)) {
            continue;
        }
        let stripped = strip(&raw);
        let spans = fn_spans(&stripped);
        let excluded = cfg_test_spans(&stripped);
        for needle in VALIDATOR_NEEDLES {
            for at in token_hits(&stripped.text, needle) {
                if is_excluded(&excluded, at) {
                    continue;
                }
                let owner = enclosing_fn(&spans, at)
                    .unwrap_or("<file scope>")
                    .to_string();
                out.entry((rel(&path), owner))
                    .or_default()
                    .push(line_of(&stripped, at));
            }
        }
    }
    out
}

/// The validator construction population equals the allowlist, count by count.
#[test]
fn v2_schema_tripwires_validator_construction_sites_are_accounted_for() {
    let observed = validator_sites();
    let mut failures = String::new();

    for ((file, function), lines) in &observed {
        let Some(entry) = VALIDATOR_SITES
            .iter()
            .find(|site| site.file == file && site.function == function)
        else {
            let _ = writeln!(
                failures,
                "\n  UNKNOWN validator construction site: `{function}` in {file} at line(s) \
                 {lines:?}.\n    Every site that builds a `{JSONSCHEMA}` validator has to state \
                 which dialect policy it is under: pinned to 2020-12 (v2), frozen at \
                 auto-detection (v1), or out of scope with a written reason."
            );
            continue;
        };
        if entry.hits != lines.len() {
            let _ = writeln!(
                failures,
                "\n  COUNT CHANGED: `{function}` in {file} was recorded with {} construction \
                 site(s) and now has {} at line(s) {lines:?}.\n    A second construction inside \
                 an already-allowlisted function is exactly the shape this regression takes; \
                 re-derive the entry rather than raising the number.",
                entry.hits,
                lines.len()
            );
        }
    }

    for site in VALIDATOR_SITES {
        let key = (site.file.to_string(), site.function.to_string());
        if !observed.contains_key(&key) {
            let _ = writeln!(
                failures,
                "\n  STALE entry: `{}` in {} no longer constructs a validator. Delete the entry.",
                site.function, site.file
            );
        }
    }

    assert!(
        failures.is_empty(),
        "the `{JSONSCHEMA}` validator construction population changed:{failures}"
    );

    // The v2 arm must construct through the PINNED constructor, and the v1 arm
    // through the auto-detecting one. A straight swap keeps the counts identical.
    let pinned = read(OUTPUT_VALIDATION);
    let stripped = strip(&pinned);
    let spans = fn_spans(&stripped);
    for site in VALIDATOR_SITES {
        let needle = match site.disposition {
            ValidatorDisposition::PinnedByPolicy => "draft202012",
            ValidatorDisposition::EraFrozenV1 => "validator_for",
            ValidatorDisposition::OutOfScopeAllowlisted => continue,
        };
        let Some((_, span)) = spans.iter().find(|(name, _)| name == site.function) else {
            panic!(
                "`{}` must still exist in {}; the dialect-policy check cannot run over a \
                 function that is gone",
                site.function, site.file
            )
        };
        let body = &stripped.text[span.clone()];
        assert!(
            !token_hits(body, needle).is_empty(),
            "`{}` no longer constructs through `{needle}`. The dialect policy of that arm was \
             swapped while the site COUNT stayed the same, which the population check above \
             cannot see.",
            site.function
        );
    }

    assert_justifications(
        "VALIDATOR_SITES",
        &VALIDATOR_SITES
            .iter()
            .map(|site| (site.function, site.why))
            .collect::<Vec<_>>(),
    );
}

/// ANTI-VACUITY for the source scans — a green run must mean "checked".
#[test]
fn v2_schema_tripwires_the_source_scan_is_not_vacuous() {
    assert!(
        src_files().len() > 50,
        "the `src/` walk collapsed; every source check here would pass over nothing"
    );
    assert!(
        workspace_rs_files().len() > 300,
        "the workspace walk collapsed; the retriever and validator scans would pass over nothing"
    );

    let observed = validator_sites();
    assert!(
        !observed.is_empty(),
        "the validator construction scan found NO site at all; the allowlist check would then \
         iterate over an empty map and pass"
    );

    let source = read(OUTPUT_VALIDATION);
    for needle in VALIDATOR_NEEDLES {
        assert!(
            source.contains(needle),
            "{OUTPUT_VALIDATION} no longer mentions `{needle}`; the era-keyed compilation this \
             file fences has moved and the scan is looking in the wrong place"
        );
    }

    assert!(
        !VALIDATOR_SITES.is_empty(),
        "VALIDATOR_SITES is empty, so the population check passes over nothing"
    );
    assert!(
        !RETRIEVER_NEEDLES.is_empty() && !RESOLVER_FEATURES.is_empty(),
        "the retriever and resolver-feature needle lists must be non-empty or their scans are \
         no-ops"
    );
}
