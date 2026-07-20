//! `cargo pmcp package import` — submit + poll an async dry-run pre-flight
//! import job against the pmcp.run platform (D-02/D-04). Dry-run is the ONLY
//! supported mode this phase — there is no `--execute` flag; the client
//! always sends `dryRun: true` and the server rejects anything else (real
//! execution is Phase 172's concern). Remote, platform-side: admit / pull &
//! verify / resolve / pre-flight all run in the platform's import worker,
//! never in this CLI (D-10).

use std::io::{self, Write as _};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::Args;
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

use super::parse_reference;

/// Sleep between import-status polls (mirrors `capture.rs`'s `POLL_INTERVAL`).
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Bounded max total wait for an import job to reach a terminal status —
/// mirrors `capture.rs`'s `MAX_POLL_WAIT` (the import Lambda shares the same
/// 15-min timeout class as the capture Lambda, plus queue/retry slack).
const MAX_POLL_WAIT: Duration = Duration::from_secs(20 * 60);

/// Max consecutive TRANSPORT-level failures tolerated while polling before
/// giving up (mirrors `capture.rs`'s `MAX_TRANSIENT_RETRIES`). A `"failed"`
/// job STATUS from a successful GraphQL call is terminal, not transient —
/// this only guards network/GraphQL-request errors.
const MAX_TRANSIENT_RETRIES: u32 = 5;

/// Arguments for `cargo pmcp package import`.
///
/// Dry-run pre-flight is the ONLY supported mode this phase (D-02) — there is
/// no `--execute` flag and no way to request real execution; the CLI always
/// sends `dryRun: true` and the server rejects any other value. Real
/// execution modes are Phase 172's concern.
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Workflow reference in `name@X.Y.Z` form (e.g. `day-trip-planner-team@1.0.0`).
    #[arg(
        value_name = "WORKFLOW@VERSION",
        long_help = "Workflow reference in `name@X.Y.Z` form. Submits an async \
                      dry-run pre-flight import job (admit -> pull & verify -> \
                      resolve -> pre-flight) and polls it to a terminal status, \
                      then renders the pre-flight report. Dry-run is the ONLY \
                      mode this phase — there is no way to request real \
                      execution (deferred to Phase 172)."
    )]
    pub reference: String,

    /// Emit stable JSON instead of the human-readable pre-flight tree.
    #[arg(long)]
    pub json: bool,
}

/// Submit a dry-run import job and poll it to a terminal status, rendering
/// the pre-flight report.
pub async fn execute(args: ImportArgs, global_flags: &GlobalFlags) -> Result<()> {
    // Progress/status chatter only in human mode — `--json` must yield a
    // clean stdout stream for scripting, mirroring `show.rs`'s
    // json-takes-priority-over-quiet precedent.
    let human_output = global_flags.should_output() && !args.json;

    // Validate the reference shape up front (fail fast, same UX as `show`) —
    // the parsed halves aren't needed beyond validation since the mutation
    // takes the whole reference as a single string.
    parse_reference(&args.reference)?;

    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;
    let access_token = credentials.access_token;

    if human_output {
        println!("Submitting dry-run import: reference={}", args.reference);
    }

    let submitted = graphql::submit_package_import(&access_token, &args.reference)
        .await
        .context("Failed to submit package import")?;

    let status = poll_import_status(&access_token, &submitted.import_id, human_output).await?;

    // ALL of these are terminal — never loop past a terminal status (Codex
    // #13). `failed` surfaces the structured `errorCode`, NEVER a parsed
    // `errorMessage` (T-171-26).
    match status.status.as_str() {
        "completed_dry_run" => {
            render_preflight_report(&status, args.json, human_output)?;
            Ok(())
        },
        "blocked" => {
            render_preflight_report(&status, args.json, human_output)?;
            bail!("Pre-flight blocked — see the disposition table above for the blocking row(s)");
        },
        "awaiting_bind" => {
            render_preflight_report(&status, args.json, human_output)?;
            if human_output {
                println!(
                    "\n{}",
                    "Next step: bind the unbound slot(s) listed above (setPackageBinding), then re-run."
                        .bright_black()
                );
            }
            Ok(())
        },
        "failed" => bail_with_structured_error_code(&status),
        other => bail!("Unexpected non-terminal import status after timeout: {other}"),
    }
}

