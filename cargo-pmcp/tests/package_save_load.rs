//! Phase 123 Plan 01 (PKGX-02) — `cargo pmcp package save` -> `.tar` ->
//! `cargo pmcp package load` -> `cargo pmcp package inspect`, end to end.
//!
//! Every assertion here drives the REAL `cargo-pmcp` binary through
//! `assert_cmd`, the way `cargo-pmcp/tests/package_inspect.rs` does. The claim
//! this file makes is about the wired CLI path — that a user can save a
//! configuration server to one movable file and read it back — not about an
//! internal function, so calling the codec directly would prove the wrong
//! thing.
//!
//! # Two kinds of archive bytes, which must not be confused
//!
//! This file AUTHORS hostile tar archives in-test, with the `tar` crate's
//! builder. That is deliberate and is the one place it is allowed: a hostile
//! shape has to be constructed, and constructing it here keeps the shape and
//! the assertion about it in the same place.
//!
//! It is NOT the same thing as a golden fixture. A golden fixture is bytes
//! CHECKED IN and never regenerated from the writer under test — that is what
//! makes it able to catch the writer drifting. Bytes authored in-test can only
//! ever agree with the code that authored them. Plan 04 owns the golden
//! fixtures; do not "unify" the two.
//!
//! # A version discrepancy that is D-10 working, not a bug
//!
//! `london-tube.toml` declares `version = "1.1.0"`, while Phase 121's
//! `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs:207` hardcodes `1.0.0` in
//! a hand-built package. `save` reads the config (D-10), so it produces
//! `london-tube@1.1.0`. Nothing in this file asserts `1.0.0`.
//!
//! Run single-threaded (`-- --test-threads=1`), as `make
//! test-cargo-pmcp-integration` does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use oci_spec::image::{
    Descriptor, ImageIndexBuilder, ImageManifestBuilder, MediaType, SCHEMA_VERSION,
};
use pmcp_package::oci::media_types::{ARTIFACT_TYPE_SERVER, EMPTY_CONFIG_BLOB, MT_EMPTY_CONFIG};
use pmcp_package::oci::OciLayout;
use pmcp_package::ManifestDigest;
use predicates::str::contains;

// ---------------------------------------------------------------------
// Fixture assembly
// ---------------------------------------------------------------------

/// The checked-in london-tube config-server fixture this tracer packs.
fn golden_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/pmcp-package/tests/golden_fixtures/config_server_london_tube_v1")
}

/// A `.pmcp/deploy.toml` that parses BOTH as cargo-pmcp's own `DeployConfig`
/// and as `pmcp-package`'s narrower closed-set `DeployDescriptor`.
///
/// Adapted from the real `crates/pmcp-server/.pmcp/deploy.toml`, trimmed to the
/// tables `DeployDescriptor` models. `save` reads this file rather than
/// synthesizing a descriptor, which is the whole of D-10 on the deploy side.
const LONDON_TUBE_DEPLOY_TOML: &str = r#"[target]
type = "pmcp-run"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "london-tube"
memory_mb = 1024
timeout_seconds = 30

[environment]
RUST_LOG = "info"

[secrets]

[auth]
enabled = false
provider = "none"
callback_urls = []

[observability]
log_retention_days = 30
enable_xray = true
create_dashboard = true

[assets]
include = []
exclude = ["**/*.tmp"]
"#;

/// Lay out a saveable project under `root`: the london-tube config + spec, plus
/// a `.pmcp/deploy.toml`. Returns the config path.
fn london_tube_project(root: &Path) -> PathBuf {
    let fixture = golden_fixture_dir();
    let config = root.join("london-tube.toml");
    std::fs::copy(fixture.join("london-tube.toml"), &config).expect("copy the fixture config");
    std::fs::copy(
        fixture.join("london-tube-api.yaml"),
        root.join("london-tube-api.yaml"),
    )
    .expect("copy the fixture spec");
    write_deploy_toml(root, LONDON_TUBE_DEPLOY_TOML);
    config
}

/// Write `.pmcp/deploy.toml` under `root`.
fn write_deploy_toml(root: &Path, body: &str) {
    let pmcp_dir = root.join(".pmcp");
    std::fs::create_dir_all(&pmcp_dir).expect("create .pmcp/");
    std::fs::write(pmcp_dir.join("deploy.toml"), body).expect("write .pmcp/deploy.toml");
}

/// A digest for `--binary-digest`. A configuration server NAMES its runtime
/// binary rather than carrying one, so any well-formed digest exercises the
/// same path the real one would.
fn referenced_binary_digest() -> String {
    ManifestDigest::from_bytes(b"pmcp-openapi-server-v1.1.0-aarch64")
        .as_str()
        .to_string()
}

