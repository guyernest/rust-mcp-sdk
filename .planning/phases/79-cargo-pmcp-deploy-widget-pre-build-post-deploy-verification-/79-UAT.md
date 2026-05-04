---
status: partial
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
source: [79-SUMMARY.md, 79-01-SUMMARY.md, 79-02-SUMMARY.md, 79-03-SUMMARY.md, 79-04-SUMMARY.md, 79-05-SUMMARY.md]
started: 2026-05-03T00:00:00Z
updated: 2026-05-03T00:00:00Z
---

## Current Test

[testing paused — 1 issue found, 3 skipped without reason]

## Tests

### 1. Help text mentions widget pre-build (REQ-79-18)
expected: |
  Run `cargo pmcp deploy --help`. Output mentions widget pre-build
  behavior verbatim per CONTEXT.md (auto-detects widget/ and widgets/).
result: pass

### 2. All 6 new deploy flags appear in --help
expected: |
  Run `cargo pmcp deploy --help`. The following flags are listed:
  `--no-widget-build`, `--widgets-only`, `--no-post-deploy-test`,
  `--post-deploy-tests=<list>`, `--on-test-failure=<fail|warn>`,
  `--apps-mode=<mode>`.
result: pass

### 3. Widget pre-build runs before cargo build
expected: |
  In a project with `widget/package.json` and a `build` script, run
  `cargo pmcp deploy --widgets-only`. The detected package manager
  (bun > pnpm > yarn > npm based on lockfile) runs `<pm> run build`
  BEFORE any `cargo build`. Console output shows the build script's
  output, then exits.
