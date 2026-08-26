//! The `.tar` <-> OCI-image-layout codec behind `package save` / `package load`
//! — the ONLY module in this repository that names `tar::`.
//!
//! D-11 gives a package TWO on-disk representations: the OCI image layout
//! DIRECTORY, which is the identity-bearing working form every shipped verb
//! already operates on, and an uncompressed `.tar` of that directory, which is a
//! pure carriage envelope. The tar contributes NOTHING to package identity —
//! identity is the manifest digest `pack_server` computes over the layout's
//! blobs — and `load` discards the tar the moment its contents are verified.
//!
//! # Why the whole-archive extraction path is never used
//!
//! `tar::Archive::unpack` (and any `dest.join(entry.path()?)` written by hand)
//! takes a destination filename FROM THE ARCHIVE. Every defence against that is
//! a filter, and filters are a list of the traversals someone thought of.
//!
//! This module does not filter, because it never has a path to filter. Entries
//! are parsed into memory, gated, and then written through
//! [`OciLayout::write_blob`], which derives its destination from
//! `ManifestDigest::from_bytes(bytes)` — a digest THIS code computed over bytes
//! it is holding (`crates/pmcp-package/src/oci/layout.rs:96-101`). An archive
//! path is a lookup key during validation and is never joined onto the
//! filesystem, so traversal is unrepresentable rather than filtered.
//!
//! That is also why `cap-std` is deliberately absent. The TOCTOU class it
//! defends — a path resolving to a different object between check and open — is
//! unreachable when no archive-supplied path is ever opened. The
//! resource-exhaustion half of the untrusted-bytes problem is answered
//! separately, by the byte caps this module installs on the read path.
//!
//! # This module is a dependency-light LEAF, on purpose
//!
//! It names `tar`, `anyhow`, `serde_json`, `oci_spec`, `tempfile`,
//! `pmcp_package` and `std` — no `clap`, no `GlobalFlags`, no
//! `crate::commands::*`. `cargo-pmcp/src/lib.rs` mounts it a second time as
//! `cargo_pmcp::package_artifact` with `#[path]`, exactly as it does
//! `commands/package/kind.rs`, so the property tests run under `cargo test
//! --lib` and a fuzz target can reach the untrusted-bytes boundary. That mount
//! is what forbids `super::`/`crate::commands::` here: in the lib target this
//! file's parent is the crate root, which declares no `commands` module.
//!
//! The consequence is visible in [`install_layout`]'s signature. The semantic
//! gate it runs — `detect_kind` plus the matching `unpack_*` — lives in the
//! bin-only command tree, so it arrives as a CLOSURE rather than as a direct
//! call. The ordering the closure is called in is the whole point and is fixed
//! here, not at the call site: staging is written, the closure validates the
//! STAGING layout, and only a successful closure earns the rename into place.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use oci_spec::image::{Descriptor, ImageIndex, ImageManifest, MediaType};
use pmcp_package::oci::OciLayout;
use pmcp_package::{canonicalize, ManifestDigest};

/// The layout marker file's archive path (`crates/pmcp-package/src/oci/layout.rs:48`).
const LAYOUT_MARKER_PATH: &str = "oci-layout";

/// The image index's archive path — the required entry point of an OCI layout.
const INDEX_PATH: &str = "index.json";

/// The one directory prefix a content-addressed blob may appear under.
const BLOB_PREFIX: &str = "blobs/sha256/";

// ---------------------------------------------------------------------
// The validated artifact model
// ---------------------------------------------------------------------

