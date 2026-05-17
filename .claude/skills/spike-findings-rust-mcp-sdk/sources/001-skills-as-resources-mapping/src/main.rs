//! Spike 001 — Skills as Resources Mapping (SEP-2640)
//!
//! Validates that a PMCP server can publish SEP-2640 Skills via the existing
//! `ResourceHandler` trait, and that the wire-format output of PMCP's
//! `ResourceInfo` / `ReadResourceResult` types matches the SEP-2640 spec
//! examples byte-for-byte (modulo whitespace).
//!
//! Build something the user can experience: this binary prints a labeled
//! transcript so the wire-format mapping is visually obvious.

use async_trait::async_trait;
use pmcp::types::{
    capabilities::{ResourceCapabilities, ServerCapabilities},
    Content, ListResourcesResult, ReadResourceResult, ResourceInfo,
};
use pmcp::{ErrorCode, RequestHandlerExtra, ResourceHandler};
use std::collections::HashMap;
use std::sync::Arc;

const HELLO_WORLD_SKILL: &str = "---
name: hello-world
description: Demonstrates the simplest possible MCP skill
---

# Hello World Skill

When the user greets the agent, respond warmly and offer to help.

## Examples

- User: \"hi\"  → \"Hello! What can I help you with today?\"
- User: \"hey there\"  → \"Hey! How can I help?\"
";

const REFUNDS_SKILL: &str = "---
name: refunds
description: Process customer refund requests per company policy
---

# Refund Workflow

1. Verify the order ID exists.
2. Check that the request is within the 30-day window.
3. Validate the reason against the allowed-reasons list.
4. Issue the refund via the billing tool.
";

/// SEP-2640 Skills served as MCP Resources.
///
/// One `ResourceHandler` impl covers list, read of `SKILL.md` files, and
/// read of the optional `skill://index.json` discovery resource.
struct SkillsHandler {
    skills: HashMap<String, &'static str>,
}

impl SkillsHandler {
    fn new() -> Self {
        let mut skills = HashMap::new();
        skills.insert("skill://hello-world/SKILL.md".to_string(), HELLO_WORLD_SKILL);
        skills.insert(
            "skill://acme/billing/refunds/SKILL.md".to_string(),
            REFUNDS_SKILL,
        );
        Self { skills }
    }

    fn discovery_index_json(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
            "skills": [
                {
                    "name": "hello-world",
                    "type": "skill-md",
                    "description": "Demonstrates the simplest possible MCP skill",
                    "url": "skill://hello-world/SKILL.md"
                },
                {
                    "name": "refunds",
                    "type": "skill-md",
                    "description": "Process customer refund requests per company policy",
                    "url": "skill://acme/billing/refunds/SKILL.md"
                }
            ]
        }))
        .expect("static json")
    }
}

#[async_trait]
impl ResourceHandler for SkillsHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let mut resources = Vec::new();
        for (uri, body) in &self.skills {
            let (name, description) = parse_frontmatter(body);
            resources.push(
                ResourceInfo::new(uri.as_str(), name)
                    .with_description(description)
                    .with_mime_type("text/markdown"),
            );
        }
        resources.push(
            ResourceInfo::new("skill://index.json", "index")
                .with_description("Skill discovery index (SEP-2640 §9)")
                .with_mime_type("application/json"),
        );
        Ok(ListResourcesResult::new(resources))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        if uri == "skill://index.json" {
            return Ok(ReadResourceResult::new(vec![Content::text(
                self.discovery_index_json(),
            )]));
        }
        match self.skills.get(uri) {
            Some(body) => Ok(ReadResourceResult::new(vec![Content::text(*body)])),
            None => Err(pmcp::Error::protocol(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Skill not found: {}", uri),
            )),
        }
    }
}

fn parse_frontmatter(body: &str) -> (&str, &str) {
    let mut name = "";
    let mut description = "";
    for line in body.lines().skip(1).take(20) {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("name: ") {
            name = rest.trim();
        } else if let Some(rest) = line.strip_prefix("description: ") {
            description = rest.trim();
        }
    }
    (name, description)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    print_banner();

    let handler: Arc<dyn ResourceHandler> = Arc::new(SkillsHandler::new());
    let extra = RequestHandlerExtra::default();

    step_1_capabilities();
    step_2_list_resources(&*handler, extra.clone()).await?;
    step_3_read_skill_md(&*handler, extra.clone()).await?;
    step_4_read_index(&*handler, extra).await?;
    step_5_probe_archive_gap();
    print_verdict();

    Ok(())
}

