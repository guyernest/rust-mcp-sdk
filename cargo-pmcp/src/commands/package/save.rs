//! `cargo pmcp package save` — pack a configuration server into ONE movable
//! `.tar` file, fully offline.
//!
//! # Every packed field traces to a file the user maintains (D-10)
//!
//! `name`, `version`, `tools` and `config_slots` come from the server's own
//! `config.toml`; `deploy` comes from `.pmcp/deploy.toml` through
//! `load_deploy_descriptor`. Nothing here synthesizes a `DeployDescriptor` — an
//! author who cannot deploy the server cannot package it either, which is the
//! property D-10 exists to keep.
//!
//! # `save` leaves exactly ONE artifact
//!
//! It packs into a TEMPORARY layout, tars that layout to `--output`, and throws
//! the layout away. The tar is the MOVABLE form and the layout directory is the
//! WORKING form (D-11); leaving both behind would invite the question "which
//! one is the package?", which has a wrong answer half the time.
//!
//! # `save` writes SERVER packages only, and the asymmetry with `load` is a decision
//!
//! `load` is kind-agnostic essentially for free: it reuses `detect_kind` and
//! handles whatever it is handed. `save` covers the server kind alone, and
//! refuses any other by name rather than mis-packing it or accepting it with a
//! warning.
//!
//! That asymmetry is deliberate (D-13). **Reading costs nothing; writing does
//! not.** Packing an agent, team or workflow package needs per-kind INPUT
//! semantics — an on-disk source format for each — that nobody has designed
//! yet. Naming the missing design is honest; inventing one here to make the
//! flag symmetric would bake a source format into a CLI flag before anyone had
//! agreed what it should be.
//!
//! So the other three kinds are a DEFERRED DESIGN QUESTION, not a limitation to
//! be quietly filled in. The refusal names the kind so the user learns which
//! thing is missing, rather than discovering it from an artifact that packed
//! and then would not deploy.
//!
//! # The config is read NARROWLY, and deliberately not through the toolkit
//!
//! `pmcp-server-toolkit`'s `ServerConfig` is `#[serde(deny_unknown_fields)]`
//! with a feature-gated `[backend]` section, so a `cargo-pmcp` that adopted it
//! without `features = ["http"]` would REJECT the very london-tube fixture this
//! command is proved against. Only the three things D-10 names are read, using
//! the `toml` crate already present.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{
    pack_server, parse_declared_config_slots, BinaryMode, ConfigFile, DeclaredConfigSlot,
    OciLayout, OpenApiSpecFile,
};
use pmcp_package::package::{CedarPolicySet, ServerPackage, ToolMetadata};
use pmcp_package::{ConfigSlot, ManifestDigest, SlotType};

use super::artifact;
use super::kind::PackageKind;
use crate::commands::GlobalFlags;
use crate::deployment::config::DeployConfig;
use crate::deployment::stack_routing::load_deploy_descriptor;

/// Default descriptive media-type hint for a REFERENCED runtime binary.
///
/// A configuration server NAMES its runtime rather than carrying it, exactly as
/// `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` does. The value is a
/// hint recorded in the binary-ref layer for the target environment; the digest
/// is the load-bearing half and has no default, which is why `--binary-digest`
/// is required and this is not.
const DEFAULT_BINARY_MEDIA_TYPE: &str = "application/x-lambda-bootstrap; arch=arm64";

/// Long help for `--spec`, written out because omitting the flag silently
/// produces a package with no spec layer and the failure then surfaces much
/// later, in the target environment.
const SPEC_LONG_HELP: &str = "\
Path to the OpenAPI specification this server dispatches against.

An OpenAPI-backed Shape A server (the `pmcp-openapi-server` shape) needs its \
spec packed, and this flag is the ONLY way it gets there: the spec path is not \
derivable from the config. Measured on the london-tube fixture, whose \
`[backend]` table carries only `base_url` and names no spec at all.

A pure-configuration server that dispatches without a spec correctly omits \
this flag, and the resulting package simply carries no spec layer.";