/// Run `package save` on the london-tube project at `root`, writing `output`.
fn save_london_tube(root: &Path, config: &Path, output: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("cargo-pmcp")
        .expect("cargo-pmcp binary must be available")
        .args([
            "package",
            "save",
            "--config",
            config.to_str().unwrap(),
            "--spec",
            root.join("london-tube-api.yaml").to_str().unwrap(),
            "--project-root",
            root.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
}

/// Run `package load`, returning the assertion for the caller to judge.
fn load_artifact(input: &Path, output: &Path, force: bool) -> assert_cmd::assert::Assert {
    let mut command =
        Command::cargo_bin("cargo-pmcp").expect("cargo-pmcp binary must be available");
    command.args([
        "package",
        "load",
        input.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ]);
    if force {
        command.arg("--force");
    }
    command.assert()
}

/// Save the london-tube fixture into a fresh project and return
/// `(temp dir, tar bytes)`. The `TempDir` is returned so the caller keeps it
/// alive — dropping it deletes everything.
fn saved_london_tube_tar() -> (tempfile::TempDir, Vec<u8>) {
    let dir = tempfile::tempdir().expect("create a temp project");
    let config = london_tube_project(dir.path());
    let output = dir.path().join("london-tube.tar");
    save_london_tube(dir.path(), &config, &output).success();
    let bytes = std::fs::read(&output).expect("read the saved artifact");
    (dir, bytes)
}

// ---------------------------------------------------------------------
// Archive surgery — building the hostile shapes
// ---------------------------------------------------------------------

/// Read every `(path, bytes)` pair out of a tar archive, in archive order.
fn entries_of(tar_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let mut out = Vec::new();
    for entry in archive.entries().expect("read the archive") {
        let mut entry = entry.expect("read an archive entry");
        let path = entry
            .path()
            .expect("read an entry path")
            .to_string_lossy()
            .into_owned();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).expect("read entry content");
        out.push((path, bytes));
    }
    out
}

/// Rebuild a tar archive from `(path, bytes)` pairs, with the same normalized
/// ustar headers `write_tar` produces so the ONLY difference from a real
/// artifact is the one the test introduced.
fn build_tar(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_path(path).expect("set the entry path");
        header.set_size(bytes.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").expect("normalize the user name");
        header.set_groupname("").expect("normalize the group name");
        header.set_cksum();
        builder
            .append(&header, bytes.as_slice())
            .expect("append an entry");
    }
    builder.into_inner().expect("finish the archive")
}

/// The archive path a blob with these bytes must occupy.
fn blob_path_for(bytes: &[u8]) -> String {
    let digest = ManifestDigest::from_bytes(bytes);
    let hex = digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("a sha256 digest");
    format!("blobs/sha256/{hex}")
}

/// The archive path of the blob `index.json`'s single manifest descriptor names.
fn manifest_blob_path(entries: &[(String, Vec<u8>)]) -> String {
    let index = index_json_of(entries);
    let digest = index["manifests"][0]["digest"]
        .as_str()
        .expect("the manifest descriptor carries a digest");
    let hex = digest.strip_prefix("sha256:").expect("a sha256 digest");
    format!("blobs/sha256/{hex}")
}

/// Parse the archive's `index.json` as untyped JSON, for surgery.
fn index_json_of(entries: &[(String, Vec<u8>)]) -> serde_json::Value {
    let (_, bytes) = entries
        .iter()
        .find(|(path, _)| path == "index.json")
        .expect("the artifact carries index.json");
    serde_json::from_slice(bytes).expect("index.json is JSON")
}

/// Replace the archive's `index.json` with `value`.
fn with_index(entries: &[(String, Vec<u8>)], value: &serde_json::Value) -> Vec<(String, Vec<u8>)> {
    entries
        .iter()
        .map(|(path, bytes)| {
            if path == "index.json" {
                (
                    path.clone(),
                    serde_json::to_vec(value).expect("serialize index.json"),
                )
            } else {
                (path.clone(), bytes.clone())
            }
        })
        .collect()
}

/// A tar that passes every framing and integrity gate and whose descriptor
/// graph closes, but whose manifest is SEMANTICALLY malformed: it declares the
/// server artifact type and then carries no layers at all, so `unpack_server`
/// refuses it at its "missing layer" check.
///
/// This is the class the reviewed draft of this plan would have written to the
/// destination before discovering — which is exactly why `install_layout`
/// stages.
fn semantically_malformed_server_tar() -> Vec<u8> {
    let dir = tempfile::tempdir().expect("create a temp layout");
    let layout = OciLayout::create(dir.path()).expect("create the layout");

    let config = layout
        .write_blob(MediaType::from(MT_EMPTY_CONFIG), EMPTY_CONFIG_BLOB)
        .expect("write the empty config blob");
    let manifest = ImageManifestBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .media_type(MediaType::ImageManifest)
        .artifact_type(MediaType::from(ARTIFACT_TYPE_SERVER))
        .config(config)
        .layers(Vec::<Descriptor>::new())
        .build()
        .expect("build a layer-less server manifest");
    let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize the manifest");
    let manifest_descriptor = layout
        .write_manifest(&manifest_bytes)
        .expect("write the manifest blob");
    let index = ImageIndexBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .manifests(vec![manifest_descriptor])
        .build()
        .expect("build the index");
    layout.write_index(&index).expect("write index.json");

    let tar_path = dir.path().join("..").join("malformed.tar");
    let tar_path = tar_path
        .canonicalize()
        .unwrap_or_else(|_| dir.path().join("malformed.tar"));
    // Written OUTSIDE the layout when possible so the archive never carries
    // itself; if the parent is not canonicalizable, fall back and remove it
    // from the inventory below.
    cargo_pmcp::package_artifact::write_tar(&layout, &tar_path).expect("tar the layout");
    let bytes = std::fs::read(&tar_path).expect("read the malformed artifact");
    let _ = std::fs::remove_file(&tar_path);
    bytes
}

/// A recursive `relative path -> sha256` map of everything under `root`, for
/// asserting a destination is byte-for-byte unchanged.
fn fingerprint(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).expect("read a directory") {
            let entry = entry.expect("read a directory entry");
            let path = entry.path();
            if entry.file_type().expect("stat an entry").is_dir() {
                walk(base, &path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read a file");
                let relative = path
                    .strip_prefix(base)
                    .expect("a path under the root")
                    .to_string_lossy()
                    .into_owned();
                out.insert(
                    relative,
                    ManifestDigest::from_bytes(&bytes).as_str().to_string(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// ---------------------------------------------------------------------
// The tracer: one path, end to end
// ---------------------------------------------------------------------

/// THE tracer assertion: a configuration server saved to one movable file,
/// read back into a working layout, and opened by the shipped `inspect` verb
/// unchanged — all three steps from the real binary, fully offline.
#[test]
fn save_then_load_then_inspect_round_trips_the_london_tube_fixture() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    let tar = project.path().join("london-tube.tar");

    save_london_tube(project.path(), &config, &tar).success();
    assert!(tar.is_file(), "save must leave exactly one artifact file");

    let destination = tempfile::tempdir().unwrap();
    let layout = destination.path().join("london-tube");
    load_artifact(&tar, &layout, false)
        .success()
        .stdout(contains("server"))
        .stdout(contains("london-tube"))
        // D-10: the version comes from the config the user maintains, which
        // declares 1.1.0 — not from any hand-written constant.
        .stdout(contains("1.1.0"));
    assert!(layout.is_dir(), "load must create the layout directory");

    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "inspect", layout.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("server"))
        .stdout(contains("london-tube"));
}

/// The framing rule, asserted on the writer's own output: the archive carries
/// exactly the layout marker, the index and content-addressed blobs, all at the
/// archive ROOT with no wrapper directory.
#[test]
fn save_writes_only_layout_entries_at_the_archive_root() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);

    assert_eq!(
        entries[0].0, "oci-layout",
        "the layout marker is emitted first"
    );
    assert_eq!(entries[1].0, "index.json", "the index is emitted second");
    let blobs: Vec<&String> = entries[2..].iter().map(|(path, _)| path).collect();
    assert!(!blobs.is_empty(), "the artifact must carry blobs");
    for path in &blobs {
        let hex = path
            .strip_prefix("blobs/sha256/")
            .unwrap_or_else(|| panic!("unexpected archive entry {path}"));
        assert_eq!(hex.len(), 64, "{path} must be a sha256 blob name");
    }
    let mut sorted = blobs.clone();
    sorted.sort();
    assert_eq!(
        blobs, sorted,
        "blob entries must be emitted in sorted order"
    );
}

/// Reproducibility: the artifact is a function of its inputs alone. Header
/// normalization (mtime 0, uid/gid 0, empty user/group, fixed mode) plus a
/// fixed entry order is what makes this hold across two runs seconds apart.
#[test]
fn two_saves_of_identical_inputs_are_byte_identical() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());

    let first = project.path().join("first.tar");
    let second = project.path().join("second.tar");
    save_london_tube(project.path(), &config, &first).success();
    save_london_tube(project.path(), &config, &second).success();

    let a = std::fs::read(&first).unwrap();
    let b = std::fs::read(&second).unwrap();
    assert_eq!(
        a, b,
        "two saves of identical inputs must produce byte-identical artifacts"
    );
}

