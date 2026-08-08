//! Phase 117 (SMPL-01 / D-02) — the `v1-severance` job's BLOCKING status, proved
//! from the workflow file.
//!
//! # The rule this file encodes: `CORRECTION-116-DOC`
//!
//! Phase 116 recorded, after getting it wrong on a live gate, that **a gate's
//! blocking status is proved from the WORKFLOW FILE, not from the Makefile**. The
//! question is never "does `make quality-gate` chain it" — it is "does the `gate`
//! aggregate job actually evaluate this job's result". Checking the Makefile and
//! not the workflow step was the whole of that mistake.
//!
//! # The live counter-example, in the same file
//!
//! `.github/workflows/ci.yml` contains a job named `feature-flags`
//! (`name: Feature Flag Verification`) that is **absent from `gate.needs`**. It is
//! visible on every pull request, it goes green, and it blocks precisely nothing.
//! That job is asserted here as a NEGATIVE CONTROL: if this file's reader were
//! broken in the direction of "everything looks wired", the `feature-flags`
//! assertion would fail, so the tripwire is provably able to distinguish a
//! blocking job from a non-blocking one rather than always returning true.
//!
//! # Why THREE wirings, not one
//!
//! The `gate` job does **not** fail automatically when a new entry appears in its
//! `needs:` array. It declares `if: always()` and then reads a set of NAMED
//! environment variables — each bound to `needs.<job>.result` — and evaluates them
//! explicitly in a shell `if` chain. Adding to `needs:` alone therefore produces a
//! job that is *awaited* but whose result is *never checked*: a strictly worse
//! outcome than not adding it, because the workflow graph now looks correct.
//!
//! So three edits are required, and this file asserts all three plus their mutual
//! consistency (the env var that is BOUND must be the env var that is READ):
//!
//! 1. `v1-severance` in `gate.needs`
//! 2. an `env:` entry binding a variable to `needs.v1-severance.result`
//! 3. that same variable name evaluated inside the step's `run:` script
//!
//! # Why the workflow is PARSED, not string-matched
//!
//! Scanning YAML as text would happily "find" `v1-severance` inside a comment, or
//! inside an unrelated job, and report a wiring that does not exist. The workflow
//! is loaded with `serde_yaml` and navigated structurally. Comments are not data.
//!
//! # Why the interpreter route was REJECTED
//!
//! An earlier draft of this check shelled out to a `PyYAML`-based one-liner.
//! `PyYAML` is **not a declared dependency of this repository**. It happens to be
//! present on some GitHub-hosted runner images and on some workstations, and it is
//! absent on others. This file is reached by `make test-integration`
//! (`cargo test --test '*' --features "full"`), which `make quality-gate` runs and
//! which CI enforces — so a BLOCKING gate would have rested on an undeclared,
//! unversioned, out-of-band interpreter package. A blocking tripwire must not rest
//! on something the repository never declares.
//!
//! `serde_yaml = "0.9"` in root `[dev-dependencies]` costs ZERO new packages:
//! `crates/mcp-tester/Cargo.toml:26` already depends on the same version and
//! `serde_yaml 0.9.34` is already resolved in this workspace. Being a
//! dev-dependency it never reaches `pmcp`'s published runtime graph or its wasm
//! posture.

use serde_yaml::{Mapping, Value};

/// The workflow this file proves things about.
const WORKFLOW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/ci.yml");

/// Relative form of [`WORKFLOW`], for failure messages a reader can act on.
const WORKFLOW_REL: &str = ".github/workflows/ci.yml";

/// The job whose blocking status this file exists to prove.
const SEVERANCE_JOB: &str = "v1-severance";

/// The aggregate job named as the org ruleset's required status check.
const GATE_JOB: &str = "gate";

/// The live NON-blocking counter-example (see module docs).
const NON_BLOCKING_JOB: &str = "feature-flags";

