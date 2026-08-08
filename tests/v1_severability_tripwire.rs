//! SMPL-01 severability drift gate: `full` vs `full-v2`.
//!
//! # What this file protects
//!
//! Phase 117 makes "v1 is severable" a *compile-time fact* rather than a
//! convention. The mechanism is a default-on, dependency-free `v1-compat`
//! marker feature plus a parallel `full-v2` list that is `full` minus exactly
//! `v1-compat`. The severance proof is then a real build:
//!
//! ```text
//! RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
//! ```
//!
//! `--no-default-features` ALONE would prove nothing, because
//! `default = ["logging"]`: it would strip `http`/`streamable-http` too and
//! "prove" severability by never compiling the transport at all. Hence the
//! parallel positive list.
//!
//! # The hazard this file exists for
//!
//! `full` and `full-v2` are two ENUMERATED lists, and enumerated lists drift.
//! A feature added to `full` and forgotten in `full-v2` silently SHRINKS the
//! severance proof: the build still passes, but it now proves severability of
//! a smaller crate than the one that ships. Nothing about that failure is
//! visible — no error, no warning, just a weaker guarantee.
//!
//! # Why the scope is DERIVED, not enumerated
//!
//! Every list in this file is parsed out of `Cargo.toml` at test time. Phase
//! 116-14 proved the opposite approach wrong: an enumerated tripwire scope hid
//! two real defects, because the enumeration itself was the thing that went
//! stale. A tripwire whose scope can rot is a tripwire that reports green while
//! covering nothing.
//!
//! For the same reason the manifest is PARSED (`toml::from_str`) and never
//! string-matched line by line — see the "manifests are NEVER read as text"
//! rule recorded in `tests/v2_schema_tripwires.rs`. `[features]` values are
//! literal arrays with no rename or inheritance mechanism, so a parse is exact.
//!
//! `toml` is already a plain runtime dependency of `pmcp`, so this costs zero
//! new dependencies.

use std::collections::BTreeSet;

/// The manifest every check in this file derives its scope from.
///
/// Resolved through `CARGO_MANIFEST_DIR` so the test is independent of the
/// working directory `cargo test` happens to be invoked from.
const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// The marker feature whose presence/absence IS the severability boundary.
const V1_COMPAT: &str = "v1-compat";

/// Floor on the parsed `full` entry count.
///
/// This exists so a broken reader cannot make the difference assertion below
/// pass over an empty set: `{} - {}` is `{}`, which would compare unequal to
/// `["v1-compat"]` — but a reader that returned a *partial* list could still
/// produce a difference that looks right for the wrong reason. `full` holds 16
/// entries today (15 pre-Phase-117 plus `v1-compat`); the floor sits at 15 so
/// legitimate additions do not need to touch it.
///
/// If this fires, the remedy is to FIX THE READER. Never lower the floor.
const MIN_FULL_ENTRIES: usize = 15;

/// Floor on the parsed `full-v2` entry count, for the same reason as
/// [`MIN_FULL_ENTRIES`]. `full-v2` holds 15 entries today; the floor sits at 14.
///
/// If this fires, the remedy is to FIX THE READER. Never lower the floor.
const MIN_FULL_V2_ENTRIES: usize = 14;

/// Parse the real `Cargo.toml`.
fn manifest() -> toml::Value {
    let text =
        std::fs::read_to_string(MANIFEST).unwrap_or_else(|e| panic!("cannot read {MANIFEST}: {e}"));
    toml::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {MANIFEST} as TOML: {e}"))
}

/// Read one `[features]` entry as a set.
///
/// Panics — naming the feature — when the key is absent or is not an array of
/// strings. A missing `full-v2` must be a loud failure, not an empty set that
/// every downstream assertion then passes over vacuously.
fn feature_list(manifest: &toml::Value, name: &str) -> BTreeSet<String> {
    let features = manifest.get("features").unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {MANIFEST} has no `[features]` table, so feature `{name}` cannot be \
             read and every check in this file would pass over an empty set.\n\
             WHAT TO DO: fix the reader or restore the table; do not weaken the assertions."
        )
    });
    let entry = features.get(name).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: feature `{name}` is MISSING from `[features]` in {MANIFEST}.\n\
             `full`, `full-v2` and `default` are all load-bearing for the SMPL-01 severance \
             proof: `full-v2` IS the proof set, and `{V1_COMPAT}` in `default` is what keeps \
             every existing consumer working.\n\
             WHAT TO DO: restore `{name}` in Cargo.toml `[features]`; do not delete this check."
        )
    });
    let array = entry.as_array().unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: feature `{name}` in {MANIFEST} `[features]` is not an array \
             (found {entry:?}), so its entries cannot be compared.\n\
             WHAT TO DO: fix the reader, not the assertion."
        )
    });
    array
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| {
                    panic!(
                        "FAILURE MODE: feature `{name}` in {MANIFEST} `[features]` holds a \
                         non-string entry {value:?}.\n\
                         WHAT TO DO: fix the reader, not the assertion."
                    )
                })
                .to_string()
        })
        .collect()
}

