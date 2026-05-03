//! MCP App metadata validation for tools and resources.
//!
//! Validates that App-capable tools have correct `_meta` structure,
//! cross-references with `resources/list`, and (in ChatGPT mode)
//! checks for `openai/*` keys.

use crate::report::{TestCategory, TestResult, TestStatus};
use pmcp::types::ui::CHATGPT_DESCRIPTOR_KEYS;
use pmcp::types::{ResourceInfo, ToolInfo};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

/// Valid MIME types for MCP App resources.
const APP_MIME_TYPES: &[&str] = &[
    "text/html",
    "text/html+mcp",
    "text/html+skybridge",
    "text/html;profile=mcp-app",
];

// =====================================================================
// REGEX ACCESSORS — compile-once via OnceLock. Each accessor is cog 1.
// All regex literals are static, so .unwrap() is safe at runtime.
//
// `#[allow(dead_code)]` is applied to the regex accessors and scanner
// helpers because `mcp-tester` is a lib + bin crate and `src/main.rs`
// includes `mod app_validator;` directly. The bin currently does NOT
// invoke `AppValidator::validate_widgets` (Plan 02 wires it via
// `cargo pmcp test apps`). Until Plan 02 lands, the bin sees these
// helpers as transitively dead. The lib + tests both exercise them
// (the new public API is consumed by the unit-test mod).
// =====================================================================

#[allow(dead_code)]
fn script_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?is)<script(?P<attrs>[^>]*)>(?P<body>[\s\S]*?)</script>"#).unwrap()
    })
}

#[allow(dead_code)]
fn ext_apps_import_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"@modelcontextprotocol/ext-apps").unwrap())
}

#[allow(dead_code)]
fn ext_apps_log_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // G1: SDK runtime log prefix. Survives Vite singlefile minification because
    // it's a bracketed string literal in console.log calls inside the inlined
    // SDK runtime (e.g. `[ext-apps] App.${e}() called before connect`).
    // Source: cost-coach prod feedback 2026-05-02.
    RE.get_or_init(|| Regex::new(r"\[ext-apps\]").unwrap())
}

#[allow(dead_code)]
fn ui_initialize_method_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // G1: JSON-RPC method literal. Protocol-level identifier — minifiers
    // never rename quoted method-name strings.
    RE.get_or_init(|| Regex::new(r"ui/initialize").unwrap())
}

#[allow(dead_code)]
fn ui_tool_result_method_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // G1: JSON-RPC method literal. Same rationale as ui/initialize.
    RE.get_or_init(|| Regex::new(r"ui/notifications/tool-result").unwrap())
}

#[allow(dead_code)]
fn app_constructor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // G2 cycle-2 generalization (Plan 78-10): matches `new <id>({<prefix>
    // name:<value>,<...>version:<value>...})` where:
    //   - `<id>` is any 1-21 char JS identifier (mangled or unmangled)
    //   - `<value>` is any expression up to 100 chars (literal string,
    //     concatenation, function call, template literal, etc.)
    //   - `<prefix>` is any non-`}` content up to 200 chars
    //
    // Real-prod motivation (cost-coach @ 29f46efd, all 6 widgets):
    //   `new yl({name:"cost-coach-"+t,version:"1.0.0"})` — string
    //   concatenation, NOT a literal. The cycle-1 regex required
    //   `name:"<lit>",version:"<lit>"` and missed all 8 prod widgets per
    //   uat-evidence/2026-05-02-cost-coach-prod-rerun.md.
    //
    // Bounded character classes (`{0,200}`, `{0,100}`) prevent catastrophic
    // backtracking on adversarial input from the fuzz target's
    // `\PC{0,4096}` alphabet.
    //
    // Continues to match cycle-1 unit-test positives:
    //   new yl({name:"cost-coach-cost-summary",version:"1.0.0"})
    //   new App({name: "tool", version: "1.0.0"})
    // And NOT match cycle-1 negatives:
    //   new Date(2026,1,1)         (no `{...}` payload)
    //   new URL("http://x")        (no object payload at all)
    //   new Foo({foo:1, bar:2})    (no name/version keys)
    RE.get_or_init(|| {
        Regex::new(
            r"new [a-zA-Z_$][a-zA-Z0-9_$]{0,20}\(\s*\{[^}]{0,200}\bname\s*:[^,}]{0,100},\s*version\s*:",
        )
        .unwrap()
    })
}

#[allow(dead_code)]
fn handler_onteardown_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*onteardown\s*=").unwrap())
}

#[allow(dead_code)]
fn handler_ontoolinput_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*ontoolinput\s*=").unwrap())
}

#[allow(dead_code)]
fn handler_ontoolcancelled_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*ontoolcancelled\s*=").unwrap())
}

#[allow(dead_code)]
fn handler_onerror_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*onerror\s*=").unwrap())
}

#[allow(dead_code)]
fn handler_ontoolresult_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*ontoolresult\s*=").unwrap())
}

#[allow(dead_code)]
fn connect_call_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\.\s*connect\s*\(").unwrap())
}

#[allow(dead_code)]
fn chatgpt_only_channels_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"window\.openai|window\.mcpBridge").unwrap())
}

// REVISION HIGH-3: comment-stripping regexes. Stripped BEFORE signal sweeps so
// commented-out scaffolding code containing signal literals does not produce
// false positives.
#[allow(dead_code)]
fn html_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<!--.*?-->").unwrap())
}

#[allow(dead_code)]
fn js_block_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)/\*.*?\*/").unwrap())
}

#[allow(dead_code)]
fn js_line_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Match `//` to end of line. Best-effort: this regex does NOT understand
    // string-literal context, so `// inside a "string"` could in theory be
    // stripped incorrectly. See `strip_js_comments` docstring for the
    // accepted simplification.
    RE.get_or_init(|| Regex::new(r"//[^\r\n]*").unwrap())
}

/// Static-scan signals for one widget body. Pure data, no methods.
/// Visibility: `pub(crate)` — internal scanner state can change without
/// breaking downstream consumers (RESEARCH Open Question 3 RESOLVED).
#[allow(dead_code)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct WidgetSignals {
    // === SDK-presence signals (G1: any one suffices) ===
    /// Legacy: literal `@modelcontextprotocol/ext-apps` import. Survives
    /// in unminified source (Plan 07 `--widgets-dir` path). Vite
    /// singlefile inlines it away in production bundles.
    has_ext_apps_import: bool,
    /// G1: `[ext-apps]` log-prefix string inside the inlined SDK runtime.
    /// Survives minification because bracketed string literals inside
    /// console.log calls are not renamed.
    has_log_prefix: bool,
    /// G1: `ui/initialize` JSON-RPC method literal. Protocol-level
    /// identifier; minifiers never rename quoted method-name strings.
    has_method_initialize: bool,
    /// G1: `ui/notifications/tool-result` JSON-RPC method literal.
    has_method_tool_result: bool,
    /// G1 verdict: OR of the four SDK-presence signals above. This is the
    /// field consumed by emission helpers (G3 cascade-free).
    has_sdk: bool,
    // === Constructor signal (G2 — mangled-id-tolerant) ===
    /// G2: `true` when a `new <id>({name:"...",version:"..."})` call is
    /// found. Identifier `<id>` may be any 1-6 char JS identifier
    /// (mangled by Vite singlefile or the literal `App`).
    has_app_constructor: bool,
    // === Connect + handlers (G3 — independent of has_sdk) ===
    has_connect: bool,
    has_chatgpt_only_channels: bool,
    /// Each entry is the handler name found (e.g. "onteardown"). Order is
    /// stable because the four handlers are tested in a fixed order.
    /// `ontoolresult` is NOT in this vec — it's a separate field for the
    /// WARN-tier check.
    handlers_present: Vec<&'static str>,
    /// G3: derived — `true` when any handler member-assignment matched (i.e.
    /// `!handlers_present.is_empty()`). Computed in `scan_widget` from the
    /// independent member-name regexes; INDEPENDENT of `has_sdk`.
    has_handlers: bool,
    has_ontoolresult: bool,
}

