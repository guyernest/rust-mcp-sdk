//! The single shared, fail-closed [`load`] bundle verifier (Phase 92, Plan 01 —
//! WBSV-08, threats T-92-01/02/04/22).
//!
//! Every [`crate::bundle_source::BundleSource`] (local-dir or embedded) is parsed
//! and integrity-checked HERE and ONLY here, so no source impl can skip the gate
//! (the trait returns raw bytes only — threat T-92-03). [`load`]:
//!
//! 1. enforces a FAIL-CLOSED membership allow-set — any unexpected/extra member
//!    is rejected with [`BundleLoadError::UnexpectedMember`] BEFORE parsing
//!    (frozen-bundle contract, threat T-92-22);
//! 2. recomputes the evidence-dir hash (path+length-prefixed, SORTED) via the
//!    runtime's own shared [`crate::artifact_model::fold_evidence_hash`];
//! 3. recomputes the per-artifact + combined `BUNDLE.lock` hashes via the
//!    runtime's own [`crate::artifact_model::build_bundle_lock`] (it does NOT
//!    re-implement hashing), and fails closed on any mismatch
//!    ([`BundleLoadError::IntegrityMismatch`], threat T-92-01);
//! 4. cross-checks the lock's identity/provenance triple against
//!    independently-hash-covered members ([`BundleLoadError::StampMismatch`],
//!    threat T-92-02);
//! 5. parses every member total + panic-free ([`BundleLoadError::Parse`],
//!    threat T-92-04) and builds the per-cell DAG ONCE.
//!
//! It returns a fully-verified [`WorkbookBundle`].

use std::collections::HashMap;

use thiserror::Error;

use crate::artifact_model::{build_bundle_lock, fold_evidence_hash, BundleLock, CellMap};
use crate::bundle_source::{BundleSource, BundleSourceError};
use crate::changelog::VersionChangelog;
use crate::dag::Dag;
use crate::manifest_model::Manifest;
use crate::render::LayoutDescriptor;
use crate::sheet_ir::{build_dag, Cell};

/// The bundle member holding the executable IR (a `HashMap<String, Cell>`).
pub const MEMBER_IR: &str = "executable.ir.json";
/// The bundle member holding the logical manifest.
pub const MEMBER_MANIFEST: &str = "manifest.json";
/// The bundle member holding the I/O cell map.
pub const MEMBER_CELL_MAP: &str = "cell_map.json";
/// The bundle member holding the captured layout descriptor.
pub const MEMBER_LAYOUT: &str = "layout.json";
/// The bundle member holding the integrity lock.
pub const MEMBER_LOCK: &str = "BUNDLE.lock";
/// The bundle member holding the recorded version changelog.
pub const MEMBER_CHANGELOG: &str = "evidence/changelog.json";
/// The bundle member holding the parser-equivalence evidence record.
pub const MEMBER_PARSER_EQUIV: &str = "evidence/parser_equivalence.json";

/// The FROZEN member allow-set (threat T-92-22): the bundle MUST contain exactly
/// these members — any member outside this set fails closed BEFORE parsing.
///
/// Exported so the fixture generator and future emitters share the loader's
/// canonical member-name table instead of re-declaring it.
pub const ALLOWED_MEMBERS: &[&str] = &[
    MEMBER_IR,
    MEMBER_MANIFEST,
    MEMBER_CELL_MAP,
    MEMBER_LAYOUT,
    MEMBER_LOCK,
    MEMBER_CHANGELOG,
    MEMBER_PARSER_EQUIV,
];

/// The members folded into the evidence-dir hash — the evidence members PLUS
/// `cell_map.json` + `layout.json`, matching the emitter's fold (Pitfall 2: the
/// generator and loader MUST fold the identical set). Declared in SORTED
/// relative-path order (asserted by test) so the fold iterates it directly.
pub const EVIDENCE_FOLD_MEMBERS: &[&str] = &[
    MEMBER_CELL_MAP,
    MEMBER_CHANGELOG,
    MEMBER_PARSER_EQUIV,
    MEMBER_LAYOUT,
];

