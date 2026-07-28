//! Provenance tripwire for `schema/vendored/ext-tasks/`.
//!
//! # What this test is for
//!
//! `schema/vendored/ext-tasks/` holds a byte-for-byte copy of the MCP tasks
//! extension draft schema, taken from `modelcontextprotocol/ext-tasks` at one
//! pinned commit. Phase 114 drives every task-related wire value off those
//! bytes, and a reviewer's conclusions about wire correctness rest entirely on
//! the copy being unmodified.
//!
//! Nothing in the build reads those files, so nothing in the build would ever
//! notice an edit. This test is what notices. It recomputes the SHA-256 of every
//! vendored file and asserts each digest is recorded in
//! `schema/vendored/ext-tasks/PROVENANCE.md`.
//!
//! # What this test deliberately does NOT do
//!
//! It does not parse, validate, or assert anything about the schema's
//! **content** — not a type name, not a field, not a status string. This test is
//! about **attribution** only: these bytes are the bytes that were fetched, and
//! this record says where they came from. Wire shapes are asserted against the
//! vendored files by the plans that implement them.
//!
//! # Idiom
//!
//! Modelled on `tests/v2_prohibited_error_codes.rs` (plan 113-29) and
//! `tests/v2_bounded_reads_tripwire.rs` (plan 113-21): runtime discovery from
//! `CARGO_MANIFEST_DIR`, no hard-coded file list, and failure messages that name
//! what a reader should do. The scanner primitives are re-stated here rather
//! than shared because a Rust integration test is its own crate and the files
//! cannot import each other; the IDIOM is deliberately identical so the
//! repository has one source-scanning shape, not two.
//!
//! Files are discovered with `read_dir` at runtime rather than listed, so a
//! newly vendored file is in scope automatically and **cannot be added
//! un-recorded**.
//!
//! # Zero new dependencies, and no subprocess
//!
//! The digest is computed IN-PROCESS with `sha2`, which is already a
//! non-optional `[dependencies]` entry of this crate (used by `shared::pkce`,
//! `types::mrtr`, `server::request_state` and the OAuth paths). An integration
//! test links the package's dependencies, so importing it adds nothing: plan
//! 114-01's threat register books `Cargo.toml` and `Cargo.lock` as
//! byte-unchanged, and they are.
//!
//! This deliberately replaced an earlier subprocess implementation that shelled
//! out to `shasum` / `sha256sum` and SKIPPED when neither was present. A skip
//! makes a tripwire always-green on the machines least likely to have the tool,
//! which is precisely the failure mode this file's own docs say must not exist.
//! Hashing in-process removes the skip path entirely: there is no environment in
//! which these assertions do not run.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The vendored artifact directory, relative to the crate root.
const VENDORED_DIR: &str = "schema/vendored/ext-tasks";

/// The attribution record. Excluded from digest computation — it is the thing
/// the digests are recorded *in*, so digesting it would be circular.
const PROVENANCE_FILE: &str = "PROVENANCE.md";

/// A floor on how many files the scan must find, so a passing run can never mean
/// "the directory was empty".
///
/// This is an anti-vacuity guard, **not** a manifest: it is deliberately a
/// minimum rather than an exact count or a file list, so vendoring a third file
/// puts that file in scope without editing this test.
const MINIMUM_VENDORED_FILES: usize = 2;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vendored_dir() -> PathBuf {
    repo_root().join(VENDORED_DIR)
}

fn provenance_path() -> PathBuf {
    vendored_dir().join(PROVENANCE_FILE)
}

/// Path relative to the crate root, for failure messages a reader can act on.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// Every vendored file, discovered at runtime, `PROVENANCE.md` excluded.
///
/// Recurses, so a file cannot be hidden from the scan by putting it in a
/// subdirectory. Returns paths sorted, so failure messages are deterministic.
fn discover_vendored_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => panic!(
            "cannot read the vendored schema directory {}: {err}\n\
             This tripwire asserts the vendored artifact is unmodified; if the directory is \
             gone, so is the artifact.",
            rel(dir)
        ),
    };
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            discover_vendored_files(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) != Some(PROVENANCE_FILE) {
            out.push(path);
        }
    }
    out.sort();
}

