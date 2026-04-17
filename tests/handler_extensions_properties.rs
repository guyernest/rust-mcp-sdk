//! Property-based tests for RequestHandlerExtra.extensions typemap.
//!
//! Covers: insert/get round-trip, key-collision returns old value, clone
//! preserves extensions, remove::<T>() round-trip, mixed-type coexistence.

use pmcp::RequestHandlerExtra;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    /// 70-01-02: Insert + get of a typed value yields the same value.
    #[test]
    fn prop_extensions_insert_get_roundtrip(key in ".{0,64}", value in any::<u64>()) {
        let mut extra = RequestHandlerExtra::default();
        let pair = (key.clone(), value);
        extra.extensions_mut().insert(pair.clone());
        let retrieved = extra.extensions().get::<(String, u64)>();
        prop_assert_eq!(retrieved, Some(&pair));
    }

    /// 70-01-03 (property-reinforced): inserting same type twice returns Some(old).
    #[test]
    fn prop_extensions_key_collision_returns_old_value(v1: u64, v2: u64) {
        let mut extra = RequestHandlerExtra::default();
        prop_assert_eq!(extra.extensions_mut().insert(v1), None);
        prop_assert_eq!(extra.extensions_mut().insert(v2), Some(v1));
        prop_assert_eq!(extra.extensions().get::<u64>(), Some(&v2));
    }

    /// 70-01-04: extra.clone() preserves extensions key set.
    #[test]
    fn prop_extra_clone_preserves_extensions(value in ".{1,64}") {
        let mut extra = RequestHandlerExtra::default();
        extra.extensions_mut().insert(value.clone());
        let cloned = extra.clone();
        prop_assert_eq!(cloned.extensions().get::<String>(), Some(&value));
        prop_assert_eq!(extra.extensions().get::<String>(), Some(&value));
    }

    /// NEW per Codex review LOW: remove<T>() returns the value when present, None after.
    #[test]
    fn prop_extensions_remove_returns_value(v: u64) {
        let mut extra = RequestHandlerExtra::default();
        extra.extensions_mut().insert(v);
        prop_assert_eq!(extra.extensions_mut().remove::<u64>(), Some(v));
        prop_assert_eq!(extra.extensions_mut().remove::<u64>(), None);
        prop_assert_eq!(extra.extensions().get::<u64>(), None);
    }

    /// NEW per Codex review LOW: two values of DIFFERENT types coexist without interference.
    #[test]
    fn prop_extensions_two_types_coexist(s in ".{1,64}", n: u64) {
        let mut extra = RequestHandlerExtra::default();
        extra.extensions_mut().insert(s.clone());
        extra.extensions_mut().insert(n);
        prop_assert_eq!(extra.extensions().get::<String>(), Some(&s));
        prop_assert_eq!(extra.extensions().get::<u64>(), Some(&n));
        // Removing one does not affect the other
        extra.extensions_mut().remove::<u64>();
        prop_assert_eq!(extra.extensions().get::<String>(), Some(&s));
        prop_assert_eq!(extra.extensions().get::<u64>(), None);
    }
}
