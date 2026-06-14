//! Server assembly: `--bundle-dir` (+ optional `--bundle-id`) → built
//! [`pmcp::Server`].
//!
//! This is the ONE novel seam in the Shape A workbook binary. Everything else is
//! a field-for-field re-skin of `pmcp-sql-server`; here the binary turns the
//! operator's `--bundle-dir` into a [`LocalDirSource`], optionally asserts the
//! loaded bundle's identity against `--bundle-id` (D-01, fail-closed BEFORE any
//! tool is registered), and registers all five workbook tools + the `workbook://`
//! render resource through the toolkit's
//! [`pmcp_server_toolkit::workbook::WorkbookBuilderExt::try_with_workbook_bundle`].
//!
//! # Boot integrity (T-95-01)
//!
//! `try_with_workbook_bundle` recomputes the `BUNDLE.lock` hashes BEFORE
//! registering any tool; a tampered / incomplete bundle returns `Err` mapped to
//! [`RunError::Bundle`] and the server never boots on an unverified bundle.
//!
//! # Dependency posture (D-11)
//!
//! Every bundle symbol is imported from the TOOLKIT
//! (`pmcp_server_toolkit::workbook`) — the binary NEVER names
//! `pmcp-workbook-runtime` directly.

use pmcp::Server;
// SINGLE toolkit import of the bundle boot surface (D-11): LocalDirSource, the
// builder extension, and the fail-closed loader all resolve through the toolkit
// re-export — `pmcp-workbook-runtime` is never named here.
use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource, WorkbookBuilderExt};

use crate::{Args, RunError};

