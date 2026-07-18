//! Behavior-relevant deviation detection: flags a behavior-relevant slot whose
//! proposed value differs from the value that was exercised when the package was tested,
//! and never flags an identity-bearing slot.

use crate::slot::classification::{classify, SlotClass};
use crate::slot::types::SlotType;

/// A detected behavioral deviation: the proposed binding for a behavior-relevant slot
/// differs from the value that was exercised when the package was tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    pub slot_name: String,
    pub tested: String,
    pub proposed: String,
}

/// Compare a tested slot against a proposed slot. Returns `Some(Deviation)` only when BOTH
/// are the same behavior-relevant variant (`LlmProvider`/`LlmProvider` or
/// `BudgetOverride`/`BudgetOverride`) with equal name but differing `tested_value`;
/// otherwise (identity-bearing kinds, mismatched kinds/names, or equal values) returns
/// `None`.
///
/// Gated through `classification::classify` so the identity-bearing short-circuit is
/// explicit and testable (bindings supply identity, never behavior). A detected
/// deviation is a *value* the caller decides to surface/acknowledge, not a hard error —
/// contrast with `aggregate`'s `SlotConflict`, which IS a hard error because a silent
/// discard there would mask the same kind of behavioral change at capture time.
pub fn detect_deviation(tested: &SlotType, proposed: &SlotType) -> Option<Deviation> {
    if classify(tested) != SlotClass::BehaviorRelevant
        || classify(proposed) != SlotClass::BehaviorRelevant
    {
        return None;
    }
    if tested.key() != proposed.key() {
        return None;
    }
    let tested_value = tested.tested_value()?;
    let proposed_value = proposed.tested_value()?;
    if tested_value == proposed_value {
        return None;
    }
    Some(Deviation {
        slot_name: tested.key().1.to_string(),
        tested: tested_value.to_string(),
        proposed: proposed_value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_deviation_for_differing_behavior_relevant_values() {
        let tested = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        let proposed = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "openai".to_string(),
        };
        let dev = detect_deviation(&tested, &proposed).expect("must detect deviation");
        assert_eq!(dev.slot_name, "primary-llm");
        assert_eq!(dev.tested, "anthropic");
        assert_eq!(dev.proposed, "openai");
    }

    #[test]
    fn never_flags_identity_bearing_slots() {
        let tested = SlotType::Secret {
            name: "X".to_string(),
        };
        let proposed = SlotType::Secret {
            name: "X".to_string(),
        };
        assert_eq!(detect_deviation(&tested, &proposed), None);
    }

    #[test]
    fn returns_none_when_tested_equals_proposed() {
        let tested = SlotType::BudgetOverride {
            name: "monthly-cap".to_string(),
            tested_value: "1000".to_string(),
        };
        let proposed = tested.clone();
        assert_eq!(detect_deviation(&tested, &proposed), None);
    }

    #[test]
    fn returns_none_for_mismatched_kinds() {
        let tested = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        let proposed = SlotType::BudgetOverride {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        assert_eq!(detect_deviation(&tested, &proposed), None);
    }

    #[test]
    fn returns_none_for_differing_names() {
        let tested = SlotType::LlmProvider {
            name: "primary-llm".to_string(),
            tested_value: "anthropic".to_string(),
        };
        let proposed = SlotType::LlmProvider {
            name: "secondary-llm".to_string(),
            tested_value: "openai".to_string(),
        };
        assert_eq!(detect_deviation(&tested, &proposed), None);
    }

    // Sanity: `classify` must ensure this gate is testable (bindings supply identity,
    // never behavior).
    #[test]
    fn identity_bearing_variants_are_never_behavior_relevant() {
        assert_ne!(
            classify(&SlotType::Secret {
                name: "n".to_string()
            }),
            SlotClass::BehaviorRelevant
       );
    }
}