/// Minimum number of entries `gate.needs` must have.
///
/// Non-vacuity floor. A reader that silently produced an empty `needs` list would
/// make `the_feature_flags_job_is_still_not_in_gate_needs` pass for the wrong
/// reason. If this fires, FIX THE READER or restore the workflow — never lower the
/// floor.
const MINIMUM_GATE_NEEDS: usize = 6;

/// Minimum number of jobs the workflow must declare.
///
/// Non-vacuity floor, same contract as [`MINIMUM_GATE_NEEDS`]: a parse that
/// produced an empty job map would make every lookup below fail for a reason
/// unrelated to the wiring. Fix the reader, never lower the floor.
const MINIMUM_JOBS: usize = 8;

// ===========================================================================
// Reader
// ===========================================================================

/// The whole workflow, parsed structurally — ONCE.
///
/// Every reader below funnels through here, and `job()` is called from five
/// tests, so a per-call read would re-parse the workflow once per lookup. The
/// amplification is invisible at the call sites, which is exactly why it is
/// closed here rather than left to grow.
static WORKFLOW_DOC: std::sync::LazyLock<Value> = std::sync::LazyLock::new(parse_workflow);

/// Read and parse the workflow. Called exactly once, through [`WORKFLOW_DOC`].
fn parse_workflow() -> Value {
    let text = std::fs::read_to_string(WORKFLOW).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: cannot read {WORKFLOW_REL}: {e}\n\
             WHAT TO DO: this test proves the severance gate blocks merge. If the workflow moved, \
             update WORKFLOW here; do not delete the test."
        )
    });
    serde_yaml::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} is not valid YAML: {e}\n\
             WHAT TO DO: fix the workflow. An unparseable workflow does not run at all, so every \
             required check silently stops gating."
        )
    })
}

/// The `jobs:` mapping.
fn jobs() -> &'static Mapping {
    let jobs = WORKFLOW_DOC.get("jobs").unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} has no top-level `jobs:` key.\n\
             WHAT TO DO: fix the reader or the workflow; do not weaken the assertion."
        )
    });
    let mapping = jobs.as_mapping().unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: `jobs:` in {WORKFLOW_REL} is not a mapping.\n\
             WHAT TO DO: fix the reader, not the assertion."
        )
    });
    assert!(
        mapping.len() >= MINIMUM_JOBS,
        "FAILURE MODE: parsed only {} job(s) from {WORKFLOW_REL}, below the {MINIMUM_JOBS} floor. \
         A reader that sees almost nothing makes every wiring check below meaningless.\n\
         WHAT TO DO: fix the reader; never lower the floor.",
        mapping.len()
    );
    mapping
}

/// One job by name, or `None` if the workflow does not declare it.
fn job(name: &str) -> Option<&'static Value> {
    jobs().get(name)
}

/// A job's `steps:` sequence, panicking with an actionable message when either
/// the job or its `steps:` key is absent.
///
/// Both [`run_scripts`] and [`gate_eval_step`] navigate job → `steps:`, so the
/// lookup and its two panics live here once. (The previous `gate_eval_step`
/// copy justified its `expect` with "gate job presence is asserted by
/// `gate_needs()`" — which does not hold: `severance_result_is_bound_and_evaluated`
/// calls it with no prior `gate_needs()`.)
fn steps_of(job_name: &str) -> &'static [Value] {
    let job = job(job_name).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no job named `{job_name}`.\n\
             WHAT TO DO: restore the job; a missing job cannot gate anything."
        )
    });
    job.get("steps")
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: job `{job_name}` in {WORKFLOW_REL} has no `steps:` sequence.\n\
                 WHAT TO DO: fix the reader or the workflow."
            )
        })
}

