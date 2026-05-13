//! SEP-2640 Agent Skills — `ResourceHandler`-served skill resources with
//! a parallel `PromptHandler` fallback for SEP-2640-blind hosts.
//!
//! Both surfaces are derived from one [`Skill`] value, so the SKILL.md
//! content + each reference body is byte-equal whether the host fetches
//! via [`crate::server::ResourceHandler::list`]/[`crate::server::ResourceHandler::read`]
//! (SEP-2640) or via [`crate::server::PromptHandler::handle`] (legacy).
//!
//! Internal storage uses [`indexmap::IndexMap`] so resource ordering is
//! deterministic across runs — required for stable example output,
//! snapshot tests, and predictable host UX (80-REVIEWS.md Fix 8).
//!
//! Wire shape: reads return [`crate::types::Content::resource_with_text`]
//! (NOT [`crate::types::Content::text`]) so per-resource MIME types
//! survive the wire round-trip — required for SEP-2640 compliance and so
//! reference files like `schema.graphql` keep their
//! `application/graphql` MIME type (80-REVIEWS.md Fix 3).
//!
//! See the spike-findings skill at
//! `.claude/skills/spike-findings-rust-mcp-sdk/` for the design rationale.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::json;

use crate::error::{Error, ErrorCode, Result};
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::{PromptHandler, ResourceHandler};
use crate::types::content::Role;
use crate::types::{
    Content, GetPromptResult, ListResourcesResult, PromptMessage, ReadResourceResult, ResourceInfo,
};

// ── Public types ──────────────────────────────────────────────────────

/// A supporting file within a skill's directory (SEP-2640 §4 directory model).
///
/// Carries the relative path (e.g. `references/schema.graphql`), the
/// per-resource MIME type, and the body. Validation against duplicate or
/// invalid paths happens at [`Skill::with_reference`] /
/// [`Skill::try_with_reference`] time so that the parent skill's existing
/// references can be consulted.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::SkillReference;
///
/// let r = SkillReference::new("references/api.md", "text/markdown", "...");
/// assert_eq!(r.relative_path(), "references/api.md");
/// assert_eq!(r.mime_type(), "text/markdown");
/// ```
#[derive(Clone, Debug)]
pub struct SkillReference {
    relative_path: String,
    mime_type: String,
    body: String,
}

impl SkillReference {
    /// Construct a reference. Validation happens at
    /// [`Skill::with_reference`] / [`Skill::try_with_reference`] time so
    /// duplicate-within-skill checks can use the parent's reference set.
    pub fn new(
        relative_path: impl Into<String>,
        mime_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            mime_type: mime_type.into(),
            body: body.into(),
        }
    }

    /// Relative path within the skill's directory (e.g.
    /// `references/schema.graphql`).
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// Per-resource MIME type (e.g. `application/graphql`).
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Reference body text.
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// A single Agent Skill (SEP-2640).
///
/// `name` is required (derived from SKILL.md frontmatter); `body` is the
/// SKILL.md content with YAML frontmatter intact. Optional `path` overrides
/// the default `skill://<name>/SKILL.md` URI. Optional `references` carry
/// supporting files (`schema.graphql`, `examples.md`, etc.) addressable at
/// `skill://<path>/<relative_path>`.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::{Skill, SkillReference};
///
/// let s = Skill::new("refunds", "---\nname: refunds\ndescription: Issue refunds\n---\nBody")
///     .with_reference(SkillReference::new("references/policy.md", "text/markdown", "..."));
/// assert_eq!(s.name(), "refunds");
/// assert_eq!(s.resolved_description(), "Issue refunds");
/// ```
#[derive(Clone, Debug)]
pub struct Skill {
    name: String,
    body: String,
    path: Option<String>,
    description: Option<String>,
    references: Vec<SkillReference>,
}

