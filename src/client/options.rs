//! Client configuration options.
//!
//! [`ClientOptions`] is the additive surface for configuring a [`crate::Client`]
//! beyond the protocol-level [`crate::shared::ProtocolOptions`]. This type is
//! marked `#[non_exhaustive]` so future knobs (Phase 73 deferred ideas —
//! StrictMode / typed-output for PARITY-CLIENT-02) can be added without a
//! breaking change.

/// Client-level configuration.
///
/// Constructed via [`ClientOptions::default`] combined with field-update
/// syntax. From outside the `pmcp` crate the struct literal is forbidden by
/// `#[non_exhaustive]`, so callers must always spread `..Default::default()`.
///
/// # Memory amplification note
///
/// Settings like `max_iterations` bound how many pages the `list_all_*`
/// helpers will accumulate in memory before returning. Because those helpers
/// return a fully materialised `Vec`, they are memory-amplifying convenience
/// APIs. For very large servers, prefer the paginated single-page
/// [`crate::Client::list_tools`] / `list_prompts` / `list_resources` /
/// `list_resource_templates` methods and stream the output.
///
/// # `max_iterations = 0`
///
/// Setting `max_iterations = 0` is legal but degenerate: the `list_all_*`
/// bounded loop performs zero iterations and immediately returns
/// [`crate::Error::Validation`] with the cap-exceeded message. Callers should
/// treat this as "disabled" — use a small positive integer if you want at
/// least one page fetched.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::ClientOptions;
///
/// let opts = ClientOptions { max_iterations: 50, ..Default::default() };
/// assert_eq!(opts.max_iterations, 50);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ClientOptions {
    /// Maximum number of pagination iterations `list_all_*` helpers will
    /// perform before returning [`crate::Error::Validation`]. Default: `100`.
    ///
    /// `0` is legal but produces an immediate cap-exceeded error — see the
    /// struct-level docs.
    pub max_iterations: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            max_iterations: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_max_iterations_is_100() {
        let opts = ClientOptions::default();
        assert_eq!(opts.max_iterations, 100);
    }

    #[test]
    fn field_update_idiom_compiles() {
        let opts = ClientOptions {
            max_iterations: 50,
            ..Default::default()
        };
        assert_eq!(opts.max_iterations, 50);
    }

    #[test]
    fn clone_is_independent() {
        let a = ClientOptions::default();
        let mut b = a.clone();
        b.max_iterations = 7;
        assert_eq!(a.max_iterations, 100);
        assert_eq!(b.max_iterations, 7);
    }
}
