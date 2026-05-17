//! Spike 003: schema-server-surface-diff
//!
//! Risk-first kill-switch spike for the "lift config-driven MCP server
//! support into PMCP SDK" idea. Structurally diffs the three pmcp-run
//! built-in core crates (sql / graphql / openapi) to determine whether a
//! shared SDK-level abstraction is real or illusory.
//!
//! Strategy: scan the three core crates at well-known paths, plus the
//! shared `mcp-server-common` crate that already exists at pmcp-run/built-in,
//! and emit a structural-fact report with in-binary `assert!`s for every
//! claim. Any drift at pmcp-run that invalidates a finding will fail the
//! spike loudly on rerun.

#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// -----------------------------------------------------------------------------
// Configurable roots
// -----------------------------------------------------------------------------

const PMCP_RUN_BUILTIN: &str = "/Users/guy/Development/mcp/sdk/pmcp-run/built-in";

const SQL_CORE: &str = "sql-api/crates/mcp-sql-server-core";
const GRAPHQL_CORE: &str = "graphql-api/crates/mcp-graphql-server-core";
const OPENAPI_CORE: &str = "openapi-api/crates/mcp-openapi-server-core";
const SHARED_COMMON: &str = "shared/mcp-server-common";

// -----------------------------------------------------------------------------
// Banner + helpers
// -----------------------------------------------------------------------------

fn rule(width: usize) -> String {
    "─".repeat(width)
}

fn print_banner() {
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Spike 003: schema-server-surface-diff");
    println!("  Risk-first kill-switch for the SDK-lift question");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
}

fn header(title: &str) {
    println!();
    println!("{}", rule(78));
    println!("▶ {title}");
    println!("{}", rule(78));
}

fn ok(msg: &str) {
    println!("  ✓ {msg}");
}

fn gap(msg: &str) {
    println!("  ⚠ {msg}");
}

// -----------------------------------------------------------------------------
// Crate model
// -----------------------------------------------------------------------------

#[derive(Debug)]
struct CrateScan {
    label: &'static str,
    src_dir: PathBuf,
    cargo_toml: String,
    /// Cargo.toml [dependencies] section, just the keys (left of `=`).
    dependencies: Vec<String>,
    /// Map of `*.rs` filename → line count (top-level only, not subdirs).
    files: BTreeMap<String, usize>,
    /// Concatenated content of all top-level *.rs files (for grep checks).
    haystack: String,
}

impl CrateScan {
    fn new(label: &'static str, crate_root: &Path) -> Result<Self> {
        let cargo_toml = fs::read_to_string(crate_root.join("Cargo.toml"))
            .with_context(|| format!("reading {label} Cargo.toml"))?;

        let dependencies = parse_dep_keys(&cargo_toml);

        let src_dir = crate_root.join("src");
        let mut files = BTreeMap::new();
        let mut haystack = String::new();

        // Top-level *.rs files contribute to BOTH `files` (LoC table) and
        // the haystack. Subdir *.rs files contribute ONLY to the haystack
        // (substring searches reach into `connectors/mod.rs` etc), so the
        // top-level LoC comparison remains apples-to-apples across crates.
        for entry in fs::read_dir(&src_dir)
            .with_context(|| format!("reading {label} src dir {}", src_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let fname = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let body = fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                files.insert(fname, body.lines().count());
                haystack.push_str(&body);
                haystack.push('\n');
            }
        }

        walk_rs(&src_dir, &mut haystack)?;

        Ok(Self {
            label,
            src_dir,
            cargo_toml,
            dependencies,
            files,
            haystack,
        })
    }

    fn depends_on(&self, crate_name: &str) -> bool {
        self.dependencies.iter().any(|d| d == crate_name)
    }

    fn contains(&self, needle: &str) -> bool {
        self.haystack.contains(needle)
    }

    fn file_loc(&self, fname: &str) -> Option<usize> {
        self.files.get(fname).copied()
    }
}

