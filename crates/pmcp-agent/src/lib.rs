//! Deploy-anywhere agent decision loop for the PMCP SDK (experimental, 0.x).
//!
//! `pmcp-agent` is a pure decision loop that runs between three object-safe
//! async effect seams — [`CompletionSource`], [`ToolInvoker`], and
//! [`ConversationStore`] — configured from an `AgentPackage`. It is isolated
//! from `pmcp` core (like `pmcp-tasks`) so the experimental agent runtime can
//! evolve on its own 0.x cadence.
//!
//! # Module Organization
//!
//! - [`seams`] — the three object-safe effect seams + shared `RetryClass`
//! - [`config`] — [`ResolvedAgentConfig`] runtime-config contract (resolver in 108-05)
//! - [`iteration`] — pure decision functions + async engine (108-03)
//! - [`sources`] — the three `CompletionSource` implementations (108-04)
//! - [`invoker`] — tasks-aware `ToolInvoker` + connector factory (108-05)
//! - [`adapter`] — agent-as-server adapter on `ServerCore` (108-06)
//! - [`trace`] — public `EffectTrace` replay artifact (108-03)

pub mod adapter;
pub mod config;
pub mod invoker;
pub mod iteration;
pub mod seams;
pub mod sources;
pub mod trace;

pub use config::ResolvedAgentConfig;
pub use seams::{
    CompletionError, CompletionSource, RetryClass, ToolCall, ToolCallResult, ToolError, ToolInvoker,
};
// ConversationStore/RunState re-exports are added in plan 108-02 Task 3.
