//! `pmcp.toml` project-config parser for governed workbook bundles (WBCL-04).
//!
//! A project declares its workbooks → bundle IDs in a single repo-root
//! `pmcp.toml` (D-01), killing the lighthouse single-workbook path assumption.
//! Each entry maps a workbook `path` to a `bundle_id` (workflow name) and an
//! `out_dir` write target — and NOTHING else (D-02): the workbook version comes
//! from the workbook itself, and the approver comes from the `--approver` CLI
//! flag, so neither is recorded here.
//!
//! A missing `pmcp.toml` is NOT an error (D-03): [`PmcpToml::load`] returns
//! `Ok(None)` so a bare-path compile works with no toml at all.
//!
//! # Containment (threat T-94-01-PATH)
//!
//! Because a checked-in `pmcp.toml` later drives filesystem writes, every
//! `path`/`out_dir` MUST resolve UNDER the project root. [`PmcpToml::validate`]
//! takes the `project_root` and rejects any entry that escapes it — an absolute
//! path that does not live under the root, or a `..`-escaping relative path.
//! Lexical `..`-rejection alone is insufficient (an absolute `/etc/...` out_dir
//! has no `..` yet still escapes), so containment is computed against the
//! passed-in root rather than by lexical inspection alone.
//!
//! A `foo/../bar` that stays UNDER the root is tolerated (it resolves to
//! `foo`-sibling `bar`, still contained) — only escapes above the root are
//! rejected.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// The deserialized repo-root `pmcp.toml`.
///
/// Holds the full declared set of workbook → bundle-id mappings. The set is
/// enumerable via [`PmcpToml::all_entries`] (drives compile-all, D-05) and a
/// single entry is resolvable by bundle-id via [`PmcpToml::resolve`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PmcpToml {
    /// Array-of-tables `[[workbook]]` entries. Defaults to empty so a file with
    /// no workbooks (or none of the section) still deserializes cleanly.
    #[serde(default, rename = "workbook")]
    pub workbooks: Vec<WorkbookEntry>,
}

/// A single `[[workbook]]` declaration: `path → bundle_id → out_dir` ONLY.
///
/// Per D-02 there is intentionally no `version` field (the version is read from
/// the workbook) and no `approver` field (the approver comes from `--approver`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookEntry {
    /// Path to the source workbook, relative to the project root.
    pub path: PathBuf,

    /// Stable bundle / workflow identifier this workbook compiles to.
    pub bundle_id: String,

    /// Output directory for the compiled bundle, relative to the project root.
    pub out_dir: PathBuf,
}

impl PmcpToml {
    /// Load the project-root `pmcp.toml`.
    ///
    /// Returns `Ok(None)` when the file is ABSENT (D-03 optionality) so a
    /// bare-path compile works with no toml at all. Returns `Err` only on a
    /// read failure, a parse failure, or a containment/duplicate validation
    /// failure — a loaded config is therefore always contained, because `load`
    /// has the `project_root` in hand and calls [`PmcpToml::validate`].
    pub fn load(project_root: &Path) -> Result<Option<Self>> {
        let path = project_root.join("pmcp.toml");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        config.validate(project_root)?;
        Ok(Some(config))
    }

    /// Resolve a bundle-id to its declared entry (D-05).
    ///
    /// Returns an `Err` naming the missing bundle_id and `pmcp.toml` on a miss.
    pub fn resolve(&self, bundle_id: &str) -> Result<&WorkbookEntry> {
        self.workbooks
            .iter()
            .find(|e| e.bundle_id == bundle_id)
            .with_context(|| format!("no workbook '{bundle_id}' declared in pmcp.toml"))
    }

    /// The full declared set of workbook entries (drives compile-all, D-05).
    pub fn all_entries(&self) -> &[WorkbookEntry] {
        &self.workbooks
    }

    /// Validate the config against the project root.
    ///
    /// Rejects:
    /// - duplicate `bundle_id`s (T-94-01-DUP — a later entry would silently
    ///   shadow an earlier resolution);
    /// - any `path` or `out_dir` that resolves OUTSIDE `project_root`
    ///   (T-94-01-PATH) — an absolute path not under the root, or a
    ///   `..`-escaping relative path.
    ///
    /// A `foo/../bar` that stays under the root is accepted.
    pub fn validate(&self, project_root: &Path) -> Result<()> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.workbooks.len());
        for entry in &self.workbooks {
            if seen.contains(&entry.bundle_id.as_str()) {
                bail!(
                    "duplicate bundle_id '{}' in pmcp.toml: a later entry would shadow an earlier one",
                    entry.bundle_id
                );
            }
            seen.push(&entry.bundle_id);

            if resolves_outside(project_root, &entry.path) {
                bail!(
                    "workbook '{}' path '{}' resolves outside the project root",
                    entry.bundle_id,
                    entry.path.display()
                );
            }
            if resolves_outside(project_root, &entry.out_dir) {
                bail!(
                    "workbook '{}' out_dir '{}' resolves outside the project root",
                    entry.bundle_id,
                    entry.out_dir.display()
                );
            }
        }
        Ok(())
    }
}