/// Assert a derived feature list is large enough to be believable.
///
/// Separated out so the guard reads identically at every call site and so the
/// message always blames the READER, which is the actual cause, rather than the
/// invariant being checked.
fn assert_not_vacuous(name: &str, list: &BTreeSet<String>, floor: usize) {
    assert!(
        list.len() >= floor,
        "FAILURE MODE: derived `{name}` has only {} entr(y|ies), at or below the {floor} floor. \
         A reader that silently returns a partial or empty list makes every other check in this \
         file pass over nothing.\n\
         WHAT TO DO: fix the reader, not the assertion. Never lower the floor.",
        list.len()
    );
}

/// `full` minus `full-v2` must be EXACTLY `{v1-compat}`, in both directions.
#[test]
fn full_and_full_v2_differ_by_exactly_v1_compat() {
    let manifest = manifest();
    let full = feature_list(&manifest, "full");
    let full_v2 = feature_list(&manifest, "full-v2");

    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert_not_vacuous("full-v2", &full_v2, MIN_FULL_V2_ENTRIES);

    let only_in_full: Vec<String> = full.difference(&full_v2).cloned().collect();
    let only_in_v2: Vec<String> = full_v2.difference(&full).cloned().collect();

    assert_eq!(
        only_in_full,
        vec![V1_COMPAT.to_string()],
        "`full` minus `full-v2` must be EXACTLY [{V1_COMPAT}], but it is {only_in_full:?}.\n\
         CONSEQUENCE: a feature added to `full` and forgotten in `full-v2` silently shrinks the \
         severance proof — `cargo build -p pmcp --no-default-features --features full-v2` keeps \
         passing, but it now proves severability of a SMALLER crate than the one that ships.\n\
         WHAT TO DO: mirror the new feature into `full-v2` in Cargo.toml (everything `full` has \
         except `{V1_COMPAT}`)."
    );
    assert!(
        only_in_v2.is_empty(),
        "`full-v2` has entries `full` does not: {only_in_v2:?}.\n\
         CONSEQUENCE: `full-v2` must be a strict SUBSET of `full`, or the severance build is \
         compiling a configuration no consumer can actually get.\n\
         WHAT TO DO: remove the stray entries from `full-v2`, or add them to `full` too."
    );
}

/// `v1-compat` must stay default-on, and must stay inside `full`.
#[test]
fn v1_compat_is_in_default_and_full() {
    let manifest = manifest();
    let default = feature_list(&manifest, "default");
    let full = feature_list(&manifest, "full");

    assert!(
        default.contains(V1_COMPAT),
        "`{V1_COMPAT}` is missing from `default` (found {default:?}).\n\
         CONSEQUENCE: dropping `{V1_COMPAT}` from `default` silently breaks every existing user \
         — the MCP 2025-11-25 session/resumability layer would vanish from an ordinary \
         `pmcp = \"2\"` dependency with no error and no warning.\n\
         WHAT TO DO: restore `{V1_COMPAT}` in `default`. Removing it is SMPL-F1 / pmcp 3.0, \
         gated on public client adoption of v2 — see docs/v1-sunset-policy.md."
    );
    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert!(
        full.contains(V1_COMPAT),
        "`{V1_COMPAT}` is missing from `full` (found {full:?}).\n\
         CONSEQUENCE: `full` and `full-v2` would become identical, so the severance build would \
         prove nothing at all.\n\
         WHAT TO DO: restore `{V1_COMPAT}` in `full`."
    );
}