/// One blob the archive carried, paired with the descriptor facts that
/// authorize writing it.
///
/// `media_type` is NOT decoration: [`OciLayout::write_blob`] takes a
/// `MediaType` for every blob it writes, including the manifest blob and the
/// config blob (`crates/pmcp-package/src/oci/layout.rs:108`). A digest -> bytes
/// map carries none, so it cannot reconstruct a layout without inventing one.
/// Every value here came from the descriptor that referenced the blob.
#[derive(Debug, Clone)]
pub struct VerifiedBlob {
    /// The blob's exact bytes as the archive carried them.
    pub bytes: Vec<u8>,
    /// The media type of the FIRST descriptor that referenced this blob.
    ///
    /// "First" rather than "the" because an OCI layout is content-addressed and
    /// therefore de-duplicating: two layers whose payloads are byte-identical
    /// (two empty `[]` sections, say) share one blob file while carrying
    /// different vendor media types in the manifest. That is a legitimate
    /// package, so a media-type disagreement is recorded rather than refused.
    ///
    /// It is also why the value is safe: `write_blob` derives the destination
    /// filename from the BYTES, and the manifest itself is written back
    /// verbatim as its own blob, so the media type chosen here cannot change
    /// what lands on disk or what a reader sees.
    pub media_type: MediaType,
    /// The blob's length in bytes, cross-checked against every descriptor that
    /// declared a size for it.
    pub size: u64,
}

/// An artifact that has passed EVERY read-side gate, modelled as a validated
/// descriptor graph rather than as a bag of bytes.
///
/// Only [`read_verified_with_limits`] constructs one, and it touches the
/// filesystem zero times. [`write_layout`] therefore performs no validation of
/// its own: holding this type IS the proof. That split is the read-side mirror
/// of `pack_server`'s "a rejected pack adds neither a blob nor an index entry"
/// (`crates/pmcp-package/src/oci/pack.rs:913-937`).
#[derive(Debug, Clone)]
pub struct VerifiedArtifact {
    /// `index.json`, parsed.
    pub index: ImageIndex,
    /// `index.json`'s exact bytes as read, kept for byte-fidelity checks.
    ///
    /// [`write_layout`] deliberately does NOT write these back: it regenerates
    /// the destination's `index.json` through [`write_canonical_index`], so a
    /// loaded layout's index is canonical whatever the producer emitted.
    /// Keeping the originals lets a caller (and this module's tests) ask
    /// whether a producer's index was ALREADY canonical — a question about the
    /// PRODUCER, which must not be answered by silently preserving its bytes.
    pub index_bytes: Vec<u8>,
    /// THE one descriptor `index.manifests()` declares.
    pub manifest_descriptor: Descriptor,
    /// The image manifest parsed from the blob `manifest_descriptor` addresses.
    pub manifest: ImageManifest,
    /// Every blob the archive carried, keyed by lowercase hex digest.
    pub blobs: BTreeMap<String, VerifiedBlob>,
    /// The manifest digest, derived LOCALLY over the manifest blob's bytes —
    /// never read out of the archive and called derived.
    pub manifest_digest: ManifestDigest,
}

/// A layout that has been staged, semantically validated and renamed into its
/// final destination, together with whatever the validation produced.
///
/// `unpacked` is the value the caller's semantic gate returned while it ran
/// against the STAGING layout. Handing it back is what stops `load` from
/// unpacking a second time — and, more importantly, stops a second unpack call
/// site existing at all, since that is where a later change would reintroduce
/// the install-then-validate ordering this design removes.
#[derive(Debug)]
pub struct InstalledLayout<T> {
    /// The installed layout, opened at its final destination.
    pub layout: OciLayout,
    /// What the semantic gate produced against the staging layout.
    pub unpacked: T,
}

/// Which of the three slots an archive entry occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySlot {
    /// The `oci-layout` marker file. Accepted, its content ignored, and always
    /// regenerated by `OciLayout::create` on write (RESEARCH Pitfall 7).
    LayoutMarker,
    /// `index.json`.
    Index,
    /// `blobs/sha256/<hex>`, carrying the lowercase hex digest.
    Blob(String),
}

// ---------------------------------------------------------------------
// Reading: untrusted bytes in, a validated model out, nothing written
// ---------------------------------------------------------------------

/// Everything the entry loop collected, before any graph reasoning.
struct RawArchive {
    index_bytes: Option<Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
    entry_count: usize,
}