/// The fully-parsed, integrity-verified bundle the served tools operate on.
///
/// Returned by [`load`] ONLY after every fail-closed gate passes, so a
/// `WorkbookBundle` value is proof the bundle was untampered at load.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct WorkbookBundle {
    /// The executable IR (cell key → [`Cell`]).
    pub ir: HashMap<String, Cell>,
    /// The per-cell dependency DAG, built ONCE at load.
    pub dag: Dag,
    /// The logical manifest projection.
    pub manifest: Manifest,
    /// The I/O cell map (inputs/outputs).
    pub cell_map: CellMap,
    /// The captured layout descriptor.
    pub layout: LayoutDescriptor,
    /// The recorded version changelog.
    pub changelog: VersionChangelog,
    /// The verified integrity lock (the served provenance stamp source).
    pub stamp: BundleLock,
}

/// Errors [`load`] surfaces — every one is fail-closed (the bundle is rejected,
/// the server never boots on a tampered/malformed bundle).
///
/// `#[non_exhaustive]` so future verification gates add variants additively.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BundleLoadError {
    /// A member's bytes could not be read from the source.
    #[error("bundle source error reading {member}: {detail}")]
    Source {
        /// The member that failed to read.
        member: String,
        /// The underlying source error detail.
        detail: String,
    },

    /// A member's JSON could not be parsed (malformed / truncated — T-92-04).
    #[error("failed to parse bundle member {what}: {detail}")]
    Parse {
        /// The member that failed to parse.
        what: String,
        /// The serde parse-error detail.
        detail: String,
    },

    /// The recomputed integrity hashes do not match the on-disk lock (a tampered
    /// or swapped artifact — threat T-92-01). Carries a FOUND-vs-EXPECTED
    /// diagnostic.
    #[error(
        "bundle integrity mismatch: expected combined {expected}, recomputed {recomputed} \
         (expected evidence {expected_evidence}, recomputed {recomputed_evidence})"
    )]
    IntegrityMismatch {
        /// The combined hash recorded in the on-disk lock.
        expected: String,
        /// The combined hash recomputed from the member bytes.
        recomputed: String,
        /// The evidence hash recorded in the on-disk lock.
        expected_evidence: String,
        /// The evidence hash recomputed from the member bytes.
        recomputed_evidence: String,
    },

    /// The lock's identity/provenance triple does not bind to an independently
    /// hash-covered member (a tampered lock — threat T-92-02).
    #[error(
        "bundle stamp mismatch on {field}: lock has {lock_value:?} but {member} has {member_value:?}"
    )]
    StampMismatch {
        /// The lock field that failed to bind (`workbook_hash`/`bundle_id`/`version`).
        field: &'static str,
        /// The value recorded in the lock.
        lock_value: String,
        /// The value found in the cross-checked member.
        member_value: String,
        /// The member the field was cross-checked against.
        member: &'static str,
    },

    /// The bundle contains a member outside the frozen allow-set (threat T-92-22).
    #[error("unexpected bundle member (not in the frozen allow-set): {member}")]
    UnexpectedMember {
        /// The unexpected member's bundle-relative path.
        member: String,
    },
}

/// Read one member's bytes, mapping a source failure to a tagged [`BundleLoadError::Source`].
fn read_member(source: &dyn BundleSource, member: &str) -> Result<Vec<u8>, BundleLoadError> {
    source.read_artifact(member).map_err(|e| match e {
        BundleSourceError::NotFound { member } => BundleLoadError::Source {
            member: member.clone(),
            detail: format!("member not found: {member}"),
        },
        BundleSourceError::Io(detail) => BundleLoadError::Source {
            member: member.to_string(),
            detail,
        },
    })
}

/// Parse one member's JSON bytes, mapping any failure to [`BundleLoadError::Parse`].
fn parse_member<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    what: &str,
) -> Result<T, BundleLoadError> {
    serde_json::from_slice(bytes).map_err(|e| BundleLoadError::Parse {
        what: what.to_string(),
        detail: e.to_string(),
    })
}