/// Assemble a [`pmcp::Server`] from `--bundle-dir` (+ optional `--bundle-id`).
///
/// 1. `LocalDirSource::new(--bundle-dir)` — one source = one `bundle@version`,
///    the version implicit in the path (D-01).
/// 2. When `--bundle-id` is `Some`: pre-load via the toolkit-re-exported
///    `load_bundle` (the same fail-closed `BUNDLE.lock`-recompute gate), read
///    `bundle.stamp.bundle_id`, and on mismatch return
///    [`RunError::BundleIdMismatch`] BEFORE registering any tool.
/// 3. `Server::builder().try_with_workbook_bundle(&source)` registers all five
///    tools + the `workbook://` resource, then `.build()`.
///
/// # Errors
///
/// - [`RunError::Bundle`] when the bundle fails to load / integrity-verify
///   (fail-closed boot gate).
/// - [`RunError::BundleIdMismatch`] when `--bundle-id` does not match the loaded
///   bundle's identity.
/// - [`RunError::Serve`] when the final `pmcp::Server` build fails.
pub fn build_server(args: &Args) -> Result<Server, RunError> {
    let source = LocalDirSource::new(&args.bundle_dir);

    // --bundle-id assertion (D-01), fail-closed BEFORE assembly.
    //
    // Accepted double-read (NOT a TOCTOU): when --bundle-id is supplied the
    // bundle is read TWICE — once here by `load_bundle` for the id check, and
    // once below by `try_with_workbook_bundle`, which (VERIFIED workbook/mod.rs
    // :230) calls `load_bundle` again internally and exposes no preloaded-bundle
    // entrypoint. The SECURITY BOUNDARY is the SECOND (assembly) load: it
    // INDEPENDENTLY recomputes and re-verifies the BUNDLE.lock hashes fail-closed,
    // so a bundle swapped between the two loads is rejected at assembly unless the
    // swap preserves every lock hash (i.e. is the same verified bundle). The id
    // check is an operator-convenience guard layered on top of the already
    // fail-closed integrity load, not the integrity boundary itself. A single-load
    // fix would require a new toolkit entrypoint accepting an already-loaded
    // WorkbookBundle (Phase 92 scope) — deliberately OUT OF SCOPE here.
    if let Some(expected) = &args.bundle_id {
        // The re-exported `load_bundle` yields a `BundleLoadError`; route it
        // through the toolkit's `ToolkitError` (which has `From<BundleLoadError>`)
        // so the pre-load fails closed via the same `RunError::Bundle` variant the
        // assembly load uses.
        let bundle = load_bundle(&source).map_err(|e| RunError::Bundle(e.into()))?;
        let actual = &bundle.stamp.bundle_id;
        if actual != expected {
            return Err(RunError::BundleIdMismatch {
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
    }

    let server = Server::builder()
        .name("pmcp-workbook-server")
        .version(env!("CARGO_PKG_VERSION"))
        .try_with_workbook_bundle(&source)?
        .build()
        .map_err(RunError::Serve)?;

    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::build_server;
    use crate::{Args, RunError};
    use std::path::PathBuf;

    /// Path to the committed synthetic golden bundle (read-only; reuse, do NOT
    /// regenerate — D-05). Resolved from `CARGO_MANIFEST_DIR` so the test is
    /// invariant to the cwd.
    fn golden_bundle_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0")
    }

    fn args_for(bundle_dir: PathBuf, bundle_id: Option<&str>) -> Args {
        Args {
            bundle_dir,
            bundle_id: bundle_id.map(ToString::to_string),
            http: "127.0.0.1:0".to_string(),
        }
    }

    #[test]
    fn build_server_from_golden_registers_five_tools() {
        let args = args_for(golden_bundle_dir(), None);
        let server = build_server(&args).expect("golden bundle assembles a server");

        // The stable public Server::get_tool inspection API (src/server/mod.rs:515,
        // also used by pmcp-sql-server's tests/assemble.rs) confirms all five
        // workbook tools registered.
        for name in [
            "calculate",
            "explain",
            "get_manifest",
            "diff_version",
            "render_workbook",
        ] {
            assert!(
                server.get_tool(name).is_some(),
                "built server must expose the '{name}' tool"
            );
        }
    }

    #[test]
    fn matching_bundle_id_succeeds() {
        // The golden bundle's BUNDLE.lock bundle_id is "tax-calc".
        let args = args_for(golden_bundle_dir(), Some("tax-calc"));
        let server = build_server(&args).expect("matching --bundle-id assembles a server");
        assert!(server.get_tool("calculate").is_some());
    }

    #[test]
    fn mismatched_bundle_id_returns_bundle_id_mismatch() {
        let args = args_for(golden_bundle_dir(), Some("not-tax-calc"));
        let err = build_server(&args).expect_err("mismatched --bundle-id must fail closed");
        match err {
            RunError::BundleIdMismatch { expected, actual } => {
                assert_eq!(expected, "not-tax-calc");
                assert_eq!(actual, "tax-calc", "actual is the loaded BUNDLE.lock id");
            },
            other => panic!("expected RunError::BundleIdMismatch, got {other:?}"),
        }
    }

    #[test]
    fn nonexistent_bundle_dir_fails_closed() {
        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("does-not-exist@9.9.9");
        let args = args_for(missing, None);
        let err = build_server(&args).expect_err("a missing bundle dir must fail closed");
        // A nonexistent / tampered bundle maps to the bundle-load variant, never a
        // partial server.
        assert!(
            matches!(err, RunError::Bundle(_)),
            "expected RunError::Bundle for a missing bundle, got {err:?}"
        );
    }

    #[test]
    fn mismatched_id_on_missing_bundle_still_fails_closed_at_load() {
        // With --bundle-id set AND a missing dir, the pre-load itself fails closed
        // (RunError::Bundle) before the id comparison — the integrity load is the
        // boundary.
        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("does-not-exist@9.9.9");
        let args = args_for(missing, Some("tax-calc"));
        let err = build_server(&args).expect_err("missing bundle with --bundle-id fails closed");
        assert!(
            matches!(err, RunError::Bundle(_)),
            "the pre-load fails closed before the id check, got {err:?}"
        );
    }
}
