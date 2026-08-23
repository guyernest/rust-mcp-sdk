//! Pack-time validation of a config server's `[[config_slots]]` declaration
//! block against the package's own slot list (D-01, D-04, D-17).
//!
//! # Why this module exists
//!
//! A Shape A config server's whole identity is its config file. That file
//! declares, in its own `[[config_slots]]` table, which values the TARGET
//! environment must supply. The `ServerPackage` being packed carries a
//! parallel `config_slots: Vec<ConfigSlot>` list. Before this module existed
//! the two were parallel representations that nothing compared: a caller could
//! hand [`pack_server`](crate::oci::pack_server) a slot list contradicting the
//! config it ships, or edit the config's declaration block while the package
//! slot list stayed put, and no code path would notice.
//!
//! [`parse_declared_config_slots`] reads the declaration table out of the SAME
//! bytes that become the config layer, and [`validate_config_slot_agreement`]
//! requires the two to agree exactly. The TOML block is the source of truth —
//! that is what D-01's "`pack` reads them" means, and it is now exercised by
//! the real API path rather than asserted in prose.
//!
//! # Untrusted input
//!
//! These config bytes are untrusted input to THIS crate. They need not have
//! come through `pmcp-server-toolkit`'s `ServerConfig`, so the `kind`
//! vocabulary is re-validated here rather than assumed. Every function in this
//! module returns `Result` on malformed input and never panics.
//!
//! # Error hygiene
//!
//! No error raised here ever echoes a config VALUE (T-120-21). A config slot
//! may name a credential, and the whole point of the placeholder rule is to
//! keep a resolved secret out of a packed layer — an error message is the
//! wrong place to put one. Errors name the config KEY and the FIELD or RULE
//! that was violated.

use crate::error::{PackageError, Result};
use crate::slot::{ConfigSlot, SlotType};
use std::collections::BTreeMap;

/// The closed `kind` vocabulary a `[[config_slots]]` entry may declare. These
/// are byte-identical to `pmcp-server-toolkit`'s `ConfigSlotKind` snake_case
/// discriminators AND to the corresponding [`SlotType::key`] kind strings —
/// that three-way string correspondence is what lets the two crates agree
/// without either depending on the other.
const ACCEPTED_KINDS: [&str; 3] = ["endpoint", "secret", "auth_mode"];

/// Error label used when a failure is a property of the DOCUMENT rather than
/// of any single declared key.
const DOCUMENT_LABEL: &str = "<config document>";

/// Error label for the declaration table itself.
const TABLE_LABEL: &str = "config_slots";

/// Build a [`PackageError::ConfigSlotViolation`]. Centralized so every message
/// in this module goes through one place that takes a key and a reason and
/// nothing else — there is no parameter here a config VALUE could ride in on.
fn violation(key: &str, reason: impl Into<String>) -> PackageError {
    PackageError::ConfigSlotViolation {
        key: key.to_string(),
        reason: reason.into(),
    }
}

/// Parse `config_bytes` as a TOML document.
///
/// The parser's own error message is deliberately NOT propagated: `toml`'s
/// `Display` renders a snippet of the offending source line, which for a
/// credential-bearing config is exactly the value this crate exists to keep out
/// of error text. The byte span is reported instead — enough to locate the
/// problem, incapable of quoting it.
fn parse_document(config_bytes: &[u8]) -> Result<toml::Value> {
    let text = std::str::from_utf8(config_bytes)
        .map_err(|_| violation(DOCUMENT_LABEL, "config bytes are not valid UTF-8"))?;
    toml::from_str::<toml::Value>(text).map_err(|e| {
        let where_ = e
            .span()
            .map_or_else(String::new, |s| format!(" at byte offset {}", s.start));
        violation(
            DOCUMENT_LABEL,
            format!(
                "config bytes are not valid TOML{where_} (the parser's message is withheld \
                 because it would quote config content)"
            ),
        )
    })
}

/// One `[[config_slots]]` entry as it appears in the config document.
///
/// A plain mirror of `pmcp-server-toolkit`'s `ConfigSlotDecl` wire shape,
/// deliberately RE-DECLARED here rather than imported.
///
/// # Why re-declared instead of shared
///
/// `pmcp-package` is the workspace-excluded leaf crate and must not depend on
/// `pmcp-server-toolkit`; the toolkit must not depend on `pmcp-package` either,
/// because that inverts the layering (plan 120-04 machine-checks it does not).
/// So there is no place a shared type could live. The two shapes are kept in
/// step by the TOML FIELD NAMES, which are the actual contract, and by a test
/// that parses the real `london-tube.toml` the reference server boots from — if
/// a field is renamed on either side, that test stops finding three slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredConfigSlot {
    /// The dotted TOML path this slot fills, e.g. `backend.base_url`.
    pub key: String,
    /// The declared kind — one of `endpoint`, `secret`, `auth_mode`.
    pub kind: String,
    /// The slot's declared name (for a `secret`, the environment-variable name).
    pub name: String,
    /// The value exercised when the server was tested. `None` for an
    /// identity-bearing slot, which structurally carries no value.
    pub tested_value: Option<String>,
}

