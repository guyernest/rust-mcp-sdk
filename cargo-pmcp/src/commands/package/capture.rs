//! `cargo pmcp package capture` — submit + poll an async workflow capture job
//! against the pmcp.run platform (D-A/D-B). Remote, platform-side: the
//! dependency-graph walk itself runs in the platform's capture worker, never
//! in this CLI (D-10).

use std::io::{self, IsTerminal, Write as _};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// `rootComponentType` — v1 only supports capturing a team's workflow graph.
const ROOT_COMPONENT_TYPE_TEAM: &str = "team";

/// Sleep between capture-status polls (matches `poll_deployment_status`).
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Bounded max total wait for a capture job to reach a terminal status,
/// matching the platform capture Lambda's own ceiling.
const MAX_POLL_WAIT: Duration = Duration::from_secs(15 * 60);

/// Max consecutive TRANSPORT-level failures tolerated while polling before
/// giving up. A `"failed"` job STATUS from a successful GraphQL call is
/// terminal (not transient) and is handled separately — this only guards
/// network/GraphQL-request errors.
const MAX_TRANSIENT_RETRIES: u32 = 5;

const BUMP_LEVELS: [&str; 3] = ["major", "minor", "patch"];

/// Arguments for `cargo pmcp package capture`.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// AgentTeam ID (UUID) — the team's id, not its display name.
    ///
    /// v1 requires the exact AgentTeam id: `submitPackageCapture` performs a
    /// DynamoDB `GetItem` by primary key against the `AgentTeam` table, so a
    /// display name will not resolve. A server-side name -> id lookup is a
    /// documented deferral, not supported in v1.
    #[arg(
        value_name = "TEAM_ID",
        long_help = "AgentTeam ID (UUID) — the team's id, not its display name. v1 requires \
                      the exact AgentTeam id (submitPackageCapture does a GetItem by primary \
                      key); a name-to-id lookup is a documented deferral, not supported here."
    )]
    pub team_id: String,

    /// Semver version to capture the workflow package as (e.g. 1.2.3).
    #[arg(long, value_name = "X.Y.Z")]
    pub version: String,

    /// Bump level applied to every component whose deployed bytes changed
    /// since the last capture (major|minor|patch — the single level applies
    /// uniformly across all divergent components; there is no per-component
    /// override). Only consulted when the platform reports
    /// `errorCode=BUMP_REQUIRED`; supply it up front to skip the interactive
    /// TTY prompt (required in non-interactive/CI contexts, where an
    /// unresolved bump-required job fails loud rather than hanging).
    #[arg(long, value_parser = ["major", "minor", "patch"])]
    pub bump: Option<String>,
}

/// What to do after a `"failed"` terminal status is inspected.
enum FailureAction {
    /// Re-submit the capture with this bump level applied uniformly.
    Resubmit(String),
}

/// Submit a capture job and poll it to a terminal status, resolving
/// `--bump` interactively over a TTY (or failing loud non-interactively).
pub async fn execute(args: CaptureArgs, global_flags: &GlobalFlags) -> Result<()> {
    let output = global_flags.should_output();
    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;
    let access_token = credentials.access_token;

    let mut bump = args.bump.clone();

    loop {
        let submitted = submit(&access_token, &args, bump.as_deref(), output).await?;
        let status = poll_capture_status(&access_token, &submitted.capture_id, output).await?;

        match status.status.as_str() {
            "completed" => {
                report_success(&status, output);
                return Ok(());
            },
            "failed" => match handle_capture_failure(&status)? {
                FailureAction::Resubmit(level) => {
                    bump = Some(level);
                    continue;
                },
            },
            other => bail!("Unexpected terminal capture status: {other}"),
        }
    }
}

/// Submit the capture job, printing a short status line when output is
/// enabled.
async fn submit(
    access_token: &str,
    args: &CaptureArgs,
    bump: Option<&str>,
    output: bool,
) -> Result<graphql::CaptureInfo> {
    if output {
        println!(
            "Submitting capture: team={} version={}{}",
            args.team_id,
            args.version,
            bump.map(|b| format!(" bump={b}")).unwrap_or_default()
        );
    }

    graphql::submit_package_capture(
        access_token,
        ROOT_COMPONENT_TYPE_TEAM,
        &args.team_id,
        &args.version,
        bump,
    )
    .await
    .context("Failed to submit package capture")
}

