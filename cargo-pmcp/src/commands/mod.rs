pub mod add;
pub mod app;
pub mod connect;
pub mod deploy;
pub mod dev;
pub mod landing;
pub mod loadtest;
pub mod new;
pub mod preview;
pub mod schema;
pub mod secret;
pub mod test;
pub mod validate;

/// Global CLI flags shared across all commands.
///
/// Constructed in `main()` from top-level CLI args and passed to every
/// command handler. The `no_color` field reflects the *resolved* value
/// (CLI flag OR `NO_COLOR` env OR non-TTY), so downstream code can use
/// it directly without re-checking the environment.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct GlobalFlags {
    /// Enable verbose output for debugging.
    pub verbose: bool,
    /// Suppress colored output (resolved: flag, env, or non-TTY).
    pub no_color: bool,
    /// Suppress all non-error output (accepted but not yet active).
    pub quiet: bool,
}