impl Skill {
    /// Create a skill from its frontmatter `name` and full SKILL.md body.
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
            path: None,
            description: None,
            references: Vec::new(),
        }
    }

    /// Override the URI path (default: `skill://<name>/SKILL.md`).
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Explicit description override; otherwise parsed from frontmatter.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Append a reference. **Panics** on invalid `relative_path` — use
    /// [`Self::try_with_reference`] for fallible registration.
    ///
    /// Invalid inputs: empty, exactly `"SKILL.md"` (collides with the
    /// canonical URI), contains `..` segment, starts with `/`, contains
    /// `://`, or duplicates a `relative_path` already registered on this
    /// Skill.
    ///
    /// Per 80-REVIEWS.md Fix 6 / Codex C7 — silently storing these
    /// values produced unreachable or confusing URIs in earlier drafts.
    ///
    /// # Panics
    ///
    /// Panics if the reference's relative path violates any of the rules
    /// listed above. Use [`Self::try_with_reference`] to surface the same
    /// failures as `Result`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let s = Skill::new("x", "body")
    ///     .with_reference(SkillReference::new("references/a.md", "text/markdown", "a"));
    /// assert_eq!(s.references().count(), 1);
    /// ```
    #[must_use]
    pub fn with_reference(self, reference: SkillReference) -> Self {
        match self.try_with_reference(reference) {
            Ok(s) => s,
            Err(e) => panic!("Skill::with_reference: {e}"),
        }
    }

    /// Append a reference, returning Err on invalid input. Use this for
    /// runtime-dynamic registration where panicking is unacceptable
    /// (80-REVIEWS.md Fix 6 + Fix 10 / Codex C7 + suggestion G2).
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the relative path is
    /// empty, exactly `"SKILL.md"`, contains a `..` segment, starts with
    /// `/`, contains `://`, or duplicates an existing relative path on
    /// this skill.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let ok = Skill::new("x", "body")
    ///     .try_with_reference(SkillReference::new("references/a.md", "text/markdown", "a"));
    /// assert!(ok.is_ok());
    ///
    /// let bad = Skill::new("x", "body")
    ///     .try_with_reference(SkillReference::new("", "text/markdown", "a"));
    /// assert!(bad.is_err());
    /// ```
    pub fn try_with_reference(mut self, reference: SkillReference) -> Result<Self> {
        validate_reference_path(&reference.relative_path, &self.references)?;
        self.references.push(reference);
        Ok(self)
    }

    /// Skill name (from frontmatter).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Full SKILL.md body (frontmatter + recipe).
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Iterate over registered references in insertion order.
    pub fn references(&self) -> impl Iterator<Item = &SkillReference> {
        self.references.iter()
    }

    /// Resolved description: explicit `.with_description(...)` override
    /// if set, otherwise the `description:` line parsed from the SKILL.md
    /// frontmatter. Returns `""` if neither is available.
    pub fn resolved_description(&self) -> String {
        if let Some(d) = &self.description {
            return d.clone();
        }
        parse_frontmatter_description(&self.body).unwrap_or_default()
    }

    pub(crate) fn resolved_path(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn skill_md_uri(&self) -> String {
        format!("skill://{}/SKILL.md", self.resolved_path())
    }

    pub(crate) fn reference_uri(&self, relative_path: &str) -> String {
        format!("skill://{}/{}", self.resolved_path(), relative_path)
    }

    /// Synthesize the PROMPT surface — body followed by each reference
    /// inlined with labelled `--- <relative_path> ---` rules.
    ///
    /// This is the load-bearing dual-surface invariant: the value
    /// returned here is byte-equal to the concatenation of the SKILL.md
    /// body and every reference body read via the
    /// [`crate::server::ResourceHandler`] surface, with a trailing
    /// newline normalization applied to each segment.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let s = Skill::new("x", "A")
    ///     .with_reference(SkillReference::new("ref1.md", "text/markdown", "refbody"));
    /// assert_eq!(s.as_prompt_text(), "A\n\n--- ref1.md ---\nrefbody\n");
    /// ```
    pub fn as_prompt_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.body);
        if !self.body.ends_with('\n') {
            out.push('\n');
        }
        for r in &self.references {
            out.push_str("\n--- ");
            out.push_str(&r.relative_path);
            out.push_str(" ---\n");
            out.push_str(&r.body);
            if !r.body.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }
}

/// Reference-path validation per 80-REVIEWS.md Fix 6 / Codex C7.
fn validate_reference_path(path: &str, existing: &[SkillReference]) -> Result<()> {
    if path.is_empty() {
        return Err(Error::validation(
            "SkillReference relative_path must not be empty",
        ));
    }
    if path == "SKILL.md" {
        return Err(Error::validation(
            "SkillReference relative_path 'SKILL.md' collides with the canonical SKILL.md URI",
        ));
    }
    if path.split('/').any(|seg| seg == "..") {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must not contain '..' segments"
        )));
    }
    if path.starts_with('/') {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must be relative (no leading '/')"
        )));
    }
    if path.contains("://") {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must not contain a URI scheme"
        )));
    }
    if existing.iter().any(|r| r.relative_path == path) {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' is already registered on this Skill"
        )));
    }
    Ok(())
}

/// Collection of skills + auto-generated discovery index. Lifted into a
/// [`crate::server::ResourceHandler`] impl via [`Skills::into_handler`].
///
/// `Clone` is required so the builder's `try_skills` can probe duplicates
/// by cloning the registry before storing it (consume-by-value
/// `into_handler` API).
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::{Skill, Skills};
///
/// let registry = Skills::new()
///     .add(Skill::new("a", "body-a"))
///     .add(Skill::new("b", "body-b"));
/// assert_eq!(registry.skill_md_uris(), vec![
///     "skill://a/SKILL.md".to_string(),
///     "skill://b/SKILL.md".to_string(),
/// ]);
/// ```
#[derive(Default, Clone, Debug)]
pub struct Skills {
    skills: Vec<Skill>,
}

