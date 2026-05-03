//! Phase 79 Wave 1: widget pre-build schema types.
//!
//! Defines the `[[widgets]]` block that operators add to `.pmcp/deploy.toml`
//! plus the lockfile-driven `PackageManager` detection that the Wave 2 build
//! orchestrator (Plan 79-02) consumes.
//!
//! ## Why this module exists
//!
//! Cost-coach's Failure Mode A (proven in production 2026-04-23) was: developer
//! edited `widget/cost-over-time.html`, ran `cargo pmcp deploy`, and shipped the
//! OLD widget because nobody ran `npm run build` first. Wave 2's orchestrator
//! consumes `WidgetsConfig` to drive an automatic widget build before
//! `cargo build --release`; this module lays the schema contract.
//!
//! ## Phase 76 IamConfig precedent (mirrored here)
//!
//! [`WidgetsConfig::is_empty`] powers the `#[serde(skip_serializing_if)]` guard
//! on `DeployConfig::widgets` so pre-existing `.pmcp/deploy.toml` files round-
//! trip byte-identically when no `[[widgets]]` block is present.
//!
//! ## Revision-3 supersession (Codex MEDIUM)
//!
//! [`WidgetConfig::build`] and [`WidgetConfig::install`] are `Option<Vec<String>>`
//! argv arrays — NOT `Option<String>` whitespace-split shell strings. The
//! pre-revision-3 string form silently broke quoting on inputs like
//! `"npm run --silent build"` (the `--silent` flag would attach to the wrong
//! argument). The argv-array form is unambiguous. Migration path for users:
//! replace `build = "npm run build"` with `build = ["npm", "run", "build"]`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level container for `[[widgets]]` entries in `.pmcp/deploy.toml`.
///
/// `#[serde(transparent)]` means `WidgetsConfig` deserialises directly from a
/// TOML sequence — so `DeployConfig.widgets: WidgetsConfig` reads the
/// top-level `[[widgets]]` array-of-tables (operator-friendly shape), NOT a
/// nested `[widgets] widgets = [...]` map.
///
/// Empty by default — [`Self::is_empty`] powers the
/// `#[serde(skip_serializing_if)]` guard on `DeployConfig::widgets` to preserve
/// byte-identity round-trip for files lacking any `[[widgets]]` block (Phase 76
/// `IamConfig` D-05 contract).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WidgetsConfig {
    /// One entry per `[[widgets]]` block. The `transparent` derive collapses
    /// this newtype into a plain TOML sequence at the wire layer.
    pub widgets: Vec<WidgetConfig>,
}

impl WidgetsConfig {
    /// Returns `true` when no `[[widgets]]` blocks are configured. Mirrors the
    /// `IamConfig::is_empty` D-05 helper.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}

/// One `[[widgets]]` block from `.pmcp/deploy.toml`.
///
/// Per `79-CONTEXT.md` "[[widgets]] config" — `embedded_in_crates` is the
/// EXPLICIT source of truth for cache invalidation. Auto-detection via
/// `grep include_str!` is brittle (concat!/macros/computed paths defeat it)
/// and is demoted to a `cargo pmcp doctor` HINT only.
///
/// REVISION 3 (Codex MEDIUM): `build` and `install` are argv arrays
/// (`Option<Vec<String>>`), NOT whitespace-split shell strings. The previous
/// `Option<String>` form broke quoting on inputs like `"npm run --silent build"`.
/// To migrate from the string form, replace `build = "npm run build"` with
/// `build = ["npm", "run", "build"]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Workspace-root-relative path to the widget source directory.
    /// Reject `..` segments via [`Self::validate`] (T-79-02 mitigation).
    pub path: String,

    /// Explicit build command override (argv array). Default is auto-detected
    /// from the lockfile per [`PackageManager::build_args`]. Accepts ONLY array
    /// form — string form is rejected by serde with an actionable error
    /// directing the user to migrate to the array form (avoids whitespace-split
    /// quoting bugs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Vec<String>>,

    /// Explicit install command override (argv array). Default is auto-detected
    /// from the lockfile per [`PackageManager::install_args`]. Same array-only
    /// contract as [`Self::build`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install: Option<Vec<String>>,

    /// Output dir relative to [`Self::path`]. Defaults to `"dist"`.
    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// REQUIRED when present in TOML: which workspace bin crates `include_str!`
    /// files from this widget. Source of truth for cache invalidation in
    /// Plan 79-02. Defaults to an empty vec (the convention path synthesizes
    /// "all bin crates" — see Plan 79-02).
    #[serde(default)]
    pub embedded_in_crates: Vec<String>,
}