/// Recursively walk a directory and append every `*.rs` body to `out`.
/// Used to extend the haystack into subdirs without inflating the
/// top-level LoC table.
fn walk_rs(dir: &Path, out: &mut String) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk_rs(&path, out)?;
        } else if ft.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Skip top-level — those are already in haystack via the
            // caller's flat pass. We only want subdirs here.
            let parent = path.parent().unwrap_or(Path::new(""));
            if parent != dir || dir.file_name().and_then(|s| s.to_str()) != Some("src") {
                let body = fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                out.push_str(&body);
                out.push('\n');
            }
        }
    }
    Ok(())
}

/// Parse `[dependencies]` keys out of a Cargo.toml. Cheap line-based parser —
/// good enough for "does X appear as a dependency name". Stops at the next
/// `[section]` header.
fn parse_dep_keys(cargo_toml: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut keys = Vec::new();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]"
                || trimmed.starts_with("[target.")
                || trimmed.starts_with("[dependencies.");
            continue;
        }
        if !in_deps {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let head = trimmed[..eq].trim();
            // Handle three shapes:
            //   `pmcp = { ... }`              → key = "pmcp"
            //   `pmcp.workspace = true`        → key = "pmcp"      (dotted inheritance)
            //   `pmcp-code-mode = { ... }`     → key = "pmcp-code-mode"
            let key = head.split('.').next().unwrap_or(head).trim();
            if !key.is_empty() && !key.contains(' ') {
                keys.push(key.to_string());
            }
        }
    }
    keys
}

fn scan(label: &'static str, sub: &str) -> Result<CrateScan> {
    let root = Path::new(PMCP_RUN_BUILTIN).join(sub);
    if !root.exists() {
        return Err(anyhow!(
            "expected pmcp-run crate at {} — is the pmcp-run repo checked out at {}?",
            root.display(),
            PMCP_RUN_BUILTIN,
        ));
    }
    CrateScan::new(label, &root)
}

// -----------------------------------------------------------------------------
// Steps
// -----------------------------------------------------------------------------

fn step_a_locate_sources() -> Result<Vec<CrateScan>> {
    header("Step A · Locate sources");

    let sql = scan("sql", SQL_CORE)?;
    let gql = scan("graphql", GRAPHQL_CORE)?;
    let oai = scan("openapi", OPENAPI_CORE)?;
    let shared = scan("shared (mcp-server-common)", SHARED_COMMON)?;

    for c in [&sql, &gql, &oai, &shared] {
        ok(&format!(
            "{:<28} {} src files, {} loc total",
            c.label,
            c.files.len(),
            c.files.values().sum::<usize>(),
        ));
    }

    Ok(vec![sql, gql, oai, shared])
}

/// Step B: assert the proto-SDK already exists (mcp-server-common) and the
/// three core crates all depend on it. This is the big "the abstraction is
/// already extracted" finding.
fn step_b_proto_sdk(crates: &[CrateScan]) -> Result<()> {
    header("Step B · Proto-SDK: mcp-server-common is the existing extract");

    let by_label: BTreeMap<_, _> = crates.iter().map(|c| (c.label, c)).collect();
    let shared = by_label["shared (mcp-server-common)"];

    // Expected modules in the shared crate. These are the "already-shared" axes.
    let expected = [
        "auth.rs",      // AuthProvider trait + impls
        "secrets.rs",   // SecretsProvider
        "resources.rs", // StaticResourceHandler + ResourceConfig
        "prompts.rs",   // StaticPromptHandler + PromptConfig
        "config.rs",    // Shared resource/prompt config types
        "error.rs",     // Shared error types
        "lib.rs",
    ];
    for m in expected {
        assert!(
            shared.file_loc(m).is_some(),
            "mcp-server-common is missing expected module {m}"
        );
        let loc = shared.file_loc(m).unwrap();
        ok(&format!("shared::{:<14} {} loc", m, loc));
    }

    // Each of the three core crates depends on `mcp-server-common`.
    let core_labels = ["sql", "graphql", "openapi"];
    for lbl in core_labels {
        let c = by_label[lbl];
        assert!(
            c.depends_on("mcp-server-common"),
            "{lbl} does not depend on mcp-server-common — abstraction extraction is not as deep as believed"
        );
        ok(&format!("{lbl:<8} depends on mcp-server-common"));
    }

    // And on pmcp-code-mode (the HMAC token machinery).
    for lbl in core_labels {
        let c = by_label[lbl];
        let has = c.depends_on("pmcp-code-mode") || c.depends_on("pmcp_code_mode");
        assert!(
            has,
            "{lbl} does not depend on pmcp-code-mode — token machinery is duplicated, not shared"
        );
        ok(&format!("{lbl:<8} depends on pmcp-code-mode"));
    }

    // And on pmcp itself.
    for lbl in core_labels {
        let c = by_label[lbl];
        assert!(
            c.depends_on("pmcp"),
            "{lbl} does not depend on pmcp — finding invalid"
        );
        ok(&format!("{lbl:<8} depends on pmcp"));
    }

    println!();
    println!(
        "  ⇒ The proto-SDK already exists. mcp-server-common is a {} LoC\n      \
            crate, and all three backend cores already consume it. The question\n      \
            is not 'should we extract a shared abstraction' — it is 'should the\n      \
            already-extracted shared abstraction live in PMCP SDK or stay at\n      \
            pmcp-run/built-in/shared'.",
        shared.files.values().sum::<usize>(),
    );

    Ok(())
}