impl Skills {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// Append a skill to the registry.
    #[must_use]
    #[allow(clippy::should_implement_trait)] // builder-style consumer; not a std::ops::Add impl
    pub fn add(mut self, skill: Skill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Concatenate another registry onto this one (used by the builder
    /// accumulator on repeated `.skills(...)` calls per 80-REVIEWS.md Fix 1).
    #[must_use]
    pub fn merge(mut self, other: Skills) -> Self {
        self.skills.extend(other.skills);
        self
    }

    /// Snapshot of all registered SKILL.md URIs in registration order.
    ///
    /// Reference URIs are NOT included — they're readable via
    /// `resources/read` but never enumerated (SEP-2640 §9).
    pub fn skill_md_uris(&self) -> Vec<String> {
        self.skills.iter().map(Skill::skill_md_uri).collect()
    }

    /// Flatten the registry into a [`crate::server::ResourceHandler`].
    ///
    /// Returns `Err` on:
    /// - Two skills resolving to the same `skill_md_uri()`
    ///   (Implementation Decision #5).
    /// - Two skills' reference URIs colliding (80-REVIEWS.md Fix 6
    ///   extension).
    ///
    /// Insertion order is preserved via [`indexmap::IndexMap`] (Fix 8).
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` listing every duplicate URI
    /// detected. No silent overwrites.
    pub fn into_handler(self) -> Result<Arc<dyn ResourceHandler>> {
        let mut skill_md: IndexMap<String, Skill> = IndexMap::with_capacity(self.skills.len());
        let mut references: IndexMap<String, (String, String)> = IndexMap::new();
        let mut dup_skill: Vec<String> = Vec::new();
        let mut dup_ref: Vec<String> = Vec::new();
        for skill in self.skills {
            for r in &skill.references {
                let uri = skill.reference_uri(&r.relative_path);
                if references.contains_key(&uri) {
                    dup_ref.push(uri);
                } else {
                    references.insert(uri, (r.mime_type.clone(), r.body.clone()));
                }
            }
            let uri = skill.skill_md_uri();
            if skill_md.contains_key(&uri) {
                dup_skill.push(uri);
            } else {
                skill_md.insert(uri, skill);
            }
        }
        if !dup_skill.is_empty() || !dup_ref.is_empty() {
            let mut msg = String::from("Skills::into_handler: duplicate URI(s):");
            if !dup_skill.is_empty() {
                msg.push_str(&format!(" SKILL.md=[{}]", dup_skill.join(", ")));
            }
            if !dup_ref.is_empty() {
                msg.push_str(&format!(" references=[{}]", dup_ref.join(", ")));
            }
            return Err(Error::validation(msg));
        }
        Ok(Arc::new(SkillsHandler {
            skill_md,
            references,
        }))
    }
}

// ── Internal handler types ────────────────────────────────────────────

/// Internal [`crate::server::ResourceHandler`] impl synthesized by
/// [`Skills::into_handler`].
pub(crate) struct SkillsHandler {
    skill_md: IndexMap<String, Skill>,
    references: IndexMap<String, (String, String)>, // uri -> (mime, body)
}

impl SkillsHandler {
    fn discovery_index_json(&self) -> String {
        let entries: Vec<_> = self
            .skill_md
            .values()
            .map(|s| {
                json!({
                    "name": s.name(),
                    "type": "skill-md",
                    "description": s.resolved_description(),
                    "url": s.skill_md_uri(),
                })
            })
            .collect();
        serde_json::to_string_pretty(&json!({
            "$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
            "skills": entries,
        }))
        .expect("static JSON object — to_string_pretty cannot fail")
    }
}

#[async_trait]
impl ResourceHandler for SkillsHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Per SEP-2640 §9: list emits SKILL.md entries + the discovery
        // index ONLY. Reference URIs are never enumerated.
        let mut resources: Vec<ResourceInfo> = self
            .skill_md
            .values()
            .map(|s| {
                ResourceInfo::new(s.skill_md_uri(), s.name().to_string())
                    .with_description(s.resolved_description())
                    .with_mime_type("text/markdown")
            })
            .collect();
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
    ) -> Result<ReadResourceResult> {
        // 80-REVIEWS.md Fix 3 / Codex C4: emit Content::resource_with_text
        // so each read carries its URI + MIME type on the wire.
        if uri == "skill://index.json" {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                self.discovery_index_json(),
                "application/json",
            )]));
        }
        if let Some(skill) = self.skill_md.get(uri) {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                skill.body().to_string(),
                "text/markdown",
            )]));
        }
        if let Some((mime, body)) = self.references.get(uri) {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                body.clone(),
                mime.clone(),
            )]));
        }
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            format!("Skill resource not found: {uri}"),
        ))
    }
}

/// [`crate::server::PromptHandler`] impl that returns
/// [`Skill::as_prompt_text`] as a single user message.
///
/// The dual-surface invariant: the prompt body is byte-equal to the
/// concatenated SKILL.md + reference reads. Pointer-style prompts
/// (returning a `skill://` URI the host cannot fetch) are PROHIBITED
/// per Implementation Decision #7.
#[allow(dead_code)] // Wired up by builder integration in 80-02 Task 2
pub(crate) struct SkillPromptHandler {
    skill: Skill,
}

impl SkillPromptHandler {
    #[allow(dead_code)] // Wired up by builder integration in 80-02 Task 2
    pub(crate) fn new(skill: Skill) -> Self {
        Self { skill }
    }
}

#[async_trait]
impl PromptHandler for SkillPromptHandler {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Plain Content::text is correct here — this is a prompt message,
        // not a resource read. The wire-shape fix in Fix 3 applies only
        // to ResourceHandler::read, not to PromptMessage content.
        let message = PromptMessage::new(Role::User, Content::text(self.skill.as_prompt_text()));
        Ok(GetPromptResult::new(
            vec![message],
            Some(self.skill.resolved_description()),
        ))
    }
}

/// URI-prefix-routing composite [`crate::server::ResourceHandler`].
///
/// Constructed AT MOST ONCE per server, in the builder's `.build()`
/// finalization step. There is no `ComposedResources`-inside-`ComposedResources`
/// nesting (80-REVIEWS.md Fix 1).
#[allow(dead_code)] // Wired up by builder integration in 80-02 Task 2
pub(crate) struct ComposedResources {
    pub(crate) skills: Arc<dyn ResourceHandler>,
    pub(crate) other: Arc<dyn ResourceHandler>,
}

#[async_trait]
impl ResourceHandler for ComposedResources {
    async fn list(
        &self,
        cursor: Option<String>,
        extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        let mut combined = self.skills.list(cursor.clone(), extra.clone()).await?;
        let extra_other = self.other.list(cursor, extra).await?;
        combined.resources.extend(extra_other.resources);
        Ok(combined)
    }

    async fn read(
        &self,
        uri: &str,
        extra: RequestHandlerExtra,
    ) -> Result<ReadResourceResult> {
        if uri.starts_with("skill://") {
            self.skills.read(uri, extra).await
        } else {
            self.other.read(uri, extra).await
        }
    }
}

// ── Frontmatter parsing (internal) ───────────────────────────────────

fn parse_frontmatter_description(body: &str) -> Option<String> {
    // 80-REVIEWS.md Fix 9 / Gemini: strip UTF-8 BOM; `str::lines()`
    // already handles both \n and \r\n line endings.
    let body = body.strip_prefix('\u{FEFF}').unwrap_or(body);
    let mut in_frontmatter = false;
    for line in body.lines().take(40) {
        if line == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(rest) = line.strip_prefix("description: ") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn extra() -> RequestHandlerExtra {
        RequestHandlerExtra::default()
    }

    // ── Test 1.1 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_1_skill_new_and_builders() {
        let s = Skill::new("foo", "body");
        assert_eq!(s.name(), "foo");
        assert_eq!(s.body(), "body");
        assert_eq!(s.references().count(), 0);
        assert_eq!(s.resolved_description(), "");

        let s = s
            .with_path("p")
            .with_description("d")
            .with_reference(SkillReference::new(
                "references/x.md",
                "text/markdown",
                "ref body",
            ));
        assert_eq!(s.resolved_path(), "p");
        assert_eq!(s.resolved_description(), "d");
        assert_eq!(s.references().count(), 1);
    }

    // ── Test 1.2 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_2_skill_md_uri_default_and_override() {
        let s = Skill::new("foo", "");
        assert_eq!(s.skill_md_uri(), "skill://foo/SKILL.md");
        let s = s.with_path("acme/refunds");
        assert_eq!(s.skill_md_uri(), "skill://acme/refunds/SKILL.md");
    }

    // ── Test 1.3 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_3_skill_reference_uri_resolution() {
        let s = Skill::new("x", "").with_reference(SkillReference::new(
            "references/a.md",
            "text/markdown",
            "...",
        ));
        assert_eq!(s.reference_uri("references/a.md"), "skill://x/references/a.md");

        let s = s.with_path("y/z");
        assert_eq!(
            s.reference_uri("references/a.md"),
            "skill://y/z/references/a.md"
        );
    }

