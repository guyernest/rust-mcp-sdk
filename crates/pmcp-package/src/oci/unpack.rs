//! `unpack_server`/`unpack_agent`/`unpack_team`/`unpack_workflow` — read a
//! local [`OciLayout`] back into a typed package struct, verifying every
//! blob's digest BEFORE deserializing (RESEARCH System
//! Architecture Diagram, unpack() steps 1-3):
//!
//! 1. Read the manifest + blobs from the layout.
//! 2. Recompute sha256 of each blob and compare against its declared
//!    digest (`verify()` — a tampered blob fails HERE, before any
//!    deserialize).
//! 3. Deserialize each verified layer back to its typed struct
//!    (`deny_unknown_fields` on `DeployDescriptor` enforces).
//!
//! A `ServerPackage`'s layers are located by MEDIA TYPE, never by position:
//! the config and spec layers are optional and the binary layer is one of two
//! mutually exclusive media types, so no positional contract can hold. A
//! `ServerPackage` layout is indexed once by [`index_layers`] and every read
//! goes through that index. `AgentPackage`/`TeamPackage`/`WorkflowManifest`
//! each remain a single layer. The embedded bootstrap layer is returned as raw
//! bytes — it is never deserialized.

use crate::digest::{verify, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::oci::layout::OciLayout;
use crate::oci::media_types::{
    MT_SERVER_BINARY_REF, MT_SERVER_BOOTSTRAP, MT_SERVER_CEDAR_POLICY_SET, MT_SERVER_CONFIG,
    MT_SERVER_CONFIG_SLOTS, MT_SERVER_DEPLOY_DESCRIPTOR, MT_SERVER_ENVELOPE,
    MT_SERVER_OPENAPI_SPEC, MT_SERVER_TOOL_METADATA,
};
use crate::oci::pack::ServerEnvelope;
use crate::oci::SingleLayerPackage;
use crate::package::{AgentPackage, BinaryRef, ServerPackage, TeamPackage, WorkflowManifest};
use oci_spec::image::{Descriptor, ImageManifest, ANNOTATION_TITLE};
use std::collections::BTreeMap;

/// Read+verify a blob's bytes given its `Descriptor`: convert the
/// Descriptor's OCI digest into a [`ManifestDigest`] (the validated
/// boundary conversion — threat register: "OCI Descriptor digest →
/// ManifestDigest"), then [`crate::digest::verify()`] it against the actual bytes read from
/// disk (a byte-flipped blob fails HERE, before any deserialize
/// is attempted).
fn read_verified_blob(layout: &OciLayout, descriptor: &Descriptor) -> Result<Vec<u8>> {
    let expected = ManifestDigest::try_from(descriptor.digest())?;
    let bytes = layout.read_blob(descriptor)?;
    verify(&expected, &bytes)?;
    Ok(bytes)
}

/// Read `index.json`, verify the (single) manifest entry's digest, and
/// parse it. Every layout this crate produces holds exactly one package.
fn read_the_one_manifest(layout: &OciLayout) -> Result<ImageManifest> {
    let index = layout.read_index()?;
    let manifests = index.manifests();
    if manifests.len() != 1 {
        return Err(PackageError::Layout {
            reason: format!(
                "expected exactly one manifest in index.json, found {}",
                manifests.len()
            ),
        });
    }
    let bytes = read_verified_blob(layout, &manifests[0])?;
    let manifest = serde_json::from_slice(&bytes)?;
    Ok(manifest)
}

/// Verify the manifest's config blob (the standard empty-config blob) —
/// every blob referenced by the manifest is digest-verified, including the
/// one that carries no meaningful payload.
fn verify_config_blob(layout: &OciLayout, manifest: &ImageManifest) -> Result<()> {
    read_verified_blob(layout, manifest.config())?;
    Ok(())
}

fn missing_layer(name: &str) -> PackageError {
    PackageError::Layout {
        reason: format!("manifest is missing the '{name}' layer"),
    }
}

/// The binary a packed server names — the unpack-side mirror of
/// [`crate::oci::pack::BinaryMode`]. Exactly one arm is produced per package.
///
/// [`UnpackedBinary::Referenced`] deliberately has NO field holding binary
/// bytes (D-06/D-07): unpacking a referenced package is a local, offline
/// operation that never resolves, looks up, substitutes or falls back to a
/// locally present blob. The same package therefore unpacks to the same shape
/// in every environment; resolving the digest to actual bytes is the target
/// environment's job, not this crate's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnpackedBinary {
    /// The package embedded its binary; these are its exact bytes.
    Embedded(Vec<u8>),
    /// The package referenced a binary it does not carry.
    Referenced {
        /// Content digest of the binary the target environment must resolve.
        digest: ManifestDigest,
        /// Descriptive media-type hint recorded at pack time.
        media_type: String,
    },
}