/// Best-effort comment stripper. Strips HTML, JS block, and JS line comments
/// from `src` while preserving comment delimiters that appear INSIDE JS
/// string literals.
///
/// Cycle-2 fix (Plan 78-10): the cycle-1 implementation used unconditional
/// regex replacement, which destroyed thousands of bytes of code in
/// cost-coach prod bundles where a JS string literal contained
/// `"/*.example.com..."` (a CSP frame-src directive value) — the regex saw
/// that `/*` as a block-comment opener and matched the next `*/` it found,
/// far away. See `tests/fixtures/widgets/bundled/real-prod/CAPTURE.md` for
/// the step-by-step probe evidence.
///
/// State machine: outside-strings (`out`) → block_comment / line_comment /
/// single_string / double_string / template_string. Inside any string state,
/// `/*` and `//` are NOT comment delimiters and are preserved in the output.
/// Escape sequences (`\<any>`) are consumed as a unit inside string states.
///
/// HTML comments are stripped by the existing `html_comment_re` regex up
/// front (HTML comments do not appear inside JS string literals in any
/// realistic bundle).
///
/// Accepted simplification: template-literal interpolations `${...}` are
/// NOT recursively parsed — comment delimiters inside an interpolation
/// expression are still treated as part of the template string body. Real
/// prod bundles rarely embed signal literals inside template
/// interpolations, and full recursive parsing would require tracking
/// brace depth. The property test exercises arbitrary input.
#[allow(dead_code)]
fn strip_js_comments(src: &str) -> String {
    // Strip HTML comments first; the JS state machine then walks pure JS.
    let after_html = html_comment_re().replace_all(src, "");

    let bytes = after_html.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    let n = bytes.len();

    // States:
    //   0 = outside strings/comments
    //   1 = inside /* ... */ block comment
    //   2 = inside // line comment
    //   3 = inside '...' single-quoted string
    //   4 = inside "..." double-quoted string
    //   5 = inside `...` template-literal string
    let mut state: u8 = 0;

    while i < n {
        let c = bytes[i];
        match state {
            0 => {
                if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
                    state = 1;
                    i += 2;
                } else if c == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
                    state = 2;
                    i += 2;
                } else if c == b'\'' {
                    state = 3;
                    out.push(c as char);
                    i += 1;
                } else if c == b'"' {
                    state = 4;
                    out.push(c as char);
                    i += 1;
                } else if c == b'`' {
                    state = 5;
                    out.push(c as char);
                    i += 1;
                } else {
                    out.push(c as char);
                    i += 1;
                }
            },
            1 => {
                if c == b'*' && i + 1 < n && bytes[i + 1] == b'/' {
                    state = 0;
                    i += 2;
                } else {
                    i += 1;
                }
            },
            2 => {
                if c == b'\n' || c == b'\r' {
                    state = 0;
                    out.push(c as char);
                    i += 1;
                } else {
                    i += 1;
                }
            },
            3..=5 => {
                if c == b'\\' && i + 1 < n {
                    out.push(c as char);
                    out.push(bytes[i + 1] as char);
                    i += 2;
                } else {
                    let close = match state {
                        3 => b'\'',
                        4 => b'"',
                        5 => b'`',
                        _ => unreachable!(),
                    };
                    if c == close {
                        state = 0;
                    }
                    out.push(c as char);
                    i += 1;
                }
            },
            _ => unreachable!(),
        }
    }

    out
}

/// Concatenate the bodies of all inline `<script>` tags except those with
/// `type="application/json"` or `src=` attribute. Strips JS/HTML comments
/// from each body (REVISION HIGH-3) before concatenation. Returns a single
/// String for downstream regex sweeps.
#[allow(dead_code)]
fn extract_inline_scripts(html: &str) -> String {
    let mut out = String::new();
    for cap in script_block_re().captures_iter(html) {
        let attrs = cap.name("attrs").map_or("", |m| m.as_str());
        if attrs.contains("application/json") || attrs.contains("src=") {
            continue;
        }
        if let Some(body) = cap.name("body") {
            // REVISION HIGH-3: strip comments BEFORE adding to `out` so the
            // signal regexes never see commented-out signal literals.
            let stripped = strip_js_comments(body.as_str());
            out.push_str(&stripped);
            out.push('\n');
        }
    }
    out
}

/// Run the regex sweep over a widget body and return signal flags.
#[allow(dead_code)]
fn scan_widget(html: &str) -> WidgetSignals {
    let scripts = extract_inline_scripts(html);
    let mut handlers_present: Vec<&'static str> = Vec::new();
    if handler_onteardown_re().is_match(&scripts) {
        handlers_present.push("onteardown");
    }
    if handler_ontoolinput_re().is_match(&scripts) {
        handlers_present.push("ontoolinput");
    }
    if handler_ontoolcancelled_re().is_match(&scripts) {
        handlers_present.push("ontoolcancelled");
    }
    if handler_onerror_re().is_match(&scripts) {
        handlers_present.push("onerror");
    }
    // G1: four independent SDK-presence signals (any one suffices).
    let has_ext_apps_import = ext_apps_import_re().is_match(&scripts);
    let has_log_prefix = ext_apps_log_prefix_re().is_match(&scripts);
    let has_method_initialize = ui_initialize_method_re().is_match(&scripts);
    let has_method_tool_result = ui_tool_result_method_re().is_match(&scripts);
    let has_sdk =
        has_ext_apps_import || has_log_prefix || has_method_initialize || has_method_tool_result;
    let has_handlers = !handlers_present.is_empty();
    WidgetSignals {
        has_ext_apps_import,
        has_log_prefix,
        has_method_initialize,
        has_method_tool_result,
        has_sdk,
        has_app_constructor: app_constructor_re().is_match(&scripts),
        has_connect: connect_call_re().is_match(&scripts),
        has_chatgpt_only_channels: chatgpt_only_channels_re().is_match(&scripts),
        handlers_present,
        has_handlers,
        has_ontoolresult: handler_ontoolresult_re().is_match(&scripts),
    }
}

/// Validation mode controlling which keys are checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppValidationMode {
    /// Standard mode: nested `ui.resourceUri` only.
    Standard,
    /// ChatGPT mode: also checks `openai/*` keys and flat `ui/resourceUri`.
    ChatGpt,
    /// Claude Desktop mode: strictly validates widget HTML for MCP Apps SDK
    /// wiring (`@modelcontextprotocol/ext-apps` import or >=3 of 4 handler
    /// property assignments, `new App({...})` constructor, four required
    /// handlers — `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror` —
    /// and `app.connect()`). Missing signals emit one `TestStatus::Failed`
    /// row per signal (full breakdown).
    ///
    /// Per-mode widget validation emission shape (THREE-WAY split per
    /// RESEARCH Q4 RESOLVED):
    ///   * `ClaudeDesktop` — per-signal Failed rows (this variant)
    ///   * `Standard` — ONE summary Warning row per widget
    ///   * `ChatGpt` — ZERO widget-related rows (preserves AC-78-4
    ///     "chatgpt mode unchanged")
    ///
    /// See `validate_widgets`.
    ClaudeDesktop,
}

impl std::fmt::Display for AppValidationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::ChatGpt => write!(f, "chatgpt"),
            Self::ClaudeDesktop => write!(f, "claude-desktop"),
        }
    }
}

impl std::str::FromStr for AppValidationMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard" => Ok(Self::Standard),
            "chatgpt" => Ok(Self::ChatGpt),
            "claude-desktop" => Ok(Self::ClaudeDesktop),
            other => Err(format!(
                "Unknown validation mode: '{other}'. Valid: standard, chatgpt, claude-desktop"
            )),
        }
    }
}

/// Validates MCP App metadata on tools discovered via `tools/list`.
pub struct AppValidator {
    mode: AppValidationMode,
    tool_filter: Option<String>,
}

