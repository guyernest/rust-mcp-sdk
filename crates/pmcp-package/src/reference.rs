//! Typed component references.
//!
//! A [`ComponentRef`] encodes the range-vs-pin distinction that makes the
//! "converge on exact digests" property computable:
//!
//! - [`ComponentRef::Range`] — a capture-time semver range (e.g.
//!   `london-tube@^1.2`), used while authoring/discovering components.
//! - [`ComponentRef::Pinned`] — a finalized exact reference, wrapping a
//!   [`PinnedRef`] whose `version` and `digest` fields are both non-`Option`.
//!   A pin can never exist without a digest — this is a STRUCTURAL
//!   guarantee (no field is missing at the type level), not a runtime check
//!   that could be forgotten at some call site.
//!
//! `PinnedRef` is a dedicated struct (not an inline enum-variant struct) so
//! downstream helpers (e.g. `pinned_components() -> Result<Vec<&PinnedRef>>`)
//! can name the pin body directly.
//!
//! ## Component identity (D-D)
//!
//! [`ComponentType`] is a SEPARATE axis from the `kind` discriminator above:
//! `kind` says whether a reference is a range or a pin (a wire-shape
//! concern); `component_type` says WHAT the referenced thing is — a server,
//! an agent, or a team. A workflow graph can legitimately contain a server
//! and an agent that share the same `name` (e.g. an agent named "x" that
//! calls a server also named "x") — without `component_type`, those two
//! pins would be indistinguishable by name alone. Both `ComponentRef`
//! variants (`Range` and `Pinned`) carry a required `component_type` field
//! so identity is unambiguous regardless of range-vs-pin state.

use crate::digest::ManifestDigest;
use serde::{Deserialize, Serialize};

/// What kind of component a [`ComponentRef`] identifies. Ordering
/// (`server < agent < team`, derived from declaration order via `Ord`) is
/// the canonical type-then-name sort key `WorkflowManifest::new` uses (D-D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    Server,
    Agent,
    Team,
}

/// An exact, digest-verified component pin. `version` and `digest` are both
/// mandatory (non-`Option`) — a pin always carries both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedRef {
    pub name: String,
    pub component_type: ComponentType,
    pub version: semver::Version,
    pub digest: ManifestDigest,
}

/// A reference to a component: either a capture-time semver range or a
/// finalized exact pin. The `kind` discriminator (`"range"` | `"pinned"`) is
/// the wire-format contract other consumers key off of. `component_type` is
/// a separate, required axis on BOTH variants (D-D) — see module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComponentRef {
    Range {
        name: String,
        range: semver::VersionReq,
        component_type: ComponentType,
    },
    Pinned(PinnedRef),
}

impl ComponentRef {
    /// The component name, regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            ComponentRef::Range { name, .. } => name,
            ComponentRef::Pinned(pinned) => &pinned.name,
        }
    }

    /// The component type, regardless of variant (D-D identity axis).
    pub fn component_type(&self) -> ComponentType {
        match self {
            ComponentRef::Range { component_type, .. } => *component_type,
            ComponentRef::Pinned(pinned) => pinned.component_type,
        }
    }

    /// `true` if this reference is a finalized exact pin.
    pub fn is_pinned(&self) -> bool {
        matches!(self, ComponentRef::Pinned(_))
    }

    /// Borrow the pin body, if this reference is pinned.
    pub fn as_pinned(&self) -> Option<&PinnedRef> {
        match self {
            ComponentRef::Pinned(pinned) => Some(pinned),
            ComponentRef::Range { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_digest() -> ManifestDigest {
        ManifestDigest::from_bytes(b"fixture-bytes")
    }

    #[test]
    fn range_round_trips_with_kind_discriminator() {
        let r = ComponentRef::Range {
            name: "london-tube".to_string(),
            range: semver::VersionReq::parse("^1.2").unwrap(),
            component_type: ComponentType::Server,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("range"));
        assert_eq!(
            json.get("name").and_then(|v| v.as_str()),
            Some("london-tube")
        );
        assert_eq!(
            json.get("component_type").and_then(|v| v.as_str()),
            Some("server")
        );
        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn pinned_round_trips_flat_with_kind_discriminator() {
        let pinned = PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.2.3").unwrap(),
            digest: sample_digest(),
        };
        let r = ComponentRef::Pinned(pinned.clone());
        let json = serde_json::to_value(&r).unwrap();

        // FLAT shape: {"kind":"pinned","name":...,"version":...,"digest":...}
        // — NOT nested under a "Pinned" key.
        let obj = json.as_object().expect("must serialize as a flat object");
        let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["component_type", "digest", "kind", "name", "version"]
        );
        assert_eq!(obj.get("kind").and_then(|v| v.as_str()), Some("pinned"));
        assert_eq!(
            obj.get("name").and_then(|v| v.as_str()),
            Some("london-tube")
        );
        assert_eq!(obj.get("version").and_then(|v| v.as_str()), Some("1.2.3"));
        assert_eq!(
            obj.get("component_type").and_then(|v| v.as_str()),
            Some("server")
        );
        assert!(obj
            .get("digest")
            .and_then(|v| v.as_str())
            .unwrap()
            .starts_with("sha256:"));

        let back: ComponentRef = serde_json::from_value(json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn pinned_ref_always_carries_version_and_digest() {
        // Structural test: PinnedRef's fields are non-Option — this compiles
        // only because both fields are mandatory at construction.
        let pinned = PinnedRef {
            name: "x".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("0.1.0").unwrap(),
            digest: sample_digest(),
        };
        assert_eq!(pinned.version.to_string(), "0.1.0");
        assert!(pinned.digest.as_str().starts_with("sha256:"));
    }

    #[test]
    fn name_accessor_works_for_both_variants() {
        let range = ComponentRef::Range {
            name: "a".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Server,
        };
        assert_eq!(range.name(), "a");
        assert!(!range.is_pinned());
        assert!(range.as_pinned().is_none());

        let pinned = ComponentRef::Pinned(PinnedRef {
            name: "b".to_string(),
            component_type: ComponentType::Agent,
            version: semver::Version::parse("2.0.0").unwrap(),
            digest: sample_digest(),
        });
        assert_eq!(pinned.name(), "b");
        assert!(pinned.is_pinned());
        assert!(pinned.as_pinned().is_some());
    }

    #[test]
    fn component_type_orders_server_before_agent_before_team() {
        assert!(ComponentType::Server < ComponentType::Agent);
        assert!(ComponentType::Agent < ComponentType::Team);
        assert!(ComponentType::Server < ComponentType::Team);
    }

    #[test]
    fn component_type_accessor_works_for_both_variants() {
        let range = ComponentRef::Range {
            name: "x".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Team,
        };
        assert_eq!(range.component_type(), ComponentType::Team);

        let pinned = ComponentRef::Pinned(PinnedRef {
            name: "x".to_string(),
            component_type: ComponentType::Agent,
            version: semver::Version::parse("1.0.0").unwrap(),
            digest: sample_digest(),
        });
        assert_eq!(pinned.component_type(), ComponentType::Agent);
    }
}
