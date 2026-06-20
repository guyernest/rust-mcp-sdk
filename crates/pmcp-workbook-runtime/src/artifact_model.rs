//! The RUNTIME-safe bundle artifact model + hashing (Phase 11, Plan 05 / Codex
//! HIGH #2 boundary).
//!
//! These shapes describe the EMITTED bundle that the served binary deserializes
//! and integrity-checks at load:
//!
//! - [`CellEntry`]/[`CellMap`] — the manifest-driven I/O map (Codex HIGH #5).
//! - [`ArtifactHashes`]/[`BundleLock`] — the per-artifact + combined SHA-256
//!   hash-of-hashes integrity record (ART-04/D-05).
//!
//! They live HERE (umya/SWC-free) so BOTH sides share ONE definition rather than
//! the served binary re-declaring byte-for-byte serde mirrors:
//!
//! - `workbook-compiler` (the offline EMITTER) re-exports these from
//!   `artifact::{cell_map,bundle_lock}` via a re-export shim (the SAME pattern
//!   `manifest::model` uses), so the emit path keeps compiling unchanged.
//! - the served binary deserializes these types DIRECTLY and recomputes
//!   integrity via the SAME [`build_bundle_lock`] the emitter used.
//!
//! The hashing helpers ([`sha256_hex`], [`build_bundle_lock`], [`update_field`])
//! are the SINGLE source the emitter and the server-side integrity check share —
//! they MUST byte-reproduce each other or the integrity check false-positives.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::sheet_ir::value::CellValue;

/// One input/output cell entry in a [`CellMap`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellEntry {
    /// The neutral JSON key the caller uses for this cell (the LLM-facing name).
    pub json_key: String,
    /// The `CellEnv` seed coordinate — the fully-qualified `sheet!addr` cell key.
    pub seed_coord: String,
    /// The declared unit (`m2`/`GBP`/…), when known.
    pub unit: Option<String>,
}

/// One served tool — the multi-tool model lift (WBV2-03, §4.1): each output Table in
/// the source workbook becomes its OWN [`Tool`], owning its `outputs` projection and a
/// minimal, DAG-derived `input_keys` schema (the subset of the shared [`CellMap::inputs`]
/// pool transitively reachable upstream of this tool's output cells).
///
/// This type crosses the reader-free boundary (it lives HERE, beside [`CellMap`], not
/// re-declared on the served side): both the offline compiler emitter and the served
/// binary deserialize ONE definition (artifact_model.rs module doc).
///
/// Derive note: `Eq` is DROPPED — `oracle` carries [`CellValue`] (an `f64`-bearing
/// `Number`), so this type is `PartialEq` but NOT `Eq` (the [`crate::manifest_model`]
/// `GovernedDatum` precedent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Tool {
    /// The tool name — derived from the owning output Table's name (raw; MCP-charset
    /// sanitization happens in the served emit, Plan 04).
    pub name: String,
    /// The tool description — the caption cell above the output Table, when authored.
    pub description: Option<String>,
    /// The minimal input schema: the LLM-facing `json_key`s of the [`CellMap::inputs`]
    /// pool entries transitively reachable upstream of this tool's outputs (constant-only
    /// paths excluded; shared intermediates yield the union of this tool's own upstream
    /// leaves). DAG-derived via [`crate::dag::upstream_input_leaves`].
    pub input_keys: Vec<String>,
    /// One entry per output cell this tool projects (reuses [`CellEntry`] — the same
    /// `{json_key, seed_coord, unit}` shape the inputs use).
    pub outputs: Vec<CellEntry>,
    /// The per-tool reconcile oracle: `<output json_key>` → the authored expected value
    /// (the cached `<v>` cell value). Carries a typed [`CellValue`] (the `f64`-bearing
    /// `Number` that drops `Eq`).
    pub oracle: BTreeMap<String, CellValue>,
}