/// Classify one archive entry from its HEADER, before a byte of its content is
/// read, returning a typed slot or a refusal that names the offending path.
///
/// Pure by construction: it consults the filesystem for nothing. In particular
/// the path checks run over the raw [`Component`]s rather than over a
/// canonicalized string, because canonicalizing would consult the filesystem
/// and this function must not.
fn classify_entry(path: &Path) -> Result<EntrySlot> {
    let text = path
        .to_str()
        .ok_or_else(|| anyhow!("refusing an archive entry whose path is not valid UTF-8"))?;

    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            bail!(
                "refusing archive entry '{text}': only plain relative path components are \
                 admitted, and this path carries {component:?}"
            );
        }
    }

    if text == LAYOUT_MARKER_PATH {
        return Ok(EntrySlot::LayoutMarker);
    }
    if text == INDEX_PATH {
        return Ok(EntrySlot::Index);
    }
    if let Some(hex) = text.strip_prefix(BLOB_PREFIX) {
        if is_sha256_hex(hex) {
            return Ok(EntrySlot::Blob(hex.to_string()));
        }
    }
    bail!(
        "refusing archive entry '{text}': an artifact carries exactly '{LAYOUT_MARKER_PATH}', \
         '{INDEX_PATH}' and '{BLOB_PREFIX}<64 lowercase hex>' at the archive ROOT — a wrapper \
         directory or any other path shape is not part of an OCI image layout"
    );
}

/// Exactly 64 lowercase ASCII hex characters — the shape a sha256 blob file
/// name has, and the shape `ManifestDigest` guarantees.
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Parse every entry of `tar_bytes` into memory. Touches the filesystem zero
/// times.
fn collect_entries(tar_bytes: &[u8]) -> Result<RawArchive> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = archive
        .entries()
        .context("the artifact is not a readable tar archive")?;

    let mut index_bytes: Option<Vec<u8>> = None;
    let mut blobs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut entry_count = 0usize;

    for entry in entries {
        let mut entry = entry.context("read a tar entry from the artifact")?;
        let path = entry
            .path()
            .context("read a tar entry's path from the artifact")?
            .into_owned();
        let slot = classify_entry(&path)?;
        entry_count += 1;

        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("read the content of archive entry '{}'", path.display()))?;

        match slot {
            // Accepted, content ignored: nothing in `pmcp-package` ever reads
            // this file, and `OciLayout::create` regenerates it verbatim.
            EntrySlot::LayoutMarker => {},
            EntrySlot::Index => index_bytes = Some(buf),
            EntrySlot::Blob(hex) => {
                blobs.insert(hex, buf);
            },
        }
    }

    Ok(RawArchive {
        index_bytes,
        blobs,
        entry_count,
    })
}

/// Re-derive every blob's sha256 in memory and compare it to the hex in its own
/// archive path. A substituted blob fails HERE, before any write.
fn verify_blob_integrity(blobs: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for (hex, bytes) in blobs {
        let derived = ManifestDigest::from_bytes(bytes);
        let expected = format!("sha256:{hex}");
        if derived.as_str() != expected {
            bail!(
                "blob content does not match its own name: '{BLOB_PREFIX}{hex}' hashes to {}",
                derived.as_str()
            );
        }
    }
    Ok(())
}

/// Resolve `descriptor` against the archive's blobs, cross-checking its
/// declared size and recording its media type. `role` names the descriptor in
/// any refusal so two failures never share a message.
fn resolve_descriptor(
    descriptor: &Descriptor,
    raw: &BTreeMap<String, Vec<u8>>,
    resolved: &mut BTreeMap<String, VerifiedBlob>,
    role: &str,
) -> Result<Vec<u8>> {
    let digest = ManifestDigest::try_from(descriptor.digest())
        .with_context(|| format!("the {role} descriptor carries an unusable digest"))?;
    let hex = digest
        .as_str()
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("the {role} descriptor is not a sha256 digest: {digest}"))?
        .to_string();

    let bytes = raw.get(&hex).ok_or_else(|| {
        anyhow!(
            "dangling descriptor: the {role} descriptor names blob sha256:{hex}, which this \
             artifact does not carry"
        )
    })?;

    let actual = bytes.len() as u64;
    if descriptor.size() != actual {
        bail!(
            "descriptor size disagreement: the {role} descriptor for sha256:{hex} declares {} \
             bytes but the blob is {actual} bytes",
            descriptor.size()
        );
    }

    resolved.entry(hex).or_insert_with(|| VerifiedBlob {
        bytes: bytes.clone(),
        media_type: descriptor.media_type().clone(),
        size: actual,
    });

    Ok(bytes.clone())
}

