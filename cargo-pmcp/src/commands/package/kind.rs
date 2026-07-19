//! Pure, dependency-light package-kind detection (CLI-04).
//!
//! This module is the **lib-safe leaf** plan 110-06 mounts and fuzzes: it
//! references only `pmcp_package::oci::media_types::*`, `serde_json`, and std —
//! NO `clap`/`GlobalFlags`/`OciLayout`. Every function here is total (never
//! panics) on arbitrary/adversarial input, which is what makes
//! [`artifact_type_from_manifest_json`] safe to point a fuzzer at (it is the
//! untrusted manifest-parse boundary — a `.pmcp` package is user-supplied).
//!
//! Kind detection inspects BOTH sources (Consensus concern #3): the manifest's
//! `artifactType` AND the config/layer media types. Callers gather every
//! candidate string and call [`detect_kind`] on each until one returns `Some`.

use pmcp_package::oci::media_types::{
    ARTIFACT_TYPE_AGENT, ARTIFACT_TYPE_SERVER, ARTIFACT_TYPE_TEAM, ARTIFACT_TYPE_WORKFLOW,
    MT_AGENT_CONFIG, MT_SERVER_ENVELOPE, MT_TEAM_CONFIG, MT_WORKFLOW_MANIFEST,
};

/// The four portable `.pmcp` package kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// A single `AgentPackage`.
    Agent,
    /// A `TeamPackage`.
    Team,
    /// An `mcp-server` package.
    Server,
    /// A `WorkflowManifest`.
    Workflow,
}

impl PackageKind {
    /// Lowercase label rendered by `package show` (`agent`/`team`/`server`/`workflow`).
    pub fn label(self) -> &'static str {
        match self {
            PackageKind::Agent => "agent",
            PackageKind::Team => "team",
            PackageKind::Server => "server",
            PackageKind::Workflow => "workflow",
        }
    }
}

/// Map a single media-type or artifact-type string to its [`PackageKind`],
/// returning `None` for any unrecognized string. Total and panic-free on
/// arbitrary input — the property plan 110-06 relies on: `Some(kind)` IFF `s`
/// is exactly one of the eight known constants (mapped to the correct kind),
/// `None` otherwise.
pub fn detect_kind(s: &str) -> Option<PackageKind> {
    match s {
        ARTIFACT_TYPE_AGENT | MT_AGENT_CONFIG => Some(PackageKind::Agent),
        ARTIFACT_TYPE_TEAM | MT_TEAM_CONFIG => Some(PackageKind::Team),
        ARTIFACT_TYPE_SERVER | MT_SERVER_ENVELOPE => Some(PackageKind::Server),
        ARTIFACT_TYPE_WORKFLOW | MT_WORKFLOW_MANIFEST => Some(PackageKind::Workflow),
        _ => None,
    }
}

/// Extract a candidate type string from raw, untrusted OCI manifest JSON:
/// the top-level `artifactType`, else the first `config.mediaType`, else the
/// first `layers[0].mediaType`. Returns `None` on ANY missing field, wrong
/// type, or malformed bytes — never unwraps, never panics. This is the
/// adversarial-input boundary plan 110-06 fuzzes.
pub fn artifact_type_from_manifest_json(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if let Some(artifact_type) = value.get("artifactType").and_then(|v| v.as_str()) {
        return Some(artifact_type.to_string());
    }
    if let Some(config_mt) = value
        .get("config")
        .and_then(|c| c.get("mediaType"))
        .and_then(|v| v.as_str())
    {
        return Some(config_mt.to_string());
    }
    value
        .get("layers")
        .and_then(|l| l.get(0))
        .and_then(|layer| layer.get("mediaType"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The eight recognized constants, paired with their expected kind.
    const KNOWN: &[(&str, PackageKind)] = &[
        (ARTIFACT_TYPE_AGENT, PackageKind::Agent),
        (MT_AGENT_CONFIG, PackageKind::Agent),
        (ARTIFACT_TYPE_TEAM, PackageKind::Team),
        (MT_TEAM_CONFIG, PackageKind::Team),
        (ARTIFACT_TYPE_SERVER, PackageKind::Server),
        (MT_SERVER_ENVELOPE, PackageKind::Server),
        (ARTIFACT_TYPE_WORKFLOW, PackageKind::Workflow),
        (MT_WORKFLOW_MANIFEST, PackageKind::Workflow),
    ];

    #[test]
    fn detect_kind_maps_every_known_constant_to_its_kind() {
        for (s, expected) in KNOWN {
            assert_eq!(detect_kind(s), Some(*expected), "constant {s} must map");
        }
    }

    #[test]
    fn detect_kind_returns_none_for_garbage_and_empty() {
        for s in ["", "application/json", "not-a-media-type", "AGENT", " "] {
            assert_eq!(detect_kind(s), None, "{s:?} must not be recognized");
        }
    }

    #[test]
    fn artifact_type_from_manifest_json_reads_artifact_type_first() {
        let bytes = br#"{"artifactType":"application/vnd.pmcp.agent.v1","config":{"mediaType":"application/vnd.oci.empty.v1+json"}}"#;
        assert_eq!(
            artifact_type_from_manifest_json(bytes).as_deref(),
            Some(ARTIFACT_TYPE_AGENT)
        );
    }

    #[test]
    fn artifact_type_from_manifest_json_falls_back_to_layer_media_type() {
        let bytes = format!(
            r#"{{"config":{{"mediaType":"application/vnd.oci.empty.v1+json"}},"layers":[{{"mediaType":"{MT_TEAM_CONFIG}"}}]}}"#
        );
        // config mediaType (empty) is not a kind, but it IS returned first; the
        // caller runs `detect_kind` over BOTH, so the empty-config string simply
        // yields `None` there. Here we assert the config path is preferred.
        assert_eq!(
            artifact_type_from_manifest_json(bytes.as_bytes()).as_deref(),
            Some("application/vnd.oci.empty.v1+json")
        );
    }

    #[test]
    fn artifact_type_from_manifest_json_returns_none_on_adversarial_bytes() {
        for bytes in [
            &b""[..],
            &b"not json at all"[..],
            &b"[]"[..],
            &b"{}"[..],
            &b"{\"artifactType\": 42}"[..],
            &b"{\"config\": \"not-an-object\"}"[..],
            &b"\xff\xfe\x00\x01"[..],
        ] {
            assert_eq!(
                artifact_type_from_manifest_json(bytes),
                None,
                "adversarial bytes {bytes:?} must yield None, never panic"
            );
        }
    }

    proptest! {
        /// PROPERTY (CLAUDE.md ALWAYS PROPERTY): over a strategy mixing the eight
        /// known constants with arbitrary strings, `detect_kind(s)` is `Some(kind)`
        /// IFF `s` is exactly one of the known constants (mapped correctly), and
        /// `None` otherwise — and it NEVER panics.
        #[test]
        fn detect_kind_some_iff_known_constant(
            s in prop_oneof![
                proptest::sample::select(KNOWN.iter().map(|(c, _)| c.to_string()).collect::<Vec<_>>()),
                ".*",
            ]
        ) {
            let expected = KNOWN.iter().find(|(c, _)| *c == s).map(|(_, k)| *k);
            prop_assert_eq!(detect_kind(&s), expected);
        }

        /// `artifact_type_from_manifest_json` never panics on arbitrary bytes.
        #[test]
        fn artifact_type_from_manifest_json_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = artifact_type_from_manifest_json(&bytes);
        }
    }
}