/// The reader itself is not vacuous — checked independently of what it is read for.
#[test]
fn the_feature_list_reader_is_not_vacuous() {
    let manifest = manifest();

    let full = feature_list(&manifest, "full");
    let full_v2 = feature_list(&manifest, "full-v2");
    let default = feature_list(&manifest, "default");

    assert_not_vacuous("full", &full, MIN_FULL_ENTRIES);
    assert_not_vacuous("full-v2", &full_v2, MIN_FULL_V2_ENTRIES);
    assert!(
        !default.is_empty(),
        "FAILURE MODE: derived `default` is empty, so the `{V1_COMPAT}`-is-default-on check \
         would pass over nothing.\n\
         WHAT TO DO: fix the reader, not the assertion."
    );

    // `full-v2` must contain the transport, or the severance build compiles no
    // transport at all and is a false green (RESEARCH Q3.5 pitfall 1).
    assert!(
        full_v2.contains("streamable-http"),
        "FAILURE MODE: `full-v2` does not contain `streamable-http`, which is where the v1 \
         session and SSE-resumability machinery lives.\n\
         CONSEQUENCE: the severance build would compile no transport and pass vacuously — it \
         would 'prove' v1 is severable by never compiling the code being severed.\n\
         WHAT TO DO: restore `streamable-http` in `full-v2`."
    );
}

// ===========================================================================
// SMPL-02: the v1 null twin, asserted at the SOURCE level.
//
// Everything above this line protects the FEATURE LISTS. Everything below
// protects the CLAIM those lists are made for: that a `full-v2` build contains
// no MCP 2025-11-25 session lifecycle and no SSE resumability.
//
// A build cannot check that claim. `cargo build --no-default-features
// --features full-v2` proves only that the null twin COMPILES; a twin that
// quietly re-implemented a session map would compile just as well. So the claim
// is checked where it lives: in the source of `v1_session_off.rs`.
// ===========================================================================

/// The `v1-compat` half of the paired module: the real v1 state.
const V1_REAL: &str = "src/server/streamable_http_server/v1_session.rs";

/// The `full-v2` half: the null twin whose emptiness IS the SMPL-02 claim.
const V1_OFF: &str = "src/server/streamable_http_server/v1_session_off.rs";

/// The file that declares the pair with two `cfg_attr` path attributes.
const TRANSPORT: &str = "src/server/streamable_http_server.rs";

// ---------------------------------------------------------------------------
// DO NOT ADD A SUBSTRING BLACKLIST HERE.
//
// An earlier draft of this gate forbade the bare substrings `sessions`,
// `event_store`, `EventStore` and `sse_streams` in the null twin. FOUR of them
// are mechanically unsatisfiable, because plans 117-09 / 117-12 / 117-13 require
// the twin to carry signatures that are TEXTUALLY IDENTICAL to their real
// counterparts. Measured against `src/server/streamable_http_server.rs`:
//
// * `sessions` is a substring of `sessions_active_for`, `sessions_active` and
//   the `sessions_on: bool` parameter of `apply_session_header`;
// * `event_store` is a substring of the parameter names `cfg_has_event_store`
//   (`resumability_active_for`) and `event_store`
//   (`replay_sse_events_from_header`, `sse_event_for_message`);
// * `EventStore` is a substring of `EventStoreHandle`, which appears in the
//   return type `Option<&EventStoreHandle>` and in two parameter types;
// * `sse_streams` is reached by the surviving `build_response` routing seam,
//   which must work through the pair on BOTH feature sets.
//
// A tripwire that rejects those identifiers would make Wave 3 unlandable behind
// a Wave 2 gate. The invariant SMPL-02 actually needs is not "these words are
// absent" — it is "no state is held and no state or header is touched". That is
// what the checks below assert, from the DERIVED declaration sets of the two
// halves rather than from a hand-written list that rots.
// ---------------------------------------------------------------------------

/// Type tokens that would mean the null twin is HOLDING v1 state.
///
/// Each entry names a container or a concrete v1 type. Their presence in the
/// twin means the `full-v2` build allocates session/SSE state after all, which
/// is the exact claim this file exists to make false.
///
/// `EventStoreHandle` is deliberately ABSENT from this list: carrying one in a
/// mirrored SIGNATURE is required by 117-09/117-12. What is forbidden is HOLDING
/// one, which `Arc<dyn EventStore` catches.
///
/// WHAT TO DO when one of these fires: MOVE the item into `v1_session.rs`, where
/// v1 state belongs. Never shorten this list to make a failure go away.
const FORBIDDEN_STATE_TYPES: &[&str] = &[
    "HashMap",
    "BTreeMap",
    "RwLock",
    "Mutex",
    "DashMap",
    "SessionInfo",
    "InMemoryEventStore",
    "Arc<dyn EventStore",
];