result: issue
reported: |
  The new cargo pmcp test apps command strictly assumes that the widget
  UI directory (widgets/) is a Node.js project. It blindly attempts to
  run npm install and read a package.json file, resulting in a hard
  crash (os error 2) when the file doesn't exist.

  Why this is problematic for developers:
  - Breaks the "Raw HTML" Use Case: The MCP Apps architecture beautifully
    supports single-file, zero-build widgets (like keypad.html) that
    import the SDK directly via a CDN (https://esm.sh/...). By strictly
    demanding a package.json, the CLI forces developers to add
    unnecessary boilerplate.
  - Implicit npm install side-effects: Because the CLI runs npm install
    unconditionally without checking if package.json exists in widgets/,
    npm automatically traverses up the directory tree looking for one
    (audited 1,839 packages — found one in a parent workspace
    directory). This can accidentally modify node_modules or
    package-lock.json files higher up.
  - Unfriendly Error Propagation: Raw Rust OS-level I/O error
    (Caused by: No such file or directory (os error 2)) instead of a
    helpful diagnostic.

  Recommended fixes:
  1. Graceful File Detection: use Path::exists() before reading
     package.json or running npm.
  2. Support Static/Raw Workflows: if package.json is absent, gracefully
     skip npm install and Node.js dependency parsing — proceed directly
     to serving/testing the static files.
  3. Better Error Messaging: human-readable error if Node env is truly
     required (e.g. "Error: test harness requires a package.json in the
     widgets directory.")

  Reference reproduction: ~/projects/mcp/Scientific-Calculator-MCP-App
severity: major

### 4. Missing node_modules triggers auto-install
expected: |
  In a `widget/` with `package.json` but no `node_modules/`, run
  `cargo pmcp deploy --widgets-only`. The detected PM runs `install`
  automatically, then runs `build`. (Yarn PnP projects with `.pnp.cjs`
  or `.pnp.loader.mjs` skip the install step.)
result: pass

### 5. Widget build failure aborts entire deploy
expected: |
  Force a failing widget build (e.g. broken `widget/build.js`).
  Run `cargo pmcp deploy`. The deploy aborts BEFORE `cargo build --release`
  runs. Error message is verbatim and actionable (mentions the failed
  widget script + exit status).
result: pass

### 6. on_failure="rollback" HARD-REJECTED at CLI flag (HIGH-G2)
expected: |
  Run `cargo pmcp deploy --on-test-failure=rollback`. clap rejects
  the value with the verbatim ROLLBACK_REJECT_MESSAGE: "on_failure='rollback'
  is not yet implemented in this version of cargo-pmcp. Change to 'fail'
  (default) or 'warn'. Auto-rollback support will land in a future phase
  that verifies the existing DeployTarget::rollback() trait implementations."
  Exit code is non-zero.
result: pass

### 7. on_failure="rollback" HARD-REJECTED at TOML parse (HIGH-G2)
expected: |
  Add `[post_deploy_tests]\non_failure = "rollback"` to
  `.pmcp/deploy.toml`. Run any `cargo pmcp deploy` invocation that
  loads that config. Config load fails with the same verbatim
  ROLLBACK_REJECT_MESSAGE before any deploy work begins.
result: pass

### 8. cargo pmcp doctor warns on missing build.rs for include_str! crates
expected: |
  In a crate that uses `include_str!("widgets/foo.html")` but has no
  `build.rs`, run `cargo pmcp doctor`. Output includes a WARNING that
  recommends `cargo clean -p <crate>` ONCE after adding the build.rs.
  In a default WidgetDir scaffold (no `include_str!`), the same check
  emits NO warning.
result: pass

### 9. cargo pmcp app new --embed-widgets writes build.rs scaffold
expected: |
  Run `cargo pmcp app new my_app --embed-widgets`. Generated project
  contains a `build.rs` file with `cargo:rerun-if-changed` directives
  that consume `PMCP_WIDGET_DIRS` (split on `:`) AND a local
  `discover_local_widget_dirs()` fallback walking `CARGO_MANIFEST_DIR`
  parents up to 3 levels.
result: skipped

### 10. cargo pmcp app new (default) does NOT include build.rs
expected: |
  Run `cargo pmcp app new my_app` (no `--embed-widgets`). Generated
  project uses runtime WidgetDir (file serving) and contains NO
  `build.rs` and NO `include_str!`. This is the default scaffold per
  Codex MED scaffold-opt-in.
result: skipped

### 11. Post-deploy lifecycle runs warmup → check → conformance → apps
expected: |
  Run `cargo pmcp deploy` against a deployable project with widgets.
  After Lambda hot-swap, console shows in order:
  warmup grace (default 2000ms) → `cargo pmcp test check` →
  `cargo pmcp test conformance` → `cargo pmcp test apps --mode claude-desktop`
  (apps step only when widgets present). All 4 steps consume
  `--format=json` (no regex parsing of pretty output).
result: pass

### 12. Failure banner format on broken-but-live (exit 3)
expected: |
  When post-deploy tests fail, the banner explicitly says the
  Lambda revision IS LIVE, includes the metric in
  `(passed/total noun passed)` or `(failed/total noun failed)`
  format (noun = "tests" or "widgets" depending on which step failed),
  and includes a rollback command. Process exits with code 3 (NOT 2,
  which is reserved for infra errors).
result: pass

### 13. CI=true emits ::error:: annotation on broken-but-live
expected: |
  Run a deploy that triggers a broken-but-live state with `CI=true`
  in env. stderr contains the GitHub Actions / GitLab annotation
  `::error::Deployment succeeded but post-deploy tests failed (exit code 3).
  Lambda revision is LIVE.` Banner is still printed (annotation
  augments, not replaces).
result: pass
evidence: |
  ::error::Deployment succeeded but post-deploy tests failed (exit code 3). Lambda revision is LIVE. To roll back: cargo pmcp deploy rollback --target pmcp-run

### 14. widget_prebuild_demo example runs to completion
expected: |
  Run `cargo run -p cargo-pmcp --example widget_prebuild_demo`.
  Process exits 0 and prints
  `=== Example complete — Phase 79 build half verified end-to-end ===`
  (or equivalent). Demo exercises the schema-direct path.
result: skipped

## Summary

total: 14
passed: 10
issues: 1
pending: 0
skipped: 3
blocked: 0

## Gaps

- truth: "Widget pre-build supports the documented zero-config raw HTML / CDN-import widget use case (single-file widgets without package.json)"
  status: failed
  reason: "User reported: widgets/ without package.json hard-crashes with raw OS error 2 (No such file or directory). CLI runs `npm install` unconditionally — npm walks up to a parent workspace, audits ~1839 packages, and may modify node_modules / package-lock.json outside the project. Recommended: Path::exists() guard before reading package.json, gracefully skip npm install for raw HTML widgets, replace OS panic with human-readable diagnostic. Reproduction: ~/projects/mcp/Scientific-Calculator-MCP-App"
  severity: major
  test: 3
  root_cause: ""
  artifacts: []
  missing:
    - "Path::exists() guard for widgets/package.json before any npm invocation"
    - "Skip npm install + build steps when package.json is absent (raw HTML / CDN widgets)"
    - "Replace raw std::io::Error os-error-2 propagation with a friendly diagnostic if a Node env IS strictly required by the chosen mode"
  debug_session: ""