/// A verbatim vendor-content file restored from a layer, under the original
/// file name the author packed it with.
///
/// `file_name` comes from the layer descriptor's
/// `org.opencontainers.image.title` annotation, which is ATTACKER-CONTROLLED
/// input from an untrusted layout. It is returned as DATA only: this crate
/// never writes to disk using it and never builds a path from it. A caller
/// that does write these bytes out is responsible for validating the name
/// against its own destination directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredFile {
    /// The file's original name, e.g. `london-tube.toml`.
    pub file_name: String,
    /// The file's exact bytes, byte-identical to what was packed.
    pub bytes: Vec<u8>,
}

/// Everything a packed server layout yields: the typed package, its binary
/// (embedded bytes or a reference), and its optional verbatim config and spec
/// files.
#[derive(Debug, Clone, PartialEq)]
pub struct UnpackedServer {
    /// The typed `ServerPackage` reassembled from the envelope + typed layers.
    pub package: ServerPackage,
    /// The package's one binary layer, in whichever of the two shapes it had.
    pub binary: UnpackedBinary,
    /// The author's config file, if the package carried one.
    pub config: Option<RestoredFile>,
    /// The OpenAPI spec file, if the package carried one.
    pub spec: Option<RestoredFile>,
}

/// Index a manifest's layers by media type so every read is keyed by WHAT a
/// layer is rather than WHERE it sits (D-11).
///
/// A duplicate media type is rejected with [`PackageError::Layout`] naming the
/// duplicated type — never last-wins. Silently keeping one of two layers with
/// the same media type would let a crafted layout shadow the real config,
/// deploy descriptor or binary reference with an attacker's.
fn index_layers(manifest: &ImageManifest) -> Result<BTreeMap<String, &Descriptor>> {
    let mut index: BTreeMap<String, &Descriptor> = BTreeMap::new();
    for layer in manifest.layers() {
        let media_type = layer.media_type().to_string();
        if index.insert(media_type.clone(), layer).is_some() {
            return Err(PackageError::Layout {
                reason: format!("manifest carries more than one '{media_type}' layer"),
            });
        }
    }
    Ok(index)
}

/// Read the package's ONE binary layer, enforcing exactly-one-of: a package
/// carrying both an embedded bootstrap and a binary reference, or neither, is
/// a malformed layout.
///
/// The "reference has a digest" check is scoped to the WIRE decode on purpose.
/// [`BinaryRef::digest`] is `Option<ManifestDigest>` for wire tolerance, so a
/// crafted layer can decode to `None` — that is the only place a missing
/// digest can appear, and it is rejected here so the target environment is
/// never handed an instruction to run an unpinned binary. The API type
/// [`crate::oci::pack::BinaryMode::Referenced`]'s `digest` is a non-optional,
/// already-validated [`ManifestDigest`] whose only constructors enforce
/// `sha256:<64-hex>`, so an empty digest is unconstructible there and a
/// second check on it would be dead code.
fn read_binary_mode(
    layout: &OciLayout,
    by_media_type: &BTreeMap<String, &Descriptor>,
) -> Result<UnpackedBinary> {
    let bootstrap = by_media_type.get(MT_SERVER_BOOTSTRAP);
    let binary_ref = by_media_type.get(MT_SERVER_BINARY_REF);

    match (bootstrap, binary_ref) {
        (Some(_), Some(_)) => Err(PackageError::Layout {
            reason: "manifest carries BOTH an embedded bootstrap layer and a binary-ref layer \
                 (exactly one is required)"
                .to_string(),
        }),
        (None, None) => Err(missing_layer("bootstrap or binary-ref")),
        (Some(descriptor), None) => {
            // The bootstrap layer stays raw bytes — it is NEVER deserialized.
            Ok(UnpackedBinary::Embedded(read_verified_blob(
                layout, descriptor,
            )?))
        },
        (None, Some(descriptor)) => {
            let bytes = read_verified_blob(layout, descriptor)?;
            let wire: BinaryRef = serde_json::from_slice(&bytes)?;
            let digest = wire.digest.ok_or_else(|| PackageError::Layout {
                reason: "binary-ref layer carries no digest".to_string(),
            })?;
            Ok(UnpackedBinary::Referenced {
                digest,
                media_type: wire.media_type,
            })
        },
    }
}