/// Operation tokens that would mean the null twin is TOUCHING state or headers.
///
/// A constant-answer twin needs none of them: it neither reads nor writes a
/// lock, neither inserts into nor removes from a map, spawns nothing, and never
/// looks at a request or response header. A twin that needs one of these is a
/// FINDING — the v2 answer stopped being a constant.
///
/// WHAT TO DO when one of these fires: MOVE the operation into `v1_session.rs`.
/// Never shorten this list to make a failure go away.
const FORBIDDEN_OPERATIONS: &[&str] = &[
    ".read()",
    ".write()",
    ".lock()",
    ".insert(",
    ".remove(",
    ".entry(",
    ".get_mut(",
    ".contains_key(",
    "tokio::spawn",
    "LAST_EVENT_ID",
    "MCP_SESSION_ID",
    ".headers()",
    "headers.get",
];

/// Floor on the twin's stripped byte count.
///
/// Every check below is an ABSENCE check, and absence checks pass trivially over
/// an empty string. A file that failed to read, was truncated, or was emptied by
/// a bad merge would therefore report green while proving nothing. The twin is
/// comfortably above this floor today.
///
/// WHAT TO DO when this fires: restore the file. Never lower the floor.
const MIN_STRIPPED_BYTES: usize = 200;

/// The crate root, so no check in this file names an absolute path.
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &std::path::Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Read a repo-relative source file, naming it if the read fails.
fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {}: {e}.\n\
             Every check over this file would then pass over an empty string.\n\
             WHAT TO DO: restore the file, or fix the path constant — do not delete the check.",
            rel(&path)
        )
    })
}

/// Advance past a `//`-style comment, keeping the terminating newline.
fn skip_line_comment(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Advance past a `/* … */` comment, honouring Rust's nesting and preserving
/// the newlines inside it so line-oriented scans downstream keep their shape.
fn skip_block_comment(chars: &[char], mut i: usize, out: &mut String) -> usize {
    let mut depth = 1usize;
    i += 2;
    while i < chars.len() && depth > 0 {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            i += 2;
        } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            i += 2;
        } else {
            if chars[i] == '\n' {
                out.push('\n');
            }
            i += 1;
        }
    }
    i
}

/// Copy a `"…"` string literal verbatim, honouring backslash escapes.
///
/// String CONTENTS are kept rather than blanked: nothing in the twin is allowed
/// to hide a forbidden token in a string either, and keeping them means the
/// stripper has one less way to be wrong.
fn copy_string_literal(chars: &[char], mut i: usize, out: &mut String) -> usize {
    out.push(chars[i]);
    i += 1;
    while i < chars.len() {
        let c = chars[i];
        out.push(c);
        i += 1;
        if c == '\\' {
            if let Some(next) = chars.get(i) {
                out.push(*next);
                i += 1;
            }
            continue;
        }
        if c == '"' {
            break;
        }
    }
    i
}

