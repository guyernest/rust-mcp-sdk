//! Property tests for the team-mcp dispatch guards (109-05).
//!
//! Invariants proven over generated strings / ids / depth bounds
//! (`team_dispatch_surface`, threats T-109-05-01/02/04):
//! (a) strict depth parse — a non-integer string NEVER parses (and never
//!     silently yields `0`); a valid integer parses to itself;
//! (b) self-call — equal `MemberId`s are rejected, distinct ones accepted;
//! (c) ancestor-cycle — a target present in the chain is rejected, absence
//!     accepted;
//! (d) depth bound — `depth <= max` is Ok, `depth > max` is an error, exactly
//!     at/above/below the boundary;
//! (e) absent-depth `_meta` reads as a root call (`depth == 0`).

#![cfg(feature = "team-mcp")]

use pmcp::RequestHandlerExtra;
use proptest::prelude::*;
use serde_json::json;

use pmcp_team_servers::team::guards::{
    guard_ancestor_cycle, guard_depth, guard_self_call, parse_depth_strict, read_guard_state,
    GuardError, META_DEPTH,
};
use pmcp_team_servers::team::identity::MemberId;

proptest! {
    /// (a) A string that is NOT a valid `i64` must ERROR — never `0`, never Ok.
    #[test]
    fn strict_depth_rejects_non_integers(s in ".*") {
        prop_assume!(s.parse::<i64>().is_err());
        let parsed = parse_depth_strict(&s);
        prop_assert!(parsed.is_err(), "non-integer {s:?} must not parse");
        prop_assert!(
            matches!(parsed, Err(GuardError::MalformedDepth(_))),
            "garbage must be MalformedDepth, not a silent 0"
        );
    }

    /// (a') A valid integer parses to exactly itself.
    #[test]
    fn strict_depth_round_trips_integers(n in any::<i64>()) {
        prop_assert_eq!(parse_depth_strict(&n.to_string()).unwrap(), n);
    }

    /// (b) Self-call is rejected iff the ids are equal.
    #[test]
    fn self_call_rejects_equal_ids(a in "[a-z]{1,8}", b in "[a-z]{1,8}", ver in "[0-9]{1,3}") {
        let target = MemberId::from_wire(format!("{a}@{ver}.0.0"));
        let other = MemberId::from_wire(format!("{b}@{ver}.0.0"));
        // Same id -> always rejected.
        prop_assert!(guard_self_call(&target, &target).is_err());
        // Distinct string ids -> accepted; equal ones -> rejected.
        if a == b {
            prop_assert!(guard_self_call(&target, &other).is_err());
        } else {
            prop_assert!(guard_self_call(&target, &other).is_ok());
        }
    }

    /// (c) Ancestor-cycle is rejected iff the target is in the chain.
    #[test]
    fn ancestor_cycle_tracks_membership(
        names in prop::collection::vec("[a-z]{1,6}", 0..6),
        target_name in "[a-z]{1,6}",
    ) {
        let ancestors: Vec<MemberId> = names
            .iter()
            .map(|n| MemberId::from_wire(format!("{n}@1.0.0")))
            .collect();
        let target = MemberId::from_wire(format!("{target_name}@1.0.0"));
        let present = ancestors.iter().any(|a| a == &target);
        let guarded = guard_ancestor_cycle(&target, &ancestors);
        prop_assert_eq!(guarded.is_err(), present);
    }

    /// (d) Depth bound behaves at / above / below `max`.
    #[test]
    fn depth_bound_is_inclusive(depth in -5i64..50, max in 0i64..20) {
        let guarded = guard_depth(depth, max);
        prop_assert_eq!(guarded.is_err(), depth > max);
    }

    /// (e) An absent depth `_meta` key reads as a root call (`depth == 0`).
    #[test]
    fn absent_depth_meta_is_zero(extra_key in "[a-z]{0,8}") {
        // Guard against the improbable case of the generated key colliding with
        // the real depth key.
        prop_assume!(extra_key != META_DEPTH);
        // A _meta object that never carries the depth key.
        let meta = if extra_key.is_empty() {
            json!({})
        } else {
            json!({ extra_key: "irrelevant" })
        };
        let extra = RequestHandlerExtra::default().with_request_meta(Some(meta));
        let state = read_guard_state(&extra).unwrap();
        prop_assert_eq!(state.depth, 0);
        prop_assert!(state.caller.is_none());
        prop_assert!(state.ancestors.is_empty());
    }
}