/// Every `run:` script in a job's `steps:`, concatenated.
fn run_scripts(job_name: &str) -> String {
    let mut collected = String::new();
    for step in steps_of(job_name) {
        if let Some(run) = step.get("run").and_then(Value::as_str) {
            collected.push_str(run);
            collected.push('\n');
        }
    }
    assert!(
        !collected.is_empty(),
        "FAILURE MODE: job `{job_name}` in {WORKFLOW_REL} runs no commands at all — the \
         `--all-features`/`--all-targets` absence checks below would pass over an empty string.\n\
         WHAT TO DO: fix the reader or restore the build step; never relax the check."
    );
    collected
}

/// `gate.needs`, as a list of job names — a PURE structural read with no floor.
///
/// Used by the assertions that would FAIL on a vacuous read anyway (asserting a
/// name IS present in an empty list fails safely, so a floor there only replaces a
/// precise diagnosis with a misleading one). Assertions that would PASS on a
/// vacuous read — the `!contains` negative control — go through [`gate_needs`],
/// which adds the floor.
fn gate_needs_raw() -> Vec<String> {
    let gate = job(GATE_JOB).unwrap_or_else(|| {
        panic!(
            "FAILURE MODE: {WORKFLOW_REL} declares no `{GATE_JOB}` job — the org ruleset's \
             required status check does not exist.\n\
             WHAT TO DO: restore it; nothing blocks merge without it."
        )
    });
    let needs: Vec<String> = gate
        .get("needs")
        .and_then(Value::as_sequence)
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: `{GATE_JOB}.needs` in {WORKFLOW_REL} is missing or is not a \
                 sequence.\n\
                 WHAT TO DO: fix the reader or the workflow."
            )
        })
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    needs
}

/// `gate.needs` with the non-vacuity floor applied.
fn gate_needs() -> Vec<String> {
    let needs = gate_needs_raw();
    assert!(
        needs.len() >= MINIMUM_GATE_NEEDS,
        "FAILURE MODE: parsed {} entr(ies) from `{GATE_JOB}.needs`, below the \
         {MINIMUM_GATE_NEEDS} floor. TWO causes are possible and they have opposite remedies: \
         either the reader above is broken (in which case the `!contains` negative control would \
         pass vacuously), or a `needs:` entry was genuinely REMOVED and a required check stopped \
         gating merge.\n\
         WHAT TO DO: read `{WORKFLOW_REL}`'s `{GATE_JOB}.needs` and decide which. If an entry was \
         removed, restore it. If the reader is broken, fix the reader. NEVER lower the floor.\n\
         needs read: {needs:?}",
        needs.len()
    );
    needs
}

/// The `gate`'s single evaluation step: its `env:` bindings and its `run:` script.
fn gate_eval_step() -> (Mapping, String) {
    for step in steps_of(GATE_JOB) {
        let (Some(env), Some(run)) = (
            step.get("env").and_then(Value::as_mapping),
            step.get("run").and_then(Value::as_str),
        ) else {
            continue;
        };
        return (env.clone(), run.to_owned());
    }
    panic!(
        "FAILURE MODE: no step in `{GATE_JOB}` carries BOTH an `env:` block and a `run:` script, \
         so no job result is evaluated anywhere.\n\
         WHAT TO DO: restore the evaluation step; a `{GATE_JOB}` that evaluates nothing reports \
         success unconditionally."
    );
}

// ===========================================================================
// 1. The job exists and is fenced
// ===========================================================================