impl AppValidator {
    /// Create a new `AppValidator`.
    pub fn new(mode: AppValidationMode, tool_filter: Option<String>) -> Self {
        Self { mode, tool_filter }
    }

    /// Main entry point: validate all (or filtered) App-capable tools.
    pub fn validate_tools(
        &self,
        tools: &[ToolInfo],
        resources: &[ResourceInfo],
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        let app_tools: Vec<&ToolInfo> = tools
            .iter()
            .filter(|t| {
                if let Some(ref filter) = self.tool_filter {
                    t.name == *filter
                } else {
                    Self::is_app_capable(t)
                }
            })
            .collect();

        if app_tools.is_empty() {
            return results;
        }

        for tool in &app_tools {
            let uri = Self::extract_resource_uri(tool);
            results.extend(self.validate_tool_meta(tool, uri.as_deref()));

            if let Some(ref uri) = uri {
                results.extend(self.validate_resource_match(&tool.name, uri, resources));
            }

            if self.mode == AppValidationMode::ChatGpt {
                if let Some(ref meta) = tool._meta {
                    results.extend(self.validate_chatgpt_keys(&tool.name, meta));
                }
            }

            if let Some(ref schema) = tool.output_schema {
                results.extend(self.validate_output_schema(&tool.name, schema));
            }
        }

        results
    }

    /// Returns `true` if the tool has App metadata (nested or flat `resourceUri`).
    pub fn is_app_capable(tool: &ToolInfo) -> bool {
        Self::extract_resource_uri(tool).is_some()
    }

    /// Extract the resource URI from either nested `ui.resourceUri` or flat `ui/resourceUri`.
    ///
    /// Public so cargo-pmcp's `read_widget_bodies` plumbing in
    /// `commands/test/apps.rs` can derive widget URIs from tool metadata.
    pub fn extract_resource_uri(tool: &ToolInfo) -> Option<String> {
        let meta = tool._meta.as_ref()?;

        // Nested: _meta.ui.resourceUri
        if let Some(Value::Object(ui)) = meta.get("ui") {
            if let Some(Value::String(uri)) = ui.get("resourceUri") {
                return Some(uri.clone());
            }
        }

        // Flat legacy: _meta["ui/resourceUri"]
        if let Some(Value::String(uri)) = meta.get("ui/resourceUri") {
            return Some(uri.clone());
        }

        None
    }

    /// Validate the tool's `_meta` structure for App keys.
    fn validate_tool_meta(&self, tool: &ToolInfo, uri: Option<&str>) -> Vec<TestResult> {
        let mut results = Vec::new();
        let tool_name = &tool.name;

        if tool._meta.is_none() {
            results.push(TestResult {
                name: format!("[{tool_name}] _meta present"),
                category: TestCategory::Apps,
                status: TestStatus::Failed,
                duration: Duration::from_secs(0),
                error: Some("Tool has no _meta field".to_string()),
                details: None,
            });
            return results;
        }

        match uri {
            Some(uri) => {
                results.push(TestResult {
                    name: format!("[{tool_name}] ui.resourceUri present"),
                    category: TestCategory::Apps,
                    status: TestStatus::Passed,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: None,
                });

                // Validate URI format (non-empty with scheme separator)
                if uri.is_empty() || !uri.contains("://") {
                    results.push(TestResult {
                        name: format!("[{tool_name}] resourceUri format"),
                        category: TestCategory::Apps,
                        status: TestStatus::Warning,
                        duration: Duration::from_secs(0),
                        error: None,
                        details: Some(format!(
                            "URI may not be well-formed: '{uri}' (no scheme separator)"
                        )),
                    });
                } else {
                    results.push(TestResult {
                        name: format!("[{tool_name}] resourceUri format"),
                        category: TestCategory::Apps,
                        status: TestStatus::Passed,
                        duration: Duration::from_secs(0),
                        error: None,
                        details: Some(format!("URI: {uri}")),
                    });
                }
            },
            None => {
                results.push(TestResult {
                    name: format!("[{tool_name}] ui.resourceUri present"),
                    category: TestCategory::Apps,
                    status: TestStatus::Failed,
                    duration: Duration::from_secs(0),
                    error: Some(
                        "_meta exists but missing ui.resourceUri (nested or flat)".to_string(),
                    ),
                    details: None,
                });
            },
        }

        results
    }

    /// Cross-reference a tool's resource URI against the resources list.
    fn validate_resource_match(
        &self,
        tool_name: &str,
        resource_uri: &str,
        resources: &[ResourceInfo],
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        let matching = resources.iter().find(|r| r.uri == resource_uri);

        match matching {
            None => {
                results.push(TestResult {
                    name: format!("[{tool_name}] resource cross-reference"),
                    category: TestCategory::Apps,
                    status: TestStatus::Warning,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: Some(format!(
                        "No resource found with URI '{resource_uri}' in resources/list"
                    )),
                });
            },
            Some(resource) => {
                results.push(TestResult {
                    name: format!("[{tool_name}] resource cross-reference"),
                    category: TestCategory::Apps,
                    status: TestStatus::Passed,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: Some(format!("Found resource: {}", resource.name)),
                });

                // Validate MIME type
                match &resource.mime_type {
                    None => {
                        results.push(TestResult {
                            name: format!("[{tool_name}] resource MIME type"),
                            category: TestCategory::Apps,
                            status: TestStatus::Warning,
                            duration: Duration::from_secs(0),
                            error: None,
                            details: Some("Resource has no MIME type set".to_string()),
                        });
                    },
                    Some(mime) => {
                        let is_valid = APP_MIME_TYPES.iter().any(|v| mime.eq_ignore_ascii_case(v));

                        if is_valid {
                            results.push(TestResult {
                                name: format!("[{tool_name}] resource MIME type"),
                                category: TestCategory::Apps,
                                status: TestStatus::Passed,
                                duration: Duration::from_secs(0),
                                error: None,
                                details: Some(format!("MIME type: {mime}")),
                            });
                        } else {
                            results.push(TestResult {
                                name: format!("[{tool_name}] resource MIME type"),
                                category: TestCategory::Apps,
                                status: TestStatus::Warning,
                                duration: Duration::from_secs(0),
                                error: None,
                                details: Some(format!(
                                    "Unexpected MIME type '{mime}', expected one of: {}",
                                    APP_MIME_TYPES.join(", ")
                                )),
                            });
                        }
                    },
                }
            },
        }

        results
    }