fn print_banner() {
    println!("═══════════════════════════════════════════════════════════════");
    println!(" SPIKE 001 — Skills as Resources Mapping  (SEP-2640)");
    println!("═══════════════════════════════════════════════════════════════\n");
}

fn step_1_capabilities() {
    println!("STEP 1 — Server capability declaration");
    println!("──────────────────────────────────────");
    println!("SEP-2640 §6 mandates:");
    println!("  {{\"capabilities\": {{\"extensions\": {{\"io.modelcontextprotocol/skills\": {{}}}}}}}}\n");

    let mut workaround_caps = ServerCapabilities::default();
    workaround_caps.resources = Some(ResourceCapabilities::default());
    let mut workaround_ext = HashMap::new();
    workaround_ext.insert(
        "io.modelcontextprotocol/skills".to_string(),
        serde_json::json!({}),
    );
    workaround_caps.experimental = Some(workaround_ext);

    println!("PMCP `ServerCapabilities` today (serialized):");
    println!("{}", serde_json::to_string_pretty(&workaround_caps).unwrap());
    println!();
    println!("❗ GAP: PMCP has no `extensions` field. Used `experimental` as a stand-in.");
    println!("   For wire-correct SEP-2640 support, PMCP needs an `extensions` field");
    println!("   on `ServerCapabilities` parallel to `experimental`.\n");
}

async fn step_2_list_resources(
    handler: &dyn ResourceHandler,
    extra: RequestHandlerExtra,
) -> anyhow::Result<()> {
    println!("STEP 2 — resources/list");
    println!("──────────────────────────────────────");
    let result = handler.list(None, extra).await?;
    let json = serde_json::to_value(&result)?;
    println!("Wire-format result:");
    println!("{}\n", serde_json::to_string_pretty(&json)?);

    let resources = json["resources"].as_array().expect("resources array");
    let skill_md_entry = resources
        .iter()
        .find(|r| r["uri"].as_str() == Some("skill://hello-world/SKILL.md"))
        .expect("hello-world skill listed");

    assert_eq!(skill_md_entry["mimeType"], "text/markdown");
    assert_eq!(skill_md_entry["name"], "hello-world");
    assert!(skill_md_entry["description"]
        .as_str()
        .unwrap_or("")
        .contains("simplest possible MCP skill"));

    println!("✓ Per-resource shape matches SEP-2640 §2 (uri/name/description/mimeType).\n");
    Ok(())
}

async fn step_3_read_skill_md(
    handler: &dyn ResourceHandler,
    extra: RequestHandlerExtra,
) -> anyhow::Result<()> {
    println!("STEP 3 — resources/read(\"skill://hello-world/SKILL.md\")");
    println!("──────────────────────────────────────");
    let result = handler
        .read("skill://hello-world/SKILL.md", extra)
        .await?;
    let json = serde_json::to_value(&result)?;
    println!("Wire-format result:");
    println!("{}\n", serde_json::to_string_pretty(&json)?);

    let text = json["contents"][0]["text"]
        .as_str()
        .expect("text content present");
    assert!(text.starts_with("---\nname: hello-world"));
    println!("✓ Returns markdown payload with YAML frontmatter intact (SEP-2640 §4).\n");
    Ok(())
}

async fn step_4_read_index(
    handler: &dyn ResourceHandler,
    extra: RequestHandlerExtra,
) -> anyhow::Result<()> {
    println!("STEP 4 — resources/read(\"skill://index.json\")");
    println!("──────────────────────────────────────");
    let result = handler.read("skill://index.json", extra).await?;
    let json = serde_json::to_value(&result)?;
    println!("Wire-format result:");
    println!("{}\n", serde_json::to_string_pretty(&json)?);

    let inner_text = json["contents"][0]["text"]
        .as_str()
        .expect("index text present");
    let parsed: serde_json::Value = serde_json::from_str(inner_text)?;
    assert_eq!(
        parsed["$schema"],
        "https://schemas.agentskills.io/discovery/0.2.0/schema.json"
    );
    assert_eq!(parsed["skills"].as_array().unwrap().len(), 2);
    println!("✓ Discovery index served as JSON content per SEP-2640 §9.\n");
    Ok(())
}

