//! # Client example: Skills via BOTH host flows (Phase 80)
//!
//! Walks the two host flows side-by-side and proves they carry the SAME
//! content (byte-equal).
//!
//! ## Flow A — SEP-2640 host
//!
//! Capable hosts call `resources/list`, see SKILL.md + index entries, then
//! `resources/read` each URI lazily. Reference files (like
//! `skill://code-mode/references/schema.graphql`) are read by URI without
//! being enumerated. Each read response carries the URI and the
//! per-resource MIME type per SEP-2640 §4 wire shape — locked by
//! 80-REVIEWS.md Fix 3.
//!
//! ## Flow B — legacy prompt host
//!
//! Older hosts (no SEP-2640 support) call `prompts/get start_code_mode`.
//! This example exercises the same code path by building a real `Server`
//! via `pmcp::Server::builder()` and retrieving the registered
//! `PromptHandler` via `server.get_prompt("start_code_mode")`, then
//! invoking `.handle(args, extra)` — the same path `prompts/get` executes
//! per `src/server/mod.rs` `handle_get_prompt`. This is the wire-level
//! legacy flow per 80-REVIEWS.md Fix 5 / Codex C6.
//!
//! The example asserts byte-equality between the concatenated SEP-2640 read
//! results and the legacy prompt body. If the invariant is ever broken,
//! `cargo run --example c10_client_skills` panics — by design, since a
//! silently-passing example that prints "OK" when the invariant is broken
//! is worse than no example at all.
//!
//! Pair with `s44_server_skills.rs` for the server-side counterpart.
//!
//! Run with: `cargo run --example c10_client_skills --features skills,full`

use std::collections::HashMap;

use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::types::Content;
use pmcp::{RequestHandlerExtra, ResourceHandler};

const CODE_MODE: &str = include_str!("skills/code-mode/SKILL.md");
const CODE_MODE_SCHEMA: &str = include_str!("skills/code-mode/references/schema.graphql");
const CODE_MODE_EXAMPLES: &str = include_str!("skills/code-mode/references/examples.md");
const CODE_MODE_POLICIES: &str = include_str!("skills/code-mode/references/policies.md");

fn build_code_mode_skill() -> Skill {
    Skill::new("code-mode", CODE_MODE)
        .with_reference(SkillReference::new(
            "references/schema.graphql",
            "application/graphql",
            CODE_MODE_SCHEMA,
        ))
        .with_reference(SkillReference::new(
            "references/examples.md",
            "text/markdown",
            CODE_MODE_EXAMPLES,
        ))
        .with_reference(SkillReference::new(
            "references/policies.md",
            "text/markdown",
            CODE_MODE_POLICIES,
        ))
}

// 80-REVIEWS.md Fix 3 / Codex C4: SkillsHandler returns Content::Resource
// (not Content::Text), so each read response carries uri + mime_type
// alongside the body. Extract via pattern match on the Resource variant.
fn extract_resource(contents: &[Content]) -> (String, String, String) {
    match contents.first() {
        Some(Content::Resource {
            uri,
            text,
            mime_type,
            ..
        }) => (
            uri.clone(),
            text.clone().expect("skills handler always emits text body"),
            mime_type
                .clone()
                .expect("skills handler always emits mime_type"),
        ),
        other => panic!("expected Content::Resource from skill resource read, got {other:?}"),
    }
}

fn append_with_trailing_newline(out: &mut String, body: &str) {
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
}

