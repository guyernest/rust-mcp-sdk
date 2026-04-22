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
/// From downstream crates (external — `#[non_exhaustive]` forbids the struct
/// literal, so use [`ClientOptions::default`] + a setter or direct assignment):
///
/// ```rust,no_run
/// use pmcp::ClientOptions;
///
/// let opts = ClientOptions::default().with_max_iterations(50);
/// assert_eq!(opts.max_iterations, 50);
///
/// // Equivalent mutable form:
/// let mut opts = ClientOptions::default();
/// opts.max_iterations = 50;
/// assert_eq!(opts.max_iterations, 50);
/// ```
///
/// From inside the `pmcp` crate (or any crate-internal consumer) the
/// field-update idiom also compiles — `ClientOptions { max_iterations: 50,
/// ..Default::default() }` — but external crates must use the two forms
/// above.
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

impl ClientOptions {
    /// Builder-style setter for [`Self::max_iterations`].
    ///
    /// Provided so downstream crates can configure a non-default
    /// `max_iterations` without running into the `#[non_exhaustive]`
    /// struct-literal restriction.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::ClientOptions;
    /// let opts = ClientOptions::default().with_max_iterations(25);
    /// assert_eq!(opts.max_iterations, 25);
    /// ```
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }
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