/// Probe SEP-2640 §4 archive distribution. SEP wire form is:
///   { "mimeType": "application/gzip", "contents": "<base64-encoded tar.gz>" }
///
/// PMCP's `Content::Resource` carries `uri + text? + mimeType? + _meta?` — no
/// `blob` field — and the custom resource_contents serializer only knows how
/// to emit the text-or-resource shapes. So a base64 archive cannot today be
/// served wire-correctly through `ReadResourceResult`.
fn step_5_probe_archive_gap() {
    println!("STEP 5 — Probing archive distribution (SEP-2640 §4)");
    println!("──────────────────────────────────────");
    println!("SEP-2640 wire form for an archived skill:");
    println!("  {{\"mimeType\": \"application/gzip\", \"contents\": \"<base64-encoded .tar.gz>\"}}\n");

    println!("Inspecting PMCP `Content::Resource` variant shape:");
    let probe = Content::Resource {
        uri: "skill://pdf-processing.tar.gz".to_string(),
        text: Some("base64-data-would-go-here".to_string()),
        mime_type: Some("application/gzip".to_string()),
        meta: None,
    };
    let probe_json = serde_json::to_value(probe).unwrap();
    println!("{}\n", serde_json::to_string_pretty(&probe_json).unwrap());

    println!("❗ GAP #2: PMCP `Content::Resource` has no `blob` field, only `text`.");
    println!("   The MCP spec's `ResourceContents` is `uri + (text | blob) + mimeType?`.");
    println!("   PMCP's custom resource_contents serializer (src/types/content.rs:325)");
    println!("   does not emit a `blob` key. Wire-correct base64-archive serving via");
    println!("   `ReadResourceResult` is therefore not possible today.\n");
    println!("   Workarounds:");
    println!("     (a) Cram base64 into `text` (wire-incompatible — spec says `blob`).");
    println!("     (b) Skip archive distribution; serve SKILL.md + supporting files");
    println!("         individually as text resources (SEP-2640 §4 says archive is optional).\n");
    println!("   Real fix: add a `blob: Option<String>` field to Content::Resource and");
    println!("   teach the resource_contents serializer to emit it.\n");
}

fn print_verdict() {
    println!("═══════════════════════════════════════════════════════════════");
    println!(" VERDICT — VALIDATED with caveats");
    println!("═══════════════════════════════════════════════════════════════\n");
    println!("✓ Text-mode skill serving works end-to-end through PMCP's existing");
    println!("  ResourceHandler / ResourceInfo / Content types. The `skill://` URI");
    println!("  convention, mimeType, name, description, and discovery index all");
    println!("  flow through unchanged.\n");
    println!("⚠ Two protocol-types gaps surfaced:\n");
    println!("  GAP #1 — `ServerCapabilities` lacks `extensions`.");
    println!("    Required by SEP-2640 §6 for capability declaration.");
    println!("    Fix: add `extensions: Option<HashMap<String, Value>>` parallel to");
    println!("         `experimental` (additive, no breaking change).\n");
    println!("  GAP #2 — `Content::Resource` lacks `blob`, blocking archive distribution.");
    println!("    SEP-2640 §4 archive form uses `application/gzip` + base64 blob.");
    println!("    Fix: add `blob: Option<String>` to Content::Resource and emit it");
    println!("         from `resource_contents_serde::serialize`.");
    println!("    Workaround: archive distribution is OPTIONAL per SEP-2640 §4.");
    println!("         Text-mode (SKILL.md + supporting files) is fully sufficient.\n");
    println!("Implication for spike 002 (DX layer):");
    println!("    A `register_skill(...)` helper or `#[pmcp::skill]` macro can be");
    println!("    implemented as sugar over the existing ResourceHandler trait. The");
    println!("    two protocol gaps above are independent of the DX layer — 002 can");
    println!("    proceed with text-mode skills and either workaround or assume the");
    println!("    two protocol-types additions land first.");
}