/// Bail on a `"failed"` terminal status, surfacing the STRUCTURED `errorCode`
/// — NEVER the freeform `errorMessage` (T-171-26).
fn bail_with_structured_error_code(status: &graphql::ImportStatus) -> Result<()> {
    match status.error_code.as_deref() {
        Some(code) => bail!("Import failed (errorCode: {code})"),
        None => bail!("Import failed with no structured errorCode — check pmcp.run service logs"),
    }
}

/// Poll `getImportStatus` until a terminal status is reached. ALL of
/// `completed_dry_run` / `blocked` / `awaiting_bind` / `failed` are terminal
/// (Codex #13) — the loop returns immediately on any of them and never waits
/// out `MAX_POLL_WAIT` for a status that has already resolved.
async fn poll_import_status(
    access_token: &str,
    import_id: &str,
    human_output: bool,
) -> Result<graphql::ImportStatus> {
    if human_output {
        println!("⏳ Waiting for import pre-flight to complete...");
    }

    let start = Instant::now();
    let mut transient_failures = 0u32;
    let mut dots = 0u32;
    // Statuses we've already warned about — unknown (likely newer-platform)
    // statuses are treated as in-progress, warned once each, never fatal.
    let mut warned_statuses: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        if start.elapsed() > MAX_POLL_WAIT {
            bail!(
                "Timed out waiting for import {import_id} to reach a terminal status after {}s",
                MAX_POLL_WAIT.as_secs()
            );
        }

        let status = match graphql::get_import_status(access_token, import_id).await {
            Ok(status) => {
                transient_failures = 0;
                status
            },
            Err(err) => {
                transient_failures += 1;
                if transient_failures > MAX_TRANSIENT_RETRIES {
                    return Err(err).context("Exceeded max retries polling import status");
                }
                if human_output {
                    eprintln!(
                        "   transient error polling import status ({transient_failures}/{MAX_TRANSIENT_RETRIES}): {err}"
                    );
                }
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            },
        };

        match status.status.as_str() {
            // ALL FOUR are terminal (Codex #13) — return immediately, never
            // fall through to the in-progress branch below.
            "completed_dry_run" | "blocked" | "awaiting_bind" | "failed" => {
                if human_output && dots > 0 {
                    println!();
                }
                return Ok(status);
            },
            "not_found" => bail!("Import job {import_id} not found"),
            in_progress => {
                // Terminal detection stays exact — any other status is
                // treated as in-progress so an older CLI keeps working when
                // the platform adds intermediate statuses. Warn once per
                // unrecognized status.
                if !matches!(
                    in_progress,
                    "queued" | "admitting" | "pulling" | "resolving" | "preflighting" | "binding"
                ) && warned_statuses.insert(in_progress.to_string())
                {
                    if human_output && dots > 0 {
                        println!();
                        dots = 0;
                    }
                    eprintln!(
                        "   warning: unrecognized import status '{in_progress}' — treating as in-progress"
                    );
                }
                if human_output {
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
        }
    }
}

// ============================================================================
// Pre-flight report rendering (171-05's persisted JSON schema — see
// 171-05-SUMMARY.md "Pre-flight Report JSON Schema"). These types are a
// CLI-side rendering DTO only — the canonical `PreflightReport` type lives in
// the import Lambda's `model.rs` (a separate crate this CLI does not depend
// on), so the field shape is mirrored here deliberately rather than shared.
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
struct Disposition {
    #[serde(rename = "componentName")]
    component_name: String,
    disposition: String,
    blocking: bool,
    diagnostics: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UnboundSlot {
    #[serde(rename = "componentName")]
    component_name: String,
    #[serde(rename = "slotKind")]
    slot_kind: String,
    #[serde(rename = "slotName")]
    slot_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Deviation {
    #[serde(rename = "componentName")]
    component_name: String,
    #[serde(rename = "slotName")]
    slot_name: String,
    #[serde(rename = "testedValue")]
    tested_value: String,
    #[serde(rename = "proposedValue")]
    proposed_value: String,
    acknowledged: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct AllowlistResult {
    #[serde(rename = "componentName")]
    component_name: String,
    violation: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ImpactEntry {
    #[serde(rename = "componentName")]
    component_name: String,
    #[serde(rename = "referencingWorkflows")]
    referencing_workflows: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PreflightReport {
    dispositions: Vec<Disposition>,
    #[serde(rename = "unboundSlots")]
    unbound_slots: Vec<UnboundSlot>,
    deviations: Vec<Deviation>,
    #[serde(rename = "allowlistResults")]
    allowlist_results: Vec<AllowlistResult>,
    #[serde(rename = "impactList")]
    impact_list: Vec<ImpactEntry>,
    blocked: bool,
}

/// Parse and render the pre-flight report carried on a terminal
/// `ImportStatus` — human tree by default (D-D), stable `--json` on request
/// (json takes priority over quiet, mirroring `show.rs`). Never re-sorts
/// anything: the Lambda's own `to_canonical_json()` already guarantees
/// stable ordering.
fn render_preflight_report(
    status: &graphql::ImportStatus,
    json: bool,
    human_output: bool,
) -> Result<()> {
    let Some(raw) = status.preflight_report_json.as_deref() else {
        if human_output {
            eprintln!("(no pre-flight report available for this status)");
        }
        return Ok(());
    };

    let report: PreflightReport = serde_json::from_str(raw)
        .context("Failed to parse pre-flight report JSON returned by the platform")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if human_output {
        render_tree(&report, &status.status);
    }
    Ok(())
}

/// Render the default human-readable pre-flight tree.
fn render_tree(report: &PreflightReport, job_status: &str) {
    println!(
        "\n{} {}",
        "Import pre-flight:".bright_cyan().bold(),
        job_status
    );

    println!("\n  {}", "Dispositions:".bright_black());
    if report.dispositions.is_empty() {
        println!("    (none)");
    }
    for row in &report.dispositions {
        render_disposition(row);
    }

    if !report.unbound_slots.is_empty() {
        println!("\n  {}", "Unbound slots:".bright_black());
        for slot in &report.unbound_slots {
            println!(
                "    - [{}] {} :: {}",
                slot.slot_kind, slot.component_name, slot.slot_name
            );
        }
    }

    if !report.deviations.is_empty() {
        println!("\n  {}", "Deviations:".bright_black());
        for deviation in &report.deviations {
            let ack = if deviation.acknowledged {
                "acknowledged".green()
            } else {
                "UNACKNOWLEDGED".yellow().bold()
            };
            println!(
                "    - {} :: {}  tested={} proposed={}  [{}]",
                deviation.component_name,
                deviation.slot_name,
                deviation.tested_value,
                deviation.proposed_value,
                ack
            );
        }
    }

    let violations: Vec<&AllowlistResult> = report
        .allowlist_results
        .iter()
        .filter(|r| r.violation.is_some())
        .collect();
    if !violations.is_empty() {
        println!("\n  {}", "Allowlist violations:".red().bold());
        for result in violations {
            println!(
                "    - {}: {}",
                result.component_name,
                result.violation.as_deref().unwrap_or("")
            );
        }
    }

    if !report.impact_list.is_empty() {
        println!("\n  {}", "Impact (upgrade affects):".bright_black());
        for entry in &report.impact_list {
            println!(
                "    - {} <- {}",
                entry.component_name,
                entry.referencing_workflows.join(", ")
            );
        }
    }

    println!();
    if report.blocked {
        println!("  {}", "BLOCKED".red().bold());
    } else {
        println!("  {}", "Not blocked".green());
    }
    println!();
}

/// One `Dispositions:` line, color-coded by disposition/blocking state.
fn render_disposition(row: &Disposition) {
    let label = if row.blocking {
        row.disposition.red().bold()
    } else {
        match row.disposition.as_str() {
            "reuse" => row.disposition.green(),
            "install" => row.disposition.bright_blue(),
            "upgrade" => row.disposition.yellow(),
            _ => row.disposition.normal(),
        }
    };
    match &row.diagnostics {
        Some(diag) => println!(
            "    - [{label}] {}  {}",
            row.component_name,
            diag.bright_black()
        ),
        None => println!("    - [{label}] {}", row.component_name),
    }
}
