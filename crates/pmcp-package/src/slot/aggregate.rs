//! Deterministic dedup + conflict-erroring aggregation of config slots across a component
//! graph (/).
//!
//! The caller supplies the walk (e.g. `components.iter().flat_map(|c| c.slots())`); this
//! module provides the dedup + conflict check. Ordering is via `BTreeMap` (never `HashMap`)
//! so the aggregated `Vec` is stable across runs — required for digest + manifest-diff.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crate::error::{PackageError, Result};
use crate::slot::types::ConfigSlot;

/// Aggregate a flat iterator of `ConfigSlot`s into one deduplicated entry per distinct
/// `(kind, name)` slot (see `SlotType::key`), in stable deterministic order.
///
/// - Two components declaring a byte-equal slot dedup silently into one entry.
/// - Two components declaring the SAME behavior-relevant slot (same kind+name) with
///   DIFFERENT `tested_value`s return `Err(PackageError::SlotConflict)` — silently
///   discarding one tested value would mask a real behavioral difference.
/// - Identity-bearing collisions with equal declaration fields dedup silently (identity
///   slots have no tested value to conflict over).
pub fn aggregate<'a>(slots: impl IntoIterator<Item = &'a ConfigSlot>) -> Result<Vec<ConfigSlot>> {
    // Key borrows the slot's name (`key()` returns `&str`) — no per-slot key
    // allocation. `entry` does a single lookup instead of get-then-insert.
    let mut map: BTreeMap<(&'static str, &'a str), ConfigSlot> = BTreeMap::new();
    for slot in slots {
        let key = slot.slot.key();
        match map.entry(key) {
            Entry::Vacant(e) => {
                e.insert(slot.clone());
            },
            // Byte-equal declaration — pure dedup, keep the one already present.
            Entry::Occupied(e) if e.get().slot == slot.slot => {},
            Entry::Occupied(e) => {
                // Same `(kind, name)` but a different declaration. A conflict only
                // exists when BOTH carry a differing `tested_value` (a real
                // behavioral difference); identity-bearing collisions have no tested
                // value to conflict over and dedup silently.
                if let (Some(existing_val), Some(incoming_val)) =
                    (e.get().slot.tested_value(), slot.slot.tested_value())
                {
                    if existing_val != incoming_val {
                        return Err(PackageError::SlotConflict {
                            slot: key.1.to_string(),
                            tested: existing_val.to_string(),
                            proposed: incoming_val.to_string(),
                        });
                    }
                }
            },
        }
    }
    Ok(map.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot::types::SlotType;
    use proptest::prelude::*;

    #[test]
    fn dedup_two_identical_secrets_into_one_entry() {
        let a = ConfigSlot {
            slot: SlotType::Secret {
                name: "LICHESS_API_KEY".to_string(),
            },
        };
        let b = a.clone();
        let result = aggregate([&a, &b]).unwrap();
        assert_eq!(result, vec![a]);
    }

    #[test]
    fn conflicting_tested_values_return_slot_conflict_error() {
        let a = ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "anthropic".to_string(),
            },
        };
        let b = ConfigSlot {
            slot: SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "openai".to_string(),
            },
        };
        let err = aggregate([&a, &b]).unwrap_err();
        assert!(matches!(
                    err,
                    PackageError::SlotConflict {
                        tested,
                        proposed,
        ..
                    } if tested == "anthropic" && proposed == "openai"
               ));
    }

    #[test]
    fn preserves_all_distinct_conflict_free_slots() {
        let a = ConfigSlot {
            slot: SlotType::Secret {
                name: "A".to_string(),
            },
        };
        let b = ConfigSlot {
            slot: SlotType::OauthClient {
                name: "B".to_string(),
            },
        };
        let result = aggregate([&a, &b]).unwrap();
        assert_eq!(result.len(), 2);
    }

    proptest! {
            /// /: aggregating any permutation of a conflict-free slot set yields
            /// identical `Vec` output — the aggregated order must never depend on input order
            /// (so the digest stays stable regardless of which component contributed a slot
            /// first).
            #[test]
            fn aggregate_ordering_is_stable_under_permutation(seed in proptest::collection::vec(0u32..1000, 6)) {
                let slots: Vec<ConfigSlot> = (0..6)
    .map(|i| ConfigSlot {
                        slot: SlotType::Secret {
                            name: format!("SECRET_{i}"),
                        },
                    })
    .collect();

                let mut indices: Vec<usize> = (0..6).collect();
                indices.sort_by_key(|&i| seed[i]);
                let permuted: Vec<&ConfigSlot> = indices.iter().map(|&i| &slots[i]).collect();

                let baseline = aggregate(slots.iter()).unwrap();
                let shuffled = aggregate(permuted).unwrap();
                prop_assert_eq!(baseline, shuffled);
            }
        }
}