/// The manifest-driven I/O cell map (Codex HIGH #5): the shared inputs pool + the
/// per-Table [`Tool`]s the served binary fans out into one MCP tool each (WBV2-03 §4.1).
///
/// The single-tool `outputs: Vec<CellEntry>` FIELD was lifted to `tools: Vec<Tool>`:
/// each [`Tool`] owns its own outputs + minimal `input_keys`, so the N=1 (single output
/// Table) case is just `tools.len() == 1` — never special-cased. `inputs` stays the
/// shared pool every tool draws its `input_keys` from.
///
/// Derive note: `Eq` is DROPPED because [`Tool::oracle`] carries an `f64`-bearing
/// [`CellValue`]; the map is `PartialEq` but NOT `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellMap {
    /// One entry per `Role::Input` cell (the shared seedable per-call input pool every
    /// tool's `input_keys` draws from).
    pub inputs: Vec<CellEntry>,
    /// One [`Tool`] per output Table (WBV2-03 §4.1) — the multi-tool fan-out.
    pub tools: Vec<Tool>,
}

impl CellMap {
    /// TRANSITIONAL (Plan 03→04): the flattened union of every tool's `outputs`, so the
    /// still-old single-tool served call sites (`schema.rs`/`handler.rs`/`lib.rs`) that
    /// read "all the outputs" keep compiling against the multi-tool model until Plan 04
    /// reshapes them to per-tool iteration. For the N=1 case this equals the single
    /// tool's `outputs`. Removed by Plan 04 Task 1 (the served fan-out lands there).
    #[deprecated(
        note = "transitional shim — Plan 04 fan-out replaces .outputs() with per-tool \
                iteration; removed in Plan 04 Task 1"
    )]
    #[must_use]
    pub fn outputs(&self) -> Vec<CellEntry> {
        self.tools
            .iter()
            .flat_map(|t| t.outputs.iter().cloned())
            .collect()
    }
}

/// The three per-artifact content hashes recorded in a [`BundleLock`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArtifactHashes {
    /// SHA-256 over `executable.ir.json` bytes (64-char hex).
    pub executable: String,
    /// SHA-256 over `manifest.json` bytes (64-char hex).
    pub manifest: String,
    /// SHA-256 over the evidence directory's path+length-prefixed content (64-char
    /// hex; computed by the evidence emitter, which also folds `cell_map.json`).
    pub evidence: String,
}

/// The `BUNDLE.lock` record (ART-04/D-05): the bundle identity, the
/// `workbook_hash` provenance anchor, the three per-artifact content hashes, and
/// the COMBINED hash-of-hashes that flips on any single-artifact change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BundleLock {
    /// The neutral bundle identifier (D-17; e.g. `"tax-calc"`).
    pub bundle_id: String,
    /// The semver version (e.g. `"1.0.0"`).
    pub version: String,
    /// The canonical source-workbook CONTENT hash (`source_workbook_hash`), the
    /// provenance anchor binding the bundle to the exact source workbook (D-05).
    pub workbook_hash: String,
    /// The per-artifact content hashes.
    pub artifacts: ArtifactHashes,
    /// The combined hash-of-hashes over the three per-artifact hashes — flips
    /// when ANY artifact changes (tampering / partial-rebuild detection, D-05).
    pub combined: String,
}

/// `hex::encode(Sha256::digest(bytes))` — the single per-artifact content hash.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Feed one length-prefixed field to the digest: the tag, then the u64-LE byte
/// length, then the bytes. Because the length is encoded out-of-band, the field
/// bytes can contain ANY byte without creating an ambiguous boundary (T-7-11).
///
/// This is the SINGLE canonicalization the evidence-dir hash uses; the server's
/// integrity recompute and the emitter MUST share it byte-for-byte.
pub fn update_field(hasher: &mut Sha256, tag: &[u8], data: &[u8]) {
    hasher.update(tag);
    hasher.update((data.len() as u64).to_le_bytes());
    hasher.update(data);
}

/// Fold the evidence-dir hash over `(relative_path, bytes)` members.
///
/// Each member is fed as two length-prefixed fields (`evidence.path`, then
/// `evidence.body`) via [`update_field`], in SORTED relative-path order — the
/// sort happens HERE, so callers cannot desync on ordering. This is the SINGLE
/// evidence fold the emitter, the fixture generator, and the server-side loader
/// recompute share, byte-for-byte (Pitfall 2).
pub fn fold_evidence_hash(members: &[(&str, &[u8])]) -> String {
    let mut sorted: Vec<&(&str, &[u8])> = members.iter().collect();
    sorted.sort_by_key(|(path, _)| *path);
    let mut hasher = Sha256::new();
    for (path, body) in sorted {
        update_field(&mut hasher, b"evidence.path", path.as_bytes());
        update_field(&mut hasher, b"evidence.body", body);
    }
    hex::encode(hasher.finalize())
}

