//! `TeamPackage` — the captured `AgentTeam` roster + human-role declarations
//!.
//!
//! Field mapping is drawn from `amplify/data/resource.ts:1591-1785`'s
//! `AgentTeam`/`TeamMember`/`TeamHumanMember` models. The one hard structural
//! rule (permanent non-goal §12): a captured `TeamPackage` NEVER carries
//! a human identity. The source `TeamHumanMember` row DOES carry a real
//! `userId`/`channelId`/`channelAddress`/`pendingApproverEmail`, but capture
//! strips those down to a role declaration — [`HumanRole`] structurally
//! cannot represent an identity field; there is no field to accidentally
//! populate with one.

use crate::reference::ComponentRef;
use crate::slot::{ConfigSlot, SlotType};
use serde::{Deserialize, Serialize};

/// A team member's role within the roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    EntryPoint,
    Member,
}

/// One agent-roster entry: which agent (by capture-time range — see
/// `WorkflowManifest` for the pinned form), and its role in the team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent: ComponentRef,
    pub role: TeamRole,
}

/// A human role a team needs filled — role declaration ONLY. This
/// struct's field list is closed and MUST NEVER gain a `user_id`/
/// `channel_id`/`email`/`channel_address` (or any other identity) field: the
/// source `TeamHumanMember` row's identity fields are resolved at BIND time
/// (when a real human is assigned this role in a live team), never at
/// capture/package time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanRole {
    /// The role label (from `TeamHumanMember.roleLabel`).
    pub role: String,
    /// A human-readable description (from `TeamHumanMember.toolDescription`).
    pub description: String,
    /// What this role is expected to cover.
    pub responsibilities: Vec<String>,
    /// Display-only hints about suitable channel kinds for this role.
    pub channel_hints: Vec<String>,
}

impl HumanRole {
    /// Map this role declaration into its `ConfigSlot` representation
    /// (`SlotType::HumanRole`) — "human seats become human-role config
    /// slots".
    pub fn to_config_slot(&self) -> ConfigSlot {
        ConfigSlot {
            slot: SlotType::HumanRole {
                role: self.role.clone(),
                description: self.description.clone(),
                responsibilities: self.responsibilities.clone(),
                channel_hints: self.channel_hints.clone(),
            },
        }
    }
}

/// Team-wide default limits (from `AgentTeam.limits`'s JSON document).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamLimits {
    pub max_team_depth: i64,
    pub max_team_total_tokens: i64,
    pub max_team_wall_clock_seconds: i64,
    pub poll_interval_ms: i64,
}

/// The captured `team` AI-Package payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamPackage {
    pub name: String,
    pub version: semver::Version,
    /// The team's entry-point agent (from `AgentTeam.entryPointAgentId`).
    pub entry_point: ComponentRef,
    pub members: Vec<TeamMember>,
    ///: role declarations only — see [`HumanRole`] doc.
    pub human_roles: Vec<HumanRole>,
    pub limits: TeamLimits,
    /// Built-in servers available to every member (from
    /// `AgentTeam.builtInServerIds`).
    pub built_in_servers: Vec<ComponentRef>,
    /// Finalizer agents (from `AgentTeam.finalizerAgentIds`).
    pub finalizer_agents: Vec<ComponentRef>,
    /// Budget-override DEFAULTS captured at test time (same
    /// deviation-on-override semantics as `AgentPackage::budget_defaults`).
    pub budget_defaults: Vec<ConfigSlot>,
    /// All declared config slots for the team as a whole, INCLUDING each
    /// `human_roles` entry's `to_config_slot()` mapping.
    pub config_slots: Vec<ConfigSlot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::ComponentType;

    fn sample_human_role() -> HumanRole {
        HumanRole {
            role: "approver".to_string(),
            description: "Approves budget overrides".to_string(),
            responsibilities: vec!["review".to_string(), "approve".to_string()],
            channel_hints: vec!["slack".to_string()],
        }
    }

    fn sample_team_package() -> TeamPackage {
        let entry_point = ComponentRef::Range {
            name: "triage-agent".to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        };
        let human_role = sample_human_role();
        TeamPackage {
            name: "support-team".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            entry_point: entry_point.clone(),
            members: vec![TeamMember {
                agent: entry_point,
                role: TeamRole::EntryPoint,
            }],
            human_roles: vec![human_role.clone()],
            limits: TeamLimits {
                max_team_depth: 3,
                max_team_total_tokens: 200_000,
                max_team_wall_clock_seconds: 600,
                poll_interval_ms: 2000,
            },
            built_in_servers: vec![ComponentRef::Range {
                name: "team-fs".to_string(),
                range: semver::VersionReq::parse("^1").unwrap(),
                component_type: ComponentType::Server,
            }],
            finalizer_agents: vec![],
            budget_defaults: vec![],
            config_slots: vec![human_role.to_config_slot()],
        }
    }

    #[test]
    fn team_package_round_trips_losslessly() {
        let pkg = sample_team_package();
        let json = serde_json::to_string(&pkg).unwrap();
        let back: TeamPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkg);
    }

    #[test]
    fn team_role_round_trips_with_snake_case_discriminator() {
        let json = serde_json::to_value(TeamRole::EntryPoint).unwrap();
        assert_eq!(json, serde_json::json!("entry_point"));
        let back: TeamRole = serde_json::from_value(json).unwrap();
        assert_eq!(back, TeamRole::EntryPoint);
    }

    /// structural proof: `HumanRole` constructs with ONLY
    /// role/description/responsibilities/channel_hints — there is no
    /// user_id/channel_id/email/channel_address field to populate, and the
    /// serialized form carries none of those keys either.
    #[test]
    fn human_role_has_no_identity_field() {
        let role = sample_human_role();
        let json = serde_json::to_value(&role).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("user_id"));
        assert!(!obj.contains_key("userId"));
        assert!(!obj.contains_key("channel_id"));
        assert!(!obj.contains_key("channelId"));
        assert!(!obj.contains_key("email"));
        assert!(!obj.contains_key("channel_address"));
        assert!(!obj.contains_key("channelAddress"));
    }

    #[test]
    fn human_role_maps_to_human_role_config_slot() {
        let role = sample_human_role();
        let slot = role.to_config_slot();
        assert_eq!(slot.slot.key(), ("human_role", "approver"));
    }
}