fn default_output_dir() -> String {
    "dist".to_string()
}

impl WidgetConfig {
    /// T-79-02 mitigation — reject `..` segments to prevent path-traversal
    /// escape from the workspace root. Also rejects empty argv vectors so
    /// Wave 2's `Command::new(argv[0])` cannot panic on `argv[0]`.
    ///
    /// Called by the orchestrator (Plan 79-02) before any FS work. NOT invoked
    /// by serde — the schema accepts any string, validation is a separate
    /// concern (mirroring Phase 76 `iam::validate`).
    ///
    /// # Errors
    /// Returns `Err` when the path contains `..`, when the build argv is
    /// empty, or when the install argv is empty.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.path.split('/').any(|seg| seg == "..") {
            anyhow::bail!(
                "widget path '{}' contains '..' (path traversal) — only paths under workspace root are allowed",
                self.path
            );
        }
        if let Some(b) = &self.build {
            if b.is_empty() {
                anyhow::bail!("widget build argv is empty — provide at least one element");
            }
        }
        if let Some(i) = &self.install {
            if i.is_empty() {
                anyhow::bail!("widget install argv is empty — provide at least one element");
            }
        }
        Ok(())
    }

    /// Compute absolute paths from the workspace root for use by
    /// `Command::current_dir` and `cargo:rerun-if-changed` emission.
    #[must_use]
    pub fn resolve_paths(&self, workspace_root: &Path) -> ResolvedPaths {
        let path = workspace_root.join(&self.path);
        let absolute_output_dir = path.join(&self.output_dir);
        ResolvedPaths {
            path,
            absolute_output_dir,
        }
    }
}

/// Resolved absolute paths for a widget. Returned by
/// [`WidgetConfig::resolve_paths`] for downstream `Command::current_dir` and
/// `cargo:rerun-if-changed` consumers.
#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    /// Absolute path to the widget source directory (workspace root + `path`).
    pub path: PathBuf,
    /// Absolute path to the build output directory (`path` + `output_dir`).
    pub absolute_output_dir: PathBuf,
}

/// Lockfile-determined package manager. Priority order locked by
/// `79-CONTEXT.md` "Convention search": `bun > pnpm > yarn > npm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    /// Detected via `bun.lockb`.
    Bun,
    /// Detected via `pnpm-lock.yaml`.
    Pnpm,
    /// Detected via `yarn.lock`.
    Yarn,
    /// Detected via `package-lock.json`, or fallback when no lockfile is found.
    Npm,
}

impl PackageManager {
    /// Returns the highest-priority PM whose lockfile is present in `dir`,
    /// falling back to [`PackageManager::Npm`] when no lockfile is found.
    ///
    /// Priority order (locked by `79-CONTEXT.md`): bun > pnpm > yarn > npm.
    #[must_use]
    pub fn detect_from_dir(dir: &Path) -> Self {
        if dir.join("bun.lockb").exists() {
            return Self::Bun;
        }
        if dir.join("pnpm-lock.yaml").exists() {
            return Self::Pnpm;
        }
        if dir.join("yarn.lock").exists() {
            return Self::Yarn;
        }
        if dir.join("package-lock.json").exists() {
            return Self::Npm;
        }
        Self::Npm
    }

