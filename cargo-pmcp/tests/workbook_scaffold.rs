//! WBCL-05 — Shape B workbook-server scaffold integration test.
//!
//! Exercises the REAL `cargo pmcp new --kind workbook-server <name>` command
//! surface (invoked through `env!("CARGO_BIN_EXE_cargo-pmcp")`, the actual
//! binary a user runs — not an in-process `new::execute`) and asserts the full
//! emitted file tree, including the embedded `tax-calc@1.1.0` bundle.
//!
//! Two heavier smokes are `#[ignore]`d by default (they compile/resolve the
//! as-yet-unpublished workspace crates, which is slow and needs the in-repo
//! `[patch.crates-io]` overrides). Run them explicitly:
//!
//! ```sh
//! # scaffold-build smoke (cargo check inside the generated crate):
//! PMCP_RUN_SCAFFOLD_BUILD=1 cargo test -p cargo-pmcp --test workbook_scaffold \
//!     -- --ignored scaffold_crate_cargo_check_compiles --test-threads=1
//!
//! # packaging smoke (assets survive cargo publish):
//! cargo test -p cargo-pmcp --test workbook_scaffold -- --ignored \
//!     embedded_assets_appear_in_cargo_package_list
//! ```

use std::path::Path;
use std::process::Command;

/// A valid `validate_crate_name` identifier for the scaffold.
const SCAFFOLD_NAME: &str = "scaffold_workbook_demo";

/// The five contract members every emitted bundle must carry.
const BUNDLE_MEMBERS: [&str; 5] = [
    "BUNDLE.lock",
    "cell_map.json",
    "executable.ir.json",
    "layout.json",
    "manifest.json",
];

/// The in-repo workspace root (parent of the cargo-pmcp manifest dir).
fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-pmcp manifest dir must have a parent (the workspace root)")
        .to_path_buf()
}

#[test]
fn scaffold_emits_full_file_tree_including_embedded_bundle() {
    let tmp = tempfile::tempdir().expect("create tempdir");

    // Invoke the REAL built binary (the actual command surface).
    let status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "workbook-server", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .status()
        .expect("spawn the real cargo-pmcp binary to scaffold");
    assert!(
        status.success(),
        "`cargo pmcp new --kind workbook-server {SCAFFOLD_NAME}` must succeed (exit {status:?})"
    );

    let crate_dir = tmp.path().join(SCAFFOLD_NAME);

    // Top-level payload (D-06).
    for rel in [
        "Cargo.toml",
        "src/main.rs",
        "pmcp.toml",
        "workbook/tax-calc.xlsx",
    ] {
        assert!(
            crate_dir.join(rel).is_file(),
            "scaffold must emit {rel} at {}",
            crate_dir.display()
        );
    }

    // The embedded bundle and its five contract members.
    let bundle = crate_dir.join("bundle").join("tax-calc@1.1.0");
    for member in BUNDLE_MEMBERS {
        assert!(
            bundle.join(member).is_file(),
            "scaffold must emit bundle/tax-calc@1.1.0/{member}"
        );
    }

    // The emitted Cargo.toml must be purity-safe (T-95-06): default-features off,
    // workbook-embedded + http, never code-mode (stripping comment lines so the
    // prose that legitimately mentions code-mode does not trip the check).
    let cargo = std::fs::read_to_string(crate_dir.join("Cargo.toml")).expect("read Cargo.toml");
    let effective: String = cargo
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        effective.contains("default-features = false"),
        "emitted Cargo.toml must disable toolkit default features: {cargo}"
    );
    assert!(
        effective.contains(r#"features = ["workbook-embedded", "http"]"#),
        "emitted Cargo.toml must use workbook-embedded + http: {cargo}"
    );
    assert!(
        !effective.contains("code-mode"),
        "emitted Cargo.toml must NOT enable code-mode (purity gate): {cargo}"
    );

    // The emitted main.rs must wire EmbeddedSource over the local bundle path and
    // import SOLELY from the toolkit (D-11).
    let main_rs = std::fs::read_to_string(crate_dir.join("src/main.rs")).expect("read main.rs");
    assert!(
        main_rs.contains("EmbeddedSource") && main_rs.contains("try_with_workbook_bundle"),
        "emitted main.rs must wire EmbeddedSource via try_with_workbook_bundle"
    );
    assert!(
        main_rs.contains("pmcp_server_toolkit::workbook"),
        "emitted main.rs must import from pmcp_server_toolkit::workbook (D-11)"
    );
}

