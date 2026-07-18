//! `unpack_server`/`unpack_agent`/`unpack_team`/`unpack_workflow` — read a
//! local [`OciLayout`] back into a typed package struct, verifying every
//! blob's digest BEFORE deserializing (I-1/I-2 — RESEARCH System
//! Architecture Diagram, unpack() steps 1-3):
//!
//! 1. Read the manifest + blobs from the layout.
//! 2. Recompute sha256 of each blob and compare against its declared
//!    digest (`verify()` — a tampered blob fails HERE, before any
//!    deserialize).
//! 3. Deserialize each verified layer back to its typed struct
//!    (`deny_unknown_fields` on `DeployDescriptor` enforces I-4).
//!
//! Layer order mirrors [`super::pack`]'s push order exactly: for a
//! `ServerPackage`, `[bootstrap, envelope, deploy, cedar, tools,
//! config_slots]`; for `AgentPackage`/`TeamPackage`/`WorkflowManifest`, a
//! single layer. The bootstrap layer is returned as raw bytes — it is never
//! deserialized.

use crate::digest::{verify, ManifestDigest};
use crate::error::{PackageError, Result};
use crate::oci::layout::OciLayout;
use crate::oci::pack::ServerEnvelope;
use crate::oci::SingleLayerPackage;
use crate::package::{AgentPackage, ServerPackage, TeamPackage, WorkflowManifest};
use oci_spec::image::{Descriptor, ImageManifest};

/// Read+verify a blob's bytes given its `Descriptor`: convert the
/// Descriptor's OCI digest into a [`ManifestDigest`] (the validated
/// boundary conversion — T-168 threat register: "OCI Descriptor digest →
/// ManifestDigest"), then [`verify`] it against the actual bytes read from
/// disk (I-1/I-2 — a byte-flipped blob fails HERE, before any deserialize
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

/// Unpack a `ServerPackage` from `layout`, returning the typed struct AND
/// the bootstrap binary's raw bytes (never inlined on the struct itself).
pub fn unpack_server(layout: &OciLayout) -> Result<(ServerPackage, Vec<u8>)> {
    let manifest = read_the_one_manifest(layout)?;
    verify_config_blob(layout, &manifest)?;

    let layers = manifest.layers();
    let bootstrap_descriptor = layers.first().ok_or_else(|| missing_layer("bootstrap"))?;
    let envelope_descriptor = layers.get(1).ok_or_else(|| missing_layer("envelope"))?;
    let deploy_descriptor = layers
        .get(2)
        .ok_or_else(|| missing_layer("deploy-descriptor"))?;
    let cedar_descriptor = layers
        .get(3)
        .ok_or_else(|| missing_layer("cedar-policy-set"))?;
    let tools_descriptor = layers
        .get(4)
        .ok_or_else(|| missing_layer("tool-metadata"))?;
    let config_slots_descriptor = layers.get(5).ok_or_else(|| missing_layer("config-slots"))?;

    // The bootstrap layer stays raw bytes — it is NEVER deserialized.
    let bootstrap_bytes = read_verified_blob(layout, bootstrap_descriptor)?;

    let envelope_bytes = read_verified_blob(layout, envelope_descriptor)?;
    let envelope: ServerEnvelope = serde_json::from_slice(&envelope_bytes)?;

    let deploy_bytes = read_verified_blob(layout, deploy_descriptor)?;
    let deploy = serde_json::from_slice(&deploy_bytes)?;

    let cedar_bytes = read_verified_blob(layout, cedar_descriptor)?;
    let policies = serde_json::from_slice(&cedar_bytes)?;

    let tools_bytes = read_verified_blob(layout, tools_descriptor)?;
    let tools = serde_json::from_slice(&tools_bytes)?;

    let config_slots_bytes = read_verified_blob(layout, config_slots_descriptor)?;
    let config_slots = serde_json::from_slice(&config_slots_bytes)?;

    let package = ServerPackage {
        name: envelope.name,
        version: envelope.version,
        digest: envelope.digest,
        binary_ref: envelope.binary_ref,
        deploy,
        policies,
        tools,
        config_slots,
    };

    Ok((package, bootstrap_bytes))
}

/// Unpack any single-layer package (agent/team/workflow) from `layout`: verify
/// the manifest + config blob, then read+verify the single config layer and
/// deserialize it. The per-kind layer name (for the "missing layer" error)
/// comes from the [`SingleLayerPackage`] impl — one path, no per-kind
/// copy-paste. Mirrors [`super::pack::pack_single_layer`].
fn unpack_single_layer<P: SingleLayerPackage>(layout: &OciLayout) -> Result<P> {
    let manifest = read_the_one_manifest(layout)?;
    verify_config_blob(layout, &manifest)?;
    let layer = manifest
        .layers()
        .first()
        .ok_or_else(|| missing_layer(P::LAYER_NAME))?;
    let bytes = read_verified_blob(layout, layer)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        AgentPackage, AssetsSection, AuthSection, AwsSection, BinaryRef, CedarPolicy,
        CedarPolicySet, DeployDescriptor, HumanRole, ServerPackage, ServerSection, TargetSection,
        TeamLimits, TeamMember, TeamPackage, TeamRole, ToolMetadata, WorkflowManifest,
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
            binary_ref: BinaryRef {
                digest: None,
                media_type: "application/x-lambda-bootstrap; arch=arm64".to_string(),
            },
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
            config_slots: vec![ConfigSlot {
                slot: SlotType::Secret {
                    name: "API_KEY".to_string(),
                },
            }],
        };
        (package, bootstrap)
    }

    /// A representative `AgentPackage`, for pack/unpack round-trip tests.
    pub(crate) fn sample_agent_package() -> AgentPackage {
        AgentPackage {
            name: "triage-agent".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "You triage incoming support tickets.".to_string(),
            llm: ConfigSlot {
                slot: SlotType::LlmProvider {
                    name: "primary-llm".to_string(),
                    tested_value: "anthropic".to_string(),
                },
            },
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
            budget_defaults: vec![ConfigSlot {
                slot: SlotType::BudgetOverride {
                    name: "monthly-cap".to_string(),
                    tested_value: "1000".to_string(),
                },
            }],
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

    /// A representative `WorkflowManifest` (fully pinned, per I-1), for
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
            vec![ConfigSlot {
                slot: SlotType::LlmProvider {
                    name: "primary-llm".to_string(),
                    tested_value: "anthropic".to_string(),
                },
            }],
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
    use crate::oci::pack::{pack_agent, pack_server, pack_team, pack_workflow};

    #[test]
    fn server_pack_then_unpack_round_trips_losslessly_including_bootstrap_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let layout = OciLayout::create(dir.path()).unwrap();
        let (package, bootstrap) = sample_server_package();

        pack_server(&package, &bootstrap, &layout).unwrap();
        let (unpacked, unpacked_bootstrap) = unpack_server(&layout).unwrap();

        assert_eq!(unpacked, package);
        assert_eq!(unpacked_bootstrap, bootstrap);
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
        pack_server(&package, &bootstrap, &layout).unwrap();

        // Flip a single byte in the bootstrap blob's file on disk — the
        // file's name (content-addressed by the ORIGINAL digest) does not
        // change, only its contents.
        let index = layout.read_index().unwrap();
        let manifest = layout.read_manifest(&index.manifests()[0]).unwrap();
        let bootstrap_descriptor = &manifest.layers()[0];
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
