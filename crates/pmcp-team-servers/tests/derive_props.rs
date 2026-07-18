//! Property tests for the pure composition-derivation rule `derive_attachment`.
//!
//! Invariants proven over generated rosters / human-role lists / built-in-server
//! lists (D-05/D-06/D-07):
//! (a) `team_mcp <=> members.len() >= 2`
//! (b) `approval_mcp <=> !human_roles.is_empty()`
//! (c) `opt_ins` is the deduplicated `built_in_servers` (order preserved, no
//!     duplicate `(name, component_type)`)
//! (d) team-of-one blessing: a 1-member, 0-human package always yields
//!     `{ team_mcp: false, approval_mcp: false }`.

use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
use pmcp_package::reference::ComponentType;
use pmcp_package::{ComponentRef, TeamPackage};
use pmcp_team_servers::derive_attachment;
use proptest::prelude::*;

fn agent_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Agent,
    }
}

fn server_ref(name: &str) -> ComponentRef {
    ComponentRef::Range {
        name: name.to_string(),
        range: semver::VersionReq::parse("^1").unwrap(),
        component_type: ComponentType::Server,
    }
}

fn human_role(label: &str) -> HumanRole {
    HumanRole {
        role: label.to_string(),
        description: String::new(),
        responsibilities: vec![],
        channel_hints: vec![],
    }
}

fn make_pkg(members: usize, humans: usize, built_in_names: &[String]) -> TeamPackage {
    TeamPackage {
        name: "t".to_string(),
        version: semver::Version::parse("1.0.0").unwrap(),
        entry_point: agent_ref("entry"),
        members: (0..members)
            .map(|i| TeamMember {
                agent: agent_ref(&format!("a{i}")),
                role: if i == 0 {
                    TeamRole::EntryPoint
                } else {
                    TeamRole::Member
                },
            })
            .collect(),
        human_roles: (0..humans).map(|i| human_role(&format!("h{i}"))).collect(),
        limits: TeamLimits {
            max_team_depth: 3,
            max_team_total_tokens: 1,
            max_team_wall_clock_seconds: 1,
            poll_interval_ms: 1,
        },
        built_in_servers: built_in_names.iter().map(|n| server_ref(n)).collect(),
        finalizer_agents: vec![],
        budget_defaults: vec![],
        config_slots: vec![],
    }
}

/// Reference dedup: first-seen order, no duplicate `(name, component_type)`.
fn expected_dedup(names: &[String]) -> Vec<ComponentRef> {
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for n in names {
        if !seen.contains(n) {
            seen.push(n.clone());
            out.push(server_ref(n));
        }
    }
    out
}

proptest! {
    #[test]
    fn team_mcp_iff_two_or_more_members(
        members in 0usize..6,
        humans in 0usize..5,
        names in prop::collection::vec("[a-c]{1,3}", 0..6),
    ) {
        let a = derive_attachment(&make_pkg(members, humans, &names));
        prop_assert_eq!(a.team_mcp, members >= 2);
    }

    #[test]
    fn approval_mcp_iff_human_roles_present(
        members in 0usize..6,
        humans in 0usize..5,
        names in prop::collection::vec("[a-c]{1,3}", 0..6),
    ) {
        let a = derive_attachment(&make_pkg(members, humans, &names));
        prop_assert_eq!(a.approval_mcp, humans > 0);
    }

    #[test]
    fn opt_ins_is_deduped_built_in_servers(
        members in 0usize..6,
        humans in 0usize..5,
        names in prop::collection::vec("[a-c]{1,3}", 0..8),
    ) {
        let a = derive_attachment(&make_pkg(members, humans, &names));
        // Order preserved and no duplicate (name, type).
        prop_assert_eq!(&a.opt_ins, &expected_dedup(&names));
        let mut keys: Vec<(&str, ComponentType)> =
            a.opt_ins.iter().map(|r| (r.name(), r.component_type())).collect();
        let len_before = keys.len();
        keys.sort_by(|l, r| l.0.cmp(r.0).then(l.1.cmp(&r.1)));
        keys.dedup();
        prop_assert_eq!(keys.len(), len_before);
    }

    #[test]
    fn team_of_one_zero_humans_blessing(
        names in prop::collection::vec("[a-c]{1,3}", 0..6),
    ) {
        let a = derive_attachment(&make_pkg(1, 0, &names));
        prop_assert!(!a.team_mcp);
        prop_assert!(!a.approval_mcp);
    }
}