/// Rust source with `//`, `///`, `//!` and `/* … */` comments removed.
///
/// The scan below MUST run on stripped source. A doc comment that mentions a
/// session map in PROSE — and the twin's module doc says a great deal about what
/// it does not do — is documentation, not an implementation. Matching raw source
/// would turn every such sentence into a false failure and push the next author
/// toward deleting the explanation instead of the code.
///
/// The inverse hazard is worse and is unit-tested by
/// `the_stripper_does_not_over_strip`: a stripper that ate real code would make
/// every check in this section pass over nothing at all.
fn strip_comments(rust: &str) -> String {
    let chars: Vec<char> = rust.chars().collect();
    let mut out = String::with_capacity(rust.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            i = skip_line_comment(&chars, i);
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i = skip_block_comment(&chars, i, &mut out);
        } else if c == '"' {
            i = copy_string_literal(&chars, i, &mut out);
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// The `fn` / `struct` / `type` / `const` NAME a stripped line declares.
///
/// Line-oriented on purpose: both halves of the pair are small, hand-written
/// files whose declarations start their own line. Visibility and the modifiers
/// that may precede the keyword are stripped first, and `const fn` is resolved
/// as a function rather than as a constant.
fn declaration_name(line: &str) -> Option<String> {
    let mut rest = line.trim();
    for vis in ["pub(crate) ", "pub(super) ", "pub(self) ", "pub "] {
        if let Some(stripped) = rest.strip_prefix(vis) {
            rest = stripped.trim_start();
            break;
        }
    }
    while let Some(stripped) = ["async ", "unsafe ", "extern "]
        .iter()
        .find_map(|m| rest.strip_prefix(m))
    {
        rest = stripped.trim_start();
    }
    let tail = ["const fn ", "fn ", "struct ", "type ", "const "]
        .iter()
        .find_map(|kw| rest.strip_prefix(kw))?;
    let name: String = tail
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Every declaration name in a stripped source file.
fn declaration_names(stripped: &str) -> BTreeSet<String> {
    stripped.lines().filter_map(declaration_name).collect()
}

/// The null twin holds NO state: a unit `V1State`, and no state-bearing type.
#[test]
fn the_v1_null_twin_holds_no_state() {
    let stripped = strip_comments(&source(V1_OFF));

    assert!(
        stripped.contains("struct V1State;"),
        "FAILURE MODE: {V1_OFF} does not declare `V1State` as a UNIT struct.\n\
         CONSEQUENCE: the zero-sized twin is what makes 'no session map is allocated on a \
         `full-v2` build' a property of the TYPE rather than of a runtime branch someone can \
         forget to take.\n\
         WHAT TO DO: keep the declaration a unit struct and move any field into {V1_REAL}."
    );
    assert!(
        !stripped.contains("struct V1State {"),
        "FAILURE MODE: {V1_OFF} gave `V1State` a field block, so the null twin now carries data.\n\
         CONSEQUENCE: severance stops being structural — the `full-v2` build allocates v1 state \
         again, and nothing else in the test suite would notice.\n\
         WHAT TO DO: move the field into {V1_REAL}; the twin answers with a constant, not a value."
    );

    for token in FORBIDDEN_STATE_TYPES {
        assert!(
            !stripped.contains(token),
            "FAILURE MODE: state-bearing type `{token}` appears in {V1_OFF}.\n\
             CONSEQUENCE: the null twin is holding v1 state, which is precisely what a `full-v2` \
             build is supposed to prove it does not do.\n\
             WHAT TO DO: MOVE it into {V1_REAL}. Do not remove `{token}` from \
             FORBIDDEN_STATE_TYPES to silence this."
        );
    }
}

/// The null twin PERFORMS no state or header operation.
#[test]
fn the_v1_null_twin_performs_no_state_or_header_operation() {
    let stripped = strip_comments(&source(V1_OFF));

    for token in FORBIDDEN_OPERATIONS {
        assert!(
            !stripped.contains(token),
            "FAILURE MODE: operation `{token}` appears in {V1_OFF}.\n\
             CONSEQUENCE: the v2 answer stopped being a constant — the twin now reads or mutates \
             state, or looks at a session/resumability header, on a build whose whole claim is \
             that neither exists.\n\
             WHAT TO DO: MOVE the operation into {V1_REAL} and leave a constant here. Do not \
             remove `{token}` from FORBIDDEN_OPERATIONS to silence this."
        );
    }
}

/// The twin declares NOTHING the real module does not.
///
/// This is the derived replacement for an enumerated blacklist: it catches
/// invented machinery without needing a list that goes stale, and it cannot
/// reject an identifier the real module legitimately carries.
#[test]
fn the_v1_null_twin_declares_nothing_the_real_module_does_not() {
    let twin = declaration_names(&strip_comments(&source(V1_OFF)));
    let real = declaration_names(&strip_comments(&source(V1_REAL)));

    assert!(
        !twin.is_empty(),
        "FAILURE MODE: no declaration was extracted from {V1_OFF}, so this check would pass over \
         an empty set.\n\
         WHAT TO DO: fix `declaration_name`, not the assertion."
    );
    assert!(
        !real.is_empty(),
        "FAILURE MODE: no declaration was extracted from {V1_REAL}, so every twin declaration \
         would look like an addition.\n\
         WHAT TO DO: fix `declaration_name`, not the assertion."
    );

    let extra: Vec<&String> = twin.difference(&real).collect();
    for name in &extra {
        eprintln!("extra declaration in {V1_OFF}, absent from {V1_REAL}: {name}");
    }
    assert!(
        extra.is_empty(),
        "FAILURE MODE: {V1_OFF} declares {extra:?}, which {V1_REAL} does not.\n\
         CONSEQUENCE: severance grew machinery of its own. The twin's only job is to answer the \
         questions v1 asks with a constant; anything it declares alone is code that exists ONLY \
         on the build that is supposed to contain less.\n\
         WHAT TO DO: declare the item in {V1_REAL} too (signature identity is what lets the \
         transport name `v1::…` unconditionally), or delete it from the twin."
    );
}

/// The absence checks above cannot pass over an empty or truncated file.
#[test]
fn the_null_twin_check_is_not_vacuous() {
    let stripped = strip_comments(&source(V1_OFF));

    assert!(
        stripped.len() >= MIN_STRIPPED_BYTES,
        "FAILURE MODE: {V1_OFF} strips to {} byte(s), below the {MIN_STRIPPED_BYTES} floor.\n\
         CONSEQUENCE: every check in this section is an ABSENCE check, and absence checks pass \
         trivially over an empty string — a truncated or unreadable file would report green while \
         proving nothing.\n\
         WHAT TO DO: restore the file. Never lower the floor.",
        stripped.len()
    );
    assert!(
        stripped.contains("V1State"),
        "FAILURE MODE: {V1_OFF} strips to something that does not mention `V1State`.\n\
         CONSEQUENCE: the reader is looking at the wrong content, so the absence checks above are \
         vacuous.\n\
         WHAT TO DO: fix the reader or restore the file."
    );
}

/// The stripper removes comments and ONLY comments.
///
/// Both directions matter. Under-stripping turns prose into false failures;
/// over-stripping makes every check in this section pass over nothing.
#[test]
fn the_stripper_does_not_over_strip() {
    let fixture = "let sessions = 1; // sessions in a line comment\n\
                   //! sessions in a module doc\n\
                   /// sessions in an item doc\n\
                   /* sessions in a block comment */\n\
                   let kept = 2;\n";
    let stripped = strip_comments(fixture);

    assert!(
        stripped.contains("let sessions = 1;"),
        "FAILURE MODE: the stripper ate real code. Observed: {stripped:?}\n\
         CONSEQUENCE: every absence check in this section would pass over a blank file.\n\
         WHAT TO DO: fix `strip_comments`, not the assertion."
    );
    assert!(
        stripped.contains("let kept = 2;"),
        "FAILURE MODE: the stripper ate code that follows a block comment. Observed: \
         {stripped:?}\n\
         WHAT TO DO: fix `strip_comments`, not the assertion."
    );
    for prose in [
        "in a line comment",
        "in a module doc",
        "in an item doc",
        "in a block comment",
    ] {
        assert!(
            !stripped.contains(prose),
            "FAILURE MODE: the stripper left `{prose}` behind. Observed: {stripped:?}\n\
             CONSEQUENCE: a doc comment that DESCRIBES what the twin does not do would be \
             matched as if it were an implementation, and the honest remedy — deleting the \
             explanation — makes the file worse.\n\
             WHAT TO DO: fix `strip_comments`, not the assertion."
        );
    }
}

/// Both halves exist, and the transport still selects between them.
///
/// Deleting one half breaks the build on ONE feature set only, which a
/// single-configuration CI job can miss entirely. This turns that into a test
/// failure on every configuration.
#[test]
fn both_paired_module_files_exist() {
    for half in [V1_REAL, V1_OFF] {
        let path = repo_root().join(half);
        assert!(
            path.is_file(),
            "FAILURE MODE: {half} is missing.\n\
             CONSEQUENCE: the paired module has one half, so `cargo build` fails on exactly one \
             feature set — the configuration a single-config CI job does not run.\n\
             WHAT TO DO: restore the file. Deleting BOTH halves is SMPL-F1 / pmcp 3.0 and is a \
             semver-major change; see docs/v1-sunset-policy.md."
        );
    }

    let transport = strip_comments(&source(TRANSPORT));
    for attribute in [
        "cfg_attr(feature = \"v1-compat\", path",
        "cfg_attr(not(feature = \"v1-compat\"), path",
    ] {
        let hits = transport.matches(attribute).count();
        assert_eq!(
            hits, 1,
            "FAILURE MODE: {TRANSPORT} contains {hits} occurrence(s) of `{attribute}`, expected \
             exactly 1.\n\
             CONSEQUENCE: the pair is selected by exactly two attributes on one `mod v1;`. Zero \
             means the seam is gone; more than one means two declarations can disagree.\n\
             WHAT TO DO: restore the single declaration. Note the `#[rustfmt::skip]` above it is \
             load-bearing — rustfmt explodes the `not(...)` form across four lines and this match \
             is single-line."
        );
    }
}
