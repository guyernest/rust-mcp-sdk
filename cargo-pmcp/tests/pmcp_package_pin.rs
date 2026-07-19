//! Phase 110 Plan 05 — `pmcp-package` version-pin tripwire (CLI-04 / D-04b).
//!
//! cargo-pmcp MUST pin `pmcp-package` at the caret `"0.1"` string (NOT `=0.1.0`,
//! NOT `0.1.0`) so a portable `.pmcp` package produced by any published
//! `pmcp-package = 0.1.x` stays inspectable by this CLI. This test parses
//! cargo-pmcp's OWN `Cargo.toml` and asserts the exact version-req string,
//! mirroring the `const + include_str! + assert_eq!` drift-test pattern used by
//! `src/templates/workbook_server.rs`.

/// cargo-pmcp's own manifest, embedded at compile time (`../Cargo.toml` is
/// resolved relative to THIS test file, i.e. `cargo-pmcp/Cargo.toml`).
const CARGO_PMCP_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// Extract the version-requirement string for a `[dependencies]` entry, handling
/// BOTH the `name = "x.y"` shorthand and the `name = { version = "x.y", .. }`
/// table form (cargo-pmcp uses the table form with a `path`).
fn dependency_version_req(manifest: &toml::Value, name: &str) -> String {
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("cargo-pmcp Cargo.toml has no [dependencies].{name}"));
    match dep {
        // `pmcp-package = "0.1"` shorthand.
        toml::Value::String(s) => s.clone(),
        // `pmcp-package = { version = "0.1", path = "..." }` table form.
        toml::Value::Table(_) => dep
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("[dependencies].{name} table has no `version` key"))
            .to_string(),
        other => panic!("[dependencies].{name} has unexpected shape: {other:?}"),
    }
}

/// CLI-04 / D-04b: the `pmcp-package` pin must be exactly the caret `"0.1"`.
#[test]
fn pmcp_package_pin_is_caret_zero_one() {
    let manifest: toml::Value =
        toml::from_str(CARGO_PMCP_CARGO_TOML).expect("parse cargo-pmcp Cargo.toml");
    let req = dependency_version_req(&manifest, "pmcp-package");
    assert_eq!(
        req, "0.1",
        "pmcp-package pin must be caret \"0.1\" (CLI-04 / D-04b); \
         do NOT use `=0.1.0` or a fully-qualified `0.1.0`"
    );
}