async fn sep_2640_flow(handler: &dyn ResourceHandler, skill: &Skill) -> String {
    let extra = RequestHandlerExtra::default();

    // 1. resources/list — SKILL.md + index ONLY (references excluded per §9).
    let list = handler.list(None, extra.clone()).await.unwrap();
    println!(
        "resources/list returned {} resource(s):",
        list.resources.len()
    );
    for r in &list.resources {
        println!("  {} ({})", r.uri, r.mime_type.as_deref().unwrap_or(""));
    }
    println!();

    // 2. resources/read index — assert wire shape per Fix 3.
    let index_result = handler
        .read("skill://index.json", extra.clone())
        .await
        .unwrap();
    let (index_uri, index_text, index_mime) = extract_resource(&index_result.contents);
    assert_eq!(index_uri, "skill://index.json");
    assert_eq!(index_mime, "application/json");
    println!(
        "resources/read index uri={index_uri} mime={index_mime} bytes={}",
        index_text.len()
    );
    println!("{}", &index_text[..index_text.len().min(240)]);
    println!();

    // 3. resources/read SKILL.md — assert wire shape.
    let skill_uri = "skill://code-mode/SKILL.md";
    let md_result = handler.read(skill_uri, extra.clone()).await.unwrap();
    let (md_uri, main_text, md_mime) = extract_resource(&md_result.contents);
    assert_eq!(md_uri, skill_uri);
    assert_eq!(md_mime, "text/markdown");
    println!(
        "resources/read SKILL.md uri={md_uri} mime={md_mime} bytes={}",
        main_text.len()
    );

    // 4. resources/read each reference URI — registration order — per-reference MIME.
    let mut concatenated = String::new();
    append_with_trailing_newline(&mut concatenated, &main_text);
    for r in skill.references() {
        let uri = format!("skill://code-mode/{}", r.relative_path());
        let read = handler.read(&uri, extra.clone()).await.unwrap();
        let (resp_uri, body, resp_mime) = extract_resource(&read.contents);
        assert_eq!(resp_uri, uri);
        assert_eq!(
            resp_mime,
            r.mime_type(),
            "per-resource MIME type must round-trip (Fix 3)"
        );
        println!(
            "resources/read reference uri={resp_uri} mime={resp_mime} bytes={}",
            body.len()
        );
        concatenated.push_str("\n--- ");
        concatenated.push_str(r.relative_path());
        concatenated.push_str(" ---\n");
        append_with_trailing_newline(&mut concatenated, &body);
    }

    concatenated
}

/// Legacy host flow — exercises the wire-level prompt-handler path per Fix 5.
async fn legacy_prompt_flow_via_get_prompt(skill: Skill) -> String {
    let server = pmcp::Server::builder()
        .name("skills-demo-client")
        .version("0.1.0")
        .bootstrap_skill_and_prompt(skill, "start_code_mode")
        .build()
        .expect("server build");

    let prompt_handler = server
        .get_prompt("start_code_mode")
        .expect("bootstrap_skill_and_prompt registered the handler");

    let extra = RequestHandlerExtra::default();
    let result = prompt_handler.handle(HashMap::new(), extra).await.unwrap();

    // SkillPromptHandler returns a single PromptMessage::user(Content::text(...)).
    // (Prompt messages still use plain Content::Text — the wire-shape fix
    // applies only to ResourceHandler::read, not to PromptMessage content.)
    let prompt_text = match &result.messages[0].content {
        Content::Text { text } => text.clone(),
        other => panic!("expected Content::Text for prompt message, got {other:?}"),
    };

    println!(
        "prompts/get start_code_mode returned {} bytes",
        prompt_text.len()
    );
    println!("First 240 bytes of prompt body:");
    println!("{}", &prompt_text[..prompt_text.len().min(240)]);
    println!();
    prompt_text
}

#[tokio::main]
async fn main() {
    let skill = build_code_mode_skill();

    // Build the handler that an SEP-2640-capable host would interact with.
    let handler = Skills::new()
        .add(skill.clone())
        .into_handler()
        .expect("skill registration must not collide");

    println!("=== Flow A: SEP-2640-capable host (resources/list + resources/read) ===");
    let sep_2640_text = sep_2640_flow(&*handler, &skill).await;
    println!();

    println!("=== Flow B: legacy host (prompts/get start_code_mode via get_prompt) ===");
    let prompt_text = legacy_prompt_flow_via_get_prompt(skill).await;
    println!();

    println!("=== Byte-equality assertion ===");
    assert_eq!(
        sep_2640_text, prompt_text,
        "dual-surface invariant violated: SEP-2640 read concatenation != prompt body"
    );
    println!(
        "Both flows produced byte-equal context ({} bytes).",
        prompt_text.len()
    );
}