/// Walk the descriptor graph from `index.json` and close it in BOTH directions.
fn resolve_graph(raw: RawArchive) -> Result<VerifiedArtifact> {
    let index_bytes = raw.index_bytes.ok_or_else(|| {
        anyhow!(
            "artifact carries no {INDEX_PATH}: an OCI image layout without its index is not a \
             package, and this reader will not synthesize an empty one"
        )
    })?;

    if raw.blobs.is_empty() {
        bail!(
            "artifact carries {INDEX_PATH} but no blobs under '{BLOB_PREFIX}' — there is nothing \
             for the index to describe"
        );
    }

    let index: ImageIndex = serde_json::from_slice(&index_bytes)
        .context("artifact's index.json does not deserialize as an OCI ImageIndex")?;

    let manifests = index.manifests();
    if manifests.len() != 1 {
        bail!(
            "expected exactly one manifest in {INDEX_PATH}, found {} — this reader mirrors \
             `read_the_one_manifest`'s rule at the framing boundary so the refusal happens \
             before any write rather than after one",
            manifests.len()
        );
    }
    let manifest_descriptor = manifests[0].clone();

    let mut resolved: BTreeMap<String, VerifiedBlob> = BTreeMap::new();
    let manifest_bytes =
        resolve_descriptor(&manifest_descriptor, &raw.blobs, &mut resolved, "manifest")?;
    let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes)
        .context("the artifact's manifest blob does not deserialize as an OCI ImageManifest")?;

    resolve_descriptor(manifest.config(), &raw.blobs, &mut resolved, "config")?;
    for (position, layer) in manifest.layers().iter().enumerate() {
        resolve_descriptor(
            layer,
            &raw.blobs,
            &mut resolved,
            &format!("layer[{position}]"),
        )?;
    }

    // Close the graph the other way: bytes nothing references are bytes a
    // producer smuggled in, and a reader that silently drops them is a reader
    // whose output is not a function of its input.
    for hex in raw.blobs.keys() {
        if !resolved.contains_key(hex) {
            bail!(
                "orphan blob: '{BLOB_PREFIX}{hex}' is present in the artifact but no descriptor \
                 reachable from {INDEX_PATH} references it"
            );
        }
    }

    let manifest_digest = ManifestDigest::from_bytes(&manifest_bytes);

    Ok(VerifiedArtifact {
        index,
        index_bytes,
        manifest_descriptor,
        manifest,
        blobs: resolved,
        manifest_digest,
    })
}

/// Read an artifact tar into a fully validated in-memory model. Writes NOTHING,
/// on success or on failure, to any path.
///
/// # Errors
///
/// Returns a distinct, named refusal for each gate: an unreadable archive, an
/// entry whose path is not part of an OCI image layout, an empty archive, an
/// artifact with no `index.json`, an artifact with an index but no blobs, a
/// blob whose bytes do not hash to its own file name, an index declaring other
/// than exactly one manifest, a descriptor naming a blob the artifact does not
/// carry, a descriptor whose declared size disagrees with the blob, and a blob
/// no descriptor reaches.
pub fn read_verified(tar_bytes: &[u8]) -> Result<VerifiedArtifact> {
    if tar_bytes.is_empty() {
        bail!("artifact is empty (zero bytes) — there is no archive here to read");
    }

    let raw = collect_entries(tar_bytes)?;
    if raw.entry_count == 0 {
        bail!("artifact archive contains no entries at all");
    }
    verify_blob_integrity(&raw.blobs)?;
    resolve_graph(raw)
}