#[test]
fn scaffold_rejects_path_traversal_name() {
    // T-96-04: the path-traversal guard runs FIRST. A `..`-bearing name must be
    // rejected (validate_crate_name) before any fs::write.
    let tmp = tempfile::tempdir().expect("create tempdir");
    let status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "workbook-server", "../evil"])
        .current_dir(tmp.path())
        .status()
        .expect("spawn cargo-pmcp");
    assert!(
        !status.success(),
        "`new --kind workbook-server ../evil` must be REJECTED (path-traversal guard, T-96-04)"
    );
}

/// Append a `[patch.crates-io]` block covering the full transitive closure of
/// unpublished workspace crates the workbook scaffold's dependency graph touches,
/// so `cargo check` in the tempdir resolves without hitting crates.io.
fn append_workbook_patch(crate_dir: &Path) {
    let root = repo_root();
    let p = |rel: &str| root.join(rel).to_string_lossy().replace('\\', "/");
    let patch = format!(
        "\n\
         # Injected by tests/workbook_scaffold.rs so the unpublished workspace\n\
         # crates resolve against their in-repo paths (the workbook scaffold's\n\
         # transitive closure: toolkit + workbook-runtime + workbook-dialect).\n\
         [patch.crates-io]\n\
         pmcp = {{ path = \"{}\" }}\n\
         pmcp-server-toolkit = {{ path = \"{}\" }}\n\
         pmcp-workbook-runtime = {{ path = \"{}\" }}\n\
         pmcp-workbook-dialect = {{ path = \"{}\" }}\n\
         pmcp-widget-utils = {{ path = \"{}\" }}\n",
        p("."),
        p("crates/pmcp-server-toolkit"),
        p("crates/pmcp-workbook-runtime"),
        p("crates/pmcp-workbook-dialect"),
        p("crates/pmcp-widget-utils"),
    );
    let manifest = crate_dir.join("Cargo.toml");
    let mut content = std::fs::read_to_string(&manifest).expect("read scaffolded Cargo.toml");
    content.push_str(&patch);
    std::fs::write(&manifest, content).expect("write patched Cargo.toml");
}

/// Scaffold-build smoke: prove the emitted crate actually COMPILES. It is both
/// `#[ignore]`d and env-gated because it compiles the unpublished toolkit tree
/// (slow; a network-less CI without the in-repo path overrides cannot resolve
/// the pinned crates). Run with `PMCP_RUN_SCAFFOLD_BUILD=1 ... -- --ignored`.
#[test]
#[ignore = "compiles the unpublished toolkit tree; run with PMCP_RUN_SCAFFOLD_BUILD=1"]
fn scaffold_crate_cargo_check_compiles() {
    if std::env::var("PMCP_RUN_SCAFFOLD_BUILD").is_err() {
        eprintln!("skipping: set PMCP_RUN_SCAFFOLD_BUILD=1 to run the scaffold-build smoke");
        return;
    }
    let tmp = tempfile::tempdir().expect("create tempdir");
    let status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "workbook-server", SCAFFOLD_NAME])
        .current_dir(tmp.path())
        .status()
        .expect("scaffold");
    assert!(status.success(), "scaffold must succeed");

    let crate_dir = tmp.path().join(SCAFFOLD_NAME);
    append_workbook_patch(&crate_dir);

    let check = Command::new(env!("CARGO"))
        .args(["check"])
        .current_dir(&crate_dir)
        .status()
        .expect("spawn cargo check in the scaffolded crate");
    assert!(
        check.success(),
        "`cargo check` inside the scaffolded crate must compile (exit {check:?})"
    );
}

/// Packaging smoke: prove the embedded assets ship inside the published crate.
/// `#[ignore]`d because `cargo package --list` is comparatively slow and resolves
/// the workspace; the assertion is on the file LIST, not a real publish.
#[test]
#[ignore = "runs cargo package --list over the workspace; run with --ignored"]
fn embedded_assets_appear_in_cargo_package_list() {
    let output = Command::new(env!("CARGO"))
        .args(["package", "--list", "--allow-dirty", "-p", "cargo-pmcp"])
        .current_dir(repo_root())
        .output()
        .expect("spawn cargo package --list");
    assert!(
        output.status.success(),
        "`cargo package --list -p cargo-pmcp` must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let list = String::from_utf8_lossy(&output.stdout);

    // The embedded bundle members + the source .xlsx must all appear in the
    // package file list (proving they survive `cargo publish`, T-96-06b).
    let required = [
        "src/templates/workbook_bundle/tax-calc.xlsx",
        "src/templates/workbook_bundle/tax-calc@1.1.0/BUNDLE.lock",
        "src/templates/workbook_bundle/tax-calc@1.1.0/manifest.json",
    ];
    for asset in required {
        assert!(
            list.lines().any(|l| l.trim() == asset),
            "embedded asset `{asset}` missing from cargo package --list (would break the \
             published scaffold):\n{list}"
        );
    }
}
