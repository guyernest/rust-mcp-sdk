//! Trusted-oracle provenance: the owned provenance contracts plus the
//! quarantined `quick-xml`/`zip` part-reader ([`raw_parts`]) and the anchored
//! Excel-identity gate ([`gate`]).
//!
//! # Quarantine
//!
//! NO `quick_xml`/`zip` type — and NO `umya` type — appears in ANY public (or
//! `pub(crate)`) signature in this module, including inside the
//! [`ProvenanceError`] variants (they carry owned `String`/`usize`, never a
//! `quick_xml`/`zip` error). This mirrors the umya quarantine at the ingest
//! boundary ([`crate::ingest`]) exactly — the external parser is held entirely
//! inside `fn` bodies and converted to the owned plain types below. A
//! compile-time signature test (`#[cfg(test)] mod quarantine`) asserts this
//! invariant (T-93-02-LEAK).
//!
//! # Why a raw reader at all
//!
//! umya 3.0.0 cannot surface `calcPr` (`calcMode`/`fullCalcOnLoad`/`calcId`) nor
//! `docProps/app.xml`'s `<Application>`/`<AppVersion>` — and FABRICATES them on a
//! round-trip (the writer hard-codes `calcId=122211` + `"Microsoft Excel"`), so
//! [`raw_parts`] reads them from the ORIGINAL on-disk `.xlsx` bytes. The
//! [`gate::ProvenanceClass`] classifier then REFUSES a umya-fabricated identity
//! (WBCO-07).

pub mod raw_parts;

/// The four region hashes computed over a [`crate::ingest::WorkbookMap`]
/// partitioned by the [`crate::Manifest`] roles. Reuses the single
/// length-prefixed-sha256 canonicalization from the runtime's `update_field`.
pub mod region_hash;

pub use region_hash::compute_region_hashes;

/// The freshness / provenance gate: reads `calcPr` + app-identity from the
/// ORIGINAL `.xlsx` bytes, classifies provenance into [`gate::ProvenanceClass`]
/// (REFUSING umya-fabricated identity — WBCO-07), and either produces the
/// [`OracleCorpus`] or refuses with collect-all `oracle/*`
/// [`crate::LintFinding`]s.
pub mod gate;

pub use gate::{gate, ProvenanceClass};

use std::collections::BTreeMap;

use serde::Serialize;

/// Classify a workbook's provenance directly from its ORIGINAL on-disk `.xlsx`
/// bytes — the RAW classification the production [`gate`] performs internally,
/// exposed as a standalone read so a caller (or a CI fixture test) can assert a
/// `.xlsx`'s provenance class WITHOUT running a full compile and WITHOUT
/// consulting any trusted-fixture override sidecar.
///
/// This reads `docProps/app.xml` (`<Application>`/`<AppVersion>`) and
/// `xl/workbook.xml` (`calcPr@calcId`) via the quarantined raw reader
/// ([`raw_parts`]) and feeds them to the same anchored-identity classifier the
/// gate uses. The result is the override-free truth: a genuinely
/// `rust_xlsxwriter`/Excel-authored workbook returns
/// [`ProvenanceClass::ExcelTrusted`]; a umya-fabricated one returns
/// [`ProvenanceClass::UmyaFabricated`].
///
/// # Errors
/// Returns a [`ProvenanceError`] when the raw `.xlsx` parts are missing,
/// malformed, oversize, or otherwise unreadable (fail-closed — never defaults
/// to trusted).
pub fn classify_xlsx_bytes(xlsx_bytes: &[u8]) -> Result<ProvenanceClass, ProvenanceError> {
    let app = raw_parts::read_app_props(xlsx_bytes)?;
    let calc = raw_parts::read_calc_pr(xlsx_bytes)?;
    Ok(gate::classify(
        app.application.as_deref(),
        app.app_version.as_deref(),
        calc.calc_id,
    ))
}