/// Print the terminal success summary.
fn report_success(status: &graphql::CaptureStatus, output: bool) {
    if !output {
        return;
    }
    println!("{}", "✅ Capture complete".green().bold());
    if let Some(digest) = &status.manifest_digest {
        println!("   Manifest digest: {digest}");
    }
}

/// Inspect a `"failed"` terminal status and decide what to do next.
///
/// Switches on the STRUCTURED `error_code` (D-B) — NEVER parses `message` to
/// detect `BUMP_REQUIRED` or any other condition. On `BUMP_REQUIRED`: prompts
/// once over an interactive TTY and returns [`FailureAction::Resubmit`];
/// non-interactively, bails listing every divergent component and pointing
/// at `--bump`. Any other error code (or none) bails with the message.
fn handle_capture_failure(status: &graphql::CaptureStatus) -> Result<FailureAction> {
    match status.error_code.as_deref() {
        Some("BUMP_REQUIRED") => {
            let divergent = status.divergent_components.clone().unwrap_or_default();
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                let level = prompt_bump_level(&divergent)?;
                Ok(FailureAction::Resubmit(level))
            } else {
                bail!(
                    "Capture failed: bump required for {} divergent component(s): {}. \
                     Re-run with --bump <major|minor|patch> (applies uniformly to all).",
                    divergent.len(),
                    divergent.join(", ")
                );
            }
        },
        Some(code) => bail!(
            "Capture failed ({code}): {}",
            status.message.as_deref().unwrap_or("no message")
        ),
        None => bail!(
            "Capture failed: {}",
            status.message.as_deref().unwrap_or("unknown error")
        ),
    }
}

/// Prompt (once) for a bump level on an interactive TTY, re-prompting only
/// on invalid input.
fn prompt_bump_level(divergent: &[String]) -> Result<String> {
    eprintln!("The following component(s) changed and need a version bump:");
    for component in divergent {
        eprintln!("  - {component}");
    }
    loop {
        eprint!("Choose a bump level ({}): ", BUMP_LEVELS.join("/"));
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim().to_lowercase();
        if BUMP_LEVELS.contains(&choice.as_str()) {
            return Ok(choice);
        }
        eprintln!(
            "Invalid choice '{choice}' — must be one of: {}",
            BUMP_LEVELS.join(", ")
        );
    }
}

/// Poll `getPackageCaptureStatus` until a terminal status (`completed` /
/// `failed`), bounded by [`MAX_POLL_WAIT`] and tolerating up to
/// [`MAX_TRANSIENT_RETRIES`] consecutive transport-level failures.
async fn poll_capture_status(
    access_token: &str,
    capture_id: &str,
    output: bool,
) -> Result<graphql::CaptureStatus> {
    if output {
        println!("⏳ Waiting for capture to complete...");
    }

    let start = Instant::now();
    let mut transient_failures = 0u32;
    let mut dots = 0u32;

    loop {
        if start.elapsed() > MAX_POLL_WAIT {
            bail!(
                "Timed out waiting for capture {capture_id} to complete after {}s",
                MAX_POLL_WAIT.as_secs()
            );
        }

        let status = match graphql::get_package_capture_status(access_token, capture_id).await {
            Ok(status) => {
                transient_failures = 0;
                status
            },
            Err(err) => {
                transient_failures += 1;
                if transient_failures > MAX_TRANSIENT_RETRIES {
                    return Err(err).context("Exceeded max retries polling capture status");
                }
                if output {
                    eprintln!(
                        "   transient error polling capture status ({transient_failures}/{MAX_TRANSIENT_RETRIES}): {err}"
                    );
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            },
        };

        match status.status.as_str() {
            "queued" | "walking" | "extracting" | "publishing" => {
                if output {
                    print!(".");
                    dots += 1;
                    if dots >= 60 {
                        println!();
                        dots = 0;
                    }
                    io::stdout().flush()?;
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            },
            "completed" | "failed" => {
                if output && dots > 0 {
                    println!();
                }
                return Ok(status);
            },
            "not_found" => bail!("Capture job {capture_id} not found"),
            other => bail!("Unknown capture status: {other}"),
        }
    }
}