    /// Returns the install argv (`(program, args)`) for this package manager.
    #[must_use]
    pub fn install_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Bun => ("bun", &["install"]),
            Self::Pnpm => ("pnpm", &["install"]),
            Self::Yarn => ("yarn", &["install"]),
            Self::Npm => ("npm", &["install"]),
        }
    }

    /// Returns the build argv (`(program, args)`) for this package manager.
    ///
    /// Note: `yarn` omits the `run` subcommand (`yarn build`, not
    /// `yarn run build`) per `79-CONTEXT.md`'s convention list. All other
    /// managers use `<pm> run build`.
    #[must_use]
    pub fn build_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Bun => ("bun", &["run", "build"]),
            Self::Pnpm => ("pnpm", &["run", "build"]),
            Self::Yarn => ("yarn", &["build"]),
            Self::Npm => ("npm", &["run", "build"]),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests covering `<behavior>` Tests 1.1..1.8 of Plan 79-01.
    //!
    //! Many tests parse via a local `Wrapper { widgets: WidgetsConfig }` so
    //! the schema mirrors the production shape on `DeployConfig.widgets`. The
    //! `WidgetsConfig` newtype is `#[serde(transparent)]` over a sequence,
    //! so it cannot deserialize from an empty TOML document directly — only
    //! through a parent struct that supplies the `widgets` key.
    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
    struct Wrapper {
        #[serde(default)]
        widgets: WidgetsConfig,
    }

    /// Test 1.1 (round_trip_no_widgets_byte_identical): default `WidgetsConfig`
    /// reports empty AND a manually-deserialized empty wrapper also reports
    /// empty. The serialized form of an empty wrapper round-trips losslessly
    /// (the byte-identity guarantee at the `DeployConfig` level is exercised
    /// separately in `tests/widgets_config.rs` via the
    /// `skip_serializing_if = "WidgetsConfig::is_empty"` guard on
    /// `DeployConfig::widgets`).
    #[test]
    fn round_trip_no_widgets_byte_identical() {
        let cfg = WidgetsConfig::default();
        assert!(cfg.is_empty(), "default WidgetsConfig must be empty");

        let parsed: Wrapper = toml::from_str("").expect("empty TOML parses");
        assert!(
            parsed.widgets.is_empty(),
            "empty TOML must produce empty WidgetsConfig"
        );

        // Round-trip serialise → re-parse → still empty.
        let serialized = toml::to_string(&parsed).expect("serializes");
        let reparsed: Wrapper = toml::from_str(&serialized).expect("serialized empty re-parses");
        assert!(
            reparsed.widgets.is_empty(),
            "empty wrapper must round-trip to empty — got serialized:\n{serialized}"
        );
    }

    /// Test 1.2 (parses_explicit_widgets_block): the on-disk shape an operator
    /// would write — one `[[widgets]]` block with an explicit
    /// `embedded_in_crates`. Confirms defaults populate (`output_dir = "dist"`,
    /// `build = None`, `install = None`).
    #[test]
    fn parses_explicit_widgets_block() {
        let toml_str = r#"
[[widgets]]
path = "widget"
embedded_in_crates = ["cost-coach-lambda"]
"#;
        let parsed: Wrapper = toml::from_str(toml_str).expect("parses");
        assert_eq!(parsed.widgets.widgets.len(), 1);
        let w = &parsed.widgets.widgets[0];
        assert_eq!(w.path, "widget");
        assert_eq!(w.embedded_in_crates, vec!["cost-coach-lambda".to_string()]);
        assert_eq!(w.output_dir, "dist");
        assert!(w.build.is_none());
        assert!(w.install.is_none());
    }

    /// Test 1.3 (rejects_path_traversal): T-79-02 mitigation — `..` in the
    /// path is rejected by `validate()`, NOT by serde (the schema accepts any
    /// string; validation is a separate orchestrator concern).
    #[test]
    fn rejects_path_traversal() {
        let w = WidgetConfig {
            path: "../etc".to_string(),
            build: None,
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };
        let err = w.validate().expect_err("'..' must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("..") || msg.contains("path traversal"),
            "expected path-traversal error, got: {msg}"
        );

        // Embedded `..` segment also rejected.
        let w = WidgetConfig {
            path: "widget/../../etc".to_string(),
            build: None,
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };
        w.validate().expect_err("embedded '..' must be rejected");

        // Empty-build argv rejected.
        let w = WidgetConfig {
            path: "widget".to_string(),
            build: Some(vec![]),
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };
        let err = w.validate().expect_err("empty build argv rejected");
        assert!(err.to_string().contains("build"));

        // Empty-install argv rejected.
        let w = WidgetConfig {
            path: "widget".to_string(),
            build: None,
            install: Some(vec![]),
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };
        let err = w.validate().expect_err("empty install argv rejected");
        assert!(err.to_string().contains("install"));
    }

    /// Test 1.4 (pm_detection_priority_order): table-driven property-style test
    /// over every combination of lockfile presence. Higher-priority lockfile
    /// always masks lower-priority ones.
    #[test]
    fn pm_detection_priority_order() {
        // Order: bun > pnpm > yarn > npm.
        let lockfiles = [
            ("bun.lockb", PackageManager::Bun),
            ("pnpm-lock.yaml", PackageManager::Pnpm),
            ("yarn.lock", PackageManager::Yarn),
            ("package-lock.json", PackageManager::Npm),
        ];

        // For every subset of lockfiles, the detected PM must match the
        // highest-priority lockfile in the subset (bun=highest, npm=lowest).
        for mask in 0u8..16 {
            let dir = tempfile::tempdir().expect("tempdir");
            let mut expected: Option<PackageManager> = None;
            for (i, (name, pm)) in lockfiles.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    std::fs::write(dir.path().join(name), b"").expect("write lockfile");
                    if expected.is_none() {
                        expected = Some(*pm);
                    }
                }
            }
            // No lockfiles → falls back to Npm.
            let want = expected.unwrap_or(PackageManager::Npm);
            let got = PackageManager::detect_from_dir(dir.path());
            assert_eq!(got, want, "mask={mask:04b} expected {want:?} got {got:?}");
        }
    }

    /// Test 1.5 (pm_install_args_match_lock): locks the install argv shape.
    #[test]
    fn pm_install_args_match_lock() {
        assert_eq!(
            PackageManager::Bun.install_args(),
            ("bun", &["install"][..])
        );
        assert_eq!(
            PackageManager::Pnpm.install_args(),
            ("pnpm", &["install"][..])
        );
        assert_eq!(
            PackageManager::Yarn.install_args(),
            ("yarn", &["install"][..])
        );
        assert_eq!(
            PackageManager::Npm.install_args(),
            ("npm", &["install"][..])
        );
    }

    /// Test 1.6 (pm_build_args_match_lock): locks the build argv shape per
    /// `79-CONTEXT.md` priority list. Yarn's `build` form omits `run`.
    #[test]
    fn pm_build_args_match_lock() {
        assert_eq!(
            PackageManager::Bun.build_args(),
            ("bun", &["run", "build"][..])
        );
        assert_eq!(
            PackageManager::Pnpm.build_args(),
            ("pnpm", &["run", "build"][..])
        );
        assert_eq!(
            PackageManager::Yarn.build_args(),
            ("yarn", &["build"][..]),
            "yarn omits `run` per CONTEXT.md"
        );
        assert_eq!(
            PackageManager::Npm.build_args(),
            ("npm", &["run", "build"][..])
        );
    }

    /// Test 1.7 (build_install_argv_array_round_trip — REVISION 3): argv-array
    /// form preserves the `--silent` flag that the pre-revision-3
    /// `Option<String>` whitespace-split form would have broken.
    #[test]
    fn build_install_argv_array_round_trip() {
        let toml_str = r#"
[[widgets]]
path = "widget"
build = ["npm", "run", "--silent", "build"]
install = ["pnpm", "install", "--frozen-lockfile"]
embedded_in_crates = ["my-crate"]
"#;
        let parsed: Wrapper = toml::from_str(toml_str).expect("parses");
        assert_eq!(parsed.widgets.widgets.len(), 1);
        let w = &parsed.widgets.widgets[0];
        assert_eq!(
            w.build,
            Some(vec![
                "npm".to_string(),
                "run".to_string(),
                "--silent".to_string(),
                "build".to_string(),
            ])
        );
        assert_eq!(
            w.install,
            Some(vec![
                "pnpm".to_string(),
                "install".to_string(),
                "--frozen-lockfile".to_string(),
            ])
        );

        // Round-trip: serialize → re-parse → compare.
        let serialized = toml::to_string(&parsed).expect("serializes");
        let reparsed: Wrapper = toml::from_str(&serialized).expect("re-parses");
        assert_eq!(reparsed.widgets.widgets[0].build, w.build);
        assert_eq!(reparsed.widgets.widgets[0].install, w.install);
    }

    /// Test 1.8 (build_install_string_alternate_form_optional — REVISION 3):
    /// string form is REJECTED with a clear error (planner picked
    /// strict-reject for v1; migration note in CHANGELOG).
    #[test]
    fn build_string_form_rejected_with_clear_error() {
        let toml_str = r#"
[[widgets]]
path = "widget"
build = "npm run build"
embedded_in_crates = ["my-crate"]
"#;
        let err =
            toml::from_str::<Wrapper>(toml_str).expect_err("string-form build must be rejected");
        let msg = err.to_string().to_lowercase();
        // toml's error wording is "invalid type: string ... expected a sequence"
        assert!(
            msg.contains("sequence") || msg.contains("array") || msg.contains("expected"),
            "expected an actionable type-mismatch error, got: {msg}"
        );
    }

    /// `WidgetConfig::resolve_paths` joins workspace root + path correctly and
    /// honors a custom `output_dir`.
    #[test]
    fn resolve_paths_joins_workspace_root() {
        let w = WidgetConfig {
            path: "widget".to_string(),
            build: None,
            install: None,
            output_dir: "build".to_string(),
            embedded_in_crates: vec![],
        };
        let root = Path::new("/tmp/ws");
        let r = w.resolve_paths(root);
        assert_eq!(r.path, PathBuf::from("/tmp/ws/widget"));
        assert_eq!(r.absolute_output_dir, PathBuf::from("/tmp/ws/widget/build"));
    }
}