/// The SHA-256 of `path`'s bytes as lowercase hex.
///
/// Computed in-process, so there is no environment-dependent path and no skip:
/// either the file reads and hashes, or the test fails naming the file. The hex
/// encoding is lowercase to match what `shasum -a 256` prints, which is the form
/// `PROVENANCE.md` records.
fn sha256_of(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|err| {
        panic!(
            "cannot read {} to compute its SHA-256: {err}\n\
             This tripwire asserts the vendored artifact is unmodified; an unreadable file \
             cannot be verified.",
            rel(path)
        )
    });
    let digest = Sha256::digest(&bytes);
    digest.iter().fold(String::with_capacity(64), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

fn is_lower_hex(ch: char) -> bool {
    matches!(ch, '0'..='9' | 'a'..='f')
}

/// Every MAXIMAL run of lowercase-hex characters in `text`.
///
/// Maximal runs, not regex matches at arbitrary offsets: a 64-character SHA-256
/// digest contains a 40-character prefix, so a naive "does a 40-hex string
/// appear?" search is satisfied by a digest and proves nothing about a commit
/// SHA being recorded. Taking maximal runs and then filtering by exact length
/// makes the two kinds of hex distinguishable.
fn maximal_hex_runs(text: &str) -> Vec<String> {
    let mut runs = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if is_lower_hex(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// `PROVENANCE.md`'s text, or a failure naming the artifact as unattributed.
///
/// This is FAILURE MODE 2. It is separated from the digest checks because a
/// missing record is a different defect from a mismatched one: there is nothing
/// to compare against, and the vendored bytes have no stated origin at all.
fn read_provenance_or_fail() -> String {
    let path = provenance_path();
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "FAILURE MODE 2 — THE VENDORED ARTIFACT IS UNATTRIBUTED.\n\
             Could not read {}: {err}\n\n\
             The files in {VENDORED_DIR}/ are a copy of a third-party schema. Without \
             PROVENANCE.md nothing records which upstream repository and commit they came \
             from, whether they were edited after the fetch, or what obligation is held \
             against them.\n\n\
             WHAT TO DO: restore {} from git history (`git log -- {}`). Do NOT write a fresh \
             record from memory — re-fetch at a pinned commit and record the measured digests.",
            rel(&path),
            rel(&path),
            rel(&path),
        )
    })
}

/// The 64-hex digests recorded in `PROVENANCE.md`.
fn recorded_digests(provenance: &str) -> BTreeSet<String> {
    maximal_hex_runs(provenance)
        .into_iter()
        .filter(|run| run.len() == 64)
        .collect()
}

// ===========================================================================
// 1. Anti-vacuity: the scan finds files.
// ===========================================================================

#[test]
fn vendored_schema_dir_is_discovered_at_runtime_and_is_not_empty() {
    let dir = vendored_dir();
    assert!(
        dir.is_dir(),
        "{} does not exist. The vendored schema artifact is the offline, diff-able source for \
         every task wire value Phase 114 writes; without it every such value is an unreviewable \
         claim. Restore it from git history.",
        rel(&dir)
    );

    let mut files = Vec::new();
    discover_vendored_files(&dir, &mut files);

    assert!(
        files.len() >= MINIMUM_VENDORED_FILES,
        "found only {} file(s) under {} ({:?}), expected at least {MINIMUM_VENDORED_FILES}.\n\
         This is an anti-vacuity guard: with an empty or near-empty directory the digest checks \
         below would pass over nothing and report green while asserting nothing. If a vendored \
         file was deliberately removed, remove its digest from PROVENANCE.md in the same commit \
         and lower this floor with a written reason.",
        files.len(),
        rel(&dir),
        files.iter().map(|p| rel(p)).collect::<Vec<_>>(),
    );
}

// ===========================================================================
// 2. FAILURE MODE 2: PROVENANCE.md missing entirely.
// ===========================================================================

#[test]
fn vendored_schema_provenance_md_exists_so_the_artifact_is_attributed() {
    let provenance = read_provenance_or_fail();
    assert!(
        !provenance.trim().is_empty(),
        "FAILURE MODE 2 — THE VENDORED ARTIFACT IS UNATTRIBUTED.\n\
         {} exists but is empty. An empty record attributes nothing; it is indistinguishable \
         from no record at all, except that it is quieter.\n\n\
         WHAT TO DO: restore it from git history, or re-fetch the vendored files at a pinned \
         commit and write the record from the measured digests.",
        rel(&provenance_path())
    );
}

// ===========================================================================
// 3. FAILURE MODE 1: a vendored file whose digest is not recorded.
// ===========================================================================