/// A recoverable provenance-read error. Every variant is the typed outcome of a
/// malformed / missing / oversize OOXML part — the quarantined reader
/// ([`raw_parts`]) NEVER `.unwrap()`s a `quick_xml`/`zip` `Result` (the crate
/// `#![deny(clippy::unwrap_used, expect_used, panic)]` gate forbids it). The
/// [`gate`] converts each of these into a fail-closed `oracle/*`
/// [`crate::LintFinding`].
///
/// The carried payload is OWNED (`String`/`usize`) — no `quick_xml`/`zip` type
/// crosses into a public signature (the module quarantine, T-93-02-LEAK).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProvenanceError {
    /// The `.xlsx` bytes could not be opened as a zip archive, or a required
    /// zip entry could not be read (truncated/garbage zip → DoS guard). The
    /// underlying `zip` error is rendered to text, never carried.
    #[error("could not read the .xlsx zip archive: {detail}")]
    UnreadableZip {
        /// The underlying `zip` error rendered as text.
        detail: String,
    },
    /// A required OOXML part parsed as malformed XML (billion-laughs / XXE
    /// guard). Entity expansion is never enabled; a parse failure is this typed
    /// error, never a panic. The `quick_xml` error is rendered to text.
    #[error("could not parse OOXML part {part}: {detail}")]
    UnreadableXml {
        /// The OOXML part path that failed to parse (e.g. `xl/workbook.xml`).
        part: String,
        /// The underlying `quick_xml` error rendered as text.
        detail: String,
    },
    /// A REQUIRED OOXML part was absent from the archive (`xl/workbook.xml` or
    /// `docProps/app.xml`). The gate maps this to a fail-closed
    /// `oracle/missing-provenance` finding.
    #[error("required OOXML part is missing: {part}")]
    MissingPart {
        /// The absent part path (e.g. `docProps/app.xml`).
        part: String,
    },
    /// A part decompressed beyond its explicit per-entry size cap — the concrete
    /// zip-bomb mitigation (T-93-02-DOS). The read is abandoned at `limit + 1`
    /// bytes WITHOUT inflating the rest.
    #[error("OOXML part {part} exceeds its {limit}-byte size cap")]
    PartTooLarge {
        /// The over-size part path (e.g. `xl/workbook.xml`).
        part: String,
        /// The byte cap that was exceeded.
        limit: usize,
    },
    /// The cumulative decompressed bytes across the parts read on one archive
    /// exceeded [`raw_parts::MAX_TOTAL_DECOMPRESSED_BYTES`] (zip-bomb guard
    /// T-93-02-DOS).
    #[error("decompressed bytes exceed the {limit}-byte archive cap")]
    DecompressBomb {
        /// The cumulative-decompression byte cap that was exceeded.
        limit: usize,
    },
    /// An OOXML part nested XML elements beyond [`raw_parts::MAX_XML_DEPTH`]
    /// (pathological-nesting / billion-laughs guard T-93-02-DOS). The parse is
    /// abandoned WITHOUT recursing/allocating unbounded.
    #[error("OOXML part {part} nests XML beyond the {limit}-deep cap")]
    XmlTooDeep {
        /// The over-deep part path.
        part: String,
        /// The depth cap that was exceeded.
        limit: usize,
    },
}

/// The owned, umya/quick-xml/zip-free oracle-provenance record.
///
/// Produced by the freshness gate; serializable for the evidence bundle. Every
/// field is an objective metadata read — no judgment, no recompute. `Serialize`
/// only (a write-only output; it is never read back this phase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct OracleProvenance {
    /// `docProps/app.xml <Application>` verbatim (e.g. `"Microsoft Excel"` or
    /// `"LibreOffice/24.2.7.2…"`). Quarantined raw read; `None` when absent.
    pub authoring_app: Option<String>,
    /// `docProps/app.xml <AppVersion>` build string (the positive Excel marker —
    /// a genuine Excel save always carries one).
    pub app_version: Option<String>,
    /// `calcPr@calcMode` as read, or `"auto"` when absent (OOXML default applied
    /// upstream so the gate stays a boolean conjunction).
    pub calc_mode: String,
    /// `calcPr@fullCalcOnLoad` (absent ⇒ `false`). Hard-refuses when `true`.
    pub full_calc_on_load: bool,
    /// `calcPr@calcId` (`None` when absent). Requires `Some(non-zero)`.
    pub calc_id: Option<u32>,
    /// `docProps/core.xml dcterms:modified` (RFC3339). umya read
    /// ([`crate::ingest::WorkbookMap::save_timestamp`]).
    pub save_timestamp: Option<String>,
    /// The four region hashes (see [`RegionHashes`]).
    pub region_hashes: RegionHashes,
    /// `true` iff any formula cell lacked a `<v>` (the missing-cache signal).
    pub missing_cache: bool,
    /// `true` iff the gate classified the cache stale (the conjunction failed).
    pub stale: bool,
    /// `true` iff a full-recalc-on-save was evidenced (`calcMode == "auto"` AND
    /// `!full_calc_on_load` AND `calc_id` present-nonzero).
    pub full_recalc_on_save: bool,
    /// `calcPr@forceFullCalc`; RECORDED, never gated. The signal is preserved in
    /// the evidence bundle but the accept/refuse decision does NOT key on it.
    pub force_full_calc: bool,
    /// The [`gate::ProvenanceClass`] this record was classified as — the single
    /// authoritative provenance verdict (the refuse decision derives from it,
    /// never an ad-hoc check). `ExcelTrusted` is the ONLY accept class.
    pub class: ProvenanceClass,
}