/// Long help for `--kind`, written out because the refusal it documents is a
/// deferred design question rather than a bug the user should report.
const KIND_LONG_HELP: &str = "\
The package kind to write. Only `server` is supported today.

`load` reads every kind, but `save` writes only servers, and the asymmetry is \
deliberate: reading a package costs nothing, while WRITING one needs per-kind \
input semantics — an on-disk source format for agent, team and workflow \
packages — that has not been designed. Passing any other kind fails by name \
rather than mis-packing it.";

/// The package kinds `--kind` accepts.
///
/// A SEPARATE enum from [`PackageKind`] rather than a `#[derive(ValueEnum)]` on
/// that type, and not for style: `kind.rs` is `#[path]`-mounted into the
/// LIBRARY target, where it must carry no `clap` dependency at all. Deriving a
/// `clap` trait there would break every lib-target consumer of that leaf at
/// once.
///
/// The mapping below is EXHAUSTIVE with no catch-all, so adding a package kind
/// is a compile error here rather than a kind silently unreachable from the
/// command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SaveKind {
    /// A single agent package.
    Agent,
    /// A team package.
    Team,
    /// An mcp-server package — the only kind `save` writes today.
    Server,
    /// A workflow manifest.
    Workflow,
}

impl SaveKind {
    /// The [`PackageKind`] this flag value names.
    fn as_package_kind(self) -> PackageKind {
        match self {
            SaveKind::Agent => PackageKind::Agent,
            SaveKind::Team => PackageKind::Team,
            SaveKind::Server => PackageKind::Server,
            SaveKind::Workflow => PackageKind::Workflow,
        }
    }
}

/// Refuse a kind `save` cannot pack, BY NAME (D-13).
///
/// Deliberately not a warning and deliberately not a silent fallback to the
/// server path: either would produce an artifact whose contents do not match
/// what the user asked for, and the mismatch would surface much later, in a
/// target environment, as something that packed cleanly and then would not run.
fn refuse_a_kind_save_cannot_pack(kind: SaveKind) -> Result<()> {
    let package_kind = kind.as_package_kind();
    if package_kind == PackageKind::Server {
        return Ok(());
    }
    bail!(
        "`package save` does not yet support the {} kind — it writes server packages only. \
         `package load` reads every kind, but writing one needs an on-disk source format for \
         {} packages that has not been designed yet.",
        package_kind.label(),
        package_kind.label()
    );
}

/// Arguments for `cargo pmcp package save`.
#[derive(Debug, Args)]
pub struct SaveArgs {
    /// Path to the server's `config.toml`.
    #[arg(long)]
    pub config: PathBuf,

    /// The package kind to write. Only `server` is supported today.
    #[arg(long, value_enum, default_value_t = SaveKind::Server, long_help = KIND_LONG_HELP)]
    pub kind: SaveKind,

    /// Path to the OpenAPI spec, for an OpenAPI-backed server.
    #[arg(long, long_help = SPEC_LONG_HELP)]
    pub spec: Option<PathBuf>,

    /// Project root holding `.pmcp/deploy.toml` (defaults to the config's parent).
    #[arg(long)]
    pub project_root: Option<PathBuf>,

    /// Destination tar file (defaults to `<name>-<version>.tar` here).
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Replace an existing `--output` file.
    #[arg(long)]
    pub force: bool,

    /// `sha256:<hex>` digest of the runtime binary the target must resolve.
    #[arg(long)]
    pub binary_digest: String,

    /// Descriptive media-type hint for that runtime binary.
    #[arg(long, default_value = DEFAULT_BINARY_MEDIA_TYPE)]
    pub binary_media_type: String,
}

/// The narrow view of a server `config.toml` that D-10 actually needs.
#[derive(Debug, serde::Deserialize)]
struct ConfigDocument {
    server: ServerTable,
    #[serde(default)]
    tools: Vec<ToolTable>,
}

/// The `[server]` table. `version` is read as a STRING and parsed separately so
/// this command does not depend on `semver`'s `serde` feature being switched on
/// somewhere else in the workspace — feature unification makes that kind of
/// dependency invisible until the day it is severed.
#[derive(Debug, serde::Deserialize)]
struct ServerTable {
    name: String,
    version: String,
}

