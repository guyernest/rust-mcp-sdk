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

// ============================================================================
// Wave 2: widget pre-build orchestrator (Plan 79-02)
// ============================================================================
//
// The functions below drive an automatic widget build (`npm run build`,
// `pnpm run build`, …) from `cargo pmcp deploy`'s `execute_async` BEFORE the
// `cargo build --release` step. They consume the schema types above
// (`WidgetConfig`, `PackageManager`, `ResolvedPaths`).
//
// Decomposition follows Phase-75 RESEARCH.md Pattern 2 (per-stage pipeline)
// so every fn stays under cog 20 — well under the PMAT cap of 25.
//
// REVISION 3 supersessions:
// - HIGH-C1: `run_widget_build` returns `ResolvedPaths` so the caller can
//   join all widgets into a single `PMCP_WIDGET_DIRS` env var (set ONCE,
//   covers ALL widgets). Per-call `set_var` removed.
// - Codex MEDIUM (argv): `argv_to_cmd_args` replaces the pre-revision-3
//   whitespace-split helper — no shell parsing.
// - Codex MEDIUM (Yarn PnP): `is_yarn_pnp` early-returns from
//   `ensure_node_modules` when `.pnp.cjs` or `.pnp.loader.mjs` is present.

use anyhow::{bail, Context as _, Result as AnyhowResult};

/// Detect widgets to build from explicit config OR from `widget/`/`widgets/`
/// convention. Returns a Vec because deploy.toml allows multiple `[[widgets]]`
/// blocks.
///
/// Per `79-CONTEXT.md` "Convention search" LOCKED: ONLY `widget/` and
/// `widgets/` are auto-detected — `ui/` and `app/` are explicitly DROPPED to
/// avoid false-positives on Rust workspace bin-crate dirs.
///
/// When no explicit `[[widgets]]` block exists AND a convention dir is found,
/// synthesizes one `WidgetConfig` whose `embedded_in_crates` defaults to ALL
/// workspace bin crates (safe over-invalidation; `cargo pmcp doctor` hints
/// the operator to write the explicit config).
///
/// REQ-79-01 (drop ui/ + app/): test 1.10 + the `["widget", "widgets"]`
/// literal is a hard fence.
/// REQ-79-09 (synthesize embedded_in_crates to all bin crates):
/// `enumerate_workspace_bin_crates`.
#[must_use]
pub fn detect_widgets(
    config: &crate::deployment::config::DeployConfig,
    workspace_root: &Path,
) -> Vec<WidgetConfig> {
    if !config.widgets.is_empty() {
        return config.widgets.widgets.clone();
    }
    for candidate in ["widget", "widgets"] {
        if workspace_root.join(candidate).is_dir() {
            let bin_crates = enumerate_workspace_bin_crates(workspace_root);
            return vec![WidgetConfig {
                path: candidate.to_string(),
                build: None,
                install: None,
                output_dir: "dist".to_string(),
                embedded_in_crates: bin_crates,
            }];
        }
    }
    Vec::new()
}

/// List workspace bin crates by name via `cargo metadata`.
///
/// Used by [`detect_widgets`] to populate the synthesized `embedded_in_crates`
/// field when an operator hasn't written an explicit `[[widgets]]` block.
/// Falls back to an empty Vec on any cargo-metadata error — that just means
/// `cargo pmcp doctor` will hint the operator more strongly.
fn enumerate_workspace_bin_crates(workspace_root: &Path) -> Vec<String> {
    let Ok(binaries) = crate::deployment::naming::detect_workspace_binaries(workspace_root) else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    binaries
        .into_iter()
        .filter_map(|b| {
            seen.insert(b.package_name.clone())
                .then_some(b.package_name)
        })
        .collect()
}

/// REVISION 3 Codex MEDIUM helper: detect Yarn PnP via marker files. When
/// PnP is in use, `node_modules/` is intentionally absent and the
/// install heuristic should NOT fire.
///
/// Cog ≤4. Recognises both Yarn 3 (`.pnp.cjs`) and Yarn 4+
/// (`.pnp.loader.mjs`) marker forms.
fn is_yarn_pnp(widget_dir: &Path) -> bool {
    widget_dir.join(".pnp.cjs").is_file() || widget_dir.join(".pnp.loader.mjs").is_file()
}