/// Build the [`BundleLock`] over the emitted artifact bytes.
///
/// Each per-artifact hash is `hex::encode(Sha256::digest(bytes))`; the combined
/// hash is `Sha256` over the concatenation of the three 64-char hex hashes (a
/// fixed-width concatenation is unambiguous). `workbook_hash` is the
/// caller-supplied `source_workbook_hash` content projection — RECORDED, not
/// recomputed from raw bytes (D-05). A one-byte change to any artifact flips its
/// per-artifact hash, which flips the combined hash (D-05 tamper detection).
pub fn build_bundle_lock(
    bundle_id: &str,
    version: &str,
    workbook_hash: String,
    ir_json: &str,
    manifest_json: &str,
    evidence_hash: &str,
) -> BundleLock {
    let h_exec = sha256_hex(ir_json.as_bytes());
    let h_manifest = sha256_hex(manifest_json.as_bytes());
    // The evidence hash is computed over the evidence DIR (path+length-prefixed,
    // folding cell_map.json) by the emitter; the lock records it verbatim.
    let h_evidence = evidence_hash.to_string();

    let combined = sha256_hex(format!("{h_exec}{h_manifest}{h_evidence}").as_bytes());

    BundleLock {
        bundle_id: bundle_id.to_string(),
        version: version.to_string(),
        workbook_hash,
        artifacts: ArtifactHashes {
            executable: h_exec,
            manifest: h_manifest,
            evidence: h_evidence,
        },
        combined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workbook_hash() -> String {
        sha256_hex(b"S!A1|10|\nS!B1|0.37|")
    }

    fn entry(json_key: &str, seed: &str, unit: Option<&str>) -> CellEntry {
        CellEntry {
            json_key: json_key.to_string(),
            seed_coord: seed.to_string(),
            unit: unit.map(str::to_string),
        }
    }

    #[test]
    fn artifact_model_tool_round_trips_through_serde() {
        let mut oracle = BTreeMap::new();
        oracle.insert("tax_owed".to_string(), CellValue::Number(18241.0));
        let tool = Tool {
            name: "Calculate_Tax".to_string(),
            description: Some("Compute the tax owed".to_string()),
            input_keys: vec!["income".to_string(), "filing".to_string()],
            outputs: vec![entry("tax_owed", "Calc!B3", Some("USD"))],
            oracle,
        };
        let json = serde_json::to_string(&tool).expect("serialize Tool");
        let back: Tool = serde_json::from_str(&json).expect("deserialize Tool");
        assert_eq!(
            tool, back,
            "Tool must serde round-trip preserving all fields"
        );
        assert_eq!(back.name, "Calculate_Tax");
        assert_eq!(back.description.as_deref(), Some("Compute the tax owed"));
        assert_eq!(back.input_keys, vec!["income", "filing"]);
        assert_eq!(back.outputs.len(), 1);
        assert_eq!(
            back.oracle.get("tax_owed"),
            Some(&CellValue::Number(18241.0))
        );
    }

    #[test]
    fn artifact_model_cell_map_with_tools_round_trips() {
        let map = CellMap {
            inputs: vec![entry("income", "In!B4", Some("USD"))],
            tools: vec![Tool {
                name: "Calculate_Tax".to_string(),
                description: None,
                input_keys: vec!["income".to_string()],
                outputs: vec![entry("tax_owed", "Calc!B3", Some("USD"))],
                oracle: BTreeMap::new(),
            }],
        };
        let json = serde_json::to_string(&map).expect("serialize CellMap");
        let back: CellMap = serde_json::from_str(&json).expect("deserialize CellMap");
        assert_eq!(back.inputs.len(), 1);
        // The N=1 (single output Table) case is just one tool — no special path.
        assert_eq!(
            back.tools.len(),
            1,
            "a one-Table manifest yields exactly one Tool"
        );
        assert_eq!(back.tools[0].name, "Calculate_Tax");
    }

    #[test]
    fn artifact_model_outputs_accessor_flattens_tools_outputs() {
        let map = CellMap {
            inputs: vec![],
            tools: vec![
                Tool {
                    name: "A".to_string(),
                    description: None,
                    input_keys: vec![],
                    outputs: vec![entry("a1", "S!A1", None), entry("a2", "S!A2", None)],
                    oracle: BTreeMap::new(),
                },
                Tool {
                    name: "B".to_string(),
                    description: None,
                    input_keys: vec![],
                    outputs: vec![entry("b1", "S!B1", None)],
                    oracle: BTreeMap::new(),
                },
            ],
        };
        // The transitional accessor returns the union across tools.
        #[allow(deprecated)]
        let flat = map.outputs();
        let keys: Vec<&str> = flat.iter().map(|e| e.json_key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["a1", "a2", "b1"],
            "outputs() flattens tools[].outputs"
        );
    }

    #[test]
    fn artifact_model_outputs_accessor_n1_equals_single_tool() {
        let map = CellMap {
            inputs: vec![],
            tools: vec![Tool {
                name: "Only".to_string(),
                description: None,
                input_keys: vec![],
                outputs: vec![entry("answer", "S!Z9", Some("USD"))],
                oracle: BTreeMap::new(),
            }],
        };
        #[allow(deprecated)]
        let flat = map.outputs();
        assert_eq!(
            flat, map.tools[0].outputs,
            "N=1: outputs() == the single tool's outputs"
        );
    }

    #[test]
    fn bundle_lock_records_three_plus_combined() {
        let lock = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVIDENCE-DIR}"),
        );
        for h in [
            &lock.artifacts.executable,
            &lock.artifacts.manifest,
            &lock.artifacts.evidence,
            &lock.combined,
        ] {
            assert_eq!(h.len(), 64, "each hash is a 64-char sha256 hex");
        }
        assert_ne!(lock.combined, lock.artifacts.executable);
        assert_ne!(lock.combined, lock.artifacts.manifest);
        assert_ne!(lock.combined, lock.artifacts.evidence);
    }

    #[test]
    fn bundle_lock_hashes_stable_across_runs() {
        let a = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let b = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(a, b, "bundle-lock hashing is stable across runs");
    }

    #[test]
    fn combined_hash_changes_when_any_artifact_changes() {
        let base = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        let tampered = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR}",
            "{MANIFEST }", // one extra byte
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(base.artifacts.manifest, tampered.artifacts.manifest);
        assert_ne!(base.combined, tampered.combined);
        let tampered_exec = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            workbook_hash(),
            "{IR }",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_ne!(base.combined, tampered_exec.combined);
    }

    #[test]
    fn workbook_hash_reuses_content_projection() {
        let wh = workbook_hash();
        let lock = build_bundle_lock(
            "tax-calc",
            "1.0.0",
            wh.clone(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(lock.workbook_hash, wh);
        assert_ne!(lock.workbook_hash, lock.artifacts.executable);
        assert_ne!(lock.workbook_hash, lock.combined);
    }

    #[test]
    fn workflow_and_version_are_parameters_not_hardcoded() {
        let lock = build_bundle_lock(
            "other-bundle",
            "2.3.4",
            workbook_hash(),
            "{IR}",
            "{MANIFEST}",
            &sha256_hex(b"{EVID}"),
        );
        assert_eq!(lock.bundle_id, "other-bundle");
        assert_eq!(lock.version, "2.3.4");
    }

    #[test]
    fn update_field_is_length_prefixed() {
        // Two fields whose concatenation would collide are distinguished by the
        // out-of-band length prefix.
        let mut a = Sha256::new();
        update_field(&mut a, b"t", b"ab");
        update_field(&mut a, b"t", b"c");
        let mut b = Sha256::new();
        update_field(&mut b, b"t", b"a");
        update_field(&mut b, b"t", b"bc");
        assert_ne!(hex::encode(a.finalize()), hex::encode(b.finalize()));
    }
}
