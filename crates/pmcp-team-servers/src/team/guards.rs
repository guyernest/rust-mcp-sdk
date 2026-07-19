//! Dispatch guards: strict depth parsing, self-call rejection, and
//! ancestor-cycle rejection — all comparing [`super::identity::MemberId`]s,
//! never display names — over guard state carried as namespaced `_meta`.
//!
//! # Guard state travels as `_meta` (locked D-14, route A)
//!
//! Team-recursion guard state does NOT ride inside the tool `arguments`; it
//! travels as namespaced keys on the `tools/call` request's `_meta`, which the
//! 109-00 pmcp-core change surfaces to handlers via
//! [`RequestHandlerExtra::request_meta`]. [`read_guard_state`] parses that raw
//! JSON defensively (it is untrusted input crossing a trust boundary), and the
//! three guard functions enforce the `team_dispatch_surface` contract:
//!
//! - **strict depth** — an ABSENT depth key means a root/entry call
//!   (`depth = 0`); a PRESENT depth is parsed strictly ([`parse_depth_strict`]),
//!   and any garbage value is an error (it NEVER silently defaults to `0`, which
//!   would defeat the bounded-recursion guard — T-109-05-01);
//! - **self-call** — a target [`MemberId`] equal to the caller is rejected
//!   (compared by id, never by display name — T-109-05-04);
//! - **ancestor-cycle** — a target already present in the caller's ancestor
//!   chain is rejected (T-109-05-02).

use pmcp::RequestHandlerExtra;
use serde_json::Value;

use super::identity::MemberId;

/// Namespaced `_meta` key carrying the caller's current team depth.
///
/// Mapped from the `x-pmcp-team-depth` HTTP header at the binary edge (D-14) or
/// set directly on an in-memory hop's forwarded [`RequestMeta`](pmcp::types::protocol::RequestMeta).
pub const META_DEPTH: &str = "x-pmcp-team-depth";

/// Namespaced `_meta` key carrying the immediate caller's [`MemberId`] (wire form).
pub const META_CALLER: &str = "x-pmcp-team-caller";

/// Namespaced `_meta` key carrying the caller's ancestor chain (a JSON array of
/// [`MemberId`] wire strings, root-first).
pub const META_ANCESTORS: &str = "x-pmcp-team-ancestors";

/// Why a dispatch guard rejected a `team_mcp__<member>` call.
///
/// Error messages mirror the `team_dispatch_surface` contract text so a caller
/// (and the conformance runner) can key off stable substrings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    /// The present depth `_meta` value was not a strict integer (garbage is
    /// rejected — it never defaults to `0`).
    #[error("malformed team depth: strict integer parse failed for {0:?}")]
    MalformedDepth(String),
    /// The caller's depth exceeds the team's `max_team_depth` (bounded recursion).
    #[error("excessive team depth: {depth} exceeds max_team_depth {max}")]
    ExcessiveDepth {
        /// The offending (incoming) depth.
        depth: i64,
        /// The configured maximum team depth.
        max: i64,
    },
    /// A member attempted to dispatch to itself (self-call guard; ids compared).
    #[error("self-call rejected: member {0} cannot dispatch to itself")]
    SelfCall(String),
    /// The target already appears in the caller's ancestor chain (cycle guard).
    #[error("ancestor-cycle rejected: member {0} is already in the caller chain")]
    AncestorCycle(String),
    /// The requested member id is not in the configured roster.
    #[error("unknown member: {0}")]
    UnknownMember(String),
}

/// The recursion guard state extracted from a request's namespaced `_meta`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardState {
    /// The caller's current team depth (`0` for a root/entry call).
    pub depth: i64,
    /// The immediate caller's identity, if this is not a root call.
    pub caller: Option<MemberId>,
    /// The caller's ancestor chain (root-first), used for cycle detection.
    pub ancestors: Vec<MemberId>,
}