/// Plan 79-06 helper: detects whether `widget_dir` is a Node-managed project
/// by checking for a `package.json` *file* at the directory root.
///
/// Returns `false` for raw-HTML / CDN-import widget directories (the
/// documented zero-build MCP Apps archetype from Phase 45 onwards). When this
/// returns `false`, [`run_widget_build`] takes the raw-HTML early-return path
/// and spawns ZERO `npm`/`pnpm`/`yarn`/`bun` subprocesses — the directory's
/// `*.html` files are served as-is and their CDN imports resolve at runtime.
///
/// Uses [`Path::is_file`] (not [`Path::exists`]) so a directory accidentally
/// named `package.json` is treated as NOT a Node project. Cog ≤2.
///
/// This closes the Phase 79 UAT Test 3 gap-closure: prior to this guard, the
/// widget pre-build orchestrator would unconditionally spawn `npm install`
/// against any `widgets/` dir, causing npm to walk UP the directory tree (an
/// audit of 1839 packages was observed against a parent workspace) and the
/// subsequent `read_to_string(package.json)` to produce raw `os error 2`.
fn is_node_project(widget_dir: &Path) -> bool {
    widget_dir.join("package.json").is_file()
}

/// REVISION 3 Codex MEDIUM helper: split argv slice into (cmd, rest_args).
///
/// Replaces the pre-revision-3 `parse_explicit_command(s: &str)`
/// whitespace-split helper which broke quoting on inputs like
/// `"npm run --silent build"` (the `--silent` flag would attach to the
/// wrong argument). The argv-array form is unambiguous.
///
/// Cog ≤4.
///
/// # Errors
/// Returns Err when `argv` is empty.
fn argv_to_cmd_args(argv: &[String]) -> AnyhowResult<(String, Vec<String>)> {
    let mut iter = argv.iter().cloned();
    let cmd = iter
        .next()
        .context("explicit command argv is empty — provide at least one element")?;
    let args: Vec<String> = iter.collect();
    Ok((cmd, args))
}

/// Top-level orchestrator. Builds ONE widget; the caller iterates over all
/// `[[widgets]]` entries and aggregates the returned `ResolvedPaths` into a
/// single `PMCP_WIDGET_DIRS` env var (REVISION 3 HIGH-C1).
///
/// Pipeline:
/// 1. `validate()` — T-79-02 mitigation (path traversal + empty argv).
/// 2. `resolve_paths(workspace_root)` — absolute path computation.
/// 3. `PackageManager::detect_from_dir` — lockfile-based PM detection.
/// 4. `ensure_node_modules` — install if missing AND no Yarn-PnP marker.
/// 5. `invoke_build_script` — the actual `npm run build` (or PM equivalent).
/// 6. `verify_outputs_exist` — WARN if zero files in output_dir.
///
/// Cog ≤7 — Pattern 2 (per-stage pipeline) decomposition.
///
/// REVISION 3 HIGH-C1: returns `Ok(ResolvedPaths)` instead of mutating
/// global env state. Caller (`commands/deploy/mod.rs`) joins all returned
/// paths into `PMCP_WIDGET_DIRS` ONCE at the end.
///
/// # Errors
/// Returns Err on any pipeline-stage failure (validate, install, build, or
/// when the package.json has no `build` script unless explicitly overridden).
pub async fn run_widget_build(
    widget: &WidgetConfig,
    workspace_root: &Path,
    quiet: bool,
) -> AnyhowResult<ResolvedPaths> {
    widget.validate()?;
    let resolved = widget.resolve_paths(workspace_root);

    // Plan 79-06 — raw-HTML / CDN-import widget archetype guard (Phase 45
    // zero-build use case). When the widget dir has no `package.json`, we
    // treat it as a raw-HTML / CDN bundle: NO `npm install`, NO build step,
    // NO subprocess at all. Closes the UAT Test 3 regression where the
    // orchestrator hard-crashed with raw `os error 2` and risked an
    // `npm install` parent-walk audit (1839 packages observed in the
    // Scientific-Calculator-MCP-App reproduction).
    //
    // CRITICAL HIGH-C1 invariant: we still return Ok(resolved) so the caller
    // (`commands/deploy/mod.rs::pre_build_widgets_and_set_env`) appends this
    // widget's absolute_output_dir to PMCP_WIDGET_DIRS — the build.rs
    // cargo:rerun-if-changed chain still rebuilds the binary on *.html edits.
    if !is_node_project(&resolved.path) {
        if !quiet {
            println!(
                "  treating {} as raw HTML / CDN bundle, skipping build",
                resolved.path.display()
            );
        }
        return Ok(resolved);
    }

    let pm = PackageManager::detect_from_dir(&resolved.path);
    ensure_node_modules(pm, &resolved, widget.install.as_deref(), quiet).await?;
    invoke_build_script(pm, &resolved, widget.build.as_deref(), quiet).await?;
    verify_outputs_exist(&resolved, quiet);
    Ok(resolved)
}