/// Recompute the evidence-dir hash the way the emitter folded it: read the
/// [`EVIDENCE_FOLD_MEMBERS`] bytes and feed them to the runtime's own shared
/// [`fold_evidence_hash`] (so it byte-reproduces the emitter by construction).
fn recompute_evidence_hash(source: &dyn BundleSource) -> Result<String, BundleLoadError> {
    let mut bodies: Vec<(&str, Vec<u8>)> = Vec::with_capacity(EVIDENCE_FOLD_MEMBERS.len());
    for member in EVIDENCE_FOLD_MEMBERS {
        bodies.push((member, read_member(source, member)?));
    }
    let members: Vec<(&str, &[u8])> = bodies.iter().map(|(p, b)| (*p, b.as_slice())).collect();
    Ok(fold_evidence_hash(&members))
}

/// Cross-check the lock's identity/provenance triple against independently
/// hash-covered members (threat T-92-02). The recompute necessarily feeds the
/// lock's own `bundle_id`/`version`/`workbook_hash` from the lock itself, so a
/// tampered lock that rewrites the triple would pass the integrity recompute —
/// this binding is what catches it.
fn verify_stamp_binding(
    lock: &BundleLock,
    manifest: &Manifest,
    layout: &LayoutDescriptor,
    changelog: &VersionChangelog,
) -> Result<(), BundleLoadError> {
    // workbook_hash ↔ layout.source_workbook_hash. An ABSENT anchor makes the
    // binding impossible — reject it explicitly (WR-07). Defaulting to "" would let
    // an absent anchor + empty lock.workbook_hash pass vacuously ("" == ""); the
    // emitter always stamps the anchor, so an absent one is a tampered/partial bundle.
    let Some(layout_hash) = layout.source_workbook_hash.as_deref() else {
        return Err(BundleLoadError::StampMismatch {
            field: "workbook_hash",
            lock_value: lock.workbook_hash.clone(),
            member_value: "<absent>".to_string(),
            member: "layout.json (source_workbook_hash)",
        });
    };
    if layout_hash != lock.workbook_hash {
        return Err(BundleLoadError::StampMismatch {
            field: "workbook_hash",
            lock_value: lock.workbook_hash.clone(),
            member_value: layout_hash.to_string(),
            member: "layout.json (source_workbook_hash)",
        });
    }
    if manifest.workflow != lock.bundle_id {
        return Err(BundleLoadError::StampMismatch {
            field: "bundle_id",
            lock_value: lock.bundle_id.clone(),
            member_value: manifest.workflow.clone(),
            member: "manifest.json (workflow)",
        });
    }
    if changelog.to_version != lock.version {
        return Err(BundleLoadError::StampMismatch {
            field: "version",
            lock_value: lock.version.clone(),
            member_value: changelog.to_version.clone(),
            member: "evidence/changelog.json (to_version)",
        });
    }
    Ok(())
}