/// Parse a depth `_meta` value strictly as an `i64`.
///
/// Uses [`str::parse`] so ANY non-integer input — `"x"`, `""`, `"1.5"`, `"-"` —
/// yields [`GuardError::MalformedDepth`]. Garbage NEVER defaults to `0`, because
/// a silent `0` would let a forged/garbage depth restart the bounded-recursion
/// budget from the root (T-109-05-01).
///
/// # Errors
/// [`GuardError::MalformedDepth`] when `raw` is not a valid base-10 `i64`.
pub fn parse_depth_strict(raw: &str) -> Result<i64, GuardError> {
    raw.parse::<i64>()
        .map_err(|_| GuardError::MalformedDepth(raw.to_string()))
}

/// Read the depth from a `_meta` object value.
///
/// - absent (`None`) → `Ok(0)` (root/entry call);
/// - a JSON string → [`parse_depth_strict`] (strict; garbage → error);
/// - a JSON integer → used verbatim (already validated when it was written);
/// - anything else (float, bool, array, object) → [`GuardError::MalformedDepth`].
fn read_depth(value: Option<&Value>) -> Result<i64, GuardError> {
    match value {
        None | Some(Value::Null) => Ok(0),
        Some(Value::String(s)) => parse_depth_strict(s),
        Some(Value::Number(n)) => n
            .as_i64()
            .ok_or_else(|| GuardError::MalformedDepth(n.to_string())),
        Some(other) => Err(GuardError::MalformedDepth(other.to_string())),
    }
}

/// Extract the ancestor chain (`MemberId`s) from a `_meta` array value.
///
/// Non-array or non-string entries are ignored defensively (untrusted input);
/// the resulting chain is only ever used for membership tests.
fn read_ancestors(value: Option<&Value>) -> Vec<MemberId> {
    value
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(MemberId::from_wire))
                .collect()
        })
        .unwrap_or_default()
}

/// Read the recursion guard state from a handler's request `_meta` (109-00).
///
/// The `_meta` is untrusted (it crosses the client → team boundary), so every
/// field is parsed defensively: an absent depth is a root call, a present depth
/// is parsed strictly, and identity fields are reconstructed as
/// [`MemberId`]s (never trusted as display names).
///
/// # Errors
/// [`GuardError::MalformedDepth`] when a present depth key is not a strict
/// integer.
pub fn read_guard_state(extra: &RequestHandlerExtra) -> Result<GuardState, GuardError> {
    let meta = extra.request_meta.as_ref();
    let obj = meta.and_then(Value::as_object);

    let depth = read_depth(obj.and_then(|o| o.get(META_DEPTH)))?;
    let caller = obj
        .and_then(|o| o.get(META_CALLER))
        .and_then(Value::as_str)
        .map(MemberId::from_wire);
    let ancestors = read_ancestors(obj.and_then(|o| o.get(META_ANCESTORS)));

    Ok(GuardState {
        depth,
        caller,
        ancestors,
    })
}

/// Reject a call whose incoming depth already exceeds `max` (bounded recursion).
///
/// # Errors
/// [`GuardError::ExcessiveDepth`] when `depth > max`.
pub fn guard_depth(depth: i64, max: i64) -> Result<(), GuardError> {
    if depth > max {
        Err(GuardError::ExcessiveDepth { depth, max })
    } else {
        Ok(())
    }
}

/// Reject a member dispatching to itself.
///
/// Compares [`MemberId`]s (derived from the member's `ComponentRef`), never
/// display strings — so identity spoofing by name is impossible.
///
/// # Errors
/// [`GuardError::SelfCall`] when `target == caller`.
pub fn guard_self_call(target: &MemberId, caller: &MemberId) -> Result<(), GuardError> {
    if target == caller {
        Err(GuardError::SelfCall(target.to_string()))
    } else {
        Ok(())
    }
}

/// Reject a dispatch whose target already appears in the caller's ancestor chain.
///
/// # Errors
/// [`GuardError::AncestorCycle`] when `target` is present in `ancestors`.
pub fn guard_ancestor_cycle(target: &MemberId, ancestors: &[MemberId]) -> Result<(), GuardError> {
    if ancestors.iter().any(|a| a == target) {
        Err(GuardError::AncestorCycle(target.to_string()))
    } else {
        Ok(())
    }
}