/// A config declaring no `[[config_slots]]` at all is legal — such a package
/// simply claims no slots.
#[test]
fn save_succeeds_for_a_config_declaring_no_config_slots() {
    let project = tempfile::tempdir().unwrap();
    let config = project.path().join("plain.toml");
    std::fs::write(
        &config,
        "[server]\nname = \"plain\"\nversion = \"0.1.0\"\n\n[[tools]]\nname = \"ping\"\n\
         description = \"Answers pong.\"\n",
    )
    .unwrap();
    write_deploy_toml(
        project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace("london-tube", "plain"),
    );

    let output = project.path().join("plain.tar");
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args([
            "package",
            "save",
            "--config",
            config.to_str().unwrap(),
            "--project-root",
            project.path().to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
            "--binary-digest",
            &referenced_binary_digest(),
        ])
        .assert()
        .success();
    assert!(output.is_file());
}

/// Pitfall 6: existing callers treat a `load_deploy_descriptor` parse failure
/// as a graceful legacy-deploy fallback. `save` diverges deliberately — a
/// package whose deploy target was defaulted rather than authored is the exact
/// outcome D-10 exists to prevent, and it would look fine until it was deployed
/// somewhere wrong.
#[test]
fn save_refuses_a_deploy_toml_that_is_not_a_deploy_descriptor() {
    let project = tempfile::tempdir().unwrap();
    let config = london_tube_project(project.path());
    // `[aws].account_id` is the canonical unmodelled field: cargo-pmcp's own
    // `AwsConfig` accepts it, `pmcp-package`'s `AwsSection` is
    // `deny_unknown_fields` and does not.
    write_deploy_toml(
        project.path(),
        &LONDON_TUBE_DEPLOY_TOML.replace(
            "[aws]\nregion = \"us-east-1\"",
            "[aws]\nregion = \"us-east-1\"\naccount_id = \"123456789012\"",
        ),
    );

    let output = project.path().join("london-tube.tar");
    save_london_tube(project.path(), &config, &output)
        .failure()
        .stderr(contains("deploy.toml"));
    assert!(
        !output.exists(),
        "a refused save must leave no partial artifact"
    );
}

