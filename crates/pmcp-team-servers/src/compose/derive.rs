//! The pure composition-derivation rule `derive_attachment`.
//!
//! Given a captured [`pmcp_package::TeamPackage`], decide which reference
//! servers attach:
//!
//! - `team-mcp` iff the roster has ≥2 AI agents (`members.len() >= 2`) — a
//!   team-of-one needs no member-dispatch server (D-05).
//! - `approval-mcp` iff the team declares ≥1 human role
//!   (`!human_roles.is_empty()`). The channel initiator is implicit and never
//!   counted (D-05).
//! - `team-fs` / `mem-mcp` attach ONLY when explicitly listed in
//!   `built_in_servers` — those references are demoted to opt-in extras (D-06);
//!   they are surfaced here as `opt_ins`, deduplicated by `(name,
//!   component_type)` with first-seen order preserved. The wiring layer
//!   (109-06) fail-closes on unknown/uncompiled opt-in names, so derive keeps
//!   `opt_ins` as-is minus duplicates.
//!
//! A team-of-one with zero human roles therefore derives both flags false.
//!
//! Implemented atomically in this plan (109-01) — the first export of
//! `derive_attachment` is the real rule, never a placeholder.

use pmcp_package::reference::ComponentType;
use pmcp_package::{ComponentRef, TeamPackage};

/// The set of reference servers a team gets, derived from its package.
#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentSet {
    /// Whether the member-dispatch `team-mcp` server attaches (≥2 AI agents).
    pub team_mcp: bool,
    /// Whether the human-approval `approval-mcp` server attaches (≥1 human role).
    pub approval_mcp: bool,
    /// Explicitly-listed built-in servers (e.g. `team-fs`/`mem-mcp`), demoted
    /// to opt-in extras and deduplicated by `(name, component_type)`.
    pub opt_ins: Vec<ComponentRef>,
}

/// A snapshot of the composition-relevant counts, resolved once at entry (D-07).
///
/// Capturing counts up front keeps the derivation a pure total function of a
/// single immutable observation of the package — no field is re-read after the
/// decision begins.
#[derive(Debug, Clone, PartialEq)]
pub struct CompositionSnapshot {
    /// Number of AI-agent roster members.
    pub ai_agent_count: usize,
    /// Number of declared human roles.
    pub human_role_count: usize,
    /// The deduplicated built-in-server opt-ins.
    pub opt_ins: Vec<ComponentRef>,
}

impl CompositionSnapshot {
    /// Resolve the composition-relevant counts from a package once, at entry.
    #[must_use]
    pub fn from_package(pkg: &TeamPackage) -> Self {
        Self {
            ai_agent_count: pkg.members.len(),
            human_role_count: pkg.human_roles.len(),
            opt_ins: dedupe_opt_ins(&pkg.built_in_servers),
        }
    }
}

/// Deduplicate component references by `(name, component_type)`, preserving
/// first-seen order.
fn dedupe_opt_ins(refs: &[ComponentRef]) -> Vec<ComponentRef> {
    let mut seen: Vec<(String, ComponentType)> = Vec::new();
    let mut out: Vec<ComponentRef> = Vec::new();
    for r in refs {
        let key = (r.name().to_string(), r.component_type());
        if !seen.contains(&key) {
            seen.push(key);
            out.push(r.clone());
        }
    }
    out
}

/// Derive which reference servers a captured team package gets.
///
/// See the module docs for the exact rule. This is a pure, total, panic-free
/// function of the package's member/human-role counts and built-in-server list.
#[must_use]
pub fn derive_attachment(pkg: &TeamPackage) -> AttachmentSet {
    let snapshot = CompositionSnapshot::from_package(pkg);
    AttachmentSet {
        team_mcp: snapshot.ai_agent_count >= 2,
        approval_mcp: snapshot.human_role_count > 0,
        opt_ins: snapshot.opt_ins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};

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

    fn pkg(members: usize, humans: usize, built_ins: Vec<ComponentRef>) -> TeamPackage {
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
            built_in_servers: built_ins,
            finalizer_agents: vec![],
            budget_defaults: vec![],
            config_slots: vec![],
        }
    }

    #[test]
    fn team_of_one_zero_humans_yields_both_false_but_honors_opt_ins() {
        let a = derive_attachment(&pkg(1, 0, vec![server_ref("team-fs")]));
        assert!(!a.team_mcp);
        assert!(!a.approval_mcp);
        assert_eq!(a.opt_ins, vec![server_ref("team-fs")]);
    }

    #[test]
    fn two_members_attach_team_mcp() {
        let a = derive_attachment(&pkg(2, 0, vec![]));
        assert!(a.team_mcp);
        assert!(!a.approval_mcp);
    }

    #[test]
    fn one_human_role_attaches_approval_mcp() {
        let a = derive_attachment(&pkg(1, 1, vec![]));
        assert!(!a.team_mcp);
        assert!(a.approval_mcp);
    }

    #[test]
    fn duplicate_built_in_servers_are_deduped_preserving_order() {
        let a = derive_attachment(&pkg(
            1,
            0,
            vec![
                server_ref("team-fs"),
                server_ref("mem-mcp"),
                server_ref("team-fs"),
            ],
        ));
        assert_eq!(
            a.opt_ins,
            vec![server_ref("team-fs"), server_ref("mem-mcp")]
        );
    }
}
