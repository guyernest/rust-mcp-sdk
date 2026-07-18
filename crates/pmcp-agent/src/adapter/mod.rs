//! Agent-as-server adapter on the store-backed task lifecycle (plan 108-06).
//!
//! NATIVE-ONLY (`cfg(not(target_arch = "wasm32"))`): the adapter builds a
//! single-tool [`pmcp::Server`] with a `task_store` and a `SamplingSource`, both
//! of which are native-only in `pmcp`. The wasm32 CI gate (D-13) proves the LOOP
//! + SEAMS + config path compiles target-clean; it does NOT claim adapter-on-wasm.
//!
//! - [`AgentServer`] — one package-driven, task-supported tool backed by a REAL
//!   store lifecycle (create → working → completed), stateless per call.
//! - [`CompletionSourceFactory`] — builds THIS request's completion source
//!   ([`SamplingSourceFactory`] from `extra.peer()`, or [`FixedSourceFactory`]).

#[cfg(not(target_arch = "wasm32"))]
mod factory;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
pub use factory::{CompletionSourceFactory, FixedSourceFactory, SamplingSourceFactory};
#[cfg(not(target_arch = "wasm32"))]
pub use server::{derive_tool_description, AgentServer, AgentServerBuilder};
