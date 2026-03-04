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
///
/// The `quiet` field reflects the *resolved* value after verbose-wins-over-quiet
/// precedence: if both `--verbose` and `--quiet` are passed, quiet is disabled.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct GlobalFlags {
    /// Enable verbose output for debugging.
    pub verbose: bool,
    /// Suppress colored output (resolved: flag, env, or non-TTY).
    pub no_color: bool,
    /// Suppress all non-error output.
    ///
    /// When active, only error messages (from `anyhow::bail!`, `eprintln!("Error: ...")`)
    /// and explicitly requested output (schema export, test results, secret values) are shown.
    /// All informational, decorative, success, warning, and progress output is suppressed.
    pub quiet: bool,
}

#[allow(dead_code)]
impl GlobalFlags {
    /// Print to stderr if not in quiet mode.
    ///
    /// Use for informational, decorative, progress, and success messages.
    /// Error messages should use `eprintln!` directly (they always show).
    pub fn status(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{}", msg);
        }
    }

    /// Print to stderr with formatting if not in quiet mode.
    pub fn status_fmt(&self, args: std::fmt::Arguments<'_>) {
        if !self.quiet {
            eprintln!("{}", args);
        }
    }

    /// Print to stdout if not in quiet mode.
    ///
    /// Use for decorative stdout output. Requested output (e.g., schema export,
    /// test results to stdout) should use `println!` directly.
    pub fn print(&self, msg: &str) {
        if !self.quiet {
            println!("{}", msg);
        }
    }

    /// Print to stdout with formatting if not in quiet mode.
    pub fn print_fmt(&self, args: std::fmt::Arguments<'_>) {
        if !self.quiet {
            println!("{}", args);
        }
    }

    /// Returns true if output should be shown (not quiet, or verbose overrode quiet).
    pub fn should_output(&self) -> bool {
        !self.quiet
    }
}

/// Print to stderr if not in quiet mode. Equivalent to `eprintln!` but respects `--quiet`.
#[macro_export]
macro_rules! status {
    ($flags:expr, $($arg:tt)*) => {
        if !$flags.quiet {
            eprintln!($($arg)*);
        }
    };
}

/// Print to stdout if not in quiet mode. Equivalent to `println!` but respects `--quiet`.
#[macro_export]
macro_rules! qprintln {
    ($flags:expr, $($arg:tt)*) => {
        if !$flags.quiet {
            println!($($arg)*);
        }
    };
}