/// Read a verbatim vendor-content layer back into a [`RestoredFile`], taking
/// the original file name from the descriptor's `org.opencontainers.image.title`
/// annotation. Returns `Ok(None)` when the layer is absent — both the config
/// and the spec layer are optional.
///
/// A layer present without that annotation is a malformed layout: the file
/// name is part of what was packed, and inventing a substitute would silently
/// rename the author's file.
fn read_named_file_layer(
    layout: &OciLayout,
    descriptor: Option<&&Descriptor>,
    layer_name: &str,
) -> Result<Option<RestoredFile>> {
    let Some(descriptor) = descriptor else {
        return Ok(None);
    };
    let bytes = read_verified_blob(layout, descriptor)?;
    let file_name = descriptor
        .annotations()
        .as_ref()
        .and_then(|a| a.get(ANNOTATION_TITLE))
        .ok_or_else(|| PackageError::Layout {
            reason: format!("the '{layer_name}' layer has no '{ANNOTATION_TITLE}' annotation"),
        })?
        .clone();
    Ok(Some(RestoredFile { file_name, bytes }))
}

/// Read one required, digest-verified layer's RAW bytes by media type,
/// without deserializing. Split out from [`read_required_layer`] so a caller
/// that must inspect the raw JSON before it trusts a struct shape (see
/// [`detect_legacy_shape`]) can do so without a second read or a second
/// digest verification.
fn read_required_layer_bytes(
    layout: &OciLayout,
    by_media_type: &BTreeMap<String, &Descriptor>,
    media_type: &str,
    layer_name: &str,
) -> Result<Vec<u8>> {
    let descriptor = by_media_type
        .get(media_type)
        .ok_or_else(|| missing_layer(layer_name))?;
    read_verified_blob(layout, descriptor)
}