/// `load` refuses a destination that already exists unless `--force`.
#[test]
fn load_refuses_an_existing_destination_without_force() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("artifact.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();

    let destination = holder.path().join("existing");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("keep-me"), b"untouched").unwrap();
    let before = fingerprint(&destination);

    load_artifact(&tar, &destination, false)
        .failure()
        .stderr(contains("already exists"));
    assert_eq!(
        before,
        fingerprint(&destination),
        "a refused load must leave the destination byte-for-byte unchanged"
    );
}

/// With `--force`, a second load of the same artifact yields the same layout.
#[test]
fn load_replaces_an_existing_destination_with_force() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("artifact.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();

    let destination = holder.path().join("layout");
    load_artifact(&tar, &destination, false).success();
    let first = fingerprint(&destination);

    load_artifact(&tar, &destination, true).success();
    assert_eq!(
        first,
        fingerprint(&destination),
        "a forced re-load of the same artifact must yield the same layout"
    );
    // And the transactional install leaves no `.replaced-` debris behind.
    let siblings: Vec<String> = std::fs::read_dir(holder.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !siblings.iter().any(|name| name.contains(".replaced-")),
        "a successful install must remove the replaced layout: {siblings:?}"
    );
}

// ---------------------------------------------------------------------
// Graph-closure refusals — each names its own cause, none writes
// ---------------------------------------------------------------------

/// Run `load` on hostile bytes and assert both halves: a non-zero exit AND a
/// destination that does not exist afterwards.
fn assert_load_refuses(tar_bytes: &[u8], expected: &str) {
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("hostile.tar");
    std::fs::write(&tar, tar_bytes).unwrap();
    let destination = holder.path().join("destination");

    load_artifact(&tar, &destination, false)
        .failure()
        .stderr(contains(expected));
    assert!(
        !destination.exists(),
        "a refused load must not create {}",
        destination.display()
    );
}

