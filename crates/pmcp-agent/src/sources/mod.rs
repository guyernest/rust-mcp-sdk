//! [`CompletionSource`](crate::seams::CompletionSource) implementations.
//!
//! The trait is the extension point (AGNT-01), so this module ships concrete
//! sources behind one interface:
//!
//! - [`SamplingSource`] — zero-dependency, drives the model over the
//!   server-side peer's spec sampling surface (AGNT-04). Always compiled.
//! - `OpenAiCompatSource` — any OpenAI-compatible `/chat/completions` endpoint
//!   (Ollama / vLLM / OpenRouter / …), behind the `openai-compat` feature
//!   (AGNT-05).
//! - `AnthropicSource` — the Anthropic Messages API, behind the `anthropic`
//!   feature (AGNT-06).
//!
//! The default (no-feature) build compiles only [`SamplingSource`] and never
//! pulls `reqwest`, keeping the default + wasm32 build clean (D-13). API keys
//! for the HTTP sources live in a redacting [`SecretString`].

mod sampling;
mod secret;

pub use sampling::SamplingSource;
pub use secret::SecretString;
