//! Fuzz target for the team-mcp depth / self-call / ancestor-cycle guards.
//!
//! Feeds arbitrary bytes as the raw depth string and as ancestor-chain / caller
//! identity data into [`parse_depth_strict`] and the guard functions, asserting
//! they never panic (T-109-05-01 bounded recursion; T-109-05-02 cycle guard).
//! The parser is the sharp edge: strict integer parsing of untrusted, possibly
//! multi-kilobyte garbage must be total (Err, never a panic, never a silent 0).
#![no_main]

use libfuzzer_sys::fuzz_target;

use pmcp_team_servers::team::guards::{
    guard_ancestor_cycle, guard_depth, guard_self_call, parse_depth_strict,
};
use pmcp_team_servers::team::identity::MemberId;

fuzz_target!(|data: &[u8]| {
    // Split the input on NUL so a single corpus entry exercises the depth
    // parser and several distinct identity strings.
    let mut parts = data.split(|&b| b == 0);

    // 1) The raw depth string: strict parse must be total (Ok or MalformedDepth).
    let raw = parts.next().unwrap_or(&[]);
    let raw = String::from_utf8_lossy(raw);
    if let Ok(depth) = parse_depth_strict(&raw) {
        // A successfully-parsed depth is fed through the bounds guard against a
        // handful of maxima; none of these may panic.
        for max in [-1, 0, 1, 3, i64::MAX] {
            let _ = guard_depth(depth, max);
        }
    }

    // 2) Remaining segments become member ids for the identity guards.
    let ids: Vec<MemberId> = parts
        .map(|seg| MemberId::from_wire(String::from_utf8_lossy(seg).into_owned()))
        .collect();

    if let Some((target, ancestors)) = ids.split_first() {
        // Self-call against itself and every other id — pure id comparison.
        let _ = guard_self_call(target, target);
        for other in ancestors {
            let _ = guard_self_call(target, other);
        }
        // Ancestor-cycle over the remaining chain — pure membership test.
        let _ = guard_ancestor_cycle(target, ancestors);
    }
});