/// A descriptor naming a blob the artifact does not carry.
#[test]
fn load_refuses_a_dangling_descriptor_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let manifest_path = manifest_blob_path(&entries);
    let pruned: Vec<(String, Vec<u8>)> = entries
        .into_iter()
        .filter(|(path, _)| path != &manifest_path)
        .collect();

    assert_load_refuses(&build_tar(&pruned), "dangling descriptor");
}

/// A well-formed blob that no descriptor references. Bytes nothing points at
/// are bytes a producer smuggled in, and a reader that silently drops them is a
/// reader whose output is not a function of its input.
#[test]
fn load_refuses_an_orphan_blob_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let mut entries = entries_of(&tar_bytes);
    let smuggled = b"nothing references these bytes".to_vec();
    entries.push((blob_path_for(&smuggled), smuggled));

    assert_load_refuses(&build_tar(&entries), "orphan blob");
}

/// An index declaring other than exactly one manifest — the rule
/// `read_the_one_manifest` enforces, mirrored here at the framing boundary so
/// the refusal happens before a write rather than after one.
#[test]
fn load_refuses_an_index_declaring_two_manifests_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let mut index = index_json_of(&entries);
    let duplicate = index["manifests"][0].clone();
    index["manifests"]
        .as_array_mut()
        .expect("manifests is an array")
        .push(duplicate);

    assert_load_refuses(
        &build_tar(&with_index(&entries, &index)),
        "expected exactly one manifest",
    );
}

/// A descriptor whose declared size disagrees with the blob's actual length.
#[test]
fn load_refuses_a_descriptor_size_disagreement_and_writes_nothing() {
    let (_project, tar_bytes) = saved_london_tube_tar();
    let entries = entries_of(&tar_bytes);
    let mut index = index_json_of(&entries);
    index["manifests"][0]["size"] = serde_json::json!(1);

    assert_load_refuses(
        &build_tar(&with_index(&entries, &index)),
        "descriptor size disagreement",
    );
}

// ---------------------------------------------------------------------
// The SEMANTIC class — the case a post-write check would have failed
// ---------------------------------------------------------------------

/// A package that is correctly content-addressed and whose descriptor graph
/// closes, but which `unpack_server` refuses. `install_layout` runs that check
/// against a STAGING sibling, so the destination is never created.
#[test]
fn load_refuses_a_semantically_malformed_package_and_writes_nothing() {
    let tar_bytes = semantically_malformed_server_tar();
    let holder = tempfile::tempdir().unwrap();
    let tar = holder.path().join("malformed.tar");
    std::fs::write(&tar, &tar_bytes).unwrap();
    let destination = holder.path().join("destination");

    load_artifact(&tar, &destination, false).failure();
    assert!(
        !destination.exists(),
        "a semantic refusal must not create the destination — this is the case an \
         install-then-validate ordering would have failed"
    );
}

/// The `--force` variant: a semantic refusal must leave a PRE-EXISTING
/// destination byte-for-byte unchanged, not half-replaced.
#[test]
fn a_forced_load_of_a_semantically_malformed_package_leaves_the_destination_unchanged() {
    let (_project, good_bytes) = saved_london_tube_tar();
    let holder = tempfile::tempdir().unwrap();
    let good = holder.path().join("good.tar");
    std::fs::write(&good, &good_bytes).unwrap();
    let destination = holder.path().join("layout");
    load_artifact(&good, &destination, false).success();
    let before = fingerprint(&destination);
    assert!(!before.is_empty(), "the installed layout must have files");

    let malformed = holder.path().join("malformed.tar");
    std::fs::write(&malformed, semantically_malformed_server_tar()).unwrap();
    load_artifact(&malformed, &destination, true).failure();

    assert_eq!(
        before,
        fingerprint(&destination),
        "a semantic refusal under --force must leave the existing layout byte-for-byte unchanged"
    );
}

// ---------------------------------------------------------------------
// The user-visible surface
// ---------------------------------------------------------------------

/// The two new verbs are reachable from the command group (the asserted
/// exact-set pin over the whole group is plan 06's).
#[test]
fn package_help_lists_save_and_load() {
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "--help"])
        .assert()
        .success()
        .stdout(contains("save"))
        .stdout(contains("load"));
}

/// `--spec`'s long help must state the resolution rule. Omitting the flag
/// silently produces a package with no spec layer, and that failure surfaces
/// much later, in the target environment.
#[test]
fn save_help_documents_the_spec_resolution_rule() {
    Command::cargo_bin("cargo-pmcp")
        .unwrap()
        .args(["package", "save", "--help"])
        .assert()
        .success()
        .stdout(contains("OpenAPI-backed Shape A server"))
        .stdout(contains("not derivable from the config"))
        .stdout(contains("pure-configuration server"));
}