#[test]
fn severance_job_exists() {
    let script = run_scripts(SEVERANCE_JOB);

    // All FOUR fences the message below enumerates. Keeping one of them in a
    // separate hand-written assertion made the list labelled "four fences"
    // check three, and gave a fifth fence two plausible homes.
    for required in [
        "--features full-v2",
        "--no-default-features",
        "-p pmcp",
        r#"RUSTFLAGS="-D warnings""#,
    ] {
        assert!(
            script.contains(required),
            "FAILURE MODE: the `{SEVERANCE_JOB}` build command in {WORKFLOW_REL} is missing \
             `{required}`. Each of the four fences closes a specific false green: `-p pmcp` stops \
             workspace feature unification turning `v1-compat` back on, `--no-default-features` \
             stops it arriving via `default`, `--features full-v2` stops the build \"proving\" \
             severance by never compiling the transport, and `-D warnings` stops a stranded \
             helper's `dead_code` lint from passing green.\n\
             WHAT TO DO: restore the fence. The rationale block above the job in {WORKFLOW_REL} \
             explains why it is not redundant.\n\
             Command read: {script}"
        );
    }

    for forbidden in ["--all-features", "--all-targets"] {
        assert!(
            !script.contains(forbidden),
            "FAILURE MODE: the `{SEVERANCE_JOB}` build command in {WORKFLOW_REL} contains \
             `{forbidden}`. `--all-features` can NEVER prove severance — cargo features are \
             additive, so it enables `full-v2` AND `v1-compat` at once. `--all-targets` drags \
             tests and examples into a build that is deliberately lib-only, for zero additional \
             proof about the library consumers link.\n\
             WHAT TO DO: remove it. If the intent was broader coverage, add a SEPARATE job — do \
             not void this one's proof.\n\
             Command read: {script}"
        );
    }
}

// ===========================================================================
// 2. Wiring one: the job is awaited
// ===========================================================================

#[test]
fn severance_job_is_in_gate_needs() {
    // RAW read on purpose: this is a `contains` assertion, so a vacuous read fails
    // safely. Routing it through the floored reader would replace this test's
    // precise "not in gate.needs" diagnosis with a generic "fix the reader" one —
    // exactly the wrong instruction for the defect it exists to catch.
    let needs = gate_needs_raw();
    assert!(
        needs.iter().any(|n| n == SEVERANCE_JOB),
        "FAILURE MODE: `{SEVERANCE_JOB}` is not listed in `{GATE_JOB}.needs` in {WORKFLOW_REL}. \
         `{GATE_JOB}` is the org ruleset's required status check, so a job outside its `needs:` \
         array is visible, green-looking and completely non-blocking — exactly the state the \
         `{NON_BLOCKING_JOB}` job is in today.\n\
         WHAT TO DO: add `{SEVERANCE_JOB}` to `{GATE_JOB}.needs`, AND check the other two wirings \
         (the `env:` binding and the `if` chain) — all three are required.\n\
         needs read: {needs:?}"
    );
}

// ===========================================================================
// 3. Wirings two and three: the result is bound AND read
// ===========================================================================

#[test]
fn severance_result_is_bound_and_evaluated() {
    let (env, run) = gate_eval_step();
    let expected_expression = format!("needs.{SEVERANCE_JOB}.result");

    let bound_var = env
        .iter()
        .find_map(|(name, expr)| {
            let expr = expr.as_str()?;
            expr.contains(&expected_expression)
                .then(|| name.as_str().map(str::to_owned))?
        })
        .unwrap_or_else(|| {
            panic!(
                "FAILURE MODE: no variable in the `{GATE_JOB}` evaluation step's `env:` block is \
                 bound to `{expected_expression}`. A `needs:` entry alone produces a job that is \
                 AWAITED but whose result is NEVER CHECKED — `{GATE_JOB}` declares `if: always()` \
                 and only ever compares the named variables it reads, so an unbound result can \
                 never turn it red.\n\
                 WHAT TO DO: add `SEVERANCE_RESULT: ${{{{ {expected_expression} }}}}` to the \
                 `env:` block AND evaluate it in the `if` chain. Binding without evaluating is the \
                 same defect one step later.\n\
                 env read: {env:?}"
            )
        });

    assert!(
        run.contains(&bound_var),
        "FAILURE MODE: `{bound_var}` is bound to `{expected_expression}` in the `{GATE_JOB}` \
         step's `env:` block but never appears in that step's `run:` script. The result is \
         AWAITED but NEVER CHECKED: `{GATE_JOB}` runs its shell `if` chain over the variables it \
         actually reads, so a failed `{SEVERANCE_JOB}` would leave the required check green.\n\
         WHAT TO DO: add `[[ \"${bound_var}\" != \"success\" ]] || \\` to the `if` chain and name \
         `{SEVERANCE_JOB}=${bound_var}` in the failure echo, so the message identifies the cause.\n\
         run read: {run}"
    );

    assert!(
        run.contains(SEVERANCE_JOB),
        "FAILURE MODE: the `{GATE_JOB}` step's `run:` script never mentions `{SEVERANCE_JOB}`, so \
         when the gate fails its message will not name this cause and the next reader will hunt \
         through five other jobs.\n\
         WHAT TO DO: add `{SEVERANCE_JOB}=${bound_var}` to the `Required checks failed: ...` \
         echo.\n\
         run read: {run}"
    );
}

