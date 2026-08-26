//! Offline blocking contract test for the pmcp.run package-portability
//! contract (Phase 123 plan 02, PKGX-02).
//!
//! Validates that the CLI's `getPackageArtifact` operation
//! (`GET_PACKAGE_ARTIFACT_QUERY` in
//! `src/deployment/targets/pmcp_run/graphql_contract.rs`) has not drifted from
//! the vendored SDL at `contracts/pmcp-run/portability-v1.graphql`.
//!
//! This is a pure offline/static check: no network access, no pmcp.run
//! credentials. (The fifth test in this file CAN reach the network — it is
//! `#[ignore]`d behind a double env gate; see its own docs.)
//!
//! It is executed by the `test-cargo-pmcp-integration` Makefile target, which
//! is chained into `test-all` and therefore into `make quality-gate`. That
//! target does not merely run this file — it asserts a NONZERO passed count for
//! this binary BY NAME, via `scripts/named-test-binary-count.awk`. That
//! per-binary assertion is what makes the word "blocking" a measured property
//! rather than a claim: an `#[ignore]` sweep or a `#[cfg]` gate turning false
//! here reports `0 passed` and fails the build, even though the summed total
//! across the target's other binaries would stay comfortably nonzero. The
//! append registering this binary in BOTH of that target's lists was made in
//! the same commit that created this file — see the Makefile's own comment
//! block for why a name that lands BEFORE its binary is the hazard, and a name
//! that lands WITH it is the discipline.
//!
//! # What this file CANNOT prove — read this before trusting a green run
//!
//! **Both halves of the comparison are SDK-written.** The schema
//! (`contracts/pmcp-run/portability-v1.graphql`) was authored here, not exported
//! from a live API, and the query it is checked against was authored here too.
//! The sibling `package_capture_contract.rs` validates SDK queries against a
//! PLATFORM-EXPORTED schema; this file does not, and neither does the other
//! sibling, `package_attestation_contract.rs`.
//!
//! So this test pins **SDK-INTERNAL agreement** today — that the operation
//! string and the proposed schema cannot drift apart, and that a change to one
//! forces a change to the other in the same PR. **It cannot detect drift from a
//! platform that has not spoken.** A green run here is not platform agreement,
//! and nobody should read it as one.
//!
//! **And that window is LONGER than `capture-v1` would suggest.** Per the
//! pmcp.run team's 2026-08-26 correction, a real cross-boundary drift net
//! arrives with IMPLEMENTATION, not with ratification: a ratified-but-
//! unimplemented operation still has no live schema to export, so there is
//! still nothing on the other side of the boundary to be pinned against. Until
//! `getPackageArtifact` is implemented and its SDL exported, every green run
//! here means only that the SDK agrees with itself.
//!
//! The ask lives in `docs/design/package-portability-pmcp-run-handoff.md` §5.1.

use apollo_compiler::validation::Valid;
use apollo_compiler::{ExecutableDocument, Schema};

use cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_ARTIFACT_QUERY;

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/portability-v1.graphql"
);

/// The contract leaf carrying the SDK's half of this contract. Scanned by
/// `sdl_is_honest_about_its_provenance_and_parking` for parking discipline.
const CONTRACT_LEAF_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/deployment/targets/pmcp_run/graphql_contract.rs"
);

/// The fields the SDK reads back out of `GetPackageArtifactReturnType`. Kept in
/// one place so the validation test, the drift check and the live leg cannot
/// disagree about them.
const EXPECTED_RESPONSE_FIELDS: [&str; 3] = ["payloadDigest", "downloadUrl", "expiresAt"];

// `ExecutableDocument::parse_and_validate` requires `&Valid<Schema>` — keep the
// `Valid` wrapper rather than unwrapping it, since apollo-compiler 1.x only
// offers operation-vs-schema validation against an already-`Valid` schema.
fn schema() -> Valid<Schema> {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read portability-v1.graphql");
    Schema::parse_and_validate(sdl, "portability-v1.graphql").expect("vendored SDL is itself valid")
}