/// One `[[tools]]` entry. Unknown keys (`path`, `method`, `script`,
/// `parameters`, ...) are ignored by design: this is a narrow read, not a
/// schema.
#[derive(Debug, serde::Deserialize)]
struct ToolTable {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    annotations: Option<serde_json::Value>,
}

/// Map one parsed `[[config_slots]]` declaration onto the package slot it
/// describes.
///
/// Ported from `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs`'s
/// `slot_from_declaration`, with every `panic!` replaced by a typed error —
/// that helper is a test and may abort; this is production code reading a file
/// a user wrote, and a malformed declaration deserves a message rather than a
/// backtrace. The closed `endpoint`/`secret`/`auth_mode` vocabulary is kept
/// exactly.
fn slot_from_declaration(declaration: &DeclaredConfigSlot) -> Result<ConfigSlot> {
    let tested = || {
        declaration.tested_value.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "config slot '{}' is declared as kind '{}', which must carry a `tested_value`",
                declaration.key,
                declaration.kind
            )
        })
    };
    let slot = match declaration.kind.as_str() {
        "endpoint" => SlotType::Endpoint {
            name: declaration.name.clone(),
            tested_value: tested()?,
        },
        "secret" => SlotType::Secret {
            name: declaration.name.clone(),
        },
        "auth_mode" => SlotType::AuthMode {
            name: declaration.name.clone(),
            tested_value: tested()?,
        },
        unexpected => bail!(
            "config slot '{}' declares an unsupported kind '{unexpected}' — the closed \
             vocabulary is endpoint | secret | auth_mode",
            declaration.key
        ),
    };
    Ok(ConfigSlot::new(slot).with_config_key(declaration.key.as_str()))
}

/// Build the `ServerPackage` from the two files the user maintains (D-10).
fn package_from_files(config_bytes: &[u8], project_root: &Path) -> Result<ServerPackage> {
    let text = std::str::from_utf8(config_bytes).context("the server config is not valid UTF-8")?;
    let document: ConfigDocument =
        toml::from_str(text).context("parse the server config's [server] and [[tools]] tables")?;

    let version = semver::Version::parse(&document.server.version).with_context(|| {
        format!(
            "[server].version is '{}', which is not valid semver",
            document.server.version
        )
    })?;

    let declared = parse_declared_config_slots(config_bytes)
        .context("read the server config's [[config_slots]] declaration block")?;
    let config_slots = declared
        .iter()
        .map(slot_from_declaration)
        .collect::<Result<Vec<_>>>()?;

    // Pitfall 6, and a DELIBERATE divergence written here at the call site: a
    // reader who follows `load_deploy_descriptor` to its own rustdoc will read
    // the OPPOSITE instruction, because its existing callers treat a parse
    // failure as a graceful legacy-deploy fallback. For `save` it is a HARD
    // error. Falling back would produce a package whose deploy target is
    // defaulted rather than authored, which is precisely the outcome D-10
    // exists to prevent — and the package would then look fine until it was
    // deployed somewhere wrong. This verb produces an artifact whose whole
    // purpose is to be trusted in ANOTHER environment, so a deploy target that
    // could not be read is a refusal, never a default.
    //
    // The two failures stay distinguishable to the user, which is the point of
    // making either one an error at all. `DeployConfig::load` reports a MISSING
    // file ("Deployment not initialized"); the `load_deploy_descriptor` context
    // below reports one that will not PARSE, naming the file and telling the
    // user to fix the offending table. The underlying `toml` error rides the
    // `anyhow` cause chain, so `-v` shows which table it choked on.
    //
    // NO PRODUCTION PATH IN THIS REPO CONSTRUCTS A `DeployDescriptor` FROM
    // LITERALS, and none may. The only literal-built descriptors are in tests,
    // where they exist so a test can pack without a project on disk. Copying
    // one into this function would bake a deploy target, region and memory the
    // user never chose into an artifact whose entire purpose is to be trusted
    // elsewhere — a defaulted deploy target is indistinguishable from an
    // authored one once it is inside the package.
    let deploy_config = DeployConfig::load(project_root)
        .with_context(|| format!("read {}/.pmcp/deploy.toml", project_root.display()))?;
    let deploy = load_deploy_descriptor(&deploy_config).with_context(|| {
        format!(
            "{}/.pmcp/deploy.toml does not parse as a deploy descriptor. `save` refuses to \
             substitute a default here: fix the offending table in deploy.toml and re-run.",
            project_root.display()
        )
    })?;

    Ok(ServerPackage {
        name: document.server.name,
        version,
        digest: None,
        deploy,
        policies: CedarPolicySet(vec![]),
        tools: document
            .tools
            .into_iter()
            .map(|tool| ToolMetadata {
                name: tool.name,
                description: tool.description,
                annotations: tool.annotations,
            })
            .collect(),
        config_slots,
    })
}