// ---------------------------------------------------------------------
// Writing: only ever reached through an owned VerifiedArtifact
// ---------------------------------------------------------------------

/// Materialize an already-verified artifact into `staging`.
///
/// Reached only after [`read_verified`] returned `Ok`, so it validates nothing
/// of its own — the owned [`VerifiedArtifact`] IS the proof.
///
/// The parameter is named `staging` rather than `dest` so the call contract is
/// visible at every call site: this function writes a layout somewhere it is
/// safe to abandon. [`install_layout`] is what decides a layout has earned its
/// destination.
///
/// Every destination filename comes from `OciLayout::write_blob`'s digest over
/// bytes held in memory here; no archive path string reaches the filesystem.
/// Every `MediaType` handed to `write_blob` came from the descriptor that
/// referenced that blob — this function fabricates no descriptor and defaults
/// no media type.
///
/// # Errors
///
/// Returns any I/O or layout error `OciLayout::create`/`write_blob`/
/// `write_index` produces.
pub fn write_layout(artifact: &VerifiedArtifact, staging: &Path) -> Result<OciLayout> {
    let layout = OciLayout::create(staging)
        .with_context(|| format!("create a staging OCI layout at {}", staging.display()))?;
    for (hex, blob) in &artifact.blobs {
        layout
            .write_blob(blob.media_type.clone(), &blob.bytes)
            .with_context(|| format!("write blob sha256:{hex} into the staging layout"))?;
    }
    write_canonical_index(&layout, &artifact.index)
        .context("write index.json into the staging layout")?;
    Ok(layout)
}

/// Write `index.json` into `layout` with SORTED object keys.
///
/// # Why this exists, measured
///
/// `index.json` is the one file in an OCI layout that is not
/// content-addressed, and `oci_spec`'s `Descriptor::annotations` is an
/// `Option<HashMap<String, String>>`. Rust seeds `HashMap`'s hasher randomly
/// PER PROCESS, so `serde_json` emits its entries in a different order in
/// different runs. `finalize_pack` attaches exactly two annotations to the
/// manifest descriptor — `name` and `version`
/// (`crates/pmcp-package/src/oci/pack.rs:1181-1185`) — so the order flips
/// often.
///
/// Measured on the london-tube fixture across four `package save` processes:
/// the blob set was byte-identical every time, including the manifest digest
/// `sha256:afd2193b…`, and three runs emitted
/// `{"version":…,"name":…}` while the fourth emitted `{"name":…,"version":…}`.
/// Package IDENTITY was never in question; the ARTIFACT BYTES were.
///
/// Without this, `save` is not reproducible and two `load`s of one artifact
/// produce layouts that differ on disk. Both are properties this phase claims,
/// so both are enforced here rather than asserted in prose.
///
/// `canonicalize` (olpc-cjson) is the same primitive `finalize_pack` already
/// uses for the manifest BLOB; this applies the crate's own existing discipline
/// to the one file it had not been applied to.
///
/// # This is not the writer "fixing" a layout
///
/// [`write_tar`] still reads `index.json` off disk and emits it VERBATIM, and a
/// third-party artifact whose index is not canonical is still carried and still
/// loads. Normalization happens only where this code is the PRODUCER of the
/// layout — here, and in `save` over the layout it has just packed — which is
/// the same standing as `OciLayout::create` regenerating the `oci-layout`
/// marker.
///
/// The destination path is derived from the layout root and never from any
/// archive-supplied string.
///
/// # Errors
///
/// Returns an error if the index cannot be canonicalized or written.
pub fn write_canonical_index(layout: &OciLayout, index: &ImageIndex) -> Result<()> {
    let bytes = canonicalize(index).context("canonicalize index.json")?;
    let path = layout.root().join(INDEX_PATH);
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

/// Build a sibling path for `dest` by appending `suffix` to its file name.
///
/// Deliberately NOT `Path::with_extension`, which REPLACES an existing
/// extension: a destination named `london-tube.pmcp` would come back as
/// `london-tube.replaced-...`, colliding with any sibling of the same stem.
fn sibling_with_suffix(dest: &Path, suffix: &str) -> Result<PathBuf> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let name = dest
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name to install into", dest.display()))?;
    let mut renamed = name.to_os_string();
    renamed.push(suffix);
    Ok(parent.join(renamed))
}