    /// Validate ChatGPT-specific `openai/*` keys in tool metadata.
    fn validate_chatgpt_keys(
        &self,
        tool_name: &str,
        meta: &serde_json::Map<String, Value>,
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        for key in CHATGPT_DESCRIPTOR_KEYS {
            let present = meta.get(*key).is_some();

            results.push(TestResult {
                name: format!("[{tool_name}] ChatGPT key: {key}"),
                category: TestCategory::Apps,
                status: if present {
                    TestStatus::Passed
                } else {
                    TestStatus::Warning
                },
                duration: Duration::from_secs(0),
                error: None,
                details: if present {
                    None
                } else {
                    Some(format!("Missing ChatGPT key: {key}"))
                },
            });
        }

        // Also check flat legacy key ui/resourceUri
        let has_flat = meta.get("ui/resourceUri").is_some();

        results.push(TestResult {
            name: format!("[{tool_name}] ChatGPT flat ui/resourceUri"),
            category: TestCategory::Apps,
            status: if has_flat {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if has_flat {
                None
            } else {
                Some("Missing flat legacy key ui/resourceUri (needed for ChatGPT)".to_string())
            },
        });

        results
    }

    /// Validate the `outputSchema` structure on a tool.
    fn validate_output_schema(&self, tool_name: &str, schema: &Value) -> Vec<TestResult> {
        let mut results = Vec::new();

        let is_valid = schema.is_object() && schema.get("type").is_some();

        results.push(TestResult {
            name: format!("[{tool_name}] outputSchema structure"),
            category: TestCategory::Apps,
            status: if is_valid {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if is_valid {
                None
            } else {
                Some("outputSchema should be an object with a 'type' field".to_string())
            },
        });

        results
    }

    // =====================================================================
    // Plan 78-01 Task 2 — widget HTML validation (mode-driven emission)
    // =====================================================================

    /// Validate inline widget HTML for Claude Desktop / MCP Apps SDK wiring.
    ///
    /// Pure function: takes already-fetched widget bodies and returns
    /// `TestResult`s.
    ///
    /// Each tuple is `(tool_name, uri, html)`. The tool name is included in
    /// emitted `TestResult.name` strings so error reports identify which tool
    /// the widget belongs to (REVISION HIGH-4). Plan 02 applies `tool_filter`
    /// at the read site, so the bodies passed here are already filtered.
    ///
    /// Mode-driven emission shape (per RESEARCH Open Question 4 RESOLVED —
    /// THREE-WAY split):
    /// - `ClaudeDesktop` — emits ONE `TestStatus::Failed` row per missing
    ///   signal/handler (full breakdown so each error is independently
    ///   actionable). Pre-deploy gate.
    /// - `Standard` — emits ONE summary `TestStatus::Warning` row per widget
    ///   that lists which signals/handlers are missing in the `details`
    ///   field. The "permissive default" intent.
    /// - `ChatGpt` — returns an EMPTY `Vec<TestResult>`. Preserves AC-78-4
    ///   "chatgpt mode unchanged" (REVISION HIGH-1). The widget-validation
    ///   surface did not exist before this phase, so the only correct
    ///   preservation is no new rows.
    #[allow(dead_code)]
    pub fn validate_widgets(&self, widget_bodies: &[(String, String, String)]) -> Vec<TestResult> {
        // REVISION HIGH-1: ChatGpt is a no-op for widget validation. Bail
        // before scanning so the function does no work in that mode.
        if matches!(self.mode, AppValidationMode::ChatGpt) {
            return Vec::new();
        }
        let mut results = Vec::new();
        for (tool_name, uri, html) in widget_bodies {
            let signals = scan_widget(html);
            match self.mode {
                AppValidationMode::ClaudeDesktop => {
                    results.extend(self.emit_results_for_claude_desktop(tool_name, uri, &signals));
                },
                AppValidationMode::Standard => {
                    if let Some(summary) =
                        self.emit_summary_warning_for_standard(tool_name, uri, &signals)
                    {
                        results.push(summary);
                    }
                },
                // Unreachable: bailed above. Kept for exhaustive match.
                AppValidationMode::ChatGpt => {},
            }
        }
        results
    }

    /// ClaudeDesktop mode: one Failed row per missing signal/handler.
    /// `ontoolresult` stays Warning (soft) regardless of mode per RESEARCH
    /// Locked Decision 3.
    #[allow(dead_code)]
    fn emit_results_for_claude_desktop(
        &self,
        tool_name: &str,
        uri: &str,
        s: &WidgetSignals,
    ) -> Vec<TestResult> {
        let mut out = Vec::new();
        // SDK presence: G1 four-signal OR computed in scan_widget.
        out.push(self.widget_result_strict(
            tool_name,
            uri,
            "MCP Apps SDK wiring",
            s.has_sdk,
            "Widget does not contain any of the four SDK-presence signals: `@modelcontextprotocol/ext-apps` import literal, `[ext-apps]` log prefix, `ui/initialize` method literal, or `ui/notifications/tool-result` method literal. [guide:handlers-before-connect]",
        ));
        // App constructor: G2 mangled-id-tolerant regex.
        out.push(self.widget_result_strict(
            tool_name,
            uri,
            "App constructor",
            s.has_app_constructor,
            "Widget does not call `new <App>({name, version})`. Searched for any minified-id constructor (e.g. `new yl({name:..., version:...})`). [guide:handlers-before-connect]",
        ));
        // Required handlers (each is its own row so error messages name them)
        for name in ["onteardown", "ontoolinput", "ontoolcancelled", "onerror"] {
            let present = s.handlers_present.contains(&name);
            out.push(self.widget_result_strict(
                tool_name,
                uri,
                &format!("handler: {name}"),
                present,
                &format!("Widget does not register `app.{name}` before `connect()`. [guide:handlers-before-connect]"),
            ));
        }
        // ontoolresult is soft (Warning even in ClaudeDesktop)
        out.push(self.widget_ontoolresult_result(tool_name, uri, s));
        // connect()
        out.push(self.widget_result_strict(
            tool_name,
            uri,
            "connect() call",
            s.has_connect,
            "Widget does not call `app.connect()`. [guide:handlers-before-connect]",
        ));
        // ChatGPT-only channels: ERROR in ClaudeDesktop only
        if s.has_chatgpt_only_channels && !s.has_sdk && s.handlers_present.is_empty() {
            out.push(self.widget_chatgpt_only_failed(tool_name, uri));
        }
        out
    }

    /// Standard mode: ONE summary WARN row per widget listing the missing
    /// signals in the `details` field. Returns None if the widget is fully
    /// wired (zero missing signals → no summary needed).
    /// Per RESEARCH Open Question 4 RESOLVED.
    #[allow(dead_code)]
    fn emit_summary_warning_for_standard(
        &self,
        tool_name: &str,
        uri: &str,
        s: &WidgetSignals,
    ) -> Option<TestResult> {
        let mut missing: Vec<String> = Vec::new();
        if !s.has_sdk {
            missing.push(
                "MCP Apps SDK presence (any of: @modelcontextprotocol/ext-apps import, [ext-apps] log prefix, ui/initialize method, or ui/notifications/tool-result method)"
                    .to_string(),
            );
        }
        if !s.has_app_constructor {
            missing.push("App constructor (looked for `new <id>({name, version})`)".to_string());
        }
        for name in ["onteardown", "ontoolinput", "ontoolcancelled", "onerror"] {
            if !s.handlers_present.contains(&name) {
                missing.push(format!("handler: {name}"));
            }
        }
        if !s.has_connect {
            missing.push("app.connect() call".to_string());
        }
        if missing.is_empty() {
            return None;
        }
        let details = format!(
            "Widget is missing {n} required signal(s): {list}. For Claude Desktop compatibility, run `--mode claude-desktop` to see per-signal errors. [guide:handlers-before-connect]",
            n = missing.len(),
            list = missing.join(", "),
        );
        Some(TestResult {
            name: format!("[{tool_name}][{uri}] MCP Apps widget wiring (summary)"),
            category: TestCategory::Apps,
            status: TestStatus::Warning,
            duration: Duration::from_secs(0),
            error: None,
            details: Some(details),
        })
    }

    /// Build a Failed (or Passed if `present`) row for one strict-mode signal.
    /// Used only in ClaudeDesktop mode. `name` includes both tool and uri.
    #[allow(dead_code)]
    fn widget_result_strict(
        &self,
        tool_name: &str,
        uri: &str,
        label: &str,
        present: bool,
        missing_details: &str,
    ) -> TestResult {
        TestResult {
            name: format!("[{tool_name}][{uri}] {label}"),
            category: TestCategory::Apps,
            status: if present {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if present {
                None
            } else {
                Some(missing_details.to_string())
            },
        }
    }

    #[allow(dead_code)]
    fn widget_ontoolresult_result(
        &self,
        tool_name: &str,
        uri: &str,
        s: &WidgetSignals,
    ) -> TestResult {
        // ontoolresult is always soft (Warning), regardless of mode, per
        // RESEARCH Locked Decision 3 (some widgets render from
        // getHostContext().toolOutput).
        TestResult {
            name: format!("[{tool_name}][{uri}] handler: ontoolresult"),
            category: TestCategory::Apps,
            status: if s.has_ontoolresult {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if s.has_ontoolresult {
                None
            } else {
                Some("Widget does not register `app.ontoolresult` (soft warning — may render from getHostContext().toolOutput). [guide:handlers-before-connect]".to_string())
            },
        }
    }

    #[allow(dead_code)]
    fn widget_chatgpt_only_failed(&self, tool_name: &str, uri: &str) -> TestResult {
        // ChatGPT-only channels with no ext-apps wiring: ERROR in ClaudeDesktop.
        // Only called from emit_results_for_claude_desktop; never from standard.
        TestResult {
            name: format!("[{tool_name}][{uri}] chatgpt-only channels detected"),
            category: TestCategory::Apps,
            status: TestStatus::Failed,
            duration: Duration::from_secs(0),
            error: None,
            details: Some(
                "Widget uses `window.openai`/`window.mcpBridge` channels but does not wire ext-apps SDK. ChatGPT will render fine; Claude Desktop will tear down the connection. [guide:common-failures-claude]".to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_tool(name: &str, meta: Option<serde_json::Map<String, Value>>) -> ToolInfo {
        let mut tool = ToolInfo::new(name, None, json!({"type": "object"}));
        tool._meta = meta;
        tool
    }

    fn make_resource(uri: &str, mime: Option<&str>) -> ResourceInfo {
        let mut info = ResourceInfo::new(uri, uri);
        if let Some(m) = mime {
            info = info.with_mime_type(m);
        }
        info
    }

    #[test]
    fn test_is_app_capable_nested() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" }
        }))
        .unwrap();
        let tool = make_tool("t1", Some(meta));
        assert!(AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_is_app_capable_flat() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui/resourceUri": "ui://app/test"
        }))
        .unwrap();
        let tool = make_tool("t2", Some(meta));
        assert!(AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_not_app_capable() {
        let tool = make_tool("t3", None);
        assert!(!AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_validate_tools_no_app_tools() {
        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let tools = vec![make_tool("plain", None)];
        let results = validator.validate_tools(&tools, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_tools_with_resource_match() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/chess" }
        }))
        .unwrap();
        let tool = make_tool("chess", Some(meta));
        let resource = make_resource("ui://app/chess", Some("text/html"));

        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let results = validator.validate_tools(&[tool], &[resource]);

        let passed = results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count();
        assert!(
            passed >= 3,
            "Expected at least 3 passed results, got {passed}"
        );
    }

    #[test]
    fn test_chatgpt_mode_checks_openai_keys() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" },
            "openai/outputTemplate": "<div></div>"
        }))
        .unwrap();
        let tool = make_tool("t", Some(meta));