/// Pack a configuration server into one movable tar.
pub fn execute(args: SaveArgs, global_flags: &GlobalFlags) -> Result<()> {
    // FIRST, before any file is read or written. A kind this command cannot
    // pack is refused before it can produce a partial artifact.
    refuse_a_kind_save_cannot_pack(args.kind)?;

    let config_bytes = std::fs::read(&args.config)
        .with_context(|| format!("read the server config {}", args.config.display()))?;
    let config_file_name = args
        .config
        .file_name()
        .and_then(|n| n.to_str())
        .context("--config must name a file")?
        .to_string();

    let project_root = match &args.project_root {
        Some(root) => root.clone(),
        None => args
            .config
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    };

    let package = package_from_files(&config_bytes, &project_root)?;

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{}-{}.tar", package.name, package.version)));
    if output.exists() && !args.force {
        bail!(
            "{} already exists — refusing to overwrite it. Pass --force to replace it.",
            output.display()
        );
    }

    let binary_digest = ManifestDigest::parse(&args.binary_digest).with_context(|| {
        format!(
            "--binary-digest '{}' is not a sha256:<hex> digest",
            args.binary_digest
        )
    })?;

    let spec_bytes = match &args.spec {
        Some(path) => Some((
            path.file_name()
                .and_then(|n| n.to_str())
                .context("--spec must name a file")?
                .to_string(),
            std::fs::read(path)
                .with_context(|| format!("read the OpenAPI spec {}", path.display()))?,
        )),
        None => None,
    };

    let staging = tempfile::tempdir().context("create the temporary pack layout")?;
    let layout = OciLayout::create(staging.path()).context("create the temporary pack layout")?;
    pack_server(
        &package,
        BinaryMode::Referenced {
            digest: binary_digest,
            media_type: args.binary_media_type.clone(),
        },
        Some(ConfigFile {
            file_name: &config_file_name,
            bytes: &config_bytes,
        }),
        spec_bytes
            .as_ref()
            .map(|(file_name, bytes)| OpenApiSpecFile { file_name, bytes }),
        // Attestations are issued by the platform, never by this CLI (D-15).
        // The sixth parameter is Phase 122's addition and is passed explicitly
        // rather than skipped.
        None,
        &layout,
    )
    .context("pack the server package")?;

    // Normalize the index THIS command just produced, before tarring it.
    //
    // `finalize_pack` attaches the manifest descriptor's `name`/`version`
    // annotations as a `HashMap`, whose serialization order is randomized per
    // process — so without this, two `save` runs on identical inputs emit
    // byte-DIFFERENT artifacts (measured: three of four runs agreed, the fourth
    // flipped the two keys) even though every blob, and therefore the package's
    // identity digest, was byte-identical.
    //
    // This is `save` normalizing its OWN output. `write_tar` below still reads
    // the layout verbatim and re-serializes nothing, so a third-party artifact
    // is still carried exactly as it arrived.
    let index = layout
        .read_index()
        .context("read back the packed index.json")?;
    artifact::write_canonical_index(&layout, &index)?;

    artifact::write_tar(&layout, &output)?;
    drop(staging);

    if global_flags.should_output() {
        println!(
            "\n{} {}@{} {} {}",
            "Saved".bright_green().bold(),
            package.name,
            package.version,
            "->".bright_black(),
            output.display()
        );
    }
    Ok(())
}