// ===========================================================================
// 4. The live negative control
// ===========================================================================

#[test]
fn the_feature_flags_job_is_still_not_in_gate_needs() {
    assert!(
        job(NON_BLOCKING_JOB).is_some(),
        "FAILURE MODE: {WORKFLOW_REL} no longer declares a `{NON_BLOCKING_JOB}` job, so this \
         file's live negative control is gone and the other tests here can no longer be shown to \
         distinguish a blocking job from a non-blocking one.\n\
         WHAT TO DO: if `{NON_BLOCKING_JOB}` was deliberately removed or promoted into \
         `{GATE_JOB}.needs`, pick a different non-blocking job as the control — do not delete the \
         control."
    );

    let needs = gate_needs();
    assert!(
        !needs.iter().any(|n| n == NON_BLOCKING_JOB),
        "FAILURE MODE: `{NON_BLOCKING_JOB}` now appears in `{GATE_JOB}.needs`. That may well be an \
         improvement, but it destroys this file's negative control: with every job wired, a reader \
         that answered \"yes, it's wired\" to everything would pass all the tests here.\n\
         WHAT TO DO: keep the promotion if it was intended, and re-point NON_BLOCKING_JOB at \
         another job that is genuinely absent from `{GATE_JOB}.needs`.\n\
         needs read: {needs:?}"
    );
}

// ===========================================================================
// 5. The parse itself is not vacuous
// ===========================================================================

#[test]
fn the_workflow_parse_is_not_vacuous() {
    let all_jobs = jobs();
    assert!(
        all_jobs.len() >= MINIMUM_JOBS,
        "FAILURE MODE: {WORKFLOW_REL} parsed to {} job(s), below the {MINIMUM_JOBS} floor.\n\
         WHAT TO DO: fix the reader; never lower the floor.",
        all_jobs.len()
    );

    let needs = gate_needs();
    assert!(
        needs.len() >= MINIMUM_GATE_NEEDS,
        "FAILURE MODE: `{GATE_JOB}.needs` parsed to {} entr(ies), below the \
         {MINIMUM_GATE_NEEDS} floor.\n\
         WHAT TO DO: fix the reader; never lower the floor.",
        needs.len()
    );

    let (env, run) = gate_eval_step();
    // Measured against the ACTUAL `needs` length, not the floor constant: this is
    // the general form of the wiring invariant (every awaited job is also bound),
    // and pinning it to a constant would make it misfire whenever `needs` legally
    // changes size.
    assert!(
        env.len() >= needs.len(),
        "FAILURE MODE: the `{GATE_JOB}` evaluation step binds {} env var(s) for {} awaited \
         job(s). At least one `needs:` entry is awaited without being bound, which is the \
         AWAITED-but-NEVER-CHECKED defect.\n\
         WHAT TO DO: bind every entry in `{GATE_JOB}.needs` and evaluate every binding.\n\
         needs read: {needs:?}\n\
         env read: {env:?}",
        env.len(),
        needs.len()
    );
    assert!(
        !run.trim().is_empty(),
        "FAILURE MODE: the `{GATE_JOB}` evaluation step's `run:` script is empty, so it evaluates \
         nothing and reports success unconditionally.\n\
         WHAT TO DO: restore the `if` chain."
    );
}
