//! Shared workbook target-resolution + lint-phase helpers for `compile` / `emit`.
//!
//! `compile` and `emit` resolve their target set identically (a bare PATH, a
//! `pmcp.toml` bundle-id, or NOTHING → all declared entries) and run the SAME
//! lint pass before any write. This module is the ONE canonical copy of that
//! shared logic; both handlers import [`Target`], [`resolve_targets`], and
//! [`run_lint_phase`] from here rather than each carrying a byte-for-byte clone.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::config::{PmcpToml, WorkbookEntry};
use super::lint::{lint_exit_code, print_lint_report};
use super::EXIT_ERROR;

/// A single resolved target: where to read the workbook, what workflow / bundle
/// id it compiles or emits to, and where to write the bundle.
#[derive(Debug, Clone)]
pub(super) struct Target {
    /// The source `.xlsx` path.
    pub(super) path: PathBuf,
    /// The workflow / bundle id (the `{bundle_id}@{version}/` dir name).
    pub(super) workflow: String,
    /// The output root the `{bundle_id}@{version}/` dir is written under.
    pub(super) out_root: PathBuf,
}

/// Resolve the requested target set (D-03 / D-05):
/// - a bare PATH that IS a file → one ad-hoc target (`--workflow` required);
/// - otherwise a bundle-id → resolve through `pmcp.toml`;
/// - no argument → every declared `pmcp.toml` entry (compile-all / emit-all).
pub(super) fn resolve_targets(
    bundle_id_or_path: Option<&str>,
    workflow: Option<&str>,
    out: Option<&Path>,
    project_root: &Path,
) -> Result<Vec<Target>> {
    match bundle_id_or_path {
        Some(arg) if Path::new(arg).is_file() => {
            let workflow = workflow
                .map(str::to_string)
                .context("a bare workbook path requires --workflow <id>")?;
            let path = PathBuf::from(arg);
            let out_root = out
                .map(Path::to_path_buf)
                .unwrap_or_else(|| default_out_root(&path));
            Ok(vec![Target {
                path,
                workflow,
                out_root,
            }])
        },
        Some(bundle_id) => {
            let toml = load_required_toml(project_root)?;
            let entry = toml.resolve(bundle_id)?;
            Ok(vec![target_from_entry(entry, project_root, out)])
        },
        None => {
            let toml = load_required_toml(project_root)?;
            Ok(toml
                .all_entries()
                .iter()
                .map(|entry| target_from_entry(entry, project_root, out))
                .collect())
        },
    }
}

/// Load `pmcp.toml`, erroring when it is ABSENT (a bundle-id / all request needs
/// it — only a bare-path compile/emit works without a toml).
fn load_required_toml(project_root: &Path) -> Result<PmcpToml> {
    PmcpToml::load(project_root)?
        .context("no pmcp.toml found: declare workbooks or pass a workbook path")
}

/// Build a [`Target`] from a `pmcp.toml` [`WorkbookEntry`], resolving its
/// project-root-relative paths and honouring a `--out` override.
fn target_from_entry(entry: &WorkbookEntry, project_root: &Path, out: Option<&Path>) -> Target {
    let out_root = match out {
        Some(o) => o.to_path_buf(),
        None => project_root.join(&entry.out_dir),
    };
    Target {
        path: project_root.join(&entry.path),
        workflow: entry.bundle_id.clone(),
        out_root,
    }
}

