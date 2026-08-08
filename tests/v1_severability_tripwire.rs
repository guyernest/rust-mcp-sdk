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
