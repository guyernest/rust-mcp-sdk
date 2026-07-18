//! The completion seam — produce the next model completion.

use async_trait::async_trait;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResultWithTools};

use super::RetryClass;

/// A source of model completions.
///
/// Reuses the SDK sampling types verbatim (AGNT-01): it takes
/// [`CreateMessageParams`] and returns [`CreateMessageResultWithTools`] so the
/// completion can carry `tool_use` blocks. Implementations are object-safe
/// (`Arc<dyn CompletionSource>`): `SamplingSource` (zero-dep, over the server
/// peer) and the feature-gated `OpenAiCompatSource` / `AnthropicSource` all
/// satisfy this one trait — the trait is the extension point, not a provider
/// matrix.
#[async_trait]
pub trait CompletionSource: Send + Sync {
    /// Produce the next completion for `params`.
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError>;
}

/// Error from a [`CompletionSource`].
///
/// Variants never carry secret material (API keys, auth headers). The
/// [`retry_class`](CompletionError::retry_class) accessor maps the error to a
/// [`RetryClass`] the loop returns as data — the source itself applies no
/// backoff.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompletionError {
    /// Transport / connection / 5xx failure — transient.
    #[error("completion transport error: {0}")]
    Transport(String),
    /// Rate-limited / capacity (429 / 529) — retryable with backpressure.
    #[error("completion capacity error: {0}")]
    Capacity(String),
    /// Response could not be decoded into the expected shape — fatal.
    #[error("completion decode error: {0}")]
    Decode(String),
    /// Authentication/authorization failure — fatal (no secret echoed).
    #[error("completion authentication failed")]
    Auth,
}

impl CompletionError {
    /// Classify this error for the loop's retry-as-data contract.
    #[must_use]
    pub fn retry_class(&self) -> RetryClass {
        match self {
            Self::Transport(_) => RetryClass::Transient { attempt_hint: 0 },
            Self::Capacity(_) => RetryClass::Capacity { attempt_hint: 0 },
            Self::Decode(_) | Self::Auth => RetryClass::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionError, RetryClass};

    #[test]
    fn retry_class_maps_each_variant() {
        assert_eq!(
            CompletionError::Transport("x".into()).retry_class(),
            RetryClass::Transient { attempt_hint: 0 }
        );
        assert_eq!(
            CompletionError::Capacity("x".into()).retry_class(),
            RetryClass::Capacity { attempt_hint: 0 }
        );
        assert_eq!(
            CompletionError::Decode("x".into()).retry_class(),
            RetryClass::Fatal
        );
        assert_eq!(CompletionError::Auth.retry_class(), RetryClass::Fatal);
    }
}