/// Read one required, digest-verified struct layer by media type and
/// deserialize it.
fn read_required_layer<T: serde::de::DeserializeOwned>(
    layout: &OciLayout,
    by_media_type: &BTreeMap<String, &Descriptor>,
    media_type: &str,
    layer_name: &str,
) -> Result<T> {
    let bytes = read_required_layer_bytes(layout, by_media_type, media_type, layer_name)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// The envelope key whose PRESENCE identifies the pre-0.2.0 layer SHAPE: in
/// 0.1.x the binary's identity was a field inside the envelope, and in 0.2.0
/// it moved onto its own OCI layer (D-08, D-10).
const LEGACY_ENVELOPE_KEY: &str = "binary_ref";

/// Verify that a raw envelope blob does NOT carry the superseded pre-0.2.0
/// layer shape, i.e. that its top-level JSON object holds no
/// [`LEGACY_ENVELOPE_KEY`] key.
///
/// This is SHAPE detection, not producer-version detection. The check is
/// exactly "this envelope object holds a key named `binary_ref`" — it cannot
/// and does not read a version marker, so ANY envelope carrying that
/// extension key is refused regardless of who wrote it or when. Under D-10's
/// blanket refusal that is the intended behaviour, and the error text is
/// worded accordingly: it reports the shape that was found, never a claim
/// about the producer.
///
/// Inspecting the RAW JSON is load-bearing rather than stylistic.
/// [`ServerEnvelope`] carries no `deny_unknown_fields`, so `serde` would
/// happily accept a 0.1.x envelope, DROP its `binary_ref` and hand back a
/// structurally valid 0.2.0 struct whose binary identity had silently
/// vanished. A deserialize error can therefore never be the signal.
///
/// Bytes that are not a JSON object at all are left alone here — that is the
/// deserializer's error to report, and reporting it as a legacy shape would
/// be a worse message.
fn detect_legacy_shape(envelope_bytes: &[u8]) -> Result<()> {
    let Ok(serde_json::Value::Object(envelope)) =
        serde_json::from_slice::<serde_json::Value>(envelope_bytes)
    else {
        return Ok(());
    };
    if envelope.contains_key(LEGACY_ENVELOPE_KEY) {
        return Err(PackageError::Layout {
            reason: format!(
                "the envelope layer carries the pre-0.2.0 layer shape (a '{LEGACY_ENVELOPE_KEY}' \
                 key). The package format changed in 0.2.0, when the binary's identity moved off \
                 the envelope and onto its own OCI layer; 0.2.0 does not read 0.1.x packages. \
                 Repack the package with a 0.2.0 producer."
            ),
        });
    }
    Ok(())
}

/// Unpack a server package from `layout`, returning the typed struct, its
/// binary (embedded bytes or a reference) and its optional verbatim config and
/// spec files.
///
/// # `spec: None` means the package carried no spec
///
/// The spec layer is OPTIONAL. A `None` here is not a decoding default and not
/// a lossy read: it means the manifest's media-type index holds no
/// [`MT_SERVER_OPENAPI_SPEC`] entry, i.e. the author packed a curated-only
/// server — the packaging mirror of `pmcp-openapi-server`'s
/// `--spec: Option<PathBuf>`. There is no absence marker to distinguish
/// "no spec" from "spec dropped" (D-14) because `pack_server` never drops a
/// supplied spec: `Some` in, layer written; `None` in, no layer.
///
/// [`MT_SERVER_OPENAPI_SPEC`]: crate::oci::media_types::MT_SERVER_OPENAPI_SPEC
///
/// # Errors
///
/// Returns [`PackageError::Layout`] if the layout is malformed (duplicate
/// media type, missing required layer, both-or-neither binary layers, a
/// binary reference with no digest, a named-file layer with no title
/// annotation) or if the envelope carries the pre-0.2.0 layer shape
/// ([`detect_legacy_shape`]), [`PackageError::DigestMismatch`] if any blob has been
/// tampered with, or [`PackageError::Serialize`] if a verified layer fails to
/// deserialize.
///
/// [`PackageError::DigestMismatch`]: crate::error::PackageError::DigestMismatch
/// [`PackageError::Serialize`]: crate::error::PackageError::Serialize
pub fn unpack_server(layout: &OciLayout) -> Result<UnpackedServer> {
    let manifest = read_the_one_manifest(layout)?;
    verify_config_blob(layout, &manifest)?;
    // Keyed by WHAT each layer is, never by where it sits: the config and spec
    // layers are optional and the binary layer is one of two media types, so a
    // positional read would be reading a different layer than it names.
    let by_media_type = index_layers(&manifest)?;

    let binary = read_binary_mode(layout, &by_media_type)?;

    // The envelope is read as RAW bytes and shape-checked BEFORE it is
    // deserialized: `ServerEnvelope` has no `deny_unknown_fields`, so a
    // pre-0.2.0 envelope would otherwise deserialize cleanly with its
    // `binary_ref` silently dropped.
    let envelope_bytes =
        read_required_layer_bytes(layout, &by_media_type, MT_SERVER_ENVELOPE, "envelope")?;
    detect_legacy_shape(&envelope_bytes)?;
    let envelope: ServerEnvelope = serde_json::from_slice(&envelope_bytes)?;

    let package = ServerPackage {
        name: envelope.name,
        version: envelope.version,
        digest: envelope.digest,
        deploy: read_required_layer(
            layout,
            &by_media_type,
            MT_SERVER_DEPLOY_DESCRIPTOR,
            "deploy-descriptor",
        )?,
        policies: read_required_layer(
            layout,
            &by_media_type,
            MT_SERVER_CEDAR_POLICY_SET,
            "cedar-policy-set",
        )?,
        tools: read_required_layer(
            layout,
            &by_media_type,
            MT_SERVER_TOOL_METADATA,
            "tool-metadata",
        )?,
        config_slots: read_required_layer(
            layout,
            &by_media_type,
            MT_SERVER_CONFIG_SLOTS,
            "config-slots",
        )?,
    };

    let config = read_named_file_layer(layout, by_media_type.get(MT_SERVER_CONFIG), "config")?;
    let spec = read_named_file_layer(
        layout,
        by_media_type.get(MT_SERVER_OPENAPI_SPEC),
        "openapi-spec",
    )?;

    Ok(UnpackedServer {
        package,
        binary,
        config,
        spec,
    })
}

/// Unpack any single-layer package (agent/team/workflow) from `layout`: verify
/// the manifest + config blob, then read+verify the single config layer and
/// deserialize it. The per-kind layer name (for the "missing layer" error)
/// comes from the [`SingleLayerPackage`] impl — one path, no per-kind
/// copy-paste. Mirrors [`super::pack::pack_single_layer`].
///
/// # Located by MEDIA TYPE, and there must be exactly one
///
/// This used to read `layers().first()` positionally and never compare the
/// descriptor's media type against `P::LAYER_MEDIA_TYPE`, which made the
/// constant produce-only and left two holes on a layout this crate documents as
/// untrusted input: a manifest carrying an EXTRA layer had it silently ignored
/// (so a crafted layout could put an attacker's package first and the genuine
/// one second, and only the first was ever read), and a layer of the wrong kind
/// was accepted as long as its JSON happened to deserialize. `unpack_server`
/// already refuses both shapes via [`index_layers`]; this is the same rule for
/// the single-layer kinds.
fn unpack_single_layer<P: SingleLayerPackage>(layout: &OciLayout) -> Result<P> {
    let manifest = read_the_one_manifest(layout)?;
    verify_config_blob(layout, &manifest)?;
    // Rejects a duplicate media type, exactly as the server path does.
    let by_media_type = index_layers(&manifest)?;
    let expected = vendor_media_type_name(P::LAYER_MEDIA_TYPE);
    if by_media_type.len() != 1 {
        return Err(PackageError::Layout {
            reason: format!(
                "a {} package must carry exactly ONE layer ('{expected}'), found {}: {:?}",
                P::LAYER_NAME,
                by_media_type.len(),
                by_media_type.keys().collect::<Vec<_>>()
            ),
        });
    }
    let layer = by_media_type
        .get(&expected)
        .ok_or_else(|| missing_layer(P::LAYER_NAME))?;
    let bytes = read_verified_blob(layout, layer)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// The string form of the vendor media type a layer descriptor carries, so the
/// index built by [`index_layers`] (which keys on `MediaType::to_string()`) can
/// be looked up by a `P::LAYER_MEDIA_TYPE` constant.
fn vendor_media_type_name(media_type: &str) -> String {
    crate::oci::media_types::vendor_media_type(media_type).to_string()
}

/// Unpack an `AgentPackage` from `layout`.
pub fn unpack_agent(layout: &OciLayout) -> Result<AgentPackage> {
    unpack_single_layer(layout)
}

/// Unpack a `TeamPackage` from `layout`.
pub fn unpack_team(layout: &OciLayout) -> Result<TeamPackage> {
    unpack_single_layer(layout)
}

/// Unpack a `WorkflowManifest` from `layout`.
pub fn unpack_workflow(layout: &OciLayout) -> Result<WorkflowManifest> {
    unpack_single_layer(layout)
}

/// Shared sample-package builders for this module's own round-trip/tamper
/// tests AND `oci::pack`'s tests (`pub(crate)` — test-only, gated by
/// `#[cfg(test)]` on this whole module so it never ships in a release
/// build).
#[cfg(test)]
pub(crate) mod tests_support {
    use crate::digest::ManifestDigest;
    use crate::package::Provenance;
    use crate::package::{
        AgentPackage, AssetsSection, AuthSection, AwsSection, CedarPolicy, CedarPolicySet,
        DeployDescriptor, HumanRole, ServerPackage, ServerSection, TargetSection, TeamLimits,
        TeamMember, TeamPackage, TeamRole, ToolMetadata, WorkflowManifest,
    };
    use crate::reference::{ComponentRef, ComponentType, PinnedRef};
    use crate::slot::{ConfigSlot, SlotType};
    use std::collections::BTreeMap;

    fn minimal_deploy_descriptor() -> DeployDescriptor {
        DeployDescriptor {
            target: TargetSection {
                target_type: "pmcp-run".to_string(),
                version: "1.0.0".to_string(),
            },
            metadata: None,
            aws: AwsSection {
                region: "us-east-1".to_string(),
            },
            server: ServerSection {
                name: "team-fs".to_string(),
                memory_mb: Some(1024),
                timeout_seconds: 30,
                memory: None,
                cpu: None,
                ingress: None,
                allow_unauthenticated: None,
                binary: None,
            },
            environment: BTreeMap::from([("RUST_LOG".to_string(), "info".to_string())]),
            secrets: BTreeMap::new(),
            auth: AuthSection {
                enabled: false,
                provider: "none".to_string(),
                callback_urls: vec![],
                cognito: None,
                dcr: None,
                groups: None,
                scopes: None,
            },
            observability: crate::package::ObservabilitySection {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: None,
            },
            composition: None,
            assets: Some(AssetsSection {
                include: vec![],
                exclude: vec!["**/*.tmp".to_string()],
            }),
            iam: None,
            gcp: None,
            layout: None,
        }
    }

    /// A representative `ServerPackage` + fake bootstrap bytes, for
    /// pack/unpack round-trip and tamper tests.
    pub(crate) fn sample_server_package() -> (ServerPackage, Vec<u8>) {
        let bootstrap = b"fake-arm64-bootstrap-binary-bytes-for-testing".to_vec();
        let package = ServerPackage {
            name: "team-fs".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: None,
            deploy: minimal_deploy_descriptor(),
            policies: CedarPolicySet(vec![CedarPolicy {
                id: "p1".to_string(),
                cedar_text: "permit(principal, action, resource);".to_string(),
                title: "Allow all".to_string(),
                description: "test policy".to_string(),
                category: "read".to_string(),
                risk: "low".to_string(),
            }]),
            tools: vec![ToolMetadata {
                name: "fs__list".to_string(),
                description: "List files in a team workspace".to_string(),
                annotations: Some(serde_json::json!({ "read_only_hint": true })),
            }],
            config_slots: vec![ConfigSlot::new(SlotType::Secret {
                name: "API_KEY".to_string(),
            })],
        };
        (package, bootstrap)
    }

    /// A representative `AgentPackage`, for pack/unpack round-trip tests.
    pub(crate) fn sample_agent_package() -> AgentPackage {
        AgentPackage {
            name: "triage-agent".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You triage incoming support tickets.".to_string(),
            llm: ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            }),
            max_tokens: 4096,
            max_iterations: 25,
            connectors: vec![ComponentRef::Range {
                name: "london-tube".to_string(),
                range: semver::VersionReq::parse("^1.2").unwrap(),
                component_type: ComponentType::Server,
            }],
            tool_selection: Some(serde_json::json!({ "london-tube": ["get_status"] })),
            input_schema: None,
            output_schema: Some(serde_json::json!({ "type": "object" })),
            importance: Some("HIGH".to_string()),
            finalizer_role: Some("formatter".to_string()),
            budget_defaults: vec![ConfigSlot::new(SlotType::BudgetOverride {
                name: "monthly-cap".to_string(),
                tested_value: "1000".to_string(),
            })],
        }
    }

    /// A representative `TeamPackage`, for pack/unpack round-trip tests.
    pub(crate) fn sample_team_package() -> TeamPackage {
        let entry_point = ComponentRef::Range {
            name: "triage-agent".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        };
        let human_role = HumanRole {
            role: "approver".to_string(),
            description: "Approves budget overrides".to_string(),
            responsibilities: vec!["review".to_string(), "approve".to_string()],
            channel_hints: vec!["slack".to_string()],
        };
        TeamPackage {
            name: "support-team".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            entry_point: entry_point.clone(),
            members: vec![TeamMember {
                agent: entry_point,
                role: TeamRole::EntryPoint,
            }],
            human_roles: vec![human_role.clone()],
            limits: TeamLimits {
                max_team_depth: 3,
                max_team_total_tokens: 200_000,
                max_team_wall_clock_seconds: 600,
                poll_interval_ms: 2000,
            },
            built_in_servers: vec![ComponentRef::Range {
                name: "team-fs".to_string(),
                range: semver::VersionReq::parse("^1").unwrap(),
                component_type: ComponentType::Server,
            }],
            finalizer_agents: vec![],
            budget_defaults: vec![],
            config_slots: vec![human_role.to_config_slot()],
        }
    }

    /// A representative `WorkflowManifest` (fully pinned, per), for
    /// pack/unpack round-trip tests.
    pub(crate) fn sample_workflow_manifest() -> WorkflowManifest {
        WorkflowManifest::new(
            "support-triage".to_string(),
            semver::Version::parse("1.0.0").unwrap(),
            vec![
                ComponentRef::Pinned(PinnedRef {
                    name: "triage-agent".to_string(),
                    component_type: ComponentType::Agent,
                    version: semver::Version::parse("1.2.0").unwrap(),
                    digest: ManifestDigest::from_bytes(b"triage-agent"),
                }),
                ComponentRef::Pinned(PinnedRef {
                    name: "london-tube".to_string(),
                    component_type: ComponentType::Server,
                    version: semver::Version::parse("2.0.1").unwrap(),
                    digest: ManifestDigest::from_bytes(b"london-tube"),
                }),
            ],
            vec![ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            })],
            Provenance {
                source_environment: "dev".to_string(),
                capturer: "cargo-pmcp".to_string(),
                timestamp: "2026-07-16T00:00:00Z".to_string(),
                root_team_id: "support-team".to_string(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{
        sample_agent_package, sample_server_package, sample_team_package, sample_workflow_manifest,
    };
    use super::*;
    use crate::oci::pack::{pack_agent, pack_server, pack_team, pack_workflow, BinaryMode};

    #[test]
    fn server_pack_then_unpack_round_trips_losslessly_including_bootstrap_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();
        let unpacked = unpack_server(&layout).unwrap();

        assert_eq!(unpacked.package, package);
        assert_eq!(unpacked.binary, UnpackedBinary::Embedded(bootstrap));
        assert_eq!(unpacked.config, None);
        assert_eq!(unpacked.spec, None);
    }

    #[test]
    fn a_duplicated_layer_media_type_is_rejected_rather_than_last_wins() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();
        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();

        // Duplicate the envelope layer descriptor, simulating a crafted
        // layout that tries to shadow a real layer with a second one.
        let index = layout.read_index().unwrap();
        let mut manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        let mut layers = manifest.layers().clone();
        let envelope = layers
            .iter()
            .find(|l| l.media_type().to_string() == MT_SERVER_ENVELOPE)
            .unwrap()
            .clone();
        layers.push(envelope);
        manifest.set_layers(layers);
        let bytes = crate::digest::canonicalize(&manifest).unwrap();
        let new_descriptor = layout.write_manifest(&bytes).unwrap();
        let new_index = oci_spec::image::ImageIndexBuilder::default()
            .schema_version(oci_spec::image::SCHEMA_VERSION)
            .manifests(vec![new_descriptor])
            .build()
            .unwrap();
        layout.write_index(&new_index).unwrap();

        let err = unpack_server(&layout).unwrap_err();
        assert!(matches!(err, PackageError::Layout { .. }), "got {err:?}");
    }

    #[test]
    fn agent_pack_then_unpack_round_trips_losslessly() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let package = sample_agent_package();

        pack_agent(&package, &layout).unwrap();
        let unpacked = unpack_agent(&layout).unwrap();

        assert_eq!(unpacked, package);
    }

    #[test]
    fn team_pack_then_unpack_round_trips_losslessly() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let package = sample_team_package();

        pack_team(&package, &layout).unwrap();
        let unpacked = unpack_team(&layout).unwrap();

        assert_eq!(unpacked, package);
    }

    #[test]
    fn workflow_pack_then_unpack_round_trips_losslessly() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let package = sample_workflow_manifest();

        pack_workflow(&package, &layout).unwrap();
        let unpacked = unpack_workflow(&layout).unwrap();

        assert_eq!(unpacked, package);
    }

    #[test]
    fn tampering_a_blob_byte_on_disk_makes_unpack_fail_with_digest_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();
        pack_server(
            &package,
            BinaryMode::Embedded(&bootstrap),
            None,
            None,
            &layout,
        )
        .unwrap();

        // Flip a single byte in the bootstrap blob's file on disk — the
        // file's name (content-addressed by the ORIGINAL digest) does not
        // change, only its contents.
        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        let bootstrap_descriptor = manifest
            .layers()
            .iter()
            .find(|l| l.media_type().to_string() == MT_SERVER_BOOTSTRAP)
            .unwrap();
        let hex = bootstrap_descriptor.digest().digest();
        let blob_path = dir.path().join("blobs").join("sha256").join(hex);
        let mut bytes = std::fs::read(&blob_path).unwrap();
        bytes[0] ^= 0x01;
        std::fs::write(&blob_path, bytes).unwrap();

        let err = unpack_server(&layout).unwrap_err();
        assert!(
            matches!(err, PackageError::DigestMismatch { .. }),
            "expected DigestMismatch, got {err:?}"
        );
    }

    #[test]
    fn tampering_the_manifest_blob_itself_makes_unpack_fail_with_digest_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let package = sample_agent_package();
        pack_agent(&package, &layout).unwrap();

        let index = layout.read_index().unwrap();
        let manifest_descriptor = &index.manifests()[0];
        let hex = manifest_descriptor.digest().digest();
        let blob_path = dir.path().join("blobs").join("sha256").join(hex);
        let mut bytes = std::fs::read(&blob_path).unwrap();
        bytes[0] ^= 0x01;
        std::fs::write(&blob_path, bytes).unwrap();

        let err = unpack_agent(&layout).unwrap_err();
        assert!(matches!(err, PackageError::DigestMismatch { .. }));
    }

    #[test]
    fn missing_layer_yields_layout_error_not_a_panic() {
        // A manifest built with zero layers (e.g. an agent config that
        // somehow lost its only layer) must surface a structured error, not
        // an index-out-of-bounds panic.
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let package = sample_agent_package();
        pack_agent(&package, &layout).unwrap();

        // Rewrite the manifest with an empty layers list, keeping the same
        // config descriptor, to simulate a malformed/truncated manifest.
        let index = layout.read_index().unwrap();
        let mut manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        manifest.set_layers(vec![]);
        let bytes = crate::digest::canonicalize(&manifest).unwrap();
        let new_descriptor = layout.write_manifest(&bytes).unwrap();
        let new_index = oci_spec::image::ImageIndexBuilder::default()
            .schema_version(oci_spec::image::SCHEMA_VERSION)
            .manifests(vec![new_descriptor])
            .build()
            .unwrap();
        layout.write_index(&new_index).unwrap();

        let err = unpack_agent(&layout).unwrap_err();
        assert!(matches!(err, PackageError::Layout { .. }));
    }
}