        let validator = AppValidator::new(AppValidationMode::ChatGpt, None);
        let results = validator.validate_tools(&[tool], &[]);

        let chatgpt_results: Vec<_> = results
            .iter()
            .filter(|r| r.name.contains("ChatGPT"))
            .collect();
        assert!(!chatgpt_results.is_empty());
    }

    #[test]
    fn test_strict_mode_promotes_warnings() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" }
        }))
        .unwrap();
        let tool = make_tool("t", Some(meta));

        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let mut results = validator.validate_tools(&[tool], &[]);

        // Simulate strict mode as callers do via report.apply_strict_mode()
        for r in &mut results {
            if r.status == TestStatus::Warning {
                r.status = TestStatus::Failed;
            }
        }
        let warnings = results
            .iter()
            .filter(|r| r.status == TestStatus::Warning)
            .count();
        assert_eq!(warnings, 0, "Strict mode should have zero warnings");
    }

    #[test]
    fn test_tool_filter() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/chess" }
        }))
        .unwrap();
        let tool1 = make_tool("chess", Some(meta));
        let tool2 = make_tool("other", None);

        let validator = AppValidator::new(AppValidationMode::Standard, Some("other".to_string()));
        let results = validator.validate_tools(&[tool1, tool2], &[]);

        // "other" has no _meta, so validation should report failure for it
        assert!(results.iter().any(|r| r.name.contains("other")));
        assert!(!results.iter().any(|r| r.name.contains("chess")));
    }

    // ==========================================================================
    // Task 1 (Plan 78-01) — WidgetSignals scanner + comment-stripper tests
    // ==========================================================================

    /// Wrap a list of script body snippets in <script>...</script> blocks and
    /// concatenate into a minimal HTML document. Used by widget-scanner tests.
    fn make_widget_html(snippets: &[&str]) -> String {
        let mut s = String::from("<!doctype html><html><body>");
        for snip in snippets {
            s.push_str("<script>");
            s.push_str(snip);
            s.push_str("</script>");
        }
        s.push_str("</body></html>");
        s
    }

    #[test]
    fn regexes_compile() {
        // Simply touching every accessor proves they compile and panic-free.
        let _ = script_block_re();
        let _ = ext_apps_import_re();
        let _ = ext_apps_log_prefix_re();
        let _ = ui_initialize_method_re();
        let _ = ui_tool_result_method_re();
        let _ = app_constructor_re();
        let _ = handler_onteardown_re();
        let _ = handler_ontoolinput_re();
        let _ = handler_ontoolcancelled_re();
        let _ = handler_onerror_re();
        let _ = handler_ontoolresult_re();
        let _ = connect_call_re();
        let _ = chatgpt_only_channels_re();
        let _ = html_comment_re();
        let _ = js_block_comment_re();
        let _ = js_line_comment_re();
    }

    #[test]
    fn extract_inline_scripts_concatenates() {
        let out = extract_inline_scripts("<script>A</script><script>B</script>");
        assert!(out.contains('A'), "must contain script body A: {out}");
        assert!(out.contains('B'), "must contain script body B: {out}");
    }

    #[test]
    fn extract_inline_scripts_excludes_json() {
        let html = r#"<script type="application/json">{"x":"@modelcontextprotocol/ext-apps"}</script><script>real</script>"#;
        let out = extract_inline_scripts(html);
        assert!(
            !out.contains("@modelcontextprotocol/ext-apps"),
            "JSON data island must NOT be included: {out}"
        );
        assert!(
            out.contains("real"),
            "real script body must be present: {out}"
        );
    }

    #[test]
    fn extract_inline_scripts_excludes_src() {
        // A <script src="..."></script> tag's body is empty; the filter must
        // drop the tag entirely so its (empty) body never enters output.
        let html = r#"<script src="foo.js"></script><script>inline</script>"#;
        let out = extract_inline_scripts(html);
        assert!(out.contains("inline"), "inline body must remain: {out}");
        assert!(
            !out.contains("foo.js"),
            "src attribute must NOT appear in body output: {out}"
        );
    }

    #[test]
    fn scan_widget_detects_handlers_via_property_assignment() {
        // Minified form where the App binding is renamed to `n`.
        let html = make_widget_html(&[
            r#"var n=new App({name:"x",version:"1.0.0"});n.onteardown=async()=>{};n.ontoolinput=()=>{};n.ontoolcancelled=()=>{};n.onerror=()=>{};n.connect();"#,
        ]);
        let signals = scan_widget(&html);
        assert!(signals.has_app_constructor, "must detect new App({{...}})");
        assert!(signals.has_connect, "must detect .connect()");
        assert_eq!(
            signals.handlers_present.len(),
            4,
            "must detect all 4 handlers via property-assignment regex (got {:?})",
            signals.handlers_present
        );
    }

    #[test]
    fn scan_widget_detects_import_literal() {
        let html = r#"<!doctype html><html><body><script type="module">
            import { App } from "@modelcontextprotocol/ext-apps";
            const a=new App({name:"x",version:"1"});
            a.connect();
        </script></body></html>"#;
        let signals = scan_widget(html);
        assert!(
            signals.has_ext_apps_import,
            "must detect @modelcontextprotocol/ext-apps import literal"
        );
    }

    #[test]
    fn scan_widget_detects_chatgpt_only_channels() {
        let html = make_widget_html(&[r#"window.openai.something()"#]);
        let signals = scan_widget(&html);
        assert!(
            signals.has_chatgpt_only_channels,
            "must detect window.openai usage"
        );
    }

    #[test]
    fn strip_js_comments_strips_line_comments() {
        let out = strip_js_comments("a // hidden\nb");
        assert!(
            !out.contains("hidden"),
            "line-comment text must be stripped: {out}"
        );
    }

    #[test]
    fn strip_js_comments_strips_block_comments() {
        let out = strip_js_comments("a /* hidden */ b");
        assert!(
            !out.contains("hidden"),
            "block-comment text must be stripped: {out}"
        );
        assert!(out.contains('a'), "non-comment 'a' must remain: {out}");
        assert!(out.contains('b'), "non-comment 'b' must remain: {out}");
    }

    #[test]
    fn strip_js_comments_strips_html_comments() {
        let out = strip_js_comments("<!-- hidden -->visible");
        assert!(
            !out.contains("hidden"),
            "html-comment text must be stripped: {out}"
        );
        assert!(
            out.contains("visible"),
            "non-comment 'visible' must remain: {out}"
        );
    }

    #[test]
    fn scan_widget_ignores_signals_inside_comments() {
        // REVISION HIGH-3 LOAD-BEARING TEST. All signal literals appear ONLY
        // inside comments. The scanner must NOT treat them as present.
        let html = r#"<!doctype html><html><body><script type="module">
            // import { App } from "@modelcontextprotocol/ext-apps";
            /* const a = new App({name:"x",version:"1"});
               a.onteardown=()=>{}; a.ontoolinput=()=>{}; */
            <!-- a.connect(); a.onerror=()=>{}; a.ontoolcancelled=()=>{}; -->
        </script></body></html>"#;
        let signals = scan_widget(html);
        assert!(
            !signals.has_ext_apps_import,
            "ext-apps import in comment must NOT match"
        );
        assert!(
            !signals.has_app_constructor,
            "new App() in comment must NOT match"
        );
        assert!(!signals.has_connect, "connect() in comment must NOT match");
        assert!(
            signals.handlers_present.is_empty(),
            "handlers in comments must NOT match (got {:?})",
            signals.handlers_present
        );
    }

    // ==========================================================================
    // Plan 78-06 Task 1 — G1 (4-signal SDK presence) + G2 (mangled-id ctor) unit tests
    // ==========================================================================

    #[test]
    fn scan_widget_g1_log_prefix_alone_satisfies_has_sdk() {
        let html = r#"<html><body><script>console.log("[ext-apps] boot");</script></body></html>"#;
        let s = scan_widget(html);
        assert!(s.has_log_prefix, "expected has_log_prefix true");
        assert!(s.has_sdk, "G1: log prefix alone must satisfy has_sdk");
        assert!(!s.has_ext_apps_import);
        assert!(!s.has_method_initialize);
        assert!(!s.has_method_tool_result);
    }

    #[test]
    fn scan_widget_g1_method_initialize_alone_satisfies_has_sdk() {
        let html = r#"<html><body><script>rpc("ui/initialize",{});</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            s.has_method_initialize,
            "expected has_method_initialize true"
        );
        assert!(s.has_sdk, "G1: ui/initialize alone must satisfy has_sdk");
    }

    #[test]
    fn scan_widget_g1_method_tool_result_alone_satisfies_has_sdk() {
        let html =
            r#"<html><body><script>rpc("ui/notifications/tool-result",{});</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            s.has_method_tool_result,
            "expected has_method_tool_result true"
        );
        assert!(
            s.has_sdk,
            "G1: ui/notifications/tool-result alone must satisfy has_sdk"
        );
    }

    #[test]
    fn scan_widget_g1_legacy_import_still_satisfies_has_sdk() {
        let html = r#"<html><body><script type="module">import { App } from "@modelcontextprotocol/ext-apps";</script></body></html>"#;
        let s = scan_widget(html);
        assert!(s.has_ext_apps_import, "expected has_ext_apps_import true");
        assert!(
            s.has_sdk,
            "Legacy import literal must still satisfy has_sdk"
        );
    }

    #[test]
    fn scan_widget_g1_no_signals_means_no_sdk() {
        let html = r#"<html><body><script>var x=1;</script></body></html>"#;
        let s = scan_widget(html);
        assert!(!s.has_sdk, "no SDK signals → has_sdk = false");
        assert!(!s.has_ext_apps_import);
        assert!(!s.has_log_prefix);
        assert!(!s.has_method_initialize);
        assert!(!s.has_method_tool_result);
    }

    #[test]
    fn scan_widget_g2_mangled_yl_constructor_matches() {
        let html = r#"<html><body><script>var a=new yl({name:"cost-coach-cost-summary",version:"1.0.0"});</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            s.has_app_constructor,
            "G2: mangled `yl` constructor with intact payload must match"
        );
    }

    #[test]
    fn scan_widget_g2_mangled_gl_constructor_matches() {
        let html = r#"<html><body><script>var a=new gl({name:"cost-coach-cost-over-time",version:"1.0.0"});</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            s.has_app_constructor,
            "G2: mangled `gl` constructor must match"
        );
    }

    #[test]
    fn scan_widget_g2_unminified_app_constructor_still_matches() {
        let html = r#"<html><body><script type="module">const app = new App({name: "tool", version: "1.0.0"});</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            s.has_app_constructor,
            "G2: unminified `App` constructor must still match (App is a valid identifier under the regex)"
        );
    }

    #[test]
    fn scan_widget_g2_random_new_call_without_name_version_payload_does_not_match() {
        let html = r#"<html><body><script>var d=new Date(2026,1,1);var u=new URL("http://x");</script></body></html>"#;
        let s = scan_widget(html);
        assert!(
            !s.has_app_constructor,
            "G2: random `new <X>(...)` calls without {{name, version}} payload must NOT match"
        );
    }

    // ==========================================================================
    // Plan 78-06 Task 2 — G3 cascade-elimination unit tests
    // ==========================================================================

    #[test]
    fn scan_widget_g3_handlers_detected_independently_of_has_sdk() {
        // The synthetic cascade fixture shape: handlers + connect present,
        // ALL SDK signals absent.
        let html = r#"<html><body><script>
            var obj={};
            obj.onteardown=async()=>({});
            obj.ontoolinput=function(p){};
            obj.ontoolcancelled=function(p){};
            obj.onerror=function(e){};
            obj.connect();
        </script></body></html>"#;
        let s = scan_widget(html);
        assert!(!s.has_sdk, "G3: no SDK signals → has_sdk false");
        assert!(
            !s.has_app_constructor,
            "G3: no constructor → has_app_constructor false"
        );
        assert!(
            s.has_handlers,
            "G3: handlers detected independently of has_sdk"
        );
        assert!(
            s.has_connect,
            "G3: connect detected independently of has_sdk"
        );
        assert_eq!(
            s.handlers_present.len(),
            4,
            "G3: all 4 handlers detected by member-name regex"
        );
    }

    #[test]
    fn scan_widget_g3_chatgpt_only_diagnosis_requires_genuine_evidence_absence() {
        // chatgpt-only channels + no SDK + no handlers → chatgpt-only-failed fires.
        let html_a = r#"<html><body><script>window.openai.x();</script></body></html>"#;
        let s_a = scan_widget(html_a);
        assert!(s_a.has_chatgpt_only_channels);
        assert!(!s_a.has_sdk);
        assert!(s_a.handlers_present.is_empty());
        // chatgpt-only channels + has_sdk → chatgpt-only-failed must NOT fire
        // (this is what the compound predicate in emit_results_for_claude_desktop guards).
        let html_b = r#"<html><body><script>console.log("[ext-apps]");window.openai.x();</script></body></html>"#;
        let s_b = scan_widget(html_b);
        assert!(s_b.has_chatgpt_only_channels);
        assert!(s_b.has_sdk);
    }

    // ==========================================================================
    // Task 2 (Plan 78-01) — validate_widgets + per-mode emission tests
    // ==========================================================================

    /// A widget HTML body fully wired for MCP Apps SDK (used as the
    /// "corrected" baseline for several tests).
    fn corrected_widget_html() -> &'static str {
        r#"<!doctype html><html><body><script type="module">
            import { App } from "@modelcontextprotocol/ext-apps";
            const a = new App({ name: "x", version: "1.0.0" });
            a.onteardown = () => {};
            a.ontoolinput = () => {};
            a.ontoolcancelled = () => {};
            a.onerror = () => {};
            a.connect();
        </script></body></html>"#
    }

    #[test]
    fn claude_desktop_mode_emits_failed_for_missing_handlers() {
        let html = r#"<!doctype html><html><body><script type="module">
            import { App } from "@modelcontextprotocol/ext-apps";
            const a = new App({ name: "x", version: "1.0.0" });
            a.connect();
        </script></body></html>"#;
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "cost-coach".to_string(),
            "ui://test".to_string(),
            html.to_string(),
        )]);
        let failed: Vec<_> = results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .collect();
        assert!(
            failed.len() >= 4,
            "must emit >=4 Failed rows (got {})",
            failed.len()
        );
        let any_onteardown = failed.iter().any(|r| r.name.contains("onteardown"));
        assert!(any_onteardown, "must emit a Failed row naming onteardown");
        // REVISION HIGH-4: every Failed row's name must contain the tool name.
        for r in &failed {
            assert!(
                r.name.contains("cost-coach"),
                "Failed row name must include tool name (REVISION HIGH-4): {}",
                r.name
            );
        }
    }

    #[test]
    fn standard_mode_emits_one_summary_warn_per_widget() {
        let html = r#"<!doctype html><html><body><script type="module">
            import { App } from "@modelcontextprotocol/ext-apps";
            const a = new App({ name: "x", version: "1.0.0" });
            a.onerror = () => {};
            a.connect();
        </script></body></html>"#;
        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let results = validator.validate_widgets(&[(
            "cost-coach".to_string(),
            "ui://test".to_string(),
            html.to_string(),
        )]);
        let warns: Vec<_> = results
            .iter()
            .filter(|r| r.status == TestStatus::Warning)
            .collect();
        assert_eq!(
            warns.len(),
            1,
            "Standard mode must emit EXACTLY 1 Warning per widget (got {} for results: {:?})",
            warns.len(),
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
        let warn = warns[0];
        assert!(
            warn.name.contains("cost-coach"),
            "summary WARN name must include tool name (REVISION HIGH-4): {}",
            warn.name
        );
        assert!(
            warn.name.contains("ui://test"),
            "summary WARN name must include uri: {}",
            warn.name
        );
        let details = warn
            .details
            .as_ref()
            .expect("summary WARN must have details");
        assert!(
            details.contains("onteardown"),
            "summary details must list onteardown as missing: {details}"
        );
        assert!(
            details.contains("ontoolinput"),
            "summary details must list ontoolinput as missing: {details}"
        );
        assert!(
            details.contains("ontoolcancelled"),
            "summary details must list ontoolcancelled as missing: {details}"
        );
        let failed = results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        assert_eq!(
            failed, 0,
            "Standard mode must NOT emit any Failed rows from widget signals"
        );
    }

    #[test]
    fn claude_desktop_mode_passes_corrected_widget() {
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "good".to_string(),
            "ui://good".to_string(),
            corrected_widget_html().to_string(),
        )]);
        let failed = results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        assert_eq!(
            failed, 0,
            "Corrected widget must produce ZERO Failed rows under ClaudeDesktop (got {failed} for results: {:?})",
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn sdk_signal_requires_independent_evidence_no_fallback() {
        // G3 cascade-elimination contract (Plan 78-06): the v1 `>=3 of 4
        // handler-count` fallback is REMOVED. A widget with handlers but no
        // genuine SDK presence signal must report SDK Failed independently
        // of the handler rows. This test was previously
        // `sdk_signal_accepts_handler_count_fallback` and asserted the
        // opposite (the bug Plan 06 fixes).
        let html = make_widget_html(&[
            r#"var n=new App({name:"x",version:"1.0.0"});n.onteardown=()=>{};n.ontoolinput=()=>{};n.ontoolcancelled=()=>{};n.connect();"#,
        ]);
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "minified".to_string(),
            "ui://minified".to_string(),
            html,
        )]);
        let sdk_row = results
            .iter()
            .find(|r| r.name.contains("MCP Apps SDK wiring"))
            .expect("must emit MCP Apps SDK wiring row");
        assert_eq!(
            sdk_row.status,
            TestStatus::Failed,
            "G3: SDK signal must NOT cascade off handler count — handlers alone do not imply SDK presence: {sdk_row:?}"
        );
        // Handlers themselves must still pass independently.
        let onteardown_row = results
            .iter()
            .find(|r| r.name.contains("handler: onteardown"))
            .expect("must emit handler: onteardown row");
        assert_eq!(
            onteardown_row.status,
            TestStatus::Passed,
            "G3: handler row passes independently of SDK row: {onteardown_row:?}"
        );
    }

    #[test]
    fn chatgpt_only_channels_fails_in_claude_desktop() {
        // Widget uses window.openai with no ext-apps wiring at all.
        let html = make_widget_html(&[
            r#"window.openai.something();window.parent.postMessage({type:"x"}, "*");"#,
        ]);
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "chatgpt-flavored".to_string(),
            "ui://chatgpt".to_string(),
            html,
        )]);
        let chatgpt_row = results
            .iter()
            .find(|r| r.status == TestStatus::Failed && r.name.contains("chatgpt-only"));
        assert!(
            chatgpt_row.is_some(),
            "must emit a Failed row mentioning chatgpt-only channels under ClaudeDesktop (got: {:?})",
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn chatgpt_mode_emits_no_widget_results() {
        // REVISION HIGH-1 LOAD-BEARING TEST. Widget missing EVERY signal (no
        // SDK, no handlers, no new App, no connect, with chatgpt-only
        // channels). Under ChatGpt mode the validator MUST return zero
        // results — preserving AC-78-4 "chatgpt mode unchanged".
        let html = r#"<!doctype html><html><body><script>
            window.openai = {};
            window.parent.postMessage({type:"x"}, "*");
        </script></body></html>"#;
        let validator = AppValidator::new(AppValidationMode::ChatGpt, None);
        let results = validator.validate_widgets(&[(
            "broken-tool".to_string(),
            "ui://broken".to_string(),
            html.to_string(),
        )]);
        assert_eq!(
            results.len(),
            0,
            "ChatGpt mode must emit zero widget-related rows (got {} rows: {:?})",
            results.len(),
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn claude_desktop_mode_emits_failed_not_warning() {
        // Regression test for Pitfall 5 — under ClaudeDesktop mode, NO
        // Warning rows are emitted for the four required handlers. Only
        // ontoolresult MAY remain Warning (RESEARCH Locked Decision 3).
        let html = r#"<!doctype html><html><body><script type="module">
            // No handlers at all, just an import + new App + connect.
            import { App } from "@modelcontextprotocol/ext-apps";
            const a = new App({ name: "x", version: "1.0.0" });
            a.connect();
        </script></body></html>"#;
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "broken".to_string(),
            "ui://broken".to_string(),
            html.to_string(),
        )]);
        // The ONLY Warning allowed is ontoolresult.
        let warning_rows: Vec<_> = results
            .iter()
            .filter(|r| r.status == TestStatus::Warning)
            .collect();
        for w in &warning_rows {
            assert!(
                w.name.contains("ontoolresult"),
                "Under ClaudeDesktop, only `ontoolresult` may stay Warning. Found: {}",
                w.name
            );
        }
    }

    #[test]
    fn standard_mode_corrected_widget_emits_zero_warnings() {
        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let results = validator.validate_widgets(&[(
            "good".to_string(),
            "ui://good".to_string(),
            corrected_widget_html().to_string(),
        )]);
        let warnings = results
            .iter()
            .filter(|r| r.status == TestStatus::Warning)
            .count();
        assert_eq!(
            warnings, 0,
            "Fully corrected widget under Standard mode must produce ZERO Warning rows (got {warnings} for results: {:?})",
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn chatgpt_mode_corrected_widget_also_emits_zero() {
        // Re-asserts ChatGpt is silent regardless of widget shape.
        let validator = AppValidator::new(AppValidationMode::ChatGpt, None);
        let results = validator.validate_widgets(&[(
            "good".to_string(),
            "ui://good".to_string(),
            corrected_widget_html().to_string(),
        )]);
        assert_eq!(
            results.len(),
            0,
            "ChatGpt mode emits zero widget rows even for fully corrected widgets (got: {:?})",
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }

    // ===== Cycle 2 unit tests (Plan 78-10) =====
    //
    // Cycle-2 fixes the comment-stripper bug + widens G2 regex. Source:
    // tests/fixtures/widgets/bundled/real-prod/CAPTURE.md "Root cause
    // discovered" section.

    #[test]
    fn strip_js_comments_preserves_block_comment_inside_double_quoted_string() {
        // The cost-coach prod bug: a CSP frame-src directive value contains
        // `/*.example.com`. The cycle-1 stripper saw that `/*` as a
        // block-comment opener and matched the next `*/` (a license-header
        // banner thousands of bytes later), destroying the SDK section
        // between.
        let src = r#"var csp = "frame-src /*.example.com"; var i = "[ext-apps] App.connect() failed"; /* license */ var x = 1;"#;
        let stripped = strip_js_comments(src);
        assert!(
            stripped.contains("[ext-apps]"),
            "SDK literal must be preserved when /* appears inside a string literal; got: {stripped}",
        );
        assert!(
            stripped.contains("/*.example.com"),
            "CSP string content must be preserved; got: {stripped}",
        );
        assert!(
            !stripped.contains("license"),
            "Real block comments outside strings MUST still be stripped; got: {stripped}",
        );
    }

    #[test]
    fn strip_js_comments_preserves_block_comment_inside_single_quoted_string() {
        let src = "var csp = 'frame-src /*.example.com'; /* real */ var x = 1;";
        let stripped = strip_js_comments(src);
        assert!(stripped.contains("/*.example.com"));
        assert!(!stripped.contains("real"));
    }

    #[test]
    fn strip_js_comments_preserves_line_comment_marker_inside_string() {
        let src = "var url = \"https://example.com/path\"; // real comment\nvar keep = 1;";
        let stripped = strip_js_comments(src);
        assert!(
            stripped.contains("https://example.com/path"),
            "URL string with // must be preserved; got: {stripped}",
        );
        assert!(
            !stripped.contains("real comment"),
            "Real // line comment outside strings MUST still be stripped; got: {stripped}",
        );
        assert!(stripped.contains("var keep = 1"));
    }

    #[test]
    fn strip_js_comments_still_strips_real_block_comments_outside_strings() {
        let src = "var x = 1; /* this is a comment */ var y = 2;";
        let stripped = strip_js_comments(src);
        assert!(!stripped.contains("this is a comment"));
        assert!(stripped.contains("var x = 1"));
        assert!(stripped.contains("var y = 2"));
    }

    #[test]
    fn strip_js_comments_still_strips_real_line_comments_outside_strings() {
        let src = "var x = 1; // line comment\nvar y = 2;";
        let stripped = strip_js_comments(src);
        assert!(!stripped.contains("line comment"));
        assert!(stripped.contains("var x = 1"));
        assert!(stripped.contains("var y = 2"));
    }

    #[test]
    fn strip_js_comments_handles_escaped_string_delimiters() {
        // \" inside a "..." string must not exit the string state.
        let src = r#"var s = "He said \"hi\" /* not a comment */"; var z = 1;"#;
        let stripped = strip_js_comments(src);
        assert!(
            stripped.contains("not a comment"),
            "Block-comment-style text inside a string with escaped quotes must be preserved; got: {stripped}",
        );
        assert!(stripped.contains("var z = 1"));
    }

    #[test]
    fn strip_js_comments_handles_template_literal() {
        let src = r#"var t = `template /* not a comment */`; /* real */ var x = 1;"#;
        let stripped = strip_js_comments(src);
        assert!(stripped.contains("not a comment"));
        assert!(!stripped.contains("real"));
    }

    #[test]
    fn scan_widget_g2_cycle2_string_concat_name_value_matches() {
        // Real cost-coach prod constructor shape — name uses string
        // concatenation: `name:"cost-coach-"+t,version:"1.0.0"`. The
        // cycle-1 regex required a literal value; cycle-2 widening
        // accepts any expression up to 100 chars.
        let html = r#"<script>function f(t){var i=new yl({name:"cost-coach-"+t,version:"1.0.0"});}</script>"#;
        let signals = scan_widget(html);
        assert!(
            signals.has_app_constructor,
            "G2 cycle-2: must match new <id>({{name:<concat-expr>, version:<expr>}}); signals: {signals:?}",
        );
    }

    #[test]
    fn scan_widget_g2_cycle2_longer_mangled_id_matches() {
        // Some bundles emit 3-4-or-longer mangled identifiers. Cycle-1
        // capped at {0,5}; cycle-2 widens to {0,20}.
        let html = r#"<script>var i=new abcdefg({name:"x",version:"1"});</script>"#;
        let signals = scan_widget(html);
        assert!(signals.has_app_constructor);
    }

    #[test]
    fn scan_widget_g2_cycle2_random_new_call_with_unrelated_keys_does_not_match() {
        // Negative: `new Foo({key1, key2})` must NOT match — needs both
        // `name` AND `version`.
        let html = r#"<script>var i=new Date({year:2026,month:1});</script>"#;
        let signals = scan_widget(html);
        assert!(
            !signals.has_app_constructor,
            "G2 cycle-2: must NOT match new Date(...) without name/version keys; signals: {signals:?}",
        );
    }

    #[test]
    fn scan_widget_g2_cycle2_real_cost_coach_prod_pattern() {
        // Verbatim shape from cost-coach prod (cost-summary.html, function Rw).
        let html = r#"<script>function Rw(t,e){var i=new yl({name:"cost-coach-"+t,version:"1.0.0"});return i.connect(),i;}</script>"#;
        let signals = scan_widget(html);
        assert!(
            signals.has_app_constructor,
            "G2 cycle-2: real prod shape; signals: {signals:?}",
        );
    }

    #[test]
    fn scan_widget_g1_cycle2_csp_string_does_not_steal_sdk_section() {
        // Regression for the cost-coach prod root cause: a `/*` inside a
        // string literal must not strip away the SDK section that
        // follows. With the cycle-1 stripper, the `/*.example.com` CSP
        // string opened a phantom block comment that matched the next
        // `*/` (a real license-header banner), destroying the entire
        // SDK runtime between. Cycle-2's string-literal-aware stripper
        // preserves the SDK section.
        let html = concat!(
            r##"<script>var csp = "frame-src /*.example.com"; "##,
            r##"var msg = "[ext-apps] App.connect() called before connect"; "##,
            r##"function f(t){var i=new yl({name:"cost-coach-"+t,version:"1.0.0"}); "##,
            r##"i.onteardown = function(){}; "##,
            r##"i.ontoolinput = function(){}; "##,
            r##"i.ontoolcancelled = function(){}; "##,
            r##"i.onerror = function(){}; "##,
            r##"i.connect();} "##,
            r##"/* @license real banner */ var x = 1;</script>"##,
        );
        let signals = scan_widget(html);
        assert!(
            signals.has_log_prefix,
            "G1 cycle-2: [ext-apps] preserved through string-literal-aware stripping; signals: {signals:?}",
        );
        assert!(signals.has_sdk);
        assert!(signals.has_app_constructor);
        assert!(signals.has_handlers);
        assert!(signals.has_connect);
    }
}