/// Read the `[[config_slots]]` declaration table out of a server config
/// document.
///
/// A document with no `config_slots` table returns an empty vec — declaring
/// nothing is legal; such a package simply cannot then claim any config slots
/// (see [`validate_config_slot_agreement`]).
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] if the bytes are not valid
/// UTF-8/TOML, if `config_slots` is not an array of tables, or if any entry
/// is malformed — a missing or non-string `key`/`name`, a missing `kind`, or a
/// `kind` outside the closed vocabulary. The error names the entry's `key`
/// when one is readable and its index (`config_slots[N]`) otherwise.
///
/// # Examples
///
/// ```
/// use pmcp_package::parse_declared_config_slots;
///
/// let config = br#"
/// [[config_slots]]
/// key = "backend.base_url"
/// kind = "endpoint"
/// name = "TFL_BASE_URL"
/// tested_value = "https://api.tfl.gov.uk"
/// "#;
///
/// let declared = parse_declared_config_slots(config).unwrap();
/// assert_eq!(declared.len(), 1);
/// assert_eq!(declared[0].key, "backend.base_url");
/// assert_eq!(declared[0].kind, "endpoint");
/// assert_eq!(declared[0].tested_value.as_deref(), Some("https://api.tfl.gov.uk"));
///
/// // A config that declares nothing is legal, not an error.
/// assert!(parse_declared_config_slots(b"name = \"x\"\n").unwrap().is_empty());
/// ```
pub fn parse_declared_config_slots(config_bytes: &[u8]) -> Result<Vec<DeclaredConfigSlot>> {
    let document = parse_document(config_bytes)?;
    let Some(raw) = document.get(TABLE_LABEL) else {
        return Ok(Vec::new());
    };
    let entries = raw.as_array().ok_or_else(|| {
        violation(
            TABLE_LABEL,
            "`config_slots` must be an array of tables (`[[config_slots]]`)",
        )
    })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_declaration_entry(index, entry))
        .collect()
}

/// Parse one `[[config_slots]]` entry, re-validating `kind` against the closed
/// vocabulary because these bytes are untrusted input to this crate.
fn parse_declaration_entry(index: usize, entry: &toml::Value) -> Result<DeclaredConfigSlot> {
    let positional = format!("{TABLE_LABEL}[{index}]");
    let table = entry
        .as_table()
        .ok_or_else(|| violation(&positional, "declaration entry is not a table"))?;

    let key = required_string(table, "key", &positional)?;
    // Prefer the entry's own key as the error label once it is readable — a
    // positional index is only useful when the key itself is unreadable.
    let label = if key.is_empty() {
        positional.clone()
    } else {
        key.clone()
    };

    let kind = required_string(table, "kind", &label)?;
    if !ACCEPTED_KINDS.contains(&kind.as_str()) {
        // The rejected discriminator is deliberately NOT echoed: it is
        // attacker-controlled text from the document, and the uniform rule is
        // that errors name the key and the rule, never document content.
        return Err(violation(
            &label,
            format!(
                "unknown config-slot kind; the accepted kinds are {}",
                ACCEPTED_KINDS.join(", ")
            ),
        ));
    }

    let name = required_string(table, "name", &label)?;

    let tested_value = match table.get("tested_value") {
        None => None,
        Some(toml::Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(violation(&label, "`tested_value` must be a string")),
    };

    Ok(DeclaredConfigSlot {
        key,
        kind,
        name,
        tested_value,
    })
}

/// Read a required string field off a declaration entry.
fn required_string(table: &toml::Table, field: &str, label: &str) -> Result<String> {
    match table.get(field) {
        Some(toml::Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(violation(label, format!("`{field}` must be a string"))),
        None => Err(violation(
            label,
            format!("`{field}` is required on a `[[config_slots]]` entry"),
        )),
    }
}

/// The comparable projection of a slot: `(kind, name, tested_value)`.
type SlotFacts<'a> = (&'a str, &'a str, Option<&'a str>);

/// Require the config's `[[config_slots]]` declarations and the package's
/// `config_slots` list to describe the SAME slots.
///
/// Compared as SETS keyed on the config key — declaration order in the TOML is
/// not load-bearing. A [`ConfigSlot`] whose `config_key` is `None` does not
/// participate: it fills no config path, so there is nothing for a declaration
/// to correspond to. Whether such a slot is legal at all is
/// [`validate_config_slot_placeholders`](crate::validate_config_slot_placeholders)'s
/// rule, not this one.
///
/// The first disagreement is reported in a deterministic (sorted-key) order, so
/// a config with several problems always fails the same way.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the offending key when
/// a declaration has no matching package slot, a package slot has no matching
/// declaration, either side declares the same key twice, or a matched pair
/// disagrees on `kind`, `name` or `tested_value`. The message names the key and
/// the FIELD that disagreed — never the two values, so a future slot kind
/// cannot leak by inheriting an exception.
///
/// # Examples
///
/// ```
/// use pmcp_package::{
///     parse_declared_config_slots, validate_config_slot_agreement, ConfigSlot, SlotType,
/// };
///
/// let config = br#"
/// [[config_slots]]
/// key = "backend.base_url"
/// kind = "endpoint"
/// name = "TFL_BASE_URL"
/// tested_value = "https://api.tfl.gov.uk"
/// "#;
/// let declared = parse_declared_config_slots(config).unwrap();
///
/// let matching = vec![ConfigSlot::new(SlotType::Endpoint {
///     name: "TFL_BASE_URL".to_string(),
///     tested_value: "https://api.tfl.gov.uk".to_string(),
/// })
/// .with_config_key("backend.base_url")];
/// assert!(validate_config_slot_agreement(&declared, &matching).is_ok());
///
/// // A package that claims a slot its shipped config never declares is refused.
/// let invented = vec![ConfigSlot::new(SlotType::Secret {
///     name: "SOME_KEY".to_string(),
/// })
/// .with_config_key("backend.auth.api_key")];
/// assert!(validate_config_slot_agreement(&declared, &invented).is_err());
/// ```
pub fn validate_config_slot_agreement(
    declared: &[DeclaredConfigSlot],
    package_slots: &[ConfigSlot],
) -> Result<()> {
    let declared_facts = declared_fact_map(declared)?;
    let package_facts = package_fact_map(package_slots)?;

    // Sorted union of both key sets — BTreeMap iteration is already ordered,
    // and chaining two ordered maps into a BTreeSet keeps the union ordered,
    // so "the first disagreement" is reproducible.
    let keys: std::collections::BTreeSet<&str> = declared_facts
        .keys()
        .chain(package_facts.keys())
        .copied()
        .collect();

    for key in keys {
        match (declared_facts.get(key), package_facts.get(key)) {
            (Some(declaration), Some(package)) => compare_facts(key, *declaration, *package)?,
            (Some(_), None) => {
                return Err(violation(
                    key,
                    "declared in the config's `[[config_slots]]` table but absent from the \
                     package's config_slots list — the shipped config is the source of truth, \
                     so add the slot to the package rather than dropping the declaration",
                ))
            },
            (None, Some(_)) => {
                return Err(violation(
                    key,
                    "present in the package's config_slots list but absent from the config's \
                     `[[config_slots]]` table — a package may not claim a slot the config it \
                     ships does not declare",
                ))
            },
            // Unreachable: `key` came from the union of the two maps.
            (None, None) => {},
        }
    }
    Ok(())
}

/// Compare a matched declaration/package pair field by field.
fn compare_facts(key: &str, declaration: SlotFacts<'_>, package: SlotFacts<'_>) -> Result<()> {
    if declaration.0 != package.0 {
        // Kinds are a closed vocabulary, not config content, so naming both is
        // safe and is what makes the error actionable.
        return Err(violation(
            key,
            format!(
                "the config declares kind '{}' but the package slot is '{}'",
                declaration.0, package.0
            ),
        ));
    }
    if declaration.1 != package.1 {
        return Err(violation(
            key,
            "the declared slot `name` disagrees with the package slot's name",
        ));
    }
    if declaration.2 != package.2 {
        return Err(violation(
            key,
            "the declared `tested_value` disagrees with the package slot's tested value",
        ));
    }
    Ok(())
}

/// Index the declarations by config key, rejecting a duplicated key.
fn declared_fact_map(declared: &[DeclaredConfigSlot]) -> Result<BTreeMap<&str, SlotFacts<'_>>> {
    let mut map = BTreeMap::new();
    for declaration in declared {
        let facts = (
            declaration.kind.as_str(),
            declaration.name.as_str(),
            declaration.tested_value.as_deref(),
        );
        if map.insert(declaration.key.as_str(), facts).is_some() {
            return Err(violation(
                &declaration.key,
                "declared more than once in the config's `[[config_slots]]` table",
            ));
        }
    }
    Ok(map)
}