/// Install a verified artifact at `dest`, transactionally.
///
/// The ordering is the guarantee, and it is fixed here rather than at the call
/// site:
///
/// 1. refuse when `dest` exists and `force` is false, BEFORE anything else;
/// 2. create a staging directory as a SIBLING of `dest`;
/// 3. write the layout into staging;
/// 4. run `validate` against the STAGING layout — any failure returns `Err`
///    with the staging guard dropping, and the destination never touched;
/// 5. only then rename into place.
///
/// # Why the staging directory is a sibling and not a temp directory
///
/// `std::fs::rename` fails `EXDEV` across filesystems, and a
/// `tempfile::TempDir` with default placement lands wherever `TMPDIR` points —
/// routinely a different filesystem from the destination. Creating staging in
/// the DESTINATION'S PARENT is what makes the final rename a same-filesystem
/// operation, and therefore what makes step 5 possible at all.
///
/// # Why `validate` is a parameter
///
/// The semantic gate — `detect_kind` plus the matching `unpack_*` — is
/// substantive validation (manifest structure, required media types, config
/// blob, legacy-shape detection, deserialization:
/// `crates/pmcp-package/src/oci/unpack.rs:639-656`) that can only run against a
/// layout that EXISTS. Running it against staging is what turns it from a
/// post-write check into a pre-write gate. It arrives as a closure because it
/// lives in the bin-only command tree and this module is a lib-mounted leaf.
///
/// # What is deliberately NOT in this gate
///
/// Attestation SUBJECT MISMATCH (D-15). A package whose bytes are sound but
/// whose claim is false is INSTALLED and then reported, exiting non-zero. Do
/// not "tidy up" by moving the subject check into the staging gate: that would
/// harmonize two verdicts Phase 122's D-03 keeps deliberately apart — integrity
/// failure means the bytes are corrupt, subject mismatch means the bytes are
/// fine and the claim is wrong.
///
/// # The residual window, named rather than claimed away
///
/// Between the two renames in step 5 there is a crash window. If the process
/// dies there, the PREVIOUS layout is intact under a `.replaced-<nanos>`
/// sibling of `dest`, and every error path on that step names that directory to
/// the operator. This is strictly smaller than writing the destination
/// directly, which can leave a marker, an empty index and a partial blob set
/// with no record that the destination had ever been complete.
///
/// # Concurrency
///
/// GUARANTEED: every blob's file name is a digest of its own content, so no
/// interleaving can produce a blob whose bytes disagree with its name. NOT
/// guaranteed: `index.json` is not content-addressed, which is why `load`
/// refuses an existing destination without `--force`. Two concurrent `--force`
/// runs into one destination are UNSUPPORTED.
///
/// # Errors
///
/// Returns an error when `dest` exists without `force`, when staging cannot be
/// created or written, when `validate` refuses, or when either rename fails.
pub fn install_layout<T>(
    artifact: &VerifiedArtifact,
    dest: &Path,
    force: bool,
    validate: impl FnOnce(&OciLayout) -> Result<T>,
) -> Result<InstalledLayout<T>> {
    if dest.exists() && !force {
        bail!(
            "{} already exists — refusing to replace it. Pass --force to replace it.",
            dest.display()
        );
    }

    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create the destination's parent {}", parent.display()))?;
    }

    let staging = tempfile::Builder::new()
        .prefix(".pmcp-load-staging-")
        .tempdir_in(parent)
        .with_context(|| {
            format!(
                "create a staging directory beside {} (staging MUST be a sibling so the final \
                 rename is same-filesystem)",
                dest.display()
            )
        })?;

    let staged_layout = write_layout(artifact, staging.path())?;
    // Any refusal here drops `staging`, which deletes it. The destination has
    // not been touched at any point up to this line.
    let unpacked = validate(&staged_layout)?;

    let staged_path = staging.keep();
    let installed = install_staged(&staged_path, dest);
    if installed.is_err() {
        let _ = fs::remove_dir_all(&staged_path);
    }
    installed?;

    Ok(InstalledLayout {
        layout: OciLayout::open(dest),
        unpacked,
    })
}