/// The default out-root for a bare-path compile/emit with no `--out` and no toml:
/// the workbook's parent directory (the bundle lands beside the workbook).
fn default_out_root(workbook: &Path) -> PathBuf {
    workbook
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Run the lint pass (REUSING Plan-02 [`print_lint_report`] / [`lint_exit_code`] —
/// no re-rendering). Returns `Some(EXIT_ERROR)` (short-circuit, do NOT proceed)
/// when the report has errors, else `None` (proceed to compile / emit).
pub(super) fn run_lint_phase(
    target: &Target,
    format: &str,
    not_quiet: bool,
) -> Result<Option<i32>> {
    let (map, _ingest_findings) = pmcp_workbook_compiler::ingest::ingest(&target.path)
        .with_context(|| format!("failed to ingest workbook {}", target.path.display()))?;
    let src = pmcp_workbook_compiler::WorkbookCellSource::new(&map);
    let report = pmcp_workbook_compiler::dialect::linter::lint(
        &src,
        &pmcp_workbook_compiler::DialectRules::default(),
    );
    print_lint_report(&report, format, not_quiet)?;
    if lint_exit_code(&report) == EXIT_ERROR {
        return Ok(Some(EXIT_ERROR));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_out_root_is_the_workbook_parent() {
        let root = default_out_root(Path::new("/proj/wb/quote.xlsx"));
        assert_eq!(root, PathBuf::from("/proj/wb"));
        // A bare file name (no parent component) falls back to the cwd.
        assert_eq!(default_out_root(Path::new("quote.xlsx")), PathBuf::from(""));
    }

    #[test]
    fn target_from_entry_resolves_relative_paths_under_root() {
        let entry = WorkbookEntry {
            path: PathBuf::from("workbooks/quote.xlsx"),
            bundle_id: "quote".to_string(),
            out_dir: PathBuf::from("dist/quote"),
        };
        let root = Path::new("/project");
        let target = target_from_entry(&entry, root, None);
        assert_eq!(target.path, PathBuf::from("/project/workbooks/quote.xlsx"));
        assert_eq!(target.out_root, PathBuf::from("/project/dist/quote"));
        assert_eq!(target.workflow, "quote");
    }

    #[test]
    fn target_from_entry_honours_out_override() {
        let entry = WorkbookEntry {
            path: PathBuf::from("workbooks/quote.xlsx"),
            bundle_id: "quote".to_string(),
            out_dir: PathBuf::from("dist/quote"),
        };
        let root = Path::new("/project");
        let override_out = Path::new("/tmp/elsewhere");
        let target = target_from_entry(&entry, root, Some(override_out));
        assert_eq!(target.out_root, PathBuf::from("/tmp/elsewhere"));
    }

    #[test]
    fn resolve_targets_bare_path_requires_workflow() {
        // A bare PATH with NO --workflow is rejected (the bundle-id supplies the
        // workflow; an ad-hoc path must name it).
        let tmp = tempfile::tempdir().expect("tempdir");
        let wb = tmp.path().join("quote.xlsx");
        std::fs::write(&wb, b"not-a-real-xlsx").expect("write fixture file");
        let err = resolve_targets(Some(&wb.to_string_lossy()), None, None, tmp.path())
            .expect_err("bare path needs --workflow");
        assert!(
            err.to_string().contains("--workflow"),
            "names the missing flag: {err}"
        );
    }

    #[test]
    fn resolve_targets_visits_every_declared_entry() {
        // No-argument (compile-all / emit-all) resolves EVERY declared pmcp.toml
        // entry — the continue-on-error loop then attempts each (concern I).
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("pmcp.toml"),
            r#"
[[workbook]]
path = "a.xlsx"
bundle_id = "a"
out_dir = "dist/a"

[[workbook]]
path = "b.xlsx"
bundle_id = "b"
out_dir = "dist/b"
"#,
        )
        .expect("write pmcp.toml");
        let targets =
            resolve_targets(None, None, None, tmp.path()).expect("resolve compile-all/emit-all");
        assert_eq!(targets.len(), 2, "no-argument visits BOTH declared entries");
        assert_eq!(targets[0].workflow, "a");
        assert_eq!(targets[1].workflow, "b");
        // Even if the FIRST workbook would error (the file is absent), the SECOND is
        // still a resolved target the loop attempts (continue-on-error).
        assert_eq!(targets[1].path, tmp.path().join("b.xlsx"));
    }

    #[test]
    fn resolve_targets_unknown_bundle_id_errors() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("pmcp.toml"),
            "[[workbook]]\npath=\"a.xlsx\"\nbundle_id=\"a\"\nout_dir=\"dist/a\"\n",
        )
        .expect("write pmcp.toml");
        let err = resolve_targets(Some("missing"), None, None, tmp.path())
            .expect_err("unknown id must error");
        assert!(err.to_string().contains("missing"), "names the id: {err}");
    }
}