/// Index the package slots by their `config_key`, rejecting a duplicated key.
/// Slots with no `config_key` fill no config path and are not indexed.
fn package_fact_map(package_slots: &[ConfigSlot]) -> Result<BTreeMap<&str, SlotFacts<'_>>> {
    let mut map = BTreeMap::new();
    for slot in package_slots {
        let Some(config_key) = slot.config_key.as_deref() else {
            continue;
        };
        let (kind, name) = slot.slot.key();
        let facts = (kind, name, slot.slot.tested_value());
        if map.insert(config_key, facts).is_some() {
            return Err(violation(
                config_key,
                "claimed by more than one slot in the package's config_slots list",
            ));
        }
    }
    Ok(map)
}

// =======================================================================
// D-04 (as amended by D-17): a slot-declared VALUE key must hold an
// environment reference, never a resolved literal.
// =======================================================================

/// Require every slot-declared VALUE key in `config_bytes` to hold an
/// environment reference (`${VAR}` or `env:VAR`) rather than a resolved
/// literal, so no resolved secret or environment-specific endpoint can travel
/// inside a packed layer (D-04, T-120-20).
///
/// # The slot split is THREE-way and exhaustive
///
/// The rule is written as a `match` over [`SlotType`] with NO catch-all arm, so
/// a future variant is a compile error until someone decides which arm it
/// belongs in:
///
/// - **Value slots — [`SlotType::Endpoint`] and [`SlotType::Secret`].** Subject
///   to the placeholder rule. When a config file is present, a `config_key` of
///   `None` on one of these is itself a violation: a packable config server
///   whose endpoint or credential slot does not say WHERE it lives cannot be
///   validated by pack and cannot tell a target environment where to write.
/// - **Structural — [`SlotType::AuthMode`].** Exempt (D-17). The toolkit's
///   `AuthConfig` is internally tagged (`#[serde(tag = "type")]`), so a
///   reference-shaped value at that key fails serde's variant dispatch before
///   any resolution could happen — there is no placeholder form of that key
///   that both parses and defers, which makes the baked literal the only legal
///   content. Deviation on it surfaces through slot classification instead.
/// - **Not config-value slots — [`SlotType::OauthClient`],
///   [`SlotType::ChannelBinding`], [`SlotType::HumanRole`],
///   [`SlotType::LlmProvider`], [`SlotType::BudgetOverride`].** With
///   `config_key: None`, skipped. With a `config_key`, a violation: declaring a
///   TOML path for a slot kind that fills none is a defect, and silently
///   ignoring it would let a package claim a coverage it does not have.
///   ([`SlotType::HumanRole`] has no simple value field at all.)
///
/// Callers reach this through [`pack_server`](crate::oci::pack_server), which
/// runs it before writing any blob. It is public so a CLI can pre-check a
/// config before building a package at all.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the offending config
/// key. The offending VALUE is never echoed — it may be the exact resolved
/// secret the rule exists to keep out of a layer.
///
/// # Examples
///
/// ```
/// use pmcp_package::{validate_config_slot_placeholders, ConfigSlot, SlotType};
///
/// let slots = vec![ConfigSlot::new(SlotType::Secret {
///     name: "TFL_APP_KEY".to_string(),
/// })
/// .with_config_key("backend.auth.app_key")];
///
/// // Accepted: the credential key defers to the environment.
/// let deferred = b"[backend.auth]\napp_key = \"${TFL_APP_KEY}\"\n";
/// assert!(validate_config_slot_placeholders(deferred, &slots).is_ok());
///
/// // Refused: the credential was resolved into the file that is about to be
/// // packed. The error names the key and never the value.
/// let baked = b"[backend.auth]\napp_key = \"a-real-credential\"\n";
/// let err = validate_config_slot_placeholders(baked, &slots).unwrap_err();
/// assert!(err.to_string().contains("backend.auth.app_key"));
/// assert!(!err.to_string().contains("a-real-credential"));
/// ```
pub fn validate_config_slot_placeholders(config_bytes: &[u8], slots: &[ConfigSlot]) -> Result<()> {
    let document = parse_document(config_bytes)?;
    for slot in slots {
        check_slot_placeholder(&document, slot)?;
    }
    Ok(())
}