/// Step 5 of [`install_layout`]: move `staged_path` onto `dest`, displacing an
/// existing destination through a named `.replaced-<nanos>` sibling first.
fn install_staged(staged_path: &Path, dest: &Path) -> Result<()> {
    let replaced = if dest.exists() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let holding = sibling_with_suffix(dest, &format!(".replaced-{nanos}"))?;
        fs::rename(dest, &holding).with_context(|| {
            format!(
                "move the existing layout at {} aside before installing",
                dest.display()
            )
        })?;
        Some(holding)
    } else {
        None
    };

    match fs::rename(staged_path, dest) {
        Ok(()) => {
            if let Some(holding) = replaced {
                fs::remove_dir_all(&holding).with_context(|| {
                    format!("remove the replaced layout at {}", holding.display())
                })?;
            }
            Ok(())
        },
        Err(install_error) => {
            let Some(holding) = replaced else {
                return Err(install_error)
                    .with_context(|| format!("install the layout at {}", dest.display()));
            };
            match fs::rename(&holding, dest) {
                Ok(()) => Err(anyhow!(install_error)).with_context(|| {
                    format!(
                        "install the layout at {} (the previous layout was restored)",
                        dest.display()
                    )
                }),
                Err(restore_error) => Err(anyhow!(
                    "failed to install the layout at {dest} ({install_error}) AND failed to \
                     restore the previous one ({restore_error}). The previous layout is intact \
                     at {holding} — move it back by hand.",
                    dest = dest.display(),
                    holding = holding.display()
                )),
            }
        },
    }
}

// ---------------------------------------------------------------------
// Writing the movable form
// ---------------------------------------------------------------------

/// Tar `layout` to `dest` as the movable form of the package (D-11).
///
/// Reads `oci-layout`, `index.json` and each `blobs/sha256/<hex>` off disk
/// VERBATIM and re-serializes nothing. A writer that re-serialized `index.json`
/// could silently "fix" a malformed layout, which would make byte-exact
/// writer-conformance untestable.
///
/// The output is REPRODUCIBLE: entry order is fixed (`oci-layout`, then
/// `index.json`, then every blob sorted lexicographically by hex) and every
/// header is normalized (mtime 0, uid/gid 0, empty user/group name, mode 0644,
/// regular-file type, ustar so no PAX/GNU extension record is emitted).
///
/// Emitting the `oci-layout` marker is deliberate (RESEARCH Pitfall 7): a plain
/// `tar -xf` then yields a valid layout for a human debugging by hand, while
/// readers accept it, ignore its content, and always regenerate it through
/// `OciLayout::create`.
///
/// # Errors
///
/// Returns an error if the layout cannot be read, if `blobs/sha256/` carries a
/// file whose name is not a sha256 hex digest, or if `dest` cannot be written.
pub fn write_tar(layout: &OciLayout, dest: &Path) -> Result<()> {
    let root = layout.root();
    let mut builder = tar::Builder::new(Vec::new());

    let marker = root.join(LAYOUT_MARKER_PATH);
    if marker.exists() {
        let bytes = fs::read(&marker).with_context(|| format!("read {}", marker.display()))?;
        append_normalized(&mut builder, LAYOUT_MARKER_PATH, &bytes)?;
    }

    let index_path = root.join(INDEX_PATH);
    let index_bytes =
        fs::read(&index_path).with_context(|| format!("read {}", index_path.display()))?;
    append_normalized(&mut builder, INDEX_PATH, &index_bytes)?;

    for hex in sorted_blob_hexes(root)? {
        let blob_path = root.join("blobs").join("sha256").join(&hex);
        let bytes =
            fs::read(&blob_path).with_context(|| format!("read {}", blob_path.display()))?;
        append_normalized(&mut builder, &format!("{BLOB_PREFIX}{hex}"), &bytes)?;
    }

    let bytes = builder
        .into_inner()
        .context("finish writing the artifact archive")?;
    fs::write(dest, bytes).with_context(|| format!("write the artifact to {}", dest.display()))
}