/// Returns `true` when `candidate`, resolved against `root`, escapes `root`.
///
/// Containment is computed against the passed-in `root` (not by lexical
/// `..`-inspection alone — an absolute path has no `..` yet still escapes):
///
/// 1. An ABSOLUTE `candidate` escapes UNLESS it `starts_with` `root`.
/// 2. A RELATIVE `candidate` escapes when its lexically-normalized join pops
///    above `root` via `..` segments (walk components, track depth; a depth
///    below zero means an escape).
///
/// Does NOT canonicalize — the `out_dir` may not exist yet — so a `foo/../bar`
/// that stays under the root is ACCEPTED, while `../bar` or an outside absolute
/// path is REJECTED.
fn resolves_outside(root: &Path, candidate: &Path) -> bool {
    if candidate.is_absolute() {
        return !candidate.starts_with(root);
    }
    let mut depth: i64 = 0;
    for component in candidate.components() {
        match component {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            },
            Component::Normal(_) => depth += 1,
            // CurDir (`.`), RootDir, and Prefix are depth-neutral for a
            // relative candidate (RootDir/Prefix can't appear here since the
            // absolute case returned above).
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {},
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, bundle_id: &str, out_dir: &str) -> WorkbookEntry {
        WorkbookEntry {
            path: PathBuf::from(path),
            bundle_id: bundle_id.to_string(),
            out_dir: PathBuf::from(out_dir),
        }
    }

    #[test]
    fn test_pmcp_toml_round_trips_through_toml() {
        let cfg = PmcpToml {
            workbooks: vec![
                entry("workbooks/quote.xlsx", "quote", "dist/quote"),
                entry("workbooks/order.xlsx", "order", "dist/order"),
            ],
        };
        let serialized = toml::to_string(&cfg).expect("serialize");
        let deserialized: PmcpToml = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn test_load_absent_file_is_ok_none() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let loaded = PmcpToml::load(tmp.path()).expect("load");
        assert!(
            loaded.is_none(),
            "missing pmcp.toml must be Ok(None) (D-03)"
        );
    }

    #[test]
    fn test_load_present_file_parses_two_entries() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let toml_src = r#"
[[workbook]]
path = "workbooks/quote.xlsx"
bundle_id = "quote"
out_dir = "dist/quote"

[[workbook]]
path = "workbooks/order.xlsx"
bundle_id = "order"
out_dir = "dist/order"
"#;
        std::fs::write(tmp.path().join("pmcp.toml"), toml_src).expect("write");
        let cfg = PmcpToml::load(tmp.path())
            .expect("load")
            .expect("present file is Some");
        assert_eq!(cfg.all_entries().len(), 2);
        assert_eq!(cfg.all_entries()[0].bundle_id, "quote");
        assert_eq!(cfg.all_entries()[1].out_dir, PathBuf::from("dist/order"));
    }

    #[test]
    fn test_resolve_hit_and_miss() {
        let cfg = PmcpToml {
            workbooks: vec![entry("a.xlsx", "quote", "dist/quote")],
        };
        assert_eq!(
            cfg.resolve("quote").expect("hit").path,
            PathBuf::from("a.xlsx")
        );
        let err = cfg.resolve("absent").expect_err("miss must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("absent"),
            "message names the missing id: {msg}"
        );
        assert!(msg.contains("pmcp.toml"), "message names pmcp.toml: {msg}");
    }

    #[test]
    fn test_validate_accepts_clean_config() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            workbooks: vec![
                entry("workbooks/quote.xlsx", "quote", "dist/quote"),
                entry("workbooks/order.xlsx", "order", "dist/order"),
            ],
        };
        cfg.validate(root).expect("clean config validates");
    }

    #[test]
    fn test_validate_rejects_duplicate_bundle_id() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            workbooks: vec![
                entry("a.xlsx", "quote", "dist/a"),
                entry("b.xlsx", "quote", "dist/b"),
            ],
        };
        let err = cfg.validate(root).expect_err("duplicate must reject");
        assert!(format!("{err}").contains("duplicate"));
    }

    #[test]
    fn test_validate_rejects_parent_escape_out_dir() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            workbooks: vec![entry("a.xlsx", "quote", "../escape")],
        };
        let err = cfg.validate(root).expect_err("../escape must reject");
        assert!(format!("{err}").contains("out_dir"));
    }

    #[test]
    fn test_validate_rejects_absolute_out_dir_outside_root() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            workbooks: vec![entry("a.xlsx", "quote", "/etc/evil")],
        };
        let err = cfg
            .validate(root)
            .expect_err("absolute outside-root out_dir must reject (concern C)");
        assert!(format!("{err}").contains("out_dir"));
    }

    #[test]
    fn test_validate_rejects_parent_escape_path() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            workbooks: vec![entry("../secrets.xlsx", "quote", "dist/quote")],
        };
        let err = cfg.validate(root).expect_err("../path must reject");
        assert!(format!("{err}").contains("path"));
    }

    #[test]
    fn test_validate_tolerates_dotdot_within_root() {
        let root = Path::new("/project");
        let cfg = PmcpToml {
            // foo/../bar normalizes to bar — still under root, accepted.
            workbooks: vec![entry("a.xlsx", "quote", "foo/../bar")],
        };
        cfg.validate(root)
            .expect("foo/../bar stays under root and is accepted");
    }

    #[test]
    fn test_resolves_outside_absolute_under_root_is_contained() {
        let root = Path::new("/project");
        assert!(!resolves_outside(root, Path::new("/project/dist")));
        assert!(resolves_outside(root, Path::new("/other/dist")));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// A bounded path/bundle_id/out_dir segment strategy (concern G — never a
    /// bare `.*`, which generates huge/slow cases).
    fn seg() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z0-9_./-]{0,32}").expect("valid regex")
    }

    fn entries() -> impl Strategy<Value = Vec<WorkbookEntry>> {
        prop::collection::vec((seg(), seg(), seg()), 0..8).prop_map(|raw| {
            raw.into_iter()
                .map(|(path, bundle_id, out_dir)| WorkbookEntry {
                    path: PathBuf::from(path),
                    bundle_id,
                    out_dir: PathBuf::from(out_dir),
                })
                .collect()
        })
    }

    proptest! {
        // Property 1: load-path fuzz over BOUNDED TOML bytes never panics, and
        // for any successfully-parsed config, validate/resolve also never panic.
        #[test]
        fn prop_no_panic_over_bounded_toml(input in "[a-zA-Z0-9_./= \t\n\"\\[\\]-]{0,2048}") {
            let root = std::env::temp_dir();
            if let Ok(cfg) = toml::from_str::<PmcpToml>(&input) {
                // These must RETURN (the harness fails the case on any panic).
                let _ = cfg.validate(&root);
                let _ = cfg.resolve("any");
            }
        }

        // Property 2: serialize → deserialize is lossless (requires PartialEq).
        #[test]
        fn prop_round_trip(workbooks in entries()) {
            let cfg = PmcpToml { workbooks };
            // Guard the round-trip over only inputs toml::to_string accepts.
            let serialized = match toml::to_string(&cfg) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            let deserialized: PmcpToml = toml::from_str(&serialized)
                .map_err(|e| TestCaseError::fail(format!("re-parse failed: {e}")))?;
            prop_assert_eq!(deserialized, cfg);
        }

        // Property 3a: a `../`-escaping out_dir is NEVER accepted (T-94-01-PATH).
        #[test]
        fn prop_validate_rejects_parent_escape(seg in "[a-zA-Z0-9_-]{0,16}") {
            let root = Path::new("/project");
            let cfg = PmcpToml {
                workbooks: vec![WorkbookEntry {
                    path: PathBuf::from("ok.xlsx"),
                    bundle_id: "wb".to_string(),
                    out_dir: PathBuf::from(format!("../{seg}")),
                }],
            };
            prop_assert!(cfg.validate(root).is_err());
        }

        // Property 3b: an absolute out_dir outside the root is NEVER accepted
        // (concern C — containment needs the root, not just lexical `..`).
        #[test]
        fn prop_validate_rejects_absolute_escape(seg in "[a-zA-Z0-9_-]{1,16}") {
            let root = Path::new("/project");
            let cfg = PmcpToml {
                workbooks: vec![WorkbookEntry {
                    path: PathBuf::from("ok.xlsx"),
                    bundle_id: "wb".to_string(),
                    // `/elsewhere/...` is absolute and not under `/project`.
                    out_dir: PathBuf::from(format!("/elsewhere/{seg}")),
                }],
            };
            prop_assert!(cfg.validate(root).is_err());
        }

        // Property 4: resolve is exactly membership and never panics.
        #[test]
        fn prop_resolve_is_membership(workbooks in entries(), id in "[a-zA-Z0-9_-]{0,32}") {
            let cfg = PmcpToml { workbooks };
            let expected = cfg.all_entries().iter().any(|e| e.bundle_id == id);
            prop_assert_eq!(cfg.resolve(&id).is_ok(), expected);
        }
    }
}