/// The per-slot half of [`validate_config_slot_placeholders`] — the exhaustive
/// three-way match itself, split out so the public function stays a loop.
fn check_slot_placeholder(document: &toml::Value, slot: &ConfigSlot) -> Result<()> {
    match &slot.slot {
        // --- Value slots: subject to the placeholder rule -----------------
        SlotType::Endpoint { name, .. } | SlotType::Secret { name } => {
            let Some(config_key) = slot.config_key.as_deref() else {
                return Err(violation(
                    name,
                    "a value slot (endpoint or secret) on a package that ships a config file \
                     must name the config key it fills — without one, pack cannot validate it \
                     and a target environment cannot be told where to write",
                ));
            };
            let value = resolve_dotted_key(document, config_key)?;
            let toml::Value::String(raw) = value else {
                return Err(violation(
                    config_key,
                    "a slot-declared value key must hold a string; this key holds a non-string \
                     TOML value, which no environment reference can be expressed as",
                ));
            };
            if is_env_reference(raw) {
                Ok(())
            } else {
                Err(violation(
                    config_key,
                    "holds a resolved literal; a slot-declared value key must hold an \
                     environment reference (`${VAR}` or `env:VAR`) so the resolved value never \
                     travels inside a packed layer",
                ))
            }
        },
        // --- Structural: exempt (D-17) ------------------------------------
        SlotType::AuthMode { .. } => Ok(()),
        // --- Not config-value slots ---------------------------------------
        SlotType::OauthClient { .. }
        | SlotType::ChannelBinding { .. }
        | SlotType::HumanRole { .. }
        | SlotType::LlmProvider { .. }
        | SlotType::BudgetOverride { .. } => match slot.config_key.as_deref() {
            None => Ok(()),
            Some(config_key) => Err(violation(
                config_key,
                "this slot kind has no config-value semantics, so it cannot fill a TOML config \
                 key; only endpoint and secret slots address a config value, and auth_mode is \
                 structural",
            )),
        },
    }
}

/// Resolve a dotted `config_key` against a parsed config document.
///
/// # Grammar
///
/// A `config_key` is one or more `.`-separated components. Each component must
/// be a non-empty TOML **bare key** — ASCII letters, digits, `_` and `-` — and
/// every component except the last must address a TOML **table**.
///
/// Deliberately OUT OF SCOPE, and REJECTED rather than mis-resolved:
/// array indexing (`tools[0].path`), and TOML quoted keys — which means a TOML
/// key whose literal name contains a dot is unaddressable by this grammar.
/// Saying so in an error is honest where silently splitting on the dot is not.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the key and the rule it
/// broke: an empty key, an empty component (a leading dot, a trailing dot or a
/// doubled dot), a component that is not a bare key, a path traversing a
/// non-table, or a path that resolves to nothing. None of these silently
/// resolves to "absent, therefore fine".
fn resolve_dotted_key<'a>(document: &'a toml::Value, config_key: &str) -> Result<&'a toml::Value> {
    if config_key.is_empty() {
        return Err(violation(
            config_key,
            "config key is empty; the grammar is one or more dot-separated non-empty TOML bare keys",
        ));
    }
    let mut current = document;
    for component in config_key.split('.') {
        if component.is_empty() {
            return Err(violation(
                config_key,
                "config key has an empty path component (a leading dot, a trailing dot or a \
                 doubled dot); the grammar is dot-separated non-empty TOML bare keys",
            ));
        }
        if !is_bare_key(component) {
            return Err(violation(
                config_key,
                "config key has a component that is not a TOML bare key (A-Z a-z 0-9 _ -); \
                 quoted keys and array indexing are out of scope, so a TOML key whose literal \
                 name contains a dot is unaddressable by this grammar",
            ));
        }
        let table = current.as_table().ok_or_else(|| {
            violation(
                config_key,
                "config key traverses a value that is not a table; the grammar addresses TOML \
                 tables only",
            )
        })?;
        current = table.get(component).ok_or_else(|| {
            violation(
                config_key,
                "config key resolves to nothing in the packed config — a slot declaration \
                 pointing at no key is a defect, not a pass",
            )
        })?;
    }
    Ok(current)
}