/// Load + fail-closed integrity-verify a bundle from any [`BundleSource`].
///
/// This is the SINGLE shared verifier (WBSV-08): a local-dir bundle and an
/// embedded bundle are checked identically. Returns a fully-verified
/// [`WorkbookBundle`] or a fail-closed [`BundleLoadError`].
///
/// # Errors
///
/// Returns [`BundleLoadError::UnexpectedMember`] for an extra member,
/// [`BundleLoadError::IntegrityMismatch`] for a byte-flip/swap,
/// [`BundleLoadError::StampMismatch`] for a provenance desync,
/// [`BundleLoadError::Parse`] for malformed JSON, or
/// [`BundleLoadError::Source`] for a read failure.
pub fn load(source: &dyn BundleSource) -> Result<WorkbookBundle, BundleLoadError> {
    // 1. Fail-closed membership policy (threat T-92-22): reject ANY member
    //    outside the frozen allow-set BEFORE parsing.
    let members = source
        .list_artifacts()
        .map_err(|e| BundleLoadError::Source {
            member: "<list_artifacts>".to_string(),
            detail: match e {
                BundleSourceError::Io(d) => d,
                BundleSourceError::NotFound { member } => format!("not found: {member}"),
            },
        })?;
    for member in &members {
        if !ALLOWED_MEMBERS.contains(&member.as_str()) {
            return Err(BundleLoadError::UnexpectedMember {
                member: member.clone(),
            });
        }
    }

    // 2. Parse the lock + recompute integrity via the runtime's OWN hasher.
    let lock_bytes = read_member(source, MEMBER_LOCK)?;
    let lock: BundleLock = parse_member(&lock_bytes, MEMBER_LOCK)?;

    let ir_bytes = read_member(source, MEMBER_IR)?;
    let manifest_bytes = read_member(source, MEMBER_MANIFEST)?;

    let evidence_hash = recompute_evidence_hash(source)?;
    let ir_json = std::str::from_utf8(&ir_bytes).map_err(|e| BundleLoadError::Parse {
        what: MEMBER_IR.to_string(),
        detail: e.to_string(),
    })?;
    let manifest_json =
        std::str::from_utf8(&manifest_bytes).map_err(|e| BundleLoadError::Parse {
            what: MEMBER_MANIFEST.to_string(),
            detail: e.to_string(),
        })?;
    let recomputed = build_bundle_lock(
        &lock.bundle_id,
        &lock.version,
        lock.workbook_hash.clone(),
        ir_json,
        manifest_json,
        &evidence_hash,
    );
    if recomputed.artifacts != lock.artifacts || recomputed.combined != lock.combined {
        return Err(BundleLoadError::IntegrityMismatch {
            expected: lock.combined,
            recomputed: recomputed.combined,
            expected_evidence: lock.artifacts.evidence,
            recomputed_evidence: evidence_hash,
        });
    }

    // 3. Parse the remaining members (total + panic-free; threat T-92-04).
    let ir: HashMap<String, Cell> = parse_member(&ir_bytes, MEMBER_IR)?;
    let manifest: Manifest = parse_member(&manifest_bytes, MEMBER_MANIFEST)?;
    let cell_map: CellMap = parse_member(&read_member(source, MEMBER_CELL_MAP)?, MEMBER_CELL_MAP)?;
    let layout: LayoutDescriptor =
        parse_member(&read_member(source, MEMBER_LAYOUT)?, MEMBER_LAYOUT)?;
    let changelog: VersionChangelog =
        parse_member(&read_member(source, MEMBER_CHANGELOG)?, MEMBER_CHANGELOG)?;

    // 4. Cross-check the provenance triple (threat T-92-02).
    verify_stamp_binding(&lock, &manifest, &layout, &changelog)?;

    // 5. Build the per-cell DAG ONCE at load.
    let dag = build_dag(&ir);

    Ok(WorkbookBundle {
        ir,
        dag,
        manifest,
        cell_map,
        layout,
        changelog,
        stamp: lock,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_model::{sha256_hex, CellEntry};
    use crate::manifest_model::Manifest;
    use crate::render::LayoutDescriptor;

    /// An in-memory [`BundleSource`] backed by a member map — the loader tests
    /// build a valid golden, then tamper one member to prove fail-closed.
    struct MapSource {
        members: HashMap<String, Vec<u8>>,
    }

    impl BundleSource for MapSource {
        fn read_artifact(&self, name: &str) -> Result<Vec<u8>, BundleSourceError> {
            self.members
                .get(name)
                .cloned()
                .ok_or_else(|| BundleSourceError::NotFound {
                    member: name.to_string(),
                })
        }
        fn list_artifacts(&self) -> Result<Vec<String>, BundleSourceError> {
            let mut v: Vec<String> = self.members.keys().cloned().collect();
            v.sort();
            Ok(v)
        }
    }

    fn empty_manifest(workflow: &str) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: workflow.to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![],
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn sample_layout(hash: Option<&str>) -> LayoutDescriptor {
        LayoutDescriptor {
            descriptor_version: crate::render::LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: hash.map(String::from),
            sheets: vec![],
        }
    }

    fn sample_changelog(to_version: &str) -> VersionChangelog {
        VersionChangelog {
            from_version: "0.9.0".to_string(),
            to_version: to_version.to_string(),
            deltas: vec![],
            summary: "test".to_string(),
        }
    }

    fn sample_cell_map() -> CellMap {
        CellMap {
            inputs: vec![CellEntry {
                json_key: "rate".to_string(),
                seed_coord: "1_Inputs!E6".to_string(),
                unit: Some("ratio".to_string()),
            }],
            outputs: vec![CellEntry {
                json_key: "total".to_string(),
                seed_coord: "7_Out!C11".to_string(),
                unit: Some("GBP".to_string()),
            }],
        }
    }

    /// Build a golden bundle: serialize every member, fold the evidence hash via
    /// the shared [`fold_evidence_hash`], build the lock over the member bytes,
    /// then assemble the source map. `lock_workbook_hash` and `layout_anchor`
    /// diverge from each other only in the stamp-binding tests.
    fn golden_with(
        lock_version: &str,
        changelog_version: &str,
        lock_workbook_hash: String,
        layout_anchor: Option<&str>,
    ) -> MapSource {
        let bundle_id = "tax-calc";

        let ir: HashMap<String, Cell> = HashMap::new();
        let ir_json = serde_json::to_string(&ir).unwrap();
        let manifest = empty_manifest(bundle_id);
        let manifest_json = serde_json::to_string(&manifest).unwrap();
        let cell_map_json = serde_json::to_string(&sample_cell_map()).unwrap();
        let layout_json = serde_json::to_string(&sample_layout(layout_anchor)).unwrap();
        let changelog_json = serde_json::to_string(&sample_changelog(changelog_version)).unwrap();
        let parser_equiv_json = r#"{"equivalent":true}"#.to_string();

        let evidence_hash = fold_evidence_hash(&[
            (MEMBER_CELL_MAP, cell_map_json.as_bytes()),
            (MEMBER_LAYOUT, layout_json.as_bytes()),
            (MEMBER_CHANGELOG, changelog_json.as_bytes()),
            (MEMBER_PARSER_EQUIV, parser_equiv_json.as_bytes()),
        ]);

        let lock = build_bundle_lock(
            bundle_id,
            lock_version,
            lock_workbook_hash,
            &ir_json,
            &manifest_json,
            &evidence_hash,
        );
        let lock_json = serde_json::to_string(&lock).unwrap();

        let mut members = HashMap::new();
        members.insert(MEMBER_IR.to_string(), ir_json.into_bytes());
        members.insert(MEMBER_MANIFEST.to_string(), manifest_json.into_bytes());
        members.insert(MEMBER_CELL_MAP.to_string(), cell_map_json.into_bytes());
        members.insert(MEMBER_LAYOUT.to_string(), layout_json.into_bytes());
        members.insert(MEMBER_CHANGELOG.to_string(), changelog_json.into_bytes());
        members.insert(
            MEMBER_PARSER_EQUIV.to_string(),
            parser_equiv_json.into_bytes(),
        );
        members.insert(MEMBER_LOCK.to_string(), lock_json.into_bytes());
        MapSource { members }
    }

    /// A golden with a consistent workbook-hash stamp; `lock_version` and
    /// `changelog_version` diverge only in the stamp-desync test.
    fn golden_with_versions(lock_version: &str, changelog_version: &str) -> MapSource {
        let workbook_hash = sha256_hex(b"source-workbook-bytes");
        golden_with(
            lock_version,
            changelog_version,
            workbook_hash.clone(),
            Some(&workbook_hash),
        )
    }

    /// A fully self-consistent golden (every gate passes).
    fn valid_golden() -> MapSource {
        golden_with_versions("1.0.0", "1.0.0")
    }

    /// WR-07 fixture: a golden whose `layout.source_workbook_hash` is ABSENT
    /// (`None`) and whose `lock.workbook_hash` is the empty string. Every integrity
    /// hash is recomputed over these exact bytes so the integrity gate passes — the
    /// stamp gate (absent-anchor rejection) is what must fire, NOT a vacuous
    /// `"" == ""` pass.
    fn golden_with_absent_anchor_and_empty_lock_hash() -> MapSource {
        golden_with("1.0.0", "1.0.0", String::new(), None)
    }

    #[test]
    fn load_valid_golden_returns_populated_bundle() {
        let source = valid_golden();
        let bundle = load(&source).expect("valid golden loads");
        assert_eq!(bundle.stamp.bundle_id, "tax-calc");
        assert_eq!(bundle.stamp.version, "1.0.0");
        assert_eq!(bundle.cell_map.outputs.len(), 1);
        assert_eq!(bundle.changelog.to_version, "1.0.0");
        assert_eq!(bundle.manifest.workflow, "tax-calc");
    }

    #[test]
    fn byte_flip_returns_integrity_mismatch() {
        let mut source = valid_golden();
        // Flip one byte of the manifest member (recomputed hash diverges).
        source.members.insert(
            MEMBER_MANIFEST.to_string(),
            br#"{"tampered":true}"#.to_vec(),
        );
        match load(&source) {
            Err(BundleLoadError::IntegrityMismatch {
                expected,
                recomputed,
                ..
            }) => {
                assert_ne!(expected, recomputed, "diagnostic carries found-vs-expected");
            },
            other => panic!("expected IntegrityMismatch, got {other:?}"),
        }
    }

    #[test]
    fn version_desync_returns_stamp_mismatch() {
        // A golden whose lock says 1.0.0 but changelog.to_version=1.1.0, with
        // integrity hashes self-consistent so the stamp gate (not the integrity
        // gate) is what fires.
        let source = golden_with_versions("1.0.0", "1.1.0");

        match load(&source) {
            Err(BundleLoadError::StampMismatch { field, .. }) => {
                assert_eq!(field, "version");
            },
            other => panic!("expected StampMismatch on version, got {other:?}"),
        }
    }

    #[test]
    fn absent_layout_anchor_with_empty_lock_hash_fails_closed() {
        // WR-07: an absent layout.source_workbook_hash MUST be rejected even when
        // lock.workbook_hash is empty — the old empty-default made this pass vacuously
        // (empty == empty). The stamp gate must fire with member_value "<absent>".
        let source = golden_with_absent_anchor_and_empty_lock_hash();
        match load(&source) {
            Err(BundleLoadError::StampMismatch {
                field,
                member_value,
                ..
            }) => {
                assert_eq!(field, "workbook_hash");
                assert_eq!(
                    member_value, "<absent>",
                    "an absent anchor must be reported as <absent>, never defaulted to \"\""
                );
            },
            other => panic!("expected StampMismatch <absent> on workbook_hash, got {other:?}"),
        }
    }

    #[test]
    fn malformed_member_returns_parse_not_panic() {
        let mut source = valid_golden();
        // Corrupt the lock JSON so the FIRST parse (the lock) fails closed.
        source
            .members
            .insert(MEMBER_LOCK.to_string(), b"{ not valid json".to_vec());
        match load(&source) {
            Err(BundleLoadError::Parse { what, .. }) => {
                assert_eq!(what, MEMBER_LOCK);
            },
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn unexpected_extra_member_fails_closed() {
        let mut source = valid_golden();
        source
            .members
            .insert("evidence/sneaky.json".to_string(), b"{}".to_vec());
        match load(&source) {
            Err(BundleLoadError::UnexpectedMember { member }) => {
                assert_eq!(member, "evidence/sneaky.json");
            },
            other => panic!("expected UnexpectedMember, got {other:?}"),
        }
    }

    #[test]
    fn evidence_fold_members_const_is_sorted() {
        // EVIDENCE_FOLD_MEMBERS is declared pre-sorted so the fold iterates it
        // directly; this guard keeps the declaration honest.
        assert!(
            EVIDENCE_FOLD_MEMBERS.windows(2).all(|w| w[0] < w[1]),
            "EVIDENCE_FOLD_MEMBERS must be declared in sorted relative-path order"
        );
    }

    #[test]
    fn recompute_evidence_hash_equals_lock_evidence_for_valid_golden() {
        // Pitfall 2 guard: the loader's evidence fold byte-reproduces the
        // generator's, so the recomputed evidence hash equals lock.artifacts.evidence.
        let source = valid_golden();
        let lock: BundleLock =
            parse_member(&source.read_artifact(MEMBER_LOCK).unwrap(), MEMBER_LOCK).unwrap();
        let recomputed = recompute_evidence_hash(&source).unwrap();
        assert_eq!(
            recomputed, lock.artifacts.evidence,
            "loader and generator must fold the identical evidence member set"
        );
    }
}