#[test]
fn vendored_schema_every_file_digest_is_recorded_in_provenance_md() {
    let provenance = read_provenance_or_fail();
    let recorded = recorded_digests(&provenance);

    let mut files = Vec::new();
    discover_vendored_files(&vendored_dir(), &mut files);

    for file in &files {
        let digest = sha256_of(file);
        assert!(
            recorded.contains(&digest),
            "FAILURE MODE 1 — A VENDORED FILE WAS EDITED OR REPLACED WITHOUT UPDATING \
             PROVENANCE.\n\n\
             File:              {}\n\
             Computed SHA-256:  {digest}\n\
             Recorded digests:  {:?}\n\n\
             That file is a byte-for-byte copy of a third-party schema and is a READ-ONLY \
             reference artifact: nothing in the build reads it, and its whole value is that a \
             reviewer can trust it is what upstream published. An edit — including a reformat, \
             a line-ending conversion, or a whitespace fix — destroys that.\n\n\
             WHAT TO DO:\n\
             1. If the edit was accidental: `git checkout -- {}` and re-run.\n\
             2. If upstream genuinely changed: do NOT patch in place. Re-fetch at a NEW pinned \
             commit SHA and rewrite {} following its own § Change protocol — the pinned commit, \
             its date, the fetch date, the sizes and every digest.",
            rel(file),
            recorded,
            rel(file),
            rel(&provenance_path()),
        );
    }
}

// ===========================================================================
// 4. FAILURE MODE 3: a recorded digest whose file is gone.
// ===========================================================================

#[test]
fn vendored_schema_every_recorded_digest_belongs_to_a_file_that_still_exists() {
    let provenance = read_provenance_or_fail();
    let recorded = recorded_digests(&provenance);
    assert!(
        !recorded.is_empty(),
        "{} records no SHA-256 digest at all (no 64-character lowercase-hex run found).\n\
         A record with no digests cannot detect an edit, so the tripwire it is half of would \
         pass over nothing.\n\n\
         WHAT TO DO: recompute `shasum -a 256` for every file in {VENDORED_DIR}/ and record each \
         digest in the record's file table.",
        rel(&provenance_path())
    );

    let mut files = Vec::new();
    discover_vendored_files(&vendored_dir(), &mut files);
    let computed: BTreeSet<String> = files.iter().map(|file| sha256_of(file)).collect();

    for digest in &recorded {
        assert!(
            computed.contains(digest),
            "FAILURE MODE 3 — A STALE ENTRY: PROVENANCE RECORDS A DIGEST FOR A FILE THAT NO \
             LONGER EXISTS.\n\n\
             Recorded SHA-256:  {digest}\n\
             Files present:     {:?}\n\
             Their digests:     {:?}\n\n\
             Every digest in the record must belong to a file on disk. A digest left behind \
             after its file was deleted or renamed makes the record describe an artifact that \
             is not there — and a reader checking the record against the directory would be \
             comparing against a ghost.\n\n\
             WHAT TO DO: if the file was deliberately removed, delete its row from {} in the \
             same commit. If it was renamed, update the row's local path AND re-verify the \
             digest still matches.",
            files.iter().map(|p| rel(p)).collect::<Vec<_>>(),
            computed,
            rel(&provenance_path()),
        );
    }
}

// ===========================================================================
// 5. The record can never degrade into "fetched from main".
// ===========================================================================

#[test]
fn vendored_schema_provenance_pins_a_full_40_character_commit_sha() {
    let provenance = read_provenance_or_fail();
    let pins_a_commit_sha = maximal_hex_runs(&provenance)
        .iter()
        .any(|run| run.len() == 40);

    assert!(
        pins_a_commit_sha,
        "{} contains no full 40-character commit SHA.\n\n\
         `main` is a moving, force-pushable ref. A record that says \"fetched from main\" cannot \
         be reproduced by anyone and cannot detect upstream drift, which defeats the entire \
         purpose of vendoring. The pin must be a SHA.\n\n\
         Note this assertion counts MAXIMAL hex runs of exactly 40 characters. A 64-character \
         SHA-256 digest contains a 40-character prefix, so a looser search would be satisfied by \
         a digest and would prove nothing.\n\n\
         WHAT TO DO: resolve the SHA with `gh api repos/<owner>/<repo>/commits/main --jq .sha`, \
         re-fetch AT THAT SHA, and record the full 40 characters.",
        rel(&provenance_path())
    );
}
