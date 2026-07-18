//! Runtime configuration contract for a resolved agent.
//!
//! [`ResolvedAgentConfig`] is the fully-resolved runtime config the engine and
//! adapter consume — the product of resolving an `AgentPackage`'s config slots
//! against a host environment. The `SlotResolver` seam, `resolve_agent`, and the
//! endpoint map impls land in plan 108-05 (resolver.rs / endpoint.rs in this
//! module).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Fully-resolved runtime configuration for one agent.
///
/// Binds everything the engine/adapter needs to run a package-defined agent:
/// instructions, the selected tool set, integer limits, optional I/O schemas,
/// the resolved model identifier, and a connector-name → endpoint map. Mirrors
/// the `AgentPackage` fields (which carries no `description`, so neither does
/// this). All limits are integers — no floats — to keep replay deterministic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResolvedAgentConfig {
    /// System instructions for the agent.
    pub instructions: String,
    /// Names of the tools this agent may call (resolved tool selection).
    pub tools: Vec<String>,
    /// Maximum tokens the completion may generate per turn.
    pub max_tokens: u32,
    /// Maximum loop iterations before the run is forced to stop.
    pub max_iterations: u32,
    /// Optional JSON Schema for the agent's tool input.
    pub input_schema: Option<serde_json::Value>,
    /// Optional JSON Schema for the agent's structured output.
    pub output_schema: Option<serde_json::Value>,
    /// Resolved model identifier (the resolved value of the `llm` config slot).
    pub model: String,
    /// Connector-name → endpoint (URL or command) map (D-16).
    pub endpoints: HashMap<String, String>,
}

impl ResolvedAgentConfig {
    /// Create a resolved config with the required fields; optional schemas and
    /// endpoints default to empty.
    #[must_use]
    pub fn new(
        instructions: impl Into<String>,
        model: impl Into<String>,
        max_tokens: u32,
        max_iterations: u32,
    ) -> Self {
        Self {
            instructions: instructions.into(),
            tools: Vec::new(),
            max_tokens,
            max_iterations,
            input_schema: None,
            output_schema: None,
            model: model.into(),
            endpoints: HashMap::new(),
        }
    }
}