/// Skip if `node_modules/` exists OR Yarn-PnP markers present
/// (Pitfall 2 + REVISION 3 Codex MEDIUM mitigations).
///
/// Cog ≤8.
async fn ensure_node_modules(
    pm: PackageManager,
    resolved: &ResolvedPaths,
    explicit_install: Option<&[String]>,
    quiet: bool,
) -> AnyhowResult<()> {
    if resolved.path.join("node_modules").is_dir() {
        return Ok(());
    }
    if is_yarn_pnp(&resolved.path) {
        // REVISION 3 Codex MEDIUM: Yarn PnP intentionally omits node_modules.
        // PnP resolves dependencies from .pnp.cjs at runtime — install is a
        // no-op on the first build and would just slow us down on every run.
        return Ok(());
    }
    if !quiet {
        println!("  Installing widget dependencies...");
    }
    let (cmd, args) = resolve_command_argv(explicit_install, || {
        let (c, a) = pm.install_args();
        (c.to_string(), a.iter().map(|s| (*s).to_string()).collect())
    })?;
    let label = format!("widget install (`{cmd}`)");
    spawn_streaming(&cmd, &args, &resolved.path, &label).await
}

/// Verify package.json has build script before spawning UNLESS explicit
/// argv override is provided (REVISION 3 Codex MEDIUM: skip the package.json
/// check when the user supplied an explicit build argv — they take
/// responsibility for whatever invocation they configured).
///
/// Cog ≤8.
async fn invoke_build_script(
    pm: PackageManager,
    resolved: &ResolvedPaths,
    explicit_build: Option<&[String]>,
    quiet: bool,
) -> AnyhowResult<()> {
    if explicit_build.is_none() {
        verify_build_script_exists(&resolved.path)?;
    }
    if !quiet {
        println!("  Building widget bundle...");
    }
    let (cmd, args) = resolve_command_argv(explicit_build, || {
        let (c, a) = pm.build_args();
        (c.to_string(), a.iter().map(|s| (*s).to_string()).collect())
    })?;
    let label = format!("widget build (`{cmd}`)");
    spawn_streaming(&cmd, &args, &resolved.path, &label).await
}

/// Helper to pick the `(cmd, args)` tuple from EITHER an explicit argv slice
/// OR a `PackageManager`-supplied default. Centralises the
/// `Some -> argv_to_cmd_args` / `None -> default` branch so the two callers
/// (`ensure_node_modules`, `invoke_build_script`) stay under cog 8 each.
///
/// Cog ≤3.
fn resolve_command_argv<F>(
    explicit: Option<&[String]>,
    default: F,
) -> AnyhowResult<(String, Vec<String>)>
where
    F: FnOnce() -> (String, Vec<String>),
{
    match explicit {
        Some(argv) => argv_to_cmd_args(argv),
        None => Ok(default()),
    }
}