/// Whether `component` is a TOML bare key (non-emptiness is checked by the
/// caller, which reports it as its own distinct rule).
fn is_bare_key(component: &str) -> bool {
    component
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Whether `raw` is an environment REFERENCE rather than a resolved literal.
///
/// Recognises exactly two forms: an `env:` prefix with a non-empty remainder,
/// and a `${` … `}` wrapper with a non-empty inner name. Everything else — a
/// bare literal, an unterminated brace, text after the closing brace, the
/// malformed empty-name forms `${}` and `env:` — is a literal as far as this
/// rule is concerned, because none of them names a variable a target
/// environment could supply.
///
/// # A deliberate duplication, kept honest by a table
///
/// The crate that OWNS this grammar is `pmcp-server-toolkit`
/// (`src/env_ref.rs::parse_env_ref`). It is duplicated here rather than shared
/// because neither crate may depend on the other: `pmcp-package` is the
/// workspace-excluded leaf, and a toolkit dependency on it inverts the
/// layering. A silent divergence would be a real bug — a config that packs
/// cleanly and then fails to resolve at boot, or one the runtime resolves being
/// refused at pack — so the two implementations are held to a shared
/// accept/reject table, `tests/golden_fixtures/env_ref_grammar_v1.tsv`,
/// asserted from BOTH crates. A row one side disagrees with fails a test in
/// whichever crate is wrong.
///
/// Note the two implementations differ in SHAPE, not in verdict:
/// `parse_env_ref` returns `Some("")` for `${}` / `env:` (its caller resolves
/// an empty name to omission), while this predicate answers `false` for both
/// because an empty name is not something a package can ask an environment to
/// fill. The table encodes that correspondence explicitly rather than papering
/// over it.
fn is_env_reference(raw: &str) -> bool {
    if let Some(rest) = raw.strip_prefix("env:") {
        return !rest.is_empty();
    }
    raw.strip_prefix("${")
        .and_then(|inner| inner.strip_suffix('}'))
        .is_some_and(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The REAL config the reference OpenAPI server boots from, vendored
    /// byte-for-byte into this crate's fixtures (a drift guard in
    /// `tests/config_server.rs` fails if the copy diverges from its source).
    const LONDON_TUBE_TOML: &[u8] =
        include_bytes!("../../tests/golden_fixtures/config_server_london_tube_v1/london-tube.toml");

    fn endpoint_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url")
    }

    fn secret_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key")
    }

    fn auth_mode_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::AuthMode {
            name: "backend-auth-mode".to_string(),
            tested_value: "api_key".to_string(),
        })
        .with_config_key("backend.auth.type")
    }

    fn london_tube_package_slots() -> Vec<ConfigSlot> {
        vec![endpoint_slot(), secret_slot(), auth_mode_slot()]
    }

    fn expect_violation(err: PackageError) -> (String, String) {
        match err {
            PackageError::ConfigSlotViolation { key, reason } => (key, reason),
            other => panic!("expected ConfigSlotViolation, got: {other}"),
        }
    }

    // --- Test 1: the real fixture parses to exactly its three declarations ---

    #[test]
    fn the_real_fixture_parses_to_its_three_declared_slots() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        assert_eq!(declared.len(), 3, "declared slots were: {declared:?}");

        assert_eq!(declared[0].key, "backend.base_url");
        assert_eq!(declared[0].kind, "endpoint");
        assert_eq!(declared[0].name, "TFL_BASE_URL");
        assert_eq!(
            declared[0].tested_value.as_deref(),
            Some("https://api.tfl.gov.uk"),
            "the endpoint records the value it was tested against"
        );

        assert_eq!(declared[1].key, "backend.auth.query_params.app_key");
        assert_eq!(declared[1].kind, "secret");
        assert_eq!(declared[1].name, "TFL_APP_KEY");
        assert_eq!(
            declared[1].tested_value, None,
            "an identity-bearing slot structurally carries no tested value"
        );

        assert_eq!(declared[2].key, "backend.auth.type");
        assert_eq!(declared[2].kind, "auth_mode");
        assert_eq!(declared[2].name, "backend-auth-mode");
        assert_eq!(declared[2].tested_value.as_deref(), Some("api_key"));
    }

    // --- Test 2: no declaration table is legal, not an error ---------------

    #[test]
    fn a_config_with_no_declaration_table_parses_to_an_empty_vec() {
        let declared =
            parse_declared_config_slots(b"name = \"x\"\n[backend]\nkind = \"openapi\"\n")
                .expect("a config that declares nothing is legal");
        assert!(declared.is_empty());
    }

    // --- Agreement: the matched case, order-insensitively -----------------

    #[test]
    fn agreement_holds_when_both_sides_describe_the_same_three_slots() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        validate_config_slot_agreement(&declared, &london_tube_package_slots()).unwrap();
    }

    #[test]
    fn agreement_compares_as_sets_so_declaration_order_is_not_load_bearing() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let reordered = vec![auth_mode_slot(), secret_slot(), endpoint_slot()];
        validate_config_slot_agreement(&declared, &reordered).unwrap();
    }

    // --- Test 4: declared in TOML, absent from the package -----------------

    #[test]
    fn a_declaration_with_no_matching_package_slot_names_that_key() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let missing_the_secret = vec![endpoint_slot(), auth_mode_slot()];
        let (key, reason) = expect_violation(
            validate_config_slot_agreement(&declared, &missing_the_secret).unwrap_err(),
        );
        assert_eq!(key, "backend.auth.query_params.app_key");
        assert!(reason.contains("absent from the package"), "was: {reason}");
    }

    // --- Test 5: invented by the package, absent from the TOML -------------

    #[test]
    fn a_package_slot_the_config_never_declares_names_that_key() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut invented = london_tube_package_slots();
        invented.push(
            ConfigSlot::new(SlotType::Secret {
                name: "INVENTED".to_string(),
            })
            .with_config_key("backend.auth.invented"),
        );
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &invented).unwrap_err());
        assert_eq!(key, "backend.auth.invented");
        assert!(reason.contains("absent from the config"), "was: {reason}");
    }

    // --- Test 6: same key, different kind ---------------------------------

    #[test]
    fn a_kind_disagreement_names_the_key_and_both_kinds() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_kind = vec![
            ConfigSlot::new(SlotType::Secret {
                name: "TFL_BASE_URL".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_kind).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("endpoint"), "was: {reason}");
        assert!(reason.contains("secret"), "was: {reason}");
    }

    // --- Test 7: same key and kind, different name / tested_value ----------

    #[test]
    fn a_name_disagreement_names_the_key_and_the_field_but_not_the_values() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_name = vec![
            ConfigSlot::new(SlotType::Endpoint {
                name: "SOMETHING_ELSE".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_name).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`name`"), "was: {reason}");
        assert!(
            !reason.contains("SOMETHING_ELSE"),
            "the message must name the FIELD, not echo the values; was: {reason}"
        );
    }

    #[test]
    fn a_tested_value_disagreement_names_the_key_and_the_field_but_not_the_values() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_tested = vec![
            ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://sentinel.invalid/other".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_tested).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`tested_value`"), "was: {reason}");
        assert!(
            !reason.contains("sentinel.invalid"),
            "the message must not echo either value; was: {reason}"
        );
    }

    // --- Slots with no config_key do not participate ----------------------

    #[test]
    fn a_package_slot_with_no_config_key_does_not_participate_in_agreement() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut with_unkeyed = london_tube_package_slots();
        with_unkeyed.push(ConfigSlot::new(SlotType::LlmProvider {
            name: "primary".to_string(),
            tested_value: "anthropic".to_string(),
        }));
        validate_config_slot_agreement(&declared, &with_unkeyed).unwrap();
    }

    // --- Duplicates on either side ----------------------------------------

    #[test]
    fn the_same_key_declared_twice_in_the_config_is_a_violation() {
        let declared = vec![
            DeclaredConfigSlot {
                key: "backend.base_url".to_string(),
                kind: "endpoint".to_string(),
                name: "A".to_string(),
                tested_value: None,
            },
            DeclaredConfigSlot {
                key: "backend.base_url".to_string(),
                kind: "endpoint".to_string(),
                name: "B".to_string(),
                tested_value: None,
            },
        ];
        let (key, reason) = expect_violation(
            validate_config_slot_agreement(&declared, &london_tube_package_slots()).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("more than once"), "was: {reason}");
    }

    #[test]
    fn the_same_config_key_claimed_by_two_package_slots_is_a_violation() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut doubled = london_tube_package_slots();
        doubled.push(endpoint_slot());
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &doubled).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("more than one slot"), "was: {reason}");
    }

    // --- Test 9: kind is re-validated here, not trusted -------------------

    #[test]
    fn an_unknown_kind_names_the_key_and_the_accepted_kinds_without_echoing_it() {
        let config = br#"
[[config_slots]]
key = "backend.base_url"
kind = "endpont"
name = "TFL_BASE_URL"
"#;
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("endpoint"), "was: {reason}");
        assert!(reason.contains("secret"), "was: {reason}");
        assert!(reason.contains("auth_mode"), "was: {reason}");
        assert!(
            !reason.contains("endpont"),
            "the rejected discriminator is document content and must not be echoed; was: {reason}"
        );
    }

    // --- Malformed entries ------------------------------------------------

    #[test]
    fn a_declaration_missing_its_key_is_named_by_position() {
        let config = b"[[config_slots]]\nkind = \"secret\"\nname = \"A\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "config_slots[0]");
        assert!(reason.contains("`key` is required"), "was: {reason}");
    }

    #[test]
    fn a_declaration_missing_its_name_is_named_by_key() {
        let config = b"[[config_slots]]\nkey = \"backend.base_url\"\nkind = \"endpoint\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`name` is required"), "was: {reason}");
    }

    #[test]
    fn a_non_string_tested_value_is_a_violation() {
        let config =
            b"[[config_slots]]\nkey = \"k\"\nkind = \"endpoint\"\nname = \"N\"\ntested_value = 7\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "k");
        assert!(reason.contains("`tested_value` must be"), "was: {reason}");
    }

    #[test]
    fn a_config_slots_key_that_is_not_an_array_of_tables_is_a_violation() {
        let config = b"config_slots = \"not-an-array\"\n";
        let (key, _) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "config_slots");
    }

    #[test]
    fn a_toml_syntax_error_is_reported_without_quoting_the_offending_line() {
        let config = b"backend = { api_key = \"super-secret-sentinel\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, DOCUMENT_LABEL);
        assert!(
            !reason.contains("super-secret-sentinel"),
            "the parser's snippet must not reach the message; was: {reason}"
        );
    }

    #[test]
    fn non_utf8_config_bytes_are_an_error_not_a_panic() {
        let (key, _) =
            expect_violation(parse_declared_config_slots(&[0xff, 0xfe, 0x00]).unwrap_err());
        assert_eq!(key, DOCUMENT_LABEL);
    }

    // --- Test 10: never-panic property over arbitrary bytes ---------------

    proptest! {
        #[test]
        fn parse_declared_config_slots_never_panics_on_arbitrary_bytes(
            bytes in proptest::collection::vec(any::<u8>(), 0..512)
        ) {
            // The contract is total: every input yields Ok or Err, never an unwind.
            let _ = parse_declared_config_slots(&bytes);
        }

        #[test]
        fn parse_declared_config_slots_never_panics_on_arbitrary_text(
            text in "\\PC{0,200}"
        ) {
            let _ = parse_declared_config_slots(text.as_bytes());
        }
    }

    // ===================================================================
    // Task 2 — D-04 placeholder validation, scoped by the exhaustive
    // three-way slot split (D-17).
    // ===================================================================

    /// A distinctive value that must never appear in an error message. If it
    /// does, the validator is echoing config content and a real credential
    /// would leak the same way.
    const SENTINEL_CREDENTIAL: &str = "sentinel-leaked-credential";
    const SENTINEL_ENDPOINT: &str = "https://sentinel.invalid/leaked";

    fn assert_names_key_without_echoing(err: PackageError, key: &str, forbidden: &str) {
        let message = err.to_string();
        assert!(
            message.contains(key),
            "the error must name the config key; message was: {message}"
        );
        assert!(
            !message.contains(forbidden),
            "the error must NOT echo the offending value; message was: {message}"
        );
    }

    // --- Test 1: an environment reference at a value key packs ------------

    #[test]
    fn an_endpoint_slot_over_an_environment_reference_is_accepted() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap();
    }

    #[test]
    fn the_env_colon_reference_form_is_accepted_too() {
        let config = b"[backend]\nbase_url = \"env:TFL_BASE_URL\"\n";
        validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap();
    }

    // --- Test 2: an endpoint holding a resolved literal is refused --------

    #[test]
    fn an_endpoint_slot_over_a_resolved_literal_is_refused_without_echoing_it() {
        let config = format!("[backend]\nbase_url = \"{SENTINEL_ENDPOINT}\"\n");
        let err =
            validate_config_slot_placeholders(config.as_bytes(), &[endpoint_slot()]).unwrap_err();
        assert_names_key_without_echoing(err, "backend.base_url", SENTINEL_ENDPOINT);
    }

    // --- Test 3: a credential holding a resolved literal is refused -------

    #[test]
    fn a_secret_slot_over_a_resolved_literal_is_refused_without_echoing_it() {
        let config = format!("[backend.auth.query_params]\napp_key = \"{SENTINEL_CREDENTIAL}\"\n");
        let err =
            validate_config_slot_placeholders(config.as_bytes(), &[secret_slot()]).unwrap_err();
        assert_names_key_without_echoing(
            err,
            "backend.auth.query_params.app_key",
            SENTINEL_CREDENTIAL,
        );
    }

    // --- Test 4: the auth-mode key is structurally exempt (D-17) ----------

    #[test]
    fn an_auth_mode_slot_over_a_baked_literal_is_accepted_because_it_is_structural() {
        // `AuthConfig` is internally tagged, so no placeholder form of this key
        // deserializes at all — the literal IS the only legal content.
        let config = b"[backend.auth]\ntype = \"api_key\"\n";
        validate_config_slot_placeholders(config, &[auth_mode_slot()]).unwrap();
    }

    // --- Test 5: config_key is conditional, not unconditionally skipped ---

    #[test]
    fn a_value_slot_with_no_config_key_is_a_violation_when_a_config_is_present() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        for slot in [
            ConfigSlot::new(SlotType::Secret {
                name: "TFL_APP_KEY".to_string(),
            }),
            ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            }),
        ] {
            let err =
                validate_config_slot_placeholders(config, std::slice::from_ref(&slot)).unwrap_err();
            let (key, reason) = expect_violation(err);
            assert_eq!(key, slot.slot.key().1);
            assert!(reason.contains("must name the config key"), "was: {reason}");
        }
    }

    #[test]
    fn a_non_config_slot_kind_with_no_config_key_is_skipped_not_rejected() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let slots = vec![
            ConfigSlot::new(SlotType::LlmProvider {
                name: "primary".to_string(),
                tested_value: "anthropic".to_string(),
            }),
            ConfigSlot::new(SlotType::BudgetOverride {
                name: "cap".to_string(),
                tested_value: "10".to_string(),
            }),
            ConfigSlot::new(SlotType::OauthClient {
                name: "client".to_string(),
            }),
            ConfigSlot::new(SlotType::ChannelBinding {
                name: "notify".to_string(),
            }),
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: "approves".to_string(),
                responsibilities: vec![],
                channel_hints: vec![],
            }),
        ];
        validate_config_slot_placeholders(config, &slots).unwrap();
    }

    // --- Test 6: a non-config variant that DOES carry a config_key --------

    #[test]
    fn a_non_config_slot_kind_carrying_a_config_key_is_a_violation() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let cases = [
            ConfigSlot::new(SlotType::LlmProvider {
                name: "primary".to_string(),
                tested_value: "anthropic".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: "approves".to_string(),
                responsibilities: vec![],
                channel_hints: vec![],
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::BudgetOverride {
                name: "cap".to_string(),
                tested_value: "10".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::OauthClient {
                name: "client".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::ChannelBinding {
                name: "notify".to_string(),
            })
            .with_config_key("backend.base_url"),
        ];
        for slot in cases {
            let err =
                validate_config_slot_placeholders(config, std::slice::from_ref(&slot)).unwrap_err();
            let (key, reason) = expect_violation(err);
            assert_eq!(key, "backend.base_url");
            assert!(
                reason.contains("no config-value semantics"),
                "was: {reason}"
            );
        }
    }

    // --- Test 7: a config_key naming nothing is a defect ------------------

    #[test]
    fn a_config_key_that_resolves_to_nothing_is_a_violation() {
        let config = b"[backend]\nother = \"x\"\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("resolves to nothing"), "was: {reason}");
    }

    // --- Test 8: the config_key grammar, stated and enforced --------------

    #[test]
    fn every_malformed_config_key_is_a_named_violation_not_a_silent_pass() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let cases: [(&str, &str); 6] = [
            ("", "is empty"),
            (".", "empty path component"),
            (".backend", "empty path component"),
            ("backend.", "empty path component"),
            ("backend..base_url", "empty path component"),
            ("backend.base_url.inner", "not a table"),
        ];
        for (config_key, expected_rule) in cases {
            let slot = ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            })
            .with_config_key(config_key);
            let (key, reason) =
                expect_violation(validate_config_slot_placeholders(config, &[slot]).unwrap_err());
            assert_eq!(key, config_key, "the error must name the offending key");
            assert!(
                reason.contains(expected_rule),
                "key {config_key:?} must state which rule it broke; was: {reason}"
            );
        }
    }

    #[test]
    fn a_quoted_or_indexed_config_key_is_rejected_rather_than_mis_resolved() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        for config_key in ["backend.\"base.url\"", "tools[0].path"] {
            let slot = ConfigSlot::new(SlotType::Endpoint {
                name: "N".to_string(),
                tested_value: "v".to_string(),
            })
            .with_config_key(config_key);
            let (_, reason) =
                expect_violation(validate_config_slot_placeholders(config, &[slot]).unwrap_err());
            assert!(reason.contains("bare key"), "was: {reason}");
        }
    }

    #[test]
    fn a_value_slot_key_addressing_a_non_string_is_a_violation() {
        let config = b"[backend]\nbase_url = 7\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("must hold a string"), "was: {reason}");
    }

    // --- The env-reference grammar itself ---------------------------------

    #[test]
    fn is_env_reference_recognises_exactly_the_two_reference_forms() {
        assert!(is_env_reference("${TFL_BASE_URL}"));
        assert!(is_env_reference("env:TFL_BASE_URL"));
        // Malformed empty-name forms name no variable, so they are not
        // references a target environment could fill.
        assert!(!is_env_reference("${}"));
        assert!(!is_env_reference("env:"));
        // Unterminated, trailing text, plain literals, whitespace-wrapped.
        assert!(!is_env_reference("${TFL_BASE_URL"));
        assert!(!is_env_reference("${TFL_BASE_URL}-suffix"));
        assert!(!is_env_reference("https://api.tfl.gov.uk"));
        assert!(!is_env_reference(""));
        assert!(!is_env_reference("  ${TFL_BASE_URL}  "));
    }

    // --- The real fixture passes both gates -------------------------------

    #[test]
    fn the_real_fixture_passes_placeholder_validation_for_all_three_slots() {
        validate_config_slot_placeholders(LONDON_TUBE_TOML, &london_tube_package_slots()).unwrap();
    }

    // --- Test 11: never-panic property (the FUZZ leg) ---------------------

    proptest! {
        /// FUZZ (CLAUDE.md ALWAYS): `pmcp-package` is workspace-excluded with its
        /// own `[workspace]` table, so a `cargo fuzz` target would need a second
        /// fuzz workspace outside every gate that runs today. A `proptest`
        /// never-panic property over the same newly-promoted TOML parse boundary
        /// buys the same guarantee INSIDE `make pmcp-package-gate`.
        #[test]
        fn validate_config_slot_placeholders_never_panics(
            bytes in proptest::collection::vec(any::<u8>(), 0..512),
            config_key in "\\PC{0,40}"
        ) {
            let slot = ConfigSlot::new(SlotType::Secret { name: "N".to_string() })
                .with_config_key(config_key);
            // Total on both axes: arbitrary config bytes (including non-UTF-8)
            // and arbitrary config keys (empty, dot-only, deeply dotted).
            let _ = validate_config_slot_placeholders(&bytes, &[slot]);
        }

        #[test]
        fn resolve_dotted_key_never_panics_on_arbitrary_dotted_keys(
            config_key in "[.a-zA-Z0-9_-]{0,40}"
        ) {
            let document: toml::Value =
                toml::from_str("[backend]\nbase_url = \"${X}\"\n[backend.auth]\ntype = \"t\"\n")
                    .unwrap();
            let _ = resolve_dotted_key(&document, &config_key);
        }
    }
}