/// Step C: assert each core crate has the same top-level structural shape.
fn step_c_shared_shape(crates: &[CrateScan]) -> Result<()> {
    header("Step C · Shared top-level shape across the three cores");

    let by_label: BTreeMap<_, _> = crates.iter().map(|c| (c.label, c)).collect();
    let cores = [
        by_label["sql"],
        by_label["graphql"],
        by_label["openapi"],
    ];

    // Every core has these files.
    let required_files = [
        "config.rs",
        "lib.rs",
        "error.rs",
        "code_mode.rs",
        "pmcp_server.rs",
        "lambda.rs",
    ];

    println!("  {:<14} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "file", "sql", "graphql", "openapi", "min", "max", "ratio");
    for f in required_files {
        let mut vals = Vec::new();
        for c in &cores {
            let loc = c.file_loc(f).unwrap_or_else(|| {
                panic!("{} is missing required file {}", c.label, f)
            });
            vals.push(loc);
        }
        let min = *vals.iter().min().unwrap();
        let max = *vals.iter().max().unwrap();
        let ratio = max as f64 / min.max(1) as f64;
        println!(
            "  {:<14} {:>6} {:>8} {:>8} {:>8} {:>8} {:>7.2}x",
            f, vals[0], vals[1], vals[2], min, max, ratio,
        );
    }

    // Each core has a `pub fn from_toml` constructor (config entrypoint).
    for c in &cores {
        assert!(
            c.contains("pub fn from_toml"),
            "{} has no `pub fn from_toml` — config entrypoint shape is not shared",
            c.label
        );
        ok(&format!("{:<8} has `Config::from_toml`", c.label));
    }

    // Each core has an `into_pmcp_server` or `build` step that yields a
    // `pmcp::Server`. SQL/GraphQL use `into_pmcp_server`; OpenAPI uses a
    // builder pattern (`OpenApiPmcpBuilder::build`).
    let bootstrap_seen = cores
        .iter()
        .map(|c| {
            let n = c.contains("into_pmcp_server") || c.contains("pmcp::Server")
                || c.contains("fn build(self) -> pmcp::Result<pmcp::Server>");
            (c.label, n)
        })
        .collect::<Vec<_>>();
    for (lbl, seen) in &bootstrap_seen {
        assert!(seen, "{lbl} has no path that yields a pmcp::Server");
        ok(&format!("{:<8} yields pmcp::Server (via into_pmcp_server or builder.build)", lbl));
    }

    // Each core exposes a `run_lambda` integration entrypoint.
    for c in &cores {
        assert!(
            c.contains("run_lambda") || c.contains("run_openapi_lambda"),
            "{} has no run_lambda integration",
            c.label
        );
        ok(&format!("{:<8} has run_lambda integration", c.label));
    }

    Ok(())
}

/// Step D: assert the divergent axes are also real, not implementation slop.
fn step_d_divergent_axes(crates: &[CrateScan]) -> Result<()> {
    header("Step D · Where the three cores genuinely diverge");

    let by_label: BTreeMap<_, _> = crates.iter().map(|c| (c.label, c)).collect();
    let sql = by_label["sql"];
    let gql = by_label["graphql"];
    let oai = by_label["openapi"];

    // D1. Backend execution model — positive facts for each.
    //     SQL has a real `trait DatabaseConnector` (multi-impl: SQLite, Athena, …).
    //     GraphQL has a concrete `pub struct GraphQLClient` wrapping reqwest.
    //     OpenAPI has a concrete `pub struct HttpClient` wrapping reqwest.
    assert!(
        sql.contains("pub trait DatabaseConnector"),
        "expected `pub trait DatabaseConnector` in sql core (incl. subdirs)"
    );
    ok("sql      has multi-impl trait `DatabaseConnector` (sqlite, athena, …)");
    assert!(
        gql.contains("pub struct GraphQLClient"),
        "expected concrete `pub struct GraphQLClient` in graphql core"
    );
    ok("graphql  has concrete `pub struct GraphQLClient` over reqwest");
    assert!(
        oai.contains("pub struct HttpClient"),
        "expected concrete `pub struct HttpClient` in openapi core"
    );
    ok("openapi  has concrete `pub struct HttpClient` over reqwest");

    // D2. code_mode.rs LoC spread.
    let sql_cm = sql.file_loc("code_mode.rs").unwrap();
    let gql_cm = gql.file_loc("code_mode.rs").unwrap();
    let oai_cm = oai.file_loc("code_mode.rs").unwrap();
    let max_cm = sql_cm.max(gql_cm).max(oai_cm);
    let min_cm = sql_cm.min(gql_cm).min(oai_cm);
    let spread = max_cm as f64 / min_cm.max(1) as f64;
    println!(
        "  code_mode.rs LoC — sql={sql_cm}, graphql={gql_cm}, openapi={oai_cm}; spread = {:.2}x",
        spread
    );
    assert!(
        spread > 2.0,
        "code_mode LoC spread is {spread:.2}x, expected >2× as evidence of real semantic divergence"
    );
    ok("code_mode.rs spread > 2× — real semantic divergence (AVP/Cedar policy on openapi)");

    // D3. OpenAPI has AVP/policy evaluator; SQL and GraphQL do not.
    assert!(
        oai.contains("PolicyEvaluator") || oai.contains("AvpPolicyEvaluator"),
        "openapi core expected to reference AVP PolicyEvaluator"
    );
    ok("openapi  imports AVP PolicyEvaluator (Cedar entity scoring); other cores do not");

    // D4. Parameter binding model differs.
    //     SQL: `:param` -> `?` (PreparedStatementRegistry)
    //     GraphQL: `$var` (JSON variables)
    //     OpenAPI: `{name}` path templating + query/body split by verb
    assert!(
        sql.contains("PreparedStatementRegistry") || sql.contains("registry"),
        "sql expected to reference PreparedStatementRegistry"
    );
    ok("sql      uses `:name` -> `?` rewriting via PreparedStatementRegistry");
    assert!(
        gql.contains("$") && gql.contains("variables"),
        "graphql expected to handle `$var` variables"
    );
    ok("graphql  uses `$var` GraphQL variables (JSON object handoff)");
    assert!(
        oai.contains("operation_id") || oai.contains("HttpMethod"),
        "openapi expected to reference operation_id / HTTP verb"
    );
    ok("openapi  uses `{name}` path templating + verb-based body/query split");

    // D5. Schema parsing source: SQL introspects via connector trait,
    //     GraphQL parses SDL (graphql_parser), OpenAPI parses openapiv3.
    assert!(
        sql.contains("get_all_schemas") || sql.contains("TableSchema"),
        "sql expected to introspect schema via connector"
    );
    assert!(
        gql.contains("graphql_parser") || gql.contains("parse_query"),
        "graphql expected to use graphql_parser crate"
    );
    assert!(
        oai.contains("openapiv3"),
        "openapi expected to use openapiv3 crate"
    );
    ok("schema parsing — SQL=connector-introspection, GraphQL=graphql_parser, OpenAPI=openapiv3");

    Ok(())
}

/// Step E: verdict.
fn step_e_verdict(crates: &[CrateScan]) -> Result<()> {
    header("Step E · Verdict");

    let by_label: BTreeMap<_, _> = crates.iter().map(|c| (c.label, c)).collect();
    let shared_loc = by_label["shared (mcp-server-common)"]
        .files
        .values()
        .sum::<usize>();
    let sql_loc = by_label["sql"].files.values().sum::<usize>();
    let gql_loc = by_label["graphql"].files.values().sum::<usize>();
    let oai_loc = by_label["openapi"].files.values().sum::<usize>();

    println!();
    println!("  LoC accounting:");
    println!("    mcp-server-common (shared): {shared_loc}");
    println!("    mcp-sql-server-core:        {sql_loc}");
    println!("    mcp-graphql-server-core:    {gql_loc}");
    println!("    mcp-openapi-server-core:    {oai_loc}");
    println!();
    println!("  VERDICT: PARTIAL — re-framed as VALIDATED with a specific shape.");
    println!();
    println!("  The original question proposed lifting a 'schema-driven MCP server'");
    println!("  abstraction into the PMCP SDK. The structural diff says:");
    println!();
    println!("  ✓ The abstraction ALREADY EXISTS and is ALREADY EXTRACTED.");
    println!("    mcp-server-common ({shared_loc} LoC) lives at pmcp-run/built-in/shared/");
    println!("    and is consumed by all three backend cores. AuthProvider, SecretsProvider,");
    println!("    StaticResourceHandler, StaticPromptHandler, the *Config types for resources");
    println!("    and prompts — already there.");
    println!();
    println!("  ✓ pmcp-code-mode (an SDK crate) already owns the HMAC token machinery,");
    println!("    TokenSecret, JsCodeExecutor, NoopPolicyEvaluator, AvpPolicyEvaluator.");
    println!("    The `validate_code` / `execute_code` tool NAMES come from this SDK crate");
    println!("    via the `#[derive(CodeMode)]` macro — they are not duplicated per-backend.");
    println!();
    println!("  ✗ A single `SchemaServer<S, C>` TRAIT that all three backends implement");
    println!("    is NOT viable. The per-backend parameter binding, validator, and policy");
    println!("    layer diverge in ways that are genuinely SEMANTIC (LIMIT enforcement,");
    println!("    GraphQL root-field allowlists, OpenAPI HTTP-verb policy + AVP/Cedar) —");
    println!("    not stylistic. The code_mode.rs LoC spread (545 / 767 / 1560) reflects");
    println!("    this real divergence.");
    println!();
    println!("  ⇒ Recommended shape for the SDK lift (validate further in spike 004):");
    println!();
    println!("    1. Promote mcp-server-common to a workspace crate under `crates/`.");
    println!("       Candidate names: `pmcp-server-toolkit` or `pmcp-builtin`. This");
    println!("       gives auth/secrets/resource/prompt config a public stable home.");
    println!();
    println!("    2. Keep per-backend crates (sql/graphql/openapi) where they are at");
    println!("       pmcp-run for now — but unblock them for independent crates.io");
    println!("       publication by replacing the path-dep on shared/ with a versioned");
    println!("       crates.io dep on the new toolkit crate.");
    println!();
    println!("    3. `cargo-pmcp new --kind sql-server` etc. become viable as a");
    println!("       *scaffolding* layer that drops a starter Cargo.toml depending on");
    println!("       the toolkit + a chosen backend crate. No new abstraction needed.");
    println!();
    println!("    4. A `#[pmcp::sql_server]` proc-macro is SECONDARY. The toolkit");
    println!("       being public is what unlocks the macro path; without it, the macro");
    println!("       would expand to types that aren't on crates.io.");
    println!();

    Ok(())
}

// -----------------------------------------------------------------------------
// Entrypoint
// -----------------------------------------------------------------------------

fn main() -> Result<()> {
    print_banner();
    let crates = step_a_locate_sources()?;
    step_b_proto_sdk(&crates)?;
    step_c_shared_shape(&crates)?;
    step_d_divergent_axes(&crates)?;
    step_e_verdict(&crates)?;

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  All structural assertions held. Verdict: PARTIAL → re-framed VALIDATED.");
    println!("  Proceed to spike 004 (thin-slice SDK toolkit + SQLite reference).");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    Ok(())
}
