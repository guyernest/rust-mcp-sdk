//! `pmcp-package` version-pin tripwire (PKG-04 / D-03), Phase 121 Plan 01.
//!
//! `pmcp-openapi-server` MUST pin `pmcp-package` at the caret `"0.2"` string
//! (NOT `=0.2.0`, NOT `0.2.0`) so the PKG-04 round-trip E2E keeps resolving
//! against the 0.2 line it was written against. This test parses
//! `pmcp-openapi-server`'s OWN `Cargo.toml` and asserts the exact
//! version-req string, mirroring the `const + include_str! + assert_eq!`
//! drift-test pattern used by `cargo-pmcp/tests/pmcp_package_pin.rs`.
//!
//! # What this tripwire does NOT cover
//!
//! It checks exactly ONE `pmcp-package` version emitter: this crate's own
//! `[dev-dependencies]` entry. It is deliberately narrow, and a reader must not
//! assume one file guards every in-repo pin:
//!
//! - `cargo-pmcp`'s own `[dependencies].pmcp-package` entry is covered by the
//!   SIBLING tripwire at `cargo-pmcp/tests/pmcp_package_pin.rs`. This file
//!   cannot see it, and that file cannot see this one.
//! - `cargo-pmcp/src/templates/agent.rs` EMITS a `pmcp-package` dependency line
//!   into every project created by `cargo pmcp agent new`. A stale requirement
//!   there is invisible to `cargo build` — the workspace resolves green while
//!   the scaffold ships broken. That emitter has its own drift test.
//! - `crates/pmcp-agent`, `crates/pmcp-team-servers` and
//!   `crates/pmcp-cfn-renderer` each carry their own manifest requirement.
//!   Those DO fail `cargo build` if left behind (the workspace cannot resolve),
//!   so the compiler is their tripwire.
//!
//! A future bump must move all of them together. Phase 124 still owns the
//! release-time half — publish order, the release ledger, and the crates.io
//! tag — plus the out-of-repo pmcp.run pin check.
//!
//! Note that this crate reads the `[dev-dependencies]` table, NOT
//! `[dependencies]`: the round-trip E2E is test-only, and nothing published
//! depends on `pmcp-package` from here. A copy of the sibling tripwire left
//! reading `dependencies` would panic on a CORRECT manifest.

/// `pmcp-openapi-server`'s own manifest, embedded at compile time
/// (`../Cargo.toml` is resolved relative to THIS test file, i.e.
/// `crates/pmcp-openapi-server/Cargo.toml`).
const OPENAPI_SERVER_CARGO_TOML: &str = include_str!("../Cargo.toml");

/// The exact version-requirement string `pmcp-openapi-server` must declare.
const EXPECTED_PIN: &str = "0.2";

/// Extract the version-requirement string for an entry in the named dependency
/// table, handling BOTH the `name = "x.y"` shorthand and the
/// `name = { version = "x.y", .. }` table form (this crate uses the table form
/// with a `path`).
///
/// `table` is a parameter rather than a hardcoded `"dependencies"` because this
/// crate declares `pmcp-package` under `[dev-dependencies]`. Every panic and
/// assert message below names the table actually read, so a future misuse
/// reports the table it looked in instead of implying `[dependencies]`.
fn dependency_version_req(manifest: &toml::Value, table: &str, name: &str) -> String {
    let dep = manifest
        .get(table)
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("pmcp-openapi-server Cargo.toml has no [{table}].{name}"));
    match dep {
        // `pmcp-package = "0.2"` shorthand.
        toml::Value::String(s) => s.clone(),
        // `pmcp-package = { version = "0.2", path = "..." }` table form.
        toml::Value::Table(_) => dep
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("[{table}].{name} table has no `version` key"))
            .to_string(),
        other => panic!("[{table}].{name} has unexpected shape: {other:?}"),
    }
}

/// PKG-04 / D-03: the `pmcp-package` pin must be exactly the caret `"0.2"`.
///
/// The caret matters. `=0.2.0` would refuse every later 0.2.x patch, and a
/// fully-qualified `0.2.0` reads like an exact pin to a human even though Cargo
/// treats it as a caret — both forms are rejected here so the manifest says
/// what it means.
#[test]
fn pmcp_package_pin_is_the_expected_caret_line() {
    let manifest: toml::Value =
        toml::from_str(OPENAPI_SERVER_CARGO_TOML).expect("parse pmcp-openapi-server Cargo.toml");
    let req = dependency_version_req(&manifest, "dev-dependencies", "pmcp-package");
    assert_eq!(
        req, EXPECTED_PIN,
        "pmcp-package pin in [dev-dependencies] must be the caret \"{EXPECTED_PIN}\" \
         (PKG-04 / D-03); do NOT use `=0.2.0` or a fully-qualified `0.2.0`"
    );
}