/// Shared spawn helper. stdout/stderr stream LIVE to the parent terminal —
/// no `.stdout(Stdio::piped())` capture — so operators see the JS toolchain's
/// progress output as it runs (REQ-79-05).
///
/// REVISION 3 HIGH-C1: env-var setup MOVED to caller. The orchestrator joins
/// ALL widgets' dirs into `PMCP_WIDGET_DIRS` once at the end.
///
/// Cog ≤6.
///
/// # Errors
/// Returns Err when:
/// - the binary cannot be spawned (e.g., `npm` not on PATH),
/// - the subprocess exits with a non-zero status.
async fn spawn_streaming(cmd: &str, args: &[String], cwd: &Path, label: &str) -> AnyhowResult<()> {
    let mut child = tokio::process::Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .spawn()
        .with_context(|| {
            format!("Failed to spawn `{cmd}`. Is `{cmd}` on PATH? See `cargo pmcp doctor`.")
        })?;
    let status = child
        .wait()
        .await
        .with_context(|| format!("Failed to wait on `{cmd}` subprocess"))?;
    if !status.success() {
        bail!(
            "{label} failed with exit code {:?} — see output above",
            status.code()
        );
    }
    Ok(())
}

/// Verify `package.json` exists and has a `scripts.build` entry.
///
/// REQ-79-03: error message is verbatim from `79-CONTEXT.md` "Convention
/// search" LOCKED: `"package.json at <path> has no 'build' script — add one
/// or configure widgets in .pmcp/deploy.toml"`.
///
/// Cog ≤6.
fn verify_build_script_exists(widget_dir: &Path) -> AnyhowResult<()> {
    let pkg_json_path = widget_dir.join("package.json");

    // Plan 79-06 defense-in-depth: bail with a friendly diagnostic BEFORE
    // `std::fs::read_to_string` raises raw `io::Error` ("os error 2"). The
    // early-return in `run_widget_build` covers the common path; this guard
    // covers the "explicit Node-shaped build/install argv against a non-Node
    // dir" edge case (e.g., a stale `node_modules/` masking a deleted
    // `package.json` — `ensure_node_modules` short-circuits on `node_modules/`
    // so this function is reached without a manifest).
    if !pkg_json_path.is_file() {
        bail!(
            "widget dir {} has no package.json — add one with a 'build' script, \
             configure widgets in .pmcp/deploy.toml, or remove the build = ... \
             override (raw-HTML widgets need no build)",
            widget_dir.display()
        );
    }

    let raw = std::fs::read_to_string(&pkg_json_path)
        .with_context(|| format!("Failed to read {}", pkg_json_path.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("Invalid JSON in {}", pkg_json_path.display()))?;
    let has_build = parsed.get("scripts").and_then(|s| s.get("build")).is_some();
    if !has_build {
        bail!(
            "package.json at {} has no 'build' script — add one or configure widgets in .pmcp/deploy.toml",
            pkg_json_path.display()
        );
    }
    Ok(())
}

/// WARN when the build emitted zero files into `output_dir` per
/// `79-CONTEXT.md` "widget build succeeds but emits zero output files →
/// WARN, do not fail". Likely a misconfigured build script, but may be
/// intentional during scaffolding.
///
/// Infallible (returns nothing) — never aborts the deploy. Reads via
/// `read_dir.ok()` so a missing dir is silently treated as "zero files".
///
/// Cog ≤4.
fn verify_outputs_exist(resolved: &ResolvedPaths, quiet: bool) {
    let entries: Vec<_> = std::fs::read_dir(&resolved.absolute_output_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    if entries.is_empty() && !quiet {
        eprintln!(
            "  WARNING: widget build emitted no files into {}; verify your build script",
            resolved.absolute_output_dir.display()
        );
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

    // ========================================================================
    // Plan 79-06: raw-HTML / CDN-import widget archetype guard tests
    //
    // These tests pin the gap-closure for UAT Test 3 (severity: major) where
    // `cargo pmcp deploy` hard-crashed with raw `os error 2` on a `widgets/`
    // directory containing only `*.html` files (no `package.json`). They lock
    // the documented zero-build MCP Apps archetype from Phase 45 onwards.
    // ========================================================================

    /// Plan 79-06 Test U1: `is_node_project` detects a Node-managed widget
    /// directory by the presence of a `package.json` *file* at the directory
    /// root. Uses `Path::is_file()` (not `Path::exists()`) so a directory
    /// accidentally named `package.json` is treated as NOT a Node project.
    #[test]
    fn is_node_project_detects_package_json() {
        // Empty tempdir → not a Node project.
        let empty = tempfile::tempdir().expect("tempdir");
        assert!(
            !is_node_project(empty.path()),
            "empty dir must not be Node project"
        );

        // Only a raw HTML file → not a Node project.
        let html_only = tempfile::tempdir().expect("tempdir");
        std::fs::write(html_only.path().join("keypad.html"), b"<html></html>")
            .expect("write keypad.html");
        assert!(
            !is_node_project(html_only.path()),
            "dir with only *.html must not be Node project"
        );

        // Only a `package.json` file → IS a Node project.
        let pkg_only = tempfile::tempdir().expect("tempdir");
        std::fs::write(pkg_only.path().join("package.json"), b"{}").expect("write package.json");
        assert!(
            is_node_project(pkg_only.path()),
            "dir with package.json must be Node project"
        );

        // Both `package.json` AND `*.html` present → IS a Node project.
        let both = tempfile::tempdir().expect("tempdir");
        std::fs::write(both.path().join("package.json"), b"{}").expect("write package.json");
        std::fs::write(both.path().join("keypad.html"), b"<html></html>")
            .expect("write keypad.html");
        assert!(
            is_node_project(both.path()),
            "dir with package.json + html must be Node project"
        );

        // Edge case: a *directory* named `package.json` → NOT a Node project
        // (we use `is_file()`, not `exists()`).
        let dir_named = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir_named.path().join("package.json"))
            .expect("create package.json/ as dir");
        assert!(
            !is_node_project(dir_named.path()),
            "directory named package.json must not be Node project (is_file vs exists)"
        );
    }

    /// Plan 79-06 Test U2: `run_widget_build` against a tempdir-rooted
    /// workspace whose `widgets/` directory contains only `keypad.html` (with
    /// a CDN import, no `package.json`, no lockfile) returns `Ok(resolved)`,
    /// resolves the expected paths, and does NOT spawn any subprocess.
    ///
    /// Proof that no subprocess was spawned: the test passes on a runner with
    /// no `npm`/`pnpm`/`yarn`/`bun` on PATH (the early-return guard in
    /// `run_widget_build` short-circuits BEFORE `spawn_streaming` runs). If
    /// the guard regressed, this test would fail with the original `os error
    /// 2` (or `npm not found`) on bare CI runners.
    #[tokio::test]
    async fn run_widget_build_raw_html_skip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace_root = tmp.path();
        let widgets_dir = workspace_root.join("widgets");
        std::fs::create_dir_all(&widgets_dir).expect("create widgets/");
        std::fs::write(
            widgets_dir.join("keypad.html"),
            r#"<!DOCTYPE html><html><body><script type="module">
import { App } from "https://esm.sh/@modelcontextprotocol/ext-apps";
new App({});
</script></body></html>"#,
        )
        .expect("write keypad.html");

        let widget = WidgetConfig {
            path: "widgets".to_string(),
            build: None,
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };

        let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
            .await
            .expect("raw-HTML widget should not error");

        assert_eq!(resolved.path, workspace_root.join("widgets"));
        assert_eq!(
            resolved.absolute_output_dir,
            workspace_root.join("widgets/dist")
        );
        assert!(
            !widgets_dir.join("node_modules").exists(),
            "no npm install should have been spawned"
        );
        assert!(
            !widgets_dir.join("package-lock.json").exists(),
            "no npm install should have been spawned"
        );
    }

    /// Plan 79-06 Test U3: same fixture as U2 but `quiet=false`. We assert
    /// behaviorally that `Ok(resolved)` is still returned and no subprocess
    /// was spawned. The println side-effect is exercised manually via
    /// integration tests; asserting on stdout would require a Write seam that
    /// adds complexity without proportional value.
    #[tokio::test]
    async fn run_widget_build_raw_html_emits_skip_line_when_not_quiet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace_root = tmp.path();
        let widgets_dir = workspace_root.join("widgets");
        std::fs::create_dir_all(&widgets_dir).expect("create widgets/");
        std::fs::write(
            widgets_dir.join("keypad.html"),
            b"<html><body>raw HTML</body></html>",
        )
        .expect("write keypad.html");

        let widget = WidgetConfig {
            path: "widgets".to_string(),
            build: None,
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };

        // quiet=false: the println! runs but we don't capture stdout (would
        // require a Write seam). The behavioral assertion (Ok + no subprocess
        // side-effects) is U2's job; this test pins that quiet=false does
        // NOT change the early-return outcome.
        let resolved = run_widget_build(&widget, workspace_root, /* quiet */ false)
            .await
            .expect("raw-HTML widget with quiet=false should not error");

        assert_eq!(resolved.path, workspace_root.join("widgets"));
        assert!(
            !widgets_dir.join("node_modules").exists(),
            "no npm install with quiet=false either"
        );
    }

    /// Plan 79-06 Test U4: defense-in-depth — `verify_build_script_exists`
    /// produces a friendly bail BEFORE `read_to_string` raises raw
    /// `io::Error: No such file or directory (os error 2)`. The error names
    /// the widget directory and points to actionable next steps.
    #[test]
    fn verify_build_script_exists_friendly_bail_on_missing_package_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = verify_build_script_exists(dir.path())
            .expect_err("missing package.json must produce friendly bail");
        let msg = err.to_string();

        // Must name the dir.
        assert!(
            msg.contains(&dir.path().display().to_string()),
            "error must name the widget dir, got: {msg}"
        );
        // Must mention the missing manifest.
        assert!(
            msg.contains("no package.json"),
            "error must mention 'no package.json', got: {msg}"
        );
        // Must point to an actionable next step.
        assert!(
            msg.contains("build") || msg.contains(".pmcp/deploy.toml"),
            "error must point to remediation, got: {msg}"
        );
        // Must NOT be the regression markers.
        assert!(
            !msg.contains("os error 2"),
            "error must not surface raw os-error-2, got: {msg}"
        );
        assert!(
            !msg.contains("Failed to read"),
            "error must not surface raw read_to_string context, got: {msg}"
        );
    }

    /// Plan 79-06 Test U5: locks the EXISTING REQ-79-03 verbatim error when
    /// `package.json` is present but lacks a `scripts.build` entry. This pins
    /// regression coverage that the new defense-in-depth guard does NOT
    /// disturb the pre-existing code path.
    #[test]
    fn verify_build_script_exists_existing_friendly_bail_unchanged_when_package_json_present_but_no_build_script(
    ) {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("package.json"), b"{}").expect("write package.json");

        let err = verify_build_script_exists(dir.path())
            .expect_err("missing build script must produce friendly bail");
        let msg = err.to_string();
        assert!(
            msg.contains("has no 'build' script"),
            "REQ-79-03 verbatim message must remain unchanged, got: {msg}"
        );
    }

    /// Plan 79-06 Test U6: regression coverage that the Node-pipeline path is
    /// NOT broken by the new guard. A widget with a real `package.json` and a
    /// stub `node_modules/.placeholder` (so `ensure_node_modules` short-
    /// circuits without requiring npm on PATH) plus an explicit
    /// `widget.build = ["true"]` argv MUST run end-to-end and return
    /// `Ok(resolved)` — proving the early-return guard was NOT taken.
    #[tokio::test]
    #[cfg(unix)]
    async fn run_widget_build_node_path_unchanged_with_package_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace_root = tmp.path();
        let widgets_dir = workspace_root.join("widgets");
        std::fs::create_dir_all(&widgets_dir).expect("create widgets/");
        std::fs::write(
            widgets_dir.join("package.json"),
            br#"{"scripts":{"build":"true"}}"#,
        )
        .expect("write package.json");
        std::fs::create_dir_all(widgets_dir.join("node_modules"))
            .expect("create node_modules/");
        std::fs::write(widgets_dir.join("node_modules/.placeholder"), b"")
            .expect("write placeholder");

        let widget = WidgetConfig {
            path: "widgets".to_string(),
            // Explicit POSIX `true` argv — exits 0 on every Unix-like CI runner
            // and sidesteps `verify_build_script_exists` per the existing
            // Wave 2 contract.
            build: Some(vec!["true".to_string()]),
            install: None,
            output_dir: "dist".to_string(),
            embedded_in_crates: vec![],
        };

        let resolved = run_widget_build(&widget, workspace_root, /* quiet */ true)
            .await
            .expect("Node pipeline happy path must still work");
        assert_eq!(resolved.path, widgets_dir);
        assert_eq!(resolved.absolute_output_dir, widgets_dir.join("dist"));
    }
}