/// The SDL with every `#` comment removed, so shape assertions read the SCHEMA
/// and not the prose about it.
///
/// This is load-bearing, not tidiness. `portability-v1.graphql`'s header
/// deliberately DISCUSSES the words `enum`, `push` and `import` in order to
/// record why they are absent, and its field comments quote the very question
/// text the schema must not assert as fact. A naive `sdl.contains("enum")`
/// would therefore be satisfied by the very comment explaining the ban — a
/// self-invalidating check. Copied in substance from
/// `package_attestation_contract.rs`, which makes the same argument.
fn sdl_body() -> String {
    std::fs::read_to_string(SDL_PATH)
        .expect("read portability-v1.graphql")
        .lines()
        .map(|line| match line.find('#') {
            Some(idx) => line[..idx].to_string(),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The field names the operation actually selects out of `getPackageArtifact`,
/// read from the PARSED operation rather than by string matching, so an added
/// field cannot hide behind formatting.
fn selected_response_fields() -> Vec<String> {
    let schema = schema();
    let doc = ExecutableDocument::parse_and_validate(
        &schema,
        GET_PACKAGE_ARTIFACT_QUERY,
        "get_package_artifact.graphql",
    )
    .expect("the operation parses and validates (see the dedicated test for the message)");

    let operation = doc
        .operations
        .get(None)
        .expect("the document declares exactly one operation");

    let root_field = operation
        .selection_set
        .selections
        .iter()
        .find_map(|selection| match selection {
            apollo_compiler::executable::Selection::Field(field)
                if field.name.as_str() == "getPackageArtifact" =>
            {
                Some(field)
            },
            _ => None,
        })
        .expect("the operation selects the `getPackageArtifact` root field");

    root_field
        .selection_set
        .selections
        .iter()
        .filter_map(|selection| match selection {
            apollo_compiler::executable::Selection::Field(field) => {
                Some(field.name.as_str().to_string())
            },
            _ => None,
        })
        .collect()
}

/// SC4's core assertion: the runtime operation must validate against the
/// vendored contract.
///
/// It validates the CLIENT'S constant — imported from
/// `cargo_pmcp::pmcp_run_graphql` — and not a copy of the query text, so the
/// test cannot drift from the client it is supposed to pin.
#[test]
fn get_package_artifact_op_validates_against_contract() {
    let schema = schema();
    ExecutableDocument::parse_and_validate(
        &schema,
        GET_PACKAGE_ARTIFACT_QUERY,
        "get_package_artifact.graphql",
    )
    .unwrap_or_else(|e| {
        panic!("`getPackageArtifact` op does not match portability-v1.graphql: {e}")
    });
}

/// Self-admitted-technical-debt markers, split across a `concat!` boundary so
/// that THIS FILE does not itself become a hit for the repo-wide SATD scanners
/// that read it. A check that creates the thing it bans is the same class of
/// self-invalidating mistake `sdl_body()` exists to avoid.
const SATD_MARKERS: [&str; 4] = [
    concat!("TO", "DO"),
    concat!("FIX", "ME"),
    concat!("HA", "CK"),
    concat!("XX", "X"),
];

/// The honesty pin, in two halves — one for each side of the parked contract.
///
/// **The SDL half.** It must say `SDK-PROPOSED` and must carry NEITHER a
/// source-naming nor an export-date provenance line. Its sibling
/// `capture-v1.graphql` carries both because it genuinely was exported from the
/// live pmcp.run AppSync API; this file was not, and imitating that header
/// would be a lie about who owns it. Asserted here rather than left to the eye.
/// These assertions read the RAW file deliberately — the provenance header IS
/// comments, so asserting its absence is precisely a statement about comments.
///
/// **The Rust half.** The parked state in `graphql_contract.rs` is expressed
/// with `#[allow(dead_code)]` plus rustdoc, never with a self-admitted-
/// technical-debt marker (C-3). A parked contract announced by a debt marker
/// reads as rot to be cleaned up rather than as a contract awaiting a
/// counterparty, and the repo has zero tolerance for SATD besides.
#[test]
fn sdl_is_honest_about_its_provenance_and_parking() {
    let raw = std::fs::read_to_string(SDL_PATH).expect("read portability-v1.graphql");

    assert!(
        raw.contains("SDK-PROPOSED"),
        "portability-v1.graphql must mark itself SDK-PROPOSED — it was not exported \
         from any live API"
    );

    for line in raw.lines() {
        let trimmed = line.trim_start();
        for forged in ["# Source:", "# Exported:"] {
            assert!(
                !trimmed.starts_with(forged),
                "portability-v1.graphql has no provenance and must not imply one; \
                 found a `{forged}` header line: {line}"
            );
        }
    }

    let leaf = std::fs::read_to_string(CONTRACT_LEAF_PATH).expect("read graphql_contract.rs");
    for marker in SATD_MARKERS {
        assert!(
            !leaf.contains(marker),
            "graphql_contract.rs must express parking with #[allow(dead_code)] plus \
             rustdoc, never with a `{marker}` debt marker (C-3)"
        );
    }
}

/// The shape pin: the three response fields are declared, `reference` is a
/// non-null `String` argument, exactly one operation field is proposed, and the
/// SDL declares no enum.
///
/// An enum would make any later schema-versus-schema diff (this proposal vs.
/// the platform's eventual export) show permanent drift — the same discipline
/// `capture-v1.graphql` records for its `status` fields and
/// `attestation-v1.graphql` for its `verdict`.
///
/// Everything here reads `sdl_body()`, the comment-stripped text. The header
/// discusses `enum`, `push` and `import` precisely in order to record why they
/// are absent, so reading the raw file would let the ban's own explanation
/// satisfy the ban.
#[test]
fn sdl_shape_is_pinned_to_d02_scope() {
    let body = sdl_body();

    for field in EXPECTED_RESPONSE_FIELDS {
        assert!(
            body.contains(&format!("{field}: String!")),
            "`{field}` must be declared `String!` in portability-v1.graphql:\n{body}"
        );
    }

    assert!(
        body.contains("getPackageArtifact(reference: String!)"),
        "the operation takes a single non-null String reference:\n{body}"
    );

    for line in body.lines() {
        assert!(
            !line.trim_start().starts_with("enum "),
            "portability-v1.graphql must declare no enum, found: {line}"
        );
        assert!(
            !line.contains(" enum "),
            "portability-v1.graphql must declare no enum, found: {line}"
        );
    }

    // Exactly one operation field is proposed — the whole point of D-02. The
    // retired `push` (D-01) and the platform-owned `import` (D-03) may appear
    // only in a `#` comment explaining their absence, which `sdl_body()` has
    // already removed.
    assert_eq!(
        body.matches("getPackageArtifact(").count(),
        1,
        "the proposal declares exactly one operation field:\n{body}"
    );
    for omitted in ["submitImport", "getImportStatus", "pushPackageArtifact"] {
        assert!(
            !body.contains(omitted),
            "`{omitted}` is outside D-02's scope and must not be declared \
             (it may appear only in a `#` comment explaining the omission)"
        );
    }
}

/// The selection-set drift check: the operation selects EXACTLY the fields the
/// SDK reads back — no more, no fewer.
///
/// "No fewer" catches a decoder that would read a field the query stopped
/// asking for. "No more" catches the opposite drift, a field added to the query
/// without being added to the SDL, which is why this and
/// `get_package_artifact_op_validates_against_contract` go red together on that
/// change (recorded as a negative control in this plan's SUMMARY) — one because
/// the schema does not declare it, one because the SDK does not read it.
///
/// It reads the PARSED selection set, not the query text, so whitespace or a
/// comment inside the operation cannot hide an added field.
#[test]
fn selection_set_matches_expected_response_fields_exactly() {
    let mut selected = selected_response_fields();
    selected.sort_unstable();

    let mut expected: Vec<String> = EXPECTED_RESPONSE_FIELDS
        .iter()
        .map(|f| (*f).to_string())
        .collect();
    expected.sort_unstable();

    assert_eq!(
        selected, expected,
        "the getPackageArtifact selection set drifted from the fields the SDK reads back"
    );
}

// ===========================================================================
// The PARKED live leg (PKGX-F1)
// ===========================================================================

/// PKGX-02's live egress leg — PARKED on a pmcp.run backend that does not
/// implement `getPackageArtifact` yet.
///
/// # This is an executable request path, not a description of one
///
/// Below the two gates, this test builds its body with the PRODUCTION builder
/// (`get_package_artifact_request_body`), POSTs it with the PRODUCTION header
/// constant (`GRAPHQL_AUTH_HEADER`), and decodes the answer with the PRODUCTION
/// decoder (`decode_get_package_artifact_response`). Nothing here is a stub:
/// the only piece that lives in the test rather than in `graphql_contract.rs`
/// is the `reqwest` transport itself, which is deliberate — that leaf must not
/// gain a `reqwest` dependency. Plan 05 wires the same transport into
/// `graphql.rs` for the shipped `pull` verb; this leg is its analogue at the
/// explicit-endpoint layer.
///
/// # Unparking, in one sentence
///
/// Delete the `#[ignore]` attribute and the two early-return gate blocks. The
/// request path below them already runs; there is nothing else to write.
///
/// # The two gates
///
/// - `PMCP_PORTABILITY_LIVE_TEST` — must be exactly `"1"`. Test-only; no
///   production path reads it.
/// - `PMCP_API_URL` — the endpoint, and `PMCP_ACCESS_TOKEN` — the credential.
///   Both REUSED, never invented: `PMCP_API_URL` is already the
///   highest-precedence endpoint source for the shipped client, and
///   `PMCP_ACCESS_TOKEN` is one of the three sources `get_credentials()` itself
///   reads (the documented CI/CD branch). SC3 forbids a second pmcp.run API
///   path, and introducing a second base-URL variable here would pre-break it.
///
/// Each gate PRINTS why it skipped. A silent skip is indistinguishable from a
/// pass, and the print is the part people drop.
///
/// # Why this reads the variables instead of calling `get_credentials()`
///
/// MEASURED and unchanged since Phase 122: `auth.rs` references
/// `crate::commands::configure::*`, so the whole module lives in the BIN target
/// only and an integration test in `tests/` cannot reach it. Mounting the
/// oauth2/browser-flow tree into the lib target to make one ignored test tidier
/// is the wrong trade. The cost: a live run cannot use the interactive
/// `~/.pmcp/credentials.toml` file or the client-credentials flow, and
/// `PMCP_API_URL` must point at the GRAPHQL endpoint rather than the API base,
/// because endpoint discovery lives in that same bin-only tree.
///
/// # What the backend must ship, and what the first live run answers
///
/// The assertions below already NAME the contract, so answering these means
/// TIGHTENING them, not writing new ones:
///
/// 1. Whether `payloadDigest` is the OCI manifest digest or a digest over the
///    tar bytes (the SDK assumes the former — SDL comment, research A4).
/// 2. Whether `downloadUrl` is fetched with a plain unauthenticated GET (A3).
///    This test deliberately does NOT fetch it: doing so before the platform
///    confirms the auth shape risks sending a pmcp.run token to another origin.
/// 3. Whether the tar behind it is uncompressed and tolerates an `oci-layout`
///    marker entry (A2, A6).
///
/// Run with:
/// ```sh
/// PMCP_PORTABILITY_LIVE_TEST=1 \
///   PMCP_API_URL=<real pmcp.run GraphQL endpoint> \
///   PMCP_ACCESS_TOKEN=<real token> \
///   cargo test -p cargo-pmcp --test package_portability_contract \
///     get_package_artifact_live -- --ignored --test-threads=1
/// ```
#[tokio::test]
#[ignore = "live network — requires PMCP_PORTABILITY_LIVE_TEST=1 + PMCP_API_URL + PMCP_ACCESS_TOKEN, and a pmcp.run backend that implements getPackageArtifact (PARKED)"]
async fn get_package_artifact_live() {
    // Gate 1 — explicit opt-in. Never reach the network by accident.
    if std::env::var("PMCP_PORTABILITY_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_PORTABILITY_LIVE_TEST=1 to enable the \
             live leg"
        );
        return;
    }

    // Gate 2 — the endpoint AND the credential. Without the credential the test
    // would POST unauthenticated and report a transport-level failure as if it
    // were a contract finding.
    let Some(api_url) = std::env::var("PMCP_API_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_API_URL to the pmcp.run GraphQL endpoint"
        );
        return;
    };
    let Some(access_token) = std::env::var("PMCP_ACCESS_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        eprintln!(
            "get_package_artifact_live skipped: set PMCP_ACCESS_TOKEN to a real pmcp.run access \
             token (one of get_credentials()'s own three sources)"
        );
        return;
    };

    let reference = std::env::var("PMCP_PORTABILITY_LIVE_REFERENCE")
        .unwrap_or_else(|_| "london-tube@1.0.0".to_string());

    let body = cargo_pmcp::pmcp_run_graphql::get_package_artifact_request_body(&reference)
        .expect("production builder accepts a non-empty reference");

    let response = reqwest::Client::new()
        .post(&api_url)
        .header(
            cargo_pmcp::pmcp_run_graphql::GRAPHQL_AUTH_HEADER,
            &access_token,
        )
        .json(&body)
        .send()
        .await
        .expect("POST to the pmcp.run GraphQL endpoint");

    let status = response.status();
    let parsed: serde_json::Value = response
        .json()
        .await
        .unwrap_or_else(|e| panic!("response (HTTP {status}) was not JSON: {e}"));

    // Decoded through the PRODUCTION decoder. Assertions read typed fields and
    // never echo `download_url` — it is a bearer credential (§5.1), so a
    // failure here must not print it into CI logs.
    let outcome = cargo_pmcp::pmcp_run_graphql::decode_get_package_artifact_response(&parsed)
        .unwrap_or_else(|e| panic!("getPackageArtifact (HTTP {status}) did not decode: {e}"));

    assert!(
        outcome.payload_digest.starts_with("sha256:"),
        "the platform must answer with a `sha256:`-prefixed digest the SDK can \
         compare its locally re-derived value against"
    );
    assert!(
        !outcome.download_url.trim().is_empty(),
        "the platform must answer with a download location — that is the entire \
         reason this call exists (value deliberately not printed: bearer credential)"
    );
    assert!(
        !outcome.expires_at.trim().is_empty(),
        "a presigned URL with no stated expiry cannot be reasoned about; §5.1 \
         proposes ~5 minutes"
    );
}
