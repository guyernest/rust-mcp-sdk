//! Config-slot type system (/ /): typed slot declarations that structurally
//! cannot carry a secret or identity value.
//!
//! `SlotType` splits into two families (see `classification::classify` for the mapping):
//! - **Identity-bearing** (`Secret`, `OauthClient`, `ChannelBinding`, `HumanRole`) declare
//!   only a *name* (or, for `HumanRole`, descriptive metadata) — a resolved secret/identity
//!   value is not representable in this type at all. This is a compile-time absence, not a
//!   runtime check that could be forgotten: the strongest form of "secrets never travel"
//!   (§12 permanent non-goal) a Rust type system can offer.
//! - **Behavior-relevant** (`LlmProvider`, `BudgetOverride`) carry the `tested_value` that
//!   was exercised when the package was tested, so a later proposed binding can be compared
//!   against it (see `deviation::detect_deviation`).

use serde::{Deserialize, Serialize};

/// A config slot's typed declaration. Serializes with a snake_case `type` discriminator
/// (e.g. `{"type":"secret","name":"LICHESS_API_KEY"}`).
///
/// Deliberately NOT `#[serde(deny_unknown_fields)]` — forward-compatible for future slot
/// kinds (RESEARCH Pitfall 4): an older reader silently ignores fields it doesn't know about
/// rather than hard-failing on a newer producer's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlotType {
    /// A named secret the component requires at runtime (e.g. an API key). Declares only
    /// the secret's `name` — CRITICAL: no `value`/`secret`/`credential` field exists on this
    /// variant; a resolved secret value cannot be constructed into it.
    Secret {
        /// The secret's declared name (e.g. `LICHESS_API_KEY`), never its resolved value.
        name: String,
    },
    /// A named OAuth client credential the component requires. Declares only `name` — never
    /// a client secret or token.
    OauthClient {
        /// The OAuth client credential's declared name.
        name: String,
    },
    /// A named channel-binding the component requires (e.g. which notification channel a
    /// component posts to). Declares only `name` — never the resolved channel/user identity.
    ChannelBinding {
        /// The channel binding's declared name.
        name: String,
    },
    /// A human role a team component needs filled (from `AgentTeam`/`TeamHumanMember`).
    /// Declares descriptive fields only — NEVER `userId`/`channelId`/`email` (those are
    /// identity, resolved at bind time, not representable here).
    HumanRole {
        /// The role label (e.g. "approver").
        role: String,
        /// A human-readable description of the role's purpose.
        description: String,
        /// The responsibilities this role is expected to cover.
        responsibilities: Vec<String>,
        /// Hints about which channel kinds are suitable for this role (display-only).
        channel_hints: Vec<String>,
    },
    /// A named LLM provider slot, carrying the `tested_value` (e.g. `"anthropic"`) that was
    /// exercised when the package was tested. Behavior-relevant — a proposed binding
    /// that differs from `tested_value` is a real behavioral change, not an identity swap.
    LlmProvider {
        /// The slot's declared name.
        name: String,
        /// The provider value exercised when the package was tested.
        tested_value: String,
    },
    /// A named budget-override slot, carrying the `tested_value` that was exercised when the
    /// package was tested. Behavior-relevant.
    BudgetOverride {
        /// The slot's declared name.
        name: String,
        /// The budget-override value exercised when the package was tested.
        tested_value: String,
    },
}

impl SlotType {
    /// A stable `(kind, name)` key identifying this slot for dedup/aggregation purposes.
    /// `name` is the variant's identifying field — the slot's own `name` for named slots,
    /// and `role` for `HumanRole` (which has no `name` field).
    pub fn key(&self) -> (&'static str, &str) {
        match self {
            SlotType::Secret { name } => ("secret", name.as_str()),
            SlotType::OauthClient { name } => ("oauth_client", name.as_str()),
            SlotType::ChannelBinding { name } => ("channel_binding", name.as_str()),
            SlotType::HumanRole { role,.. } => ("human_role", role.as_str()),
            SlotType::LlmProvider { name,.. } => ("llm_provider", name.as_str()),
            SlotType::BudgetOverride { name,.. } => ("budget_override", name.as_str()),
        }
    }

    /// The `tested_value` carried by a behavior-relevant variant, or `None` for an
    /// identity-bearing variant (which has no such field at all).
    pub fn tested_value(&self) -> Option<&str> {
        match self {
            SlotType::LlmProvider { tested_value,.. }
            | SlotType::BudgetOverride { tested_value,.. } => Some(tested_value.as_str()),
            _ => None,
        }
    }
}

/// A single declared config slot held by a package component. The one canonical "a component
/// declares this slot" type — packages hold `Vec<ConfigSlot>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSlot {
    pub slot: SlotType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::Secret {
            name: "LICHESS_API_KEY".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "secret");
        assert_eq!(json["name"], "LICHESS_API_KEY");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn oauth_client_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::OauthClient {
            name: "primary-oauth".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "oauth_client");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn channel_binding_round_trips_with_snake_case_discriminator() {
        let slot = SlotType::ChannelBinding {
            name: "notify-channel".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "channel_binding");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn human_role_round_trips_with_all_fields_and_no_identity_field() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: "Approves budget overrides".to_string(),
            responsibilities: vec!["review".to_string(), "approve".to_string()],
            channel_hints: vec!["slack".to_string()],
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "human_role");
        assert_eq!(json["role"], "approver");
        assert!(json.get("userId").is_none());
        assert!(json.get("channelId").is_none());
        assert!(json.get("email").is_none());
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn llm_provider_round_trips_with_tested_value() {
        let slot = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "llm_provider");
        assert_eq!(json["tested_value"], "anthropic");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    #[test]
    fn budget_override_round_trips_with_tested_value() {
        let slot = SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "1000".to_string(),
        };
        let json = serde_json::to_value(&slot).unwrap();
        assert_eq!(json["type"], "budget_override");
        assert_eq!(json["tested_value"], "1000");
        let round: SlotType = serde_json::from_value(json).unwrap();
        assert_eq!(round, slot);
    }

    /// Compile-documented proof: constructing `Secret` requires — and permits — only a
    /// `name` field. If a future contributor added a `value`/`secret`/`credential` field to
    /// this variant, this call site (and every other Secret construction in this crate) would
    /// fail to compile until updated, making the structural guarantee impossible to silently
    /// erode.
    #[test]
    fn secret_variant_constructs_with_only_a_name_field() {
        let _ = SlotType::Secret {
            name: "X".to_string(),
        };
    }

    #[test]
    fn key_uses_role_as_identifying_field_for_human_role() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: String::new(),
            responsibilities: vec![],
            channel_hints: vec![],
        };
        assert_eq!(slot.key(), ("human_role", "approver"));
    }

    #[test]
    fn tested_value_is_none_for_identity_bearing_variants() {
        let slot = SlotType::Secret {
            name: "X".to_string(),
        };
        assert_eq!(slot.tested_value(), None);
    }

    #[test]
    fn tested_value_is_some_for_behavior_relevant_variants() {
        let slot = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        assert_eq!(slot.tested_value(), Some("anthropic"));
    }
}