/// The four region hashes (length-prefixed sha256 over canonicalized,
/// `cell_key`-sorted cells — reuses the runtime's `update_field`).
///
/// **Empty-partition policy:** every field is `None` when its partition is
/// EMPTY — a manifest with zero `Role::Input` rows, zero `Role::Constant` rows,
/// zero `Role::Output` rows, or a workbook with zero formula cells. An empty
/// partition recorded as the SHA-256 of empty input is a constant that never
/// flips on any cell change yet reads as "a real hash"; `None` is the truthful
/// "this evidence provides no tamper coverage for this region" record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct RegionHashes {
    /// Input-region hash (manifest role = input; value only). `None` when the
    /// manifest declares ZERO `Role::Input` cells.
    pub inputs: Option<String>,
    /// Formula-region hash (every `is_formula` cell; formula TEXT only, never
    /// the cached `<v>`). `None` when the workbook has ZERO formula cells.
    pub formulas: Option<String>,
    /// Data-region hash (manifest role = constant; value only). `None` when the
    /// manifest declares ZERO `Role::Constant` cells.
    pub data: Option<String>,
    /// Output-region hash (manifest role = output; value only). `None` when the
    /// manifest declares ZERO `Role::Output` cells.
    pub outputs: Option<String>,
}

/// The verified oracle corpus: a full cell-map snapshot keyed by the canonical
/// `cell_key` (`sheet!addr`, via [`crate::ingest::cell_key`]) → the verified
/// cached `<v>` value. Populated by the gate once the cache is proven fresh; the
/// answers the gate protects. `Serialize` only (output).
///
/// A `BTreeMap` (not `HashMap`) so the snapshot serializes deterministically in
/// `cell_key` order — the evidence bundle is reproducible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct OracleCorpus {
    /// `cell_key` (`sheet!addr`) → verified cached `<v>`. Built with the reused
    /// [`crate::ingest::cell_key`] helper; never re-inline `format!`.
    pub cells: BTreeMap<String, String>,
}

#[cfg(test)]
mod quarantine {
    //! Compile-time quarantine gate: proves the `provenance` public +
    //! `pub(crate)` surface uses ONLY owned types — no `quick_xml`/`zip`/`umya`
    //! type appears in a signature (T-93-02-LEAK).

    use super::raw_parts::{RawAppProps, RawCalcPr};
    use super::{OracleCorpus, OracleProvenance, ProvenanceError, RegionHashes};

    /// The owned outputs must be `Serialize` (the evidence-bundle contract) and
    /// carry no foreign type — asserted by requiring the bound.
    fn assert_serialize<T: serde::Serialize>() {}

    /// The `pub(crate)` reader signatures must be exactly
    /// `fn(&[u8]) -> Result<Owned, ProvenanceError>` — binding them to a function
    /// pointer typed with OWNED types only would fail to compile if a
    /// `quick_xml`/`zip` type had leaked into the signature.
    #[test]
    fn reader_signatures_are_owned_only() {
        let _calc: fn(&[u8]) -> Result<RawCalcPr, ProvenanceError> = super::raw_parts::read_calc_pr;
        let _app: fn(&[u8]) -> Result<RawAppProps, ProvenanceError> =
            super::raw_parts::read_app_props;
    }

    #[test]
    fn provenance_quarantine_owned_outputs_are_serializable() {
        assert_serialize::<OracleProvenance>();
        assert_serialize::<RegionHashes>();
        assert_serialize::<OracleCorpus>();
        let _calc = RawCalcPr {
            calc_mode: None,
            full_calc_on_load: None,
            calc_id: None,
            force_full_calc: None,
        };
        let _app = RawAppProps {
            application: None,
            app_version: None,
        };
    }
}