/// Confirm a requested member id is in the configured roster.
///
/// # Errors
/// [`GuardError::UnknownMember`] when `id` is not present in `roster`.
pub fn lookup_member(id: &MemberId, roster: &[MemberId]) -> Result<(), GuardError> {
    if roster.iter().any(|m| m == id) {
        Ok(())
    } else {
        Err(GuardError::UnknownMember(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn extra_with_meta(meta: Value) -> RequestHandlerExtra {
        RequestHandlerExtra::default().with_request_meta(Some(meta))
    }

    #[test]
    fn parse_depth_strict_accepts_integers_and_rejects_garbage() {
        assert_eq!(parse_depth_strict("3").unwrap(), 3);
        assert_eq!(parse_depth_strict("0").unwrap(), 0);
        for garbage in ["x", "", "1.5", "-", "0x1", " 3", "3 "] {
            assert!(
                parse_depth_strict(garbage).is_err(),
                "garbage {garbage:?} must error, never default to 0"
            );
        }
    }

    #[test]
    fn absent_depth_meta_is_a_root_call() {
        // No _meta at all.
        let state = read_guard_state(&RequestHandlerExtra::default()).unwrap();
        assert_eq!(state.depth, 0);
        assert!(state.caller.is_none());
        assert!(state.ancestors.is_empty());

        // _meta present but no depth key.
        let state = read_guard_state(&extra_with_meta(json!({ "unrelated": 1 }))).unwrap();
        assert_eq!(state.depth, 0);
    }

    #[test]
    fn present_garbage_depth_is_an_error() {
        let err = read_guard_state(&extra_with_meta(json!({ META_DEPTH: "abc" }))).unwrap_err();
        assert!(matches!(err, GuardError::MalformedDepth(_)));
        // A float is not a strict integer either.
        let err = read_guard_state(&extra_with_meta(json!({ META_DEPTH: 1.5 }))).unwrap_err();
        assert!(matches!(err, GuardError::MalformedDepth(_)));
    }

    #[test]
    fn integer_depth_and_identities_round_trip_from_meta() {
        let state = read_guard_state(&extra_with_meta(json!({
            META_DEPTH: 2,
            META_CALLER: "triage@1.0.0",
            META_ANCESTORS: ["root@1.0.0", "triage@1.0.0"],
        })))
        .unwrap();
        assert_eq!(state.depth, 2);
        assert_eq!(state.caller, Some(MemberId::from_wire("triage@1.0.0")));
        assert_eq!(
            state.ancestors,
            vec![
                MemberId::from_wire("root@1.0.0"),
                MemberId::from_wire("triage@1.0.0")
            ]
        );
    }

    #[test]
    fn string_depth_from_http_edge_is_parsed_strictly() {
        let state = read_guard_state(&extra_with_meta(json!({ META_DEPTH: "4" }))).unwrap();
        assert_eq!(state.depth, 4);
    }

    #[test]
    fn depth_guard_bounds_recursion() {
        assert!(guard_depth(0, 3).is_ok());
        assert!(guard_depth(3, 3).is_ok());
        assert!(matches!(
            guard_depth(4, 3),
            Err(GuardError::ExcessiveDepth { depth: 4, max: 3 })
        ));
    }

    #[test]
    fn self_call_and_ancestor_cycle_compare_ids() {
        let a = MemberId::from_wire("a@1.0.0");
        let b = MemberId::from_wire("b@1.0.0");
        assert!(guard_self_call(&a, &a).is_err());
        assert!(guard_self_call(&a, &b).is_ok());
        assert!(guard_ancestor_cycle(&a, &[b.clone(), a.clone()]).is_err());
        assert!(guard_ancestor_cycle(&a, &[b]).is_ok());
    }

    #[test]
    fn lookup_member_rejects_unknown() {
        let a = MemberId::from_wire("a@1.0.0");
        let b = MemberId::from_wire("b@1.0.0");
        assert!(lookup_member(&a, &[a.clone(), b.clone()]).is_ok());
        let err = lookup_member(&MemberId::from_wire("ghost@1.0.0"), &[a, b]).unwrap_err();
        assert!(err.to_string().contains("unknown member"));
    }
}