    // ── Test 1.4 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_4_as_prompt_text_no_references() {
        let s = Skill::new("x", "---\nname: x\n---\nbody");
        assert_eq!(s.as_prompt_text(), "---\nname: x\n---\nbody\n");
    }

    // ── Test 1.5 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_5_as_prompt_text_with_references() {
        let s = Skill::new("x", "A").with_reference(SkillReference::new(
            "ref1.md",
            "text/markdown",
            "refbody",
        ));
        assert_eq!(s.as_prompt_text(), "A\n\n--- ref1.md ---\nrefbody\n");

        let s = Skill::new("x", "A")
            .with_reference(SkillReference::new("r1.md", "text/markdown", "b1"))
            .with_reference(SkillReference::new("r2.md", "text/markdown", "b2"));
        assert_eq!(
            s.as_prompt_text(),
            "A\n\n--- r1.md ---\nb1\n\n--- r2.md ---\nb2\n"
        );
    }

    // ── Test 1.6 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_6_resolved_description_frontmatter_parsing() {
        let s = Skill::new("x", "---\nname: x\ndescription: hello\n---\nbody");
        assert_eq!(s.resolved_description(), "hello");

        let s = Skill::new("x", "---\nname: x\ndescription: hello\n---\nbody")
            .with_description("override");
        assert_eq!(s.resolved_description(), "override");

        let s = Skill::new("x", "no frontmatter");
        assert_eq!(s.resolved_description(), "");
    }

    // ── Test 1.6a (CRLF) ──────────────────────────────────────────────
    #[test]
    fn test_1_6a_parse_frontmatter_crlf() {
        let s = Skill::new("x", "---\r\nname: x\r\ndescription: hello\r\n---\r\nbody");
        assert_eq!(s.resolved_description(), "hello");
    }

    // ── Test 1.6b (UTF-8 BOM) ─────────────────────────────────────────
    #[test]
    fn test_1_6b_parse_frontmatter_utf8_bom() {
        let s = Skill::new(
            "x",
            "\u{FEFF}---\nname: x\ndescription: hello\n---\nbody",
        );
        assert_eq!(s.resolved_description(), "hello");
    }

    // ── Test 1.7 ──────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_7_skills_into_handler_happy_path() {
        let handler = Skills::new()
            .add(Skill::new("a", ""))
            .add(Skill::new("b", ""))
            .into_handler()
            .unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
        assert_eq!(list.resources[2].uri, "skill://index.json");
        // No references in the list.
        for r in &list.resources {
            assert!(!r.uri.contains("/references/"));
        }
    }

    // ── Test 1.7a (registration order) ────────────────────────────────
    #[tokio::test]
    async fn test_1_7a_skills_into_handler_preserves_registration_order() {
        for _ in 0..10 {
            let handler = Skills::new()
                .add(Skill::new("zeta", ""))
                .add(Skill::new("alpha", ""))
                .add(Skill::new("mu", ""))
                .into_handler()
                .unwrap();
            let list = handler.list(None, extra()).await.unwrap();
            assert_eq!(list.resources.len(), 4);
            assert_eq!(list.resources[0].uri, "skill://zeta/SKILL.md");
            assert_eq!(list.resources[1].uri, "skill://alpha/SKILL.md");
            assert_eq!(list.resources[2].uri, "skill://mu/SKILL.md");
            assert_eq!(list.resources[3].uri, "skill://index.json");
        }
    }

    // ── Test 1.8 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_8_skills_into_handler_duplicate_skill_md_uri_rejected() {
        match Skills::new()
            .add(Skill::new("refunds", "a"))
            .add(Skill::new("refunds", "b"))
            .into_handler()
        {
            Err(Error::Validation(msg)) => {
                assert!(msg.contains("skill://refunds/SKILL.md"), "msg = {msg}");
            },
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for duplicate names"),
        }

        // Different names colliding via path.
        match Skills::new()
            .add(Skill::new("a", "").with_path("p"))
            .add(Skill::new("b", "").with_path("p"))
            .into_handler()
        {
            Err(Error::Validation(msg)) => assert!(msg.contains("skill://p/SKILL.md")),
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for colliding paths"),
        }
    }

    // ── Test 1.8a (cross-skill reference URI duplicates) ──────────────
    #[test]
    fn test_1_8a_skills_into_handler_duplicate_reference_uri_rejected() {
        let s1 = Skill::new("a", "").with_reference(SkillReference::new(
            "references/shared.md",
            "text/markdown",
            "x",
        ));
        let s2 = Skill::new("b", "")
            .with_path("a")
            .with_reference(SkillReference::new(
                "references/shared.md",
                "text/markdown",
                "y",
            ));
        match Skills::new().add(s1).add(s2).into_handler() {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("skill://a/references/shared.md"),
                    "msg = {msg}"
                );
                assert!(msg.contains("references="), "msg = {msg}");
            },
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for colliding reference URIs"),
        }
    }

    // ── Test 1.9 ──────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_9_skills_handler_list_excludes_references() {
        let s = Skill::new("a", "")
            .with_reference(SkillReference::new("references/r1.md", "text/markdown", "1"))
            .with_reference(SkillReference::new("references/r2.md", "text/markdown", "2"));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        let skill_md_count = list
            .resources
            .iter()
            .filter(|r| r.uri == "skill://a/SKILL.md")
            .count();
        assert_eq!(skill_md_count, 1);
        for r in &list.resources {
            assert!(!r.uri.contains("/references/"), "leaked: {}", r.uri);
        }
        let index_count = list
            .resources
            .iter()
            .filter(|r| r.uri == "skill://index.json")
            .count();
        assert_eq!(index_count, 1);
    }

    // ── Test 1.10 (wire shape SKILL.md) ───────────────────────────────
    #[tokio::test]
    async fn test_1_10_skills_handler_read_skill_md_returns_resource_with_text() {
        let handler = Skills::new()
            .add(Skill::new("a", "the body"))
            .into_handler()
            .unwrap();
        let res = handler
            .read("skill://a/SKILL.md", extra())
            .await
            .unwrap();
        assert_eq!(res.contents.len(), 1);
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://a/SKILL.md");
                assert_eq!(text.as_deref(), Some("the body"));
                assert_eq!(mime_type.as_deref(), Some("text/markdown"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.11 (wire shape reference) ──────────────────────────────
    #[tokio::test]
    async fn test_1_11_skills_handler_read_reference_carries_per_resource_mime() {
        let s = Skill::new("a", "").with_reference(SkillReference::new(
            "references/schema.graphql",
            "application/graphql",
            "schema { query: Q }",
        ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let res = handler
            .read("skill://a/references/schema.graphql", extra())
            .await
            .unwrap();
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://a/references/schema.graphql");
                assert_eq!(text.as_deref(), Some("schema { query: Q }"));
                assert_eq!(mime_type.as_deref(), Some("application/graphql"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.12 (wire shape index) ──────────────────────────────────
    #[tokio::test]
    async fn test_1_12_skills_handler_read_index_returns_resource_with_text() {
        let s = Skill::new("a", "").with_reference(SkillReference::new(
            "references/r.md",
            "text/markdown",
            "x",
        ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let res = handler
            .read("skill://index.json", extra())
            .await
            .unwrap();
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://index.json");
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let parsed: serde_json::Value =
                    serde_json::from_str(text.as_deref().unwrap()).unwrap();
                assert!(parsed.get("$schema").is_some());
                assert!(parsed.get("skills").is_some());
                let arr = parsed["skills"].as_array().unwrap();
                assert_eq!(arr.len(), 1);
                // Reference entries MUST NOT appear in the discovery index.
                let serialized = serde_json::to_string(&parsed).unwrap();
                assert!(!serialized.contains("references/r.md"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.13 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_13_skills_handler_read_unknown_uri_method_not_found() {
        let handler = Skills::new()
            .add(Skill::new("a", "body"))
            .into_handler()
            .unwrap();
        let err = handler
            .read("skill://nonexistent/SKILL.md", extra())
            .await
            .expect_err("unknown URI must error");
        match err {
            Error::Protocol { code, .. } => assert_eq!(code, ErrorCode::METHOD_NOT_FOUND),
            other => panic!("expected Protocol, got {other:?}"),
        }

        let err = handler
            .read("skill://a/references/missing.md", extra())
            .await
            .expect_err("unknown reference must error");
        match err {
            Error::Protocol { code, .. } => assert_eq!(code, ErrorCode::METHOD_NOT_FOUND),
            other => panic!("expected Protocol, got {other:?}"),
        }
    }

    // ── Test 1.14 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_14_skill_prompt_handler_returns_byte_equal_text() {
        let skill = Skill::new("x", "A").with_reference(SkillReference::new(
            "ref1.md",
            "text/markdown",
            "refbody",
        ));
        let handler = SkillPromptHandler::new(skill.clone());
        let result = handler.handle(HashMap::new(), extra()).await.unwrap();
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::User);
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, &skill.as_prompt_text()),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 1.15 ─────────────────────────────────────────────────────
    struct DocsHandler;

    #[async_trait]
    impl ResourceHandler for DocsHandler {
        async fn read(
            &self,
            uri: &str,
            _extra: RequestHandlerExtra,
        ) -> Result<ReadResourceResult> {
            Ok(ReadResourceResult::new(vec![Content::text(format!(
                "DOCS:{uri}"
            ))]))
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<ListResourcesResult> {
            Ok(ListResourcesResult::new(vec![ResourceInfo::new(
                "docs://handbook",
                "handbook",
            )]))
        }
    }

    #[tokio::test]
    async fn test_1_15_composed_resources_uri_prefix_routing() {
        let skills: Arc<dyn ResourceHandler> = Skills::new()
            .add(Skill::new("a", "skill-a"))
            .into_handler()
            .unwrap();
        let other: Arc<dyn ResourceHandler> = Arc::new(DocsHandler);
        let composed = ComposedResources { skills, other };

        let res = composed
            .read("skill://a/SKILL.md", extra())
            .await
            .unwrap();
        match &res.contents[0] {
            Content::Resource { uri, .. } => assert_eq!(uri, "skill://a/SKILL.md"),
            other => panic!("expected Content::Resource, got {other:?}"),
        }

        let res = composed.read("docs://handbook", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:docs://handbook"),
            other => panic!("expected Content::Text, got {other:?}"),
        }

        let res = composed.read("ftp://foo", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:ftp://foo"),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 1.16 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_16_composed_resources_list_concatenates_skills_first() {
        let skills: Arc<dyn ResourceHandler> = Skills::new()
            .add(Skill::new("a", ""))
            .into_handler()
            .unwrap();
        let other: Arc<dyn ResourceHandler> = Arc::new(DocsHandler);
        let composed = ComposedResources { skills, other };
        let list = composed.list(None, extra()).await.unwrap();
        // Skills first (SKILL.md + index = 2), then other (1).
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://index.json");
        assert_eq!(list.resources[2].uri, "docs://handbook");
    }

    // ── Test 1.17 (property: no reference ever listed) ────────────────
    fn skill_strategy() -> impl Strategy<Value = Skill> {
        let name = "[a-z]{1,8}";
        let ref_strategy = (
            "ref_[a-z]{1,6}\\.md",
            Just("text/markdown".to_string()),
            "[a-zA-Z]{1,12}",
        )
            .prop_map(|(p, m, b)| SkillReference::new(p, m, b));
        (name, "[a-zA-Z]{0,20}", proptest::collection::vec(ref_strategy, 0..=5)).prop_map(
            |(name, body, refs)| {
                let mut s = Skill::new(name, body);
                // De-duplicate within a single skill by relative_path.
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                for r in refs {
                    if seen.insert(r.relative_path().to_string()) {
                        s = s.with_reference(r);
                    }
                }
                s
            },
        )
    }

    // Strategy that preserves references but uniquifies skill paths so
    // every reference URI is globally unique.
    fn skills_strategy_with_refs() -> impl Strategy<Value = Vec<Skill>> {
        proptest::collection::vec(skill_strategy(), 1..=10).prop_map(|skills| {
            skills
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let new_path = format!("p{i}");
                    let mut rebuilt = Skill::new(s.name().to_string(), s.body().to_string())
                        .with_path(new_path)
                        .with_description(s.resolved_description());
                    for r in s.references() {
                        rebuilt = rebuilt.with_reference(SkillReference::new(
                            r.relative_path(),
                            r.mime_type(),
                            r.body(),
                        ));
                    }
                    rebuilt
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn prop_1_17_no_reference_ever_listed(skills in skills_strategy_with_refs()) {
            let mut registry = Skills::new();
            for s in skills {
                registry = registry.add(s);
            }
            let handler = match registry.into_handler() {
                Ok(h) => h,
                // Skip inputs that produce duplicate URIs — covered by Test 1.18.
                Err(_) => return Ok(()),
            };
            let rt = tokio::runtime::Runtime::new().unwrap();
            let list = rt.block_on(handler.list(None, RequestHandlerExtra::default())).unwrap();
            for r in &list.resources {
                prop_assert!(!r.uri.contains("/references/"), "leaked: {}", r.uri);
            }
        }
    }

    // ── Test 1.18 (property: duplicate URI always rejected) ───────────
    proptest! {
        #[test]
        fn prop_1_18_duplicate_uri_always_rejected(
            name in "[a-z]{1,6}",
            body_a in "[a-zA-Z]{0,12}",
            body_b in "[a-zA-Z]{0,12}",
        ) {
            // Same name → same skill_md_uri → always Err.
            let result = Skills::new()
                .add(Skill::new(name.clone(), body_a))
                .add(Skill::new(name, body_b))
                .into_handler();
            prop_assert!(result.is_err());
        }

        #[test]
        fn prop_1_18b_distinct_names_always_ok(
            name_a in "[a-z]{1,6}",
            name_b in "[a-z]{7,12}",
        ) {
            // Disjoint name lengths guarantee distinct names.
            prop_assume!(name_a != name_b);
            let result = Skills::new()
                .add(Skill::new(name_a, ""))
                .add(Skill::new(name_b, ""))
                .into_handler();
            prop_assert!(result.is_ok());
        }
    }

    // ── Test 1.19 (property: as_prompt_text byte-equal concat) ────────
    proptest! {
        #[test]
        fn prop_1_19_as_prompt_text_byte_equal_concat(skill in skill_strategy()) {
            // Manually concatenate the expected output.
            let mut expected = String::new();
            expected.push_str(skill.body());
            if !skill.body().ends_with('\n') {
                expected.push('\n');
            }
            for r in skill.references() {
                expected.push_str("\n--- ");
                expected.push_str(r.relative_path());
                expected.push_str(" ---\n");
                expected.push_str(r.body());
                if !r.body().ends_with('\n') {
                    expected.push('\n');
                }
            }
            prop_assert_eq!(skill.as_prompt_text(), expected);
        }
    }

    // ── Test 1.19a (property: read responses always have URI + MIME) ──
    proptest! {
        #[test]
        fn prop_1_19a_read_responses_always_have_uri_and_mime(skills in skills_strategy_with_refs()) {
            let mut registry = Skills::new();
            for s in skills.clone() {
                registry = registry.add(s);
            }
            let handler = match registry.into_handler() {
                Ok(h) => h,
                Err(_) => return Ok(()),
            };
            let rt = tokio::runtime::Runtime::new().unwrap();
            // Collect every URI: SKILL.md, references, index.
            let mut uris: Vec<String> = vec!["skill://index.json".to_string()];
            for s in &skills {
                uris.push(s.skill_md_uri());
                for r in s.references() {
                    uris.push(s.reference_uri(r.relative_path()));
                }
            }
            for uri in uris {
                let res = match rt.block_on(handler.read(&uri, RequestHandlerExtra::default())) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                prop_assert_eq!(res.contents.len(), 1);
                match &res.contents[0] {
                    Content::Resource { uri: u, text, mime_type, .. } => {
                        prop_assert_eq!(u, &uri);
                        prop_assert!(text.is_some(), "text missing for {}", uri);
                        prop_assert!(mime_type.is_some(), "mime missing for {}", uri);
                    },
                    other => prop_assert!(false, "expected Content::Resource, got {:?}", other),
                }
            }
        }
    }

    // ── Test 1.20 (with_reference validation panics) ──────────────────
    #[test]
    #[should_panic(expected = "must not be empty")]
    fn test_1_20_with_reference_panic_empty() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "SKILL.md")]
    fn test_1_20_with_reference_panic_skill_md_collision() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "SKILL.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "..")]
    fn test_1_20_with_reference_panic_dotdot() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "../escape.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "leading")]
    fn test_1_20_with_reference_panic_absolute() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "/abs/path.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "URI scheme")]
    fn test_1_20_with_reference_panic_scheme() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "http://example.com/x",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn test_1_20_with_reference_panic_duplicate_within_skill() {
        let _ = Skill::new("x", "b")
            .with_reference(SkillReference::new("a.md", "text/markdown", "body1"))
            .with_reference(SkillReference::new("a.md", "text/markdown", "body2"));
    }

    // ── Test 1.20a (try_with_reference returns Err) ───────────────────
    #[test]
    fn test_1_20a_try_with_reference_returns_err() {
        let invalid = [
            "",
            "SKILL.md",
            "../escape.md",
            "/abs/path.md",
            "http://example.com/x",
        ];
        for p in invalid {
            let res = Skill::new("x", "b").try_with_reference(SkillReference::new(
                p,
                "text/markdown",
                "body",
            ));
            assert!(res.is_err(), "expected Err for path = {p:?}");
            assert!(matches!(res.unwrap_err(), Error::Validation(_)));
        }
        // Duplicate within skill.
        let res = Skill::new("x", "b")
            .try_with_reference(SkillReference::new("a.md", "text/markdown", "1"))
            .and_then(|s| {
                s.try_with_reference(SkillReference::new("a.md", "text/markdown", "2"))
            });
        assert!(res.is_err());

        // Ok case.
        let res = Skill::new("x", "b").try_with_reference(SkillReference::new(
            "references/ok.md",
            "text/markdown",
            "body",
        ));
        assert!(res.is_ok());
    }

    // ── Test 1.21 (Skills::merge) ─────────────────────────────────────
    #[tokio::test]
    async fn test_1_21_skills_merge_concatenates() {
        let combined = Skills::new()
            .add(Skill::new("a", ""))
            .merge(Skills::new().add(Skill::new("b", "")));
        let handler = combined.into_handler().unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
        assert_eq!(list.resources[2].uri, "skill://index.json");
    }
}