/// Every blob file name under `blobs/sha256/`, sorted — the deterministic entry
/// order [`write_tar`] depends on, since `read_dir` order is not defined.
fn sorted_blob_hexes(root: &Path) -> Result<Vec<String>> {
    let dir = root.join("blobs").join("sha256");
    let mut hexes = Vec::new();
    let entries =
        fs::read_dir(&dir).with_context(|| format!("read the blob directory {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("read an entry of {}", dir.display()))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            anyhow!(
                "the blob directory {} carries a file whose name is not valid UTF-8",
                dir.display()
            )
        })?;
        if !is_sha256_hex(name) {
            bail!(
                "refusing to tar {}: '{name}' is not a sha256 blob file name, so this is not a \
                 layout this writer can carry verbatim",
                dir.display()
            );
        }
        hexes.push(name.to_string());
    }
    hexes.sort();
    Ok(hexes)
}

/// Append one entry under a fully normalized ustar header, so two runs over
/// identical inputs produce byte-identical archives.
fn append_normalized(builder: &mut tar::Builder<Vec<u8>>, path: &str, bytes: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_ustar();
    header
        .set_path(path)
        .with_context(|| format!("set the archive path '{path}'"))?;
    header.set_size(bytes.len() as u64);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header
        .set_username("")
        .context("normalize the archive entry's user name")?;
    header
        .set_groupname("")
        .context("normalize the archive entry's group name")?;
    header.set_cksum();
    builder
        .append(&header, bytes)
        .with_context(|| format!("append '{path}' to the artifact archive"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_entry_accepts_the_three_layout_slots() {
        assert_eq!(
            classify_entry(Path::new("oci-layout")).unwrap(),
            EntrySlot::LayoutMarker
        );
        assert_eq!(
            classify_entry(Path::new("index.json")).unwrap(),
            EntrySlot::Index
        );
        let hex = "a".repeat(64);
        assert_eq!(
            classify_entry(Path::new(&format!("blobs/sha256/{hex}"))).unwrap(),
            EntrySlot::Blob(hex)
        );
    }

    #[test]
    fn classify_entry_refuses_a_wrapper_directory() {
        let err = classify_entry(Path::new("package/index.json")).unwrap_err();
        assert!(
            err.to_string().contains("archive ROOT"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_a_parent_directory_component() {
        let err = classify_entry(Path::new("../index.json")).unwrap_err();
        assert!(
            err.to_string().contains("plain relative path components"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_an_absolute_path() {
        let err = classify_entry(Path::new("/index.json")).unwrap_err();
        assert!(
            err.to_string().contains("plain relative path components"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn classify_entry_refuses_uppercase_and_short_blob_hex() {
        for bad in [
            format!("blobs/sha256/{}", "A".repeat(64)),
            format!("blobs/sha256/{}", "a".repeat(63)),
        ] {
            assert!(
                classify_entry(Path::new(&bad)).is_err(),
                "{bad} must be refused"
            );
        }
    }

    #[test]
    fn read_verified_refuses_a_zero_byte_artifact() {
        let err = read_verified(&[]).unwrap_err();
        assert!(
            err.to_string().contains("zero bytes"),
            "unexpected message: {err}"
        );
    }
}
