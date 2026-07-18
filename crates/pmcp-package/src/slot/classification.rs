//! Identity-bearing vs behavior-relevant slot classification (I-5 / §3.5).

use crate::slot::types::SlotType;

/// Which of the two I-5 slot families a `SlotType` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotClass {
    /// `Secret` / `OauthClient` / `ChannelBinding` / `HumanRole` — declares only identity,
    /// never a value; never subject to deviation detection (I-5: binding identity is not
    /// behavior).
    IdentityBearing,
    /// `LlmProvider` / `BudgetOverride` — carries a `tested_value`; a differing proposed
    /// value is a real behavioral change, surfaced by `deviation::detect_deviation`.
    BehaviorRelevant,
}

/// Classify a `SlotType` into its I-5 family. Pure, no I/O.
///
/// The identity/behavior split has a single source of truth: a variant is
/// behavior-relevant iff it carries a `tested_value` (see
/// [`SlotType::tested_value`]). Deriving the class from that predicate keeps
/// `classify` and `tested_value` from drifting apart when a new variant is
/// added.
pub fn classify(slot: &SlotType) -> SlotClass {
    if slot.tested_value().is_some() {
        SlotClass::BehaviorRelevant
    } else {
        SlotClass::IdentityBearing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_identity_bearing() {
        let slot = SlotType::Secret {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn oauth_client_is_identity_bearing() {
        let slot = SlotType::OauthClient {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn channel_binding_is_identity_bearing() {
        let slot = SlotType::ChannelBinding {
            name: "n".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn human_role_is_identity_bearing() {
        let slot = SlotType::HumanRole {
            role: "approver".to_string(),
            description: String::new(),
            responsibilities: vec![],
            channel_hints: vec![],
        };
        assert_eq!(classify(&slot), SlotClass::IdentityBearing);
    }

    #[test]
    fn llm_provider_is_behavior_relevant() {
        let slot = SlotType::LlmProvider {
            name: "n".to_string(),
            tested_value: "anthropic".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }

    #[test]
    fn budget_override_is_behavior_relevant() {
        let slot = SlotType::BudgetOverride {
            name: "n".to_string(),
            tested_value: "1000".to_string(),
        };
        assert_eq!(classify(&slot), SlotClass::BehaviorRelevant);
    }
}
