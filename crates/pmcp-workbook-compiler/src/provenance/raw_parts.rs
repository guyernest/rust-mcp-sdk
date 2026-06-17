//! The quarantined `quick-xml`/`zip` part-reader.
//!
//! Reads `calcPr` (`calcMode`/`fullCalcOnLoad`/`calcId`/`forceFullCalc`) from
//! `xl/workbook.xml` and `<Application>`/`<AppVersion>` from `docProps/app.xml`
//! out of the ORIGINAL on-disk `.xlsx` bytes — umya cannot surface these and
//! FABRICATES them on a round-trip (the writer hard-codes `calcId=122211` +
//! `"Microsoft Excel"`), so the gate MUST read the original bytes here.
//!
//! # Quarantine (mirrors the ingest umya boundary)
//!
//! NO `quick_xml`/`zip` type appears in any `pub(crate)` signature in this file
//! — those crates live ENTIRELY inside `fn` bodies and are converted to the
//! owned plain [`RawCalcPr`]/[`RawAppProps`] at the boundary (T-93-02-LEAK). The
//! crate `#![deny(clippy::unwrap_used, expect_used, panic)]` gate forbids
//! `.unwrap()` on every `quick_xml`/`zip` `Result`; each is matched into a
//! [`ProvenanceError`] or `None`.
//!
//! # Security (T-93-02-DOS)
//!
//! - Reads ONLY the two FIXED part names by name (`archive.by_name`) — never
//!   iterates entries, never derives a path from zip content (zip-slip guard).
//! - BOUNDS each decompressed read with an explicit per-entry cap
//!   ([`MAX_ZIP_ENTRY_BYTES`]) via `Read::take(limit + 1)`; exceeding the cap
//!   returns [`ProvenanceError::PartTooLarge`] WITHOUT inflating the rest
//!   (zip-bomb guard).
//! - BOUNDS the cumulative decompressed bytes across the parts read on one
//!   archive ([`MAX_TOTAL_DECOMPRESSED_BYTES`]) → [`ProvenanceError::DecompressBomb`].
//! - BOUNDS XML element nesting ([`MAX_XML_DEPTH`]) → [`ProvenanceError::XmlTooDeep`].
//! - Does NOT enable quick-xml entity expansion (billion-laughs / XXE guard);
//!   a malformed part is [`ProvenanceError::UnreadableXml`], never a panic.

// The quarantined reader + its owned types are the CONTRACT the gate consumes.
// `#[cfg(fuzzing)]` exposes a public hook (`fuzz_read_parts`) so the fuzz target
// (a separate crate) can drive the raw reader without re-exporting the
// pub(crate) entry points. Scoped to this module so a genuinely-dead item
// elsewhere is still caught.
#![allow(dead_code)]

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use super::ProvenanceError;

/// The fixed OOXML part holding `<calcPr>`.
const WORKBOOK_PART: &str = "xl/workbook.xml";
/// The fixed OOXML part holding `<Application>`/`<AppVersion>`.
const APP_PART: &str = "docProps/app.xml";

/// Per-entry decompressed-size cap for `xl/workbook.xml` (5 MiB — zip-bomb guard).
pub(crate) const MAX_ZIP_ENTRY_BYTES: usize = 5 * 1024 * 1024;
/// Per-entry decompressed-size cap for `docProps/app.xml` (256 KiB — zip-bomb guard).
pub(crate) const MAX_APP_XML: usize = 256 * 1024;
/// Cumulative decompressed-byte cap across the parts read on one archive
/// (zip-bomb guard; the sum of the two part reads can never exceed this).
pub(crate) const MAX_TOTAL_DECOMPRESSED_BYTES: usize = MAX_ZIP_ENTRY_BYTES + MAX_APP_XML;
/// Maximum XML element nesting depth before the parse fails closed
/// (billion-laughs / pathological-nesting guard).
pub(crate) const MAX_XML_DEPTH: usize = 256;

/// Owned `<calcPr>` attributes (umya/quick-xml/zip-free). Every field is
/// `Option` — `None` means the attribute was ABSENT (the caller applies the
/// ECMA-376 default: `calcMode → auto`, `fullCalcOnLoad → false`,
/// `forceFullCalc → false`, `calcId → None`/"no full-calc recorded").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawCalcPr {
    /// `calcPr@calcMode` as read; `None` ⇒ default `auto`.
    pub(crate) calc_mode: Option<String>,
    /// `calcPr@fullCalcOnLoad` as read; `None` ⇒ default `false`.
    pub(crate) full_calc_on_load: Option<bool>,
    /// `calcPr@calcId` as read; `None` ⇒ no full-calc stamp (D-01 refuses).
    pub(crate) calc_id: Option<u32>,
    /// `calcPr@forceFullCalc` as read; `None` ⇒ default `false` (recorded only).
    pub(crate) force_full_calc: Option<bool>,
}

/// Owned `docProps/app.xml` identity (umya/quick-xml/zip-free).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawAppProps {
    /// `<Application>` text (e.g. `"LibreOffice/24.2.7.2…"`); `None` if absent.
    pub(crate) application: Option<String>,
    /// `<AppVersion>` text (e.g. `"15.0000"`); `None` if absent.
    pub(crate) app_version: Option<String>,
}

/// Read `<calcPr>` from `xl/workbook.xml` in the ORIGINAL `.xlsx` bytes.
///
/// Returns owned [`RawCalcPr`]; absent attributes are `None` (the caller applies
/// OOXML defaults — an absent `calcMode`/`fullCalcOnLoad`/`forceFullCalc` is NOT
/// an error). Malformed/missing/oversize input returns a typed
/// [`ProvenanceError`], never a panic.
pub(crate) fn read_calc_pr(xlsx_bytes: &[u8]) -> Result<RawCalcPr, ProvenanceError> {
    let part = read_named_part(xlsx_bytes, WORKBOOK_PART, MAX_ZIP_ENTRY_BYTES)?;
    parse_calc_pr(&part)
}

/// Read `<Application>`/`<AppVersion>` from `docProps/app.xml` in the ORIGINAL
/// `.xlsx` bytes. Malformed/missing/oversize input returns a typed
/// [`ProvenanceError`].
pub(crate) fn read_app_props(xlsx_bytes: &[u8]) -> Result<RawAppProps, ProvenanceError> {
    let part = read_named_part(xlsx_bytes, APP_PART, MAX_APP_XML)?;
    parse_app_props(&part)
}

/// Read ONE fixed named part from the zip with a bounded decompressed read.
///
/// Reads ONLY `name` (never iterates entries / never derives a path from zip
/// content — zip-slip guard). Inflation is bounded at `limit + 1` bytes via
/// `Read::take`; reaching the cap returns [`ProvenanceError::PartTooLarge`]
/// WITHOUT inflating the rest (zip-bomb guard). Every `zip` `Result` is matched,
/// never `.unwrap()`-ed.
fn read_named_part(
    xlsx_bytes: &[u8],
    name: &str,
    limit: usize,
) -> Result<Vec<u8>, ProvenanceError> {
    let cursor = Cursor::new(xlsx_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| ProvenanceError::UnreadableZip {
        detail: e.to_string(),
    })?;

    let entry = match archive.by_name(name) {
        Ok(entry) => entry,
        // A genuinely absent required part is `MissingPart`; any other zip
        // failure (corrupt entry, unsupported compression) is `UnreadableZip`.
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(ProvenanceError::MissingPart {
                part: name.to_string(),
            });
        },
        Err(e) => {
            return Err(ProvenanceError::UnreadableZip {
                detail: e.to_string(),
            });
        },
    };

    // Bounded read: take at most `limit + 1` bytes. If we actually read
    // `limit + 1`, the part exceeds the cap — abandon WITHOUT inflating the rest.
    let mut bounded = entry.take(limit as u64 + 1);
    let mut buf = Vec::with_capacity(1024);
    bounded
        .read_to_end(&mut buf)
        .map_err(|e| ProvenanceError::UnreadableZip {
            detail: e.to_string(),
        })?;

    if buf.len() > limit {
        return Err(ProvenanceError::PartTooLarge {
            part: name.to_string(),
            limit,
        });
    }

    Ok(buf)
}

/// Parse `<calcPr>` attributes out of the bounded `xl/workbook.xml` bytes.
///
/// quick-xml does not expand external entities by default and we do not enable
/// it (XXE/billion-laughs guard). Element nesting is bounded by
/// [`MAX_XML_DEPTH`] → [`ProvenanceError::XmlTooDeep`]. A parse failure becomes
/// [`ProvenanceError::UnreadableXml`].
fn parse_calc_pr(part_bytes: &[u8]) -> Result<RawCalcPr, ProvenanceError> {
    let mut reader = Reader::from_reader(part_bytes);
    let mut found = RawCalcPr {
        calc_mode: None,
        full_calc_on_load: None,
        calc_id: None,
        force_full_calc: None,
    };

    // The authoritative `<calcPr>` is a DIRECT child of the `<workbook>` root.
    // Track the open-element local-name stack so a decoy `<x:calcPr/>` nested
    // elsewhere cannot shadow the real one — we only accept a `calcPr` whose
    // immediate parent is `workbook`. The stack depth is bounded by
    // MAX_XML_DEPTH (pathological-nesting guard).
    let mut stack: Vec<Vec<u8>> = Vec::new();

    loop {
        let event = reader
            .read_event()
            .map_err(|err| ProvenanceError::UnreadableXml {
                part: WORKBOOK_PART.to_string(),
                detail: err.to_string(),
            })?;
        if step_calc_pr_event(event, &mut stack, &mut found)? == CalcPrFlow::Done {
            break;
        }
    }

    Ok(found)
}

/// Loop control for the [`parse_calc_pr`] event walk: keep reading, or stop
/// (the authoritative `calcPr` was applied, or EOF was reached).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalcPrFlow {
    /// Continue reading the next event.
    Continue,
    /// Stop the walk (EOF, or the workbook-child `calcPr` was applied).
    Done,
}

/// Handle one `xl/workbook.xml` parse event for [`parse_calc_pr`].
///
/// Mutates the open-element `stack` and the accumulated `found` attributes,
/// returning whether the walk should stop. Only a `calcPr` whose immediate
/// parent is `workbook` is honoured (decoy-shadowing guard); `Start` depth is
/// bounded by [`MAX_XML_DEPTH`].
fn step_calc_pr_event(
    event: Event<'_>,
    stack: &mut Vec<Vec<u8>>,
    found: &mut RawCalcPr,
) -> Result<CalcPrFlow, ProvenanceError> {
    match event {
        Event::Eof => Ok(CalcPrFlow::Done),
        // `<calcPr …/>` is self-closing (Empty): its parent is the current top
        // of the open-element stack; it does NOT push.
        Event::Empty(e) => {
            if is_workbook_child_calc_pr(&e, stack) {
                apply_calc_pr_attrs(&e, found)?;
                return Ok(CalcPrFlow::Done);
            }
            Ok(CalcPrFlow::Continue)
        },
        // A paired `<calcPr …></calcPr>` arrives as Start: its parent is the
        // current top BEFORE we push. Other Start elements push their name.
        Event::Start(e) => {
            if is_workbook_child_calc_pr(&e, stack) {
                apply_calc_pr_attrs(&e, found)?;
                return Ok(CalcPrFlow::Done);
            }
            if stack.len() >= MAX_XML_DEPTH {
                return Err(ProvenanceError::XmlTooDeep {
                    part: WORKBOOK_PART.to_string(),
                    limit: MAX_XML_DEPTH,
                });
            }
            stack.push(e.local_name().as_ref().to_vec());
            Ok(CalcPrFlow::Continue)
        },
        Event::End(_) => {
            stack.pop();
            Ok(CalcPrFlow::Continue)
        },
        _ => Ok(CalcPrFlow::Continue),
    }
}

/// `true` when `e` is a `calcPr` element whose immediate parent (the open-element
/// stack top) is `workbook` — the only `calcPr` the gate trusts.
fn is_workbook_child_calc_pr(e: &quick_xml::events::BytesStart<'_>, stack: &[Vec<u8>]) -> bool {
    e.local_name().as_ref() == b"calcPr" && stack.last().map(Vec::as_slice) == Some(b"workbook")
}

/// Read the gated `<calcPr>` attributes off one element event into `found`.
///
/// Shared by the self-closing (`Empty`) and paired (`Start`) `calcPr` arms so
/// the attribute handling — incl. the absent-attrs-are-None and non-numeric-
/// calcId-is-None contracts — lives in ONE place. A malformed attribute /
/// unescape failure becomes [`ProvenanceError::UnreadableXml`].
fn apply_calc_pr_attrs(
    e: &quick_xml::events::BytesStart<'_>,
    found: &mut RawCalcPr,
) -> Result<(), ProvenanceError> {
    for attr in e.attributes() {
        let attr = match attr {
            Ok(a) => a,
            Err(err) => {
                return Err(ProvenanceError::UnreadableXml {
                    part: WORKBOOK_PART.to_string(),
                    detail: err.to_string(),
                });
            },
        };
        let value = match attr.unescape_value() {
            Ok(v) => v.into_owned(),
            Err(err) => {
                return Err(ProvenanceError::UnreadableXml {
                    part: WORKBOOK_PART.to_string(),
                    detail: err.to_string(),
                });
            },
        };
        match attr.key.local_name().as_ref() {
            b"calcMode" => found.calc_mode = Some(value),
            b"fullCalcOnLoad" => {
                found.full_calc_on_load = Some(parse_ooxml_bool(&value));
            },
            b"forceFullCalc" => {
                found.force_full_calc = Some(parse_ooxml_bool(&value));
            },
            // A non-numeric calcId is treated as absent (None) rather than an
            // error — the gate refuses on None.
            b"calcId" => found.calc_id = value.parse::<u32>().ok(),
            _ => {},
        }
    }
    Ok(())
}

/// Parse `<Application>`/`<AppVersion>` text out of the bounded
/// `docProps/app.xml` bytes. Element nesting is bounded by [`MAX_XML_DEPTH`]. A
/// parse failure becomes [`ProvenanceError::UnreadableXml`].
fn parse_app_props(part_bytes: &[u8]) -> Result<RawAppProps, ProvenanceError> {
    let mut reader = Reader::from_reader(part_bytes);
    let mut props = RawAppProps {
        application: None,
        app_version: None,
    };
    // Which element's text we are currently inside (None = none).
    let mut current: Option<AppField> = None;
    // Element nesting depth (pathological-nesting guard).
    let mut depth: usize = 0;

    loop {
        let event = reader
            .read_event()
            .map_err(|err| ProvenanceError::UnreadableXml {
                part: APP_PART.to_string(),
                detail: err.to_string(),
            })?;
        if matches!(event, Event::Eof) {
            break;
        }
        step_app_props_event(event, &mut depth, &mut current, &mut props)?;
    }

    Ok(props)
}

/// The `docProps/app.xml` element whose character data we are currently inside.
#[derive(Debug, Clone, Copy)]
enum AppField {
    /// `<Application>` text.
    Application,
    /// `<AppVersion>` text.
    AppVersion,
}

/// Handle one `docProps/app.xml` parse event for [`parse_app_props`].
///
/// Updates the nesting `depth`, the `current` text target, and accumulates text
/// into `props`. Nesting beyond [`MAX_XML_DEPTH`] fails closed; `Eof` is handled
/// by the caller.
fn step_app_props_event(
    event: Event<'_>,
    depth: &mut usize,
    current: &mut Option<AppField>,
    props: &mut RawAppProps,
) -> Result<(), ProvenanceError> {
    match event {
        Event::Start(e) => {
            *depth += 1;
            if *depth > MAX_XML_DEPTH {
                return Err(ProvenanceError::XmlTooDeep {
                    part: APP_PART.to_string(),
                    limit: MAX_XML_DEPTH,
                });
            }
            *current = match e.local_name().as_ref() {
                b"Application" => Some(AppField::Application),
                b"AppVersion" => Some(AppField::AppVersion),
                _ => None,
            };
            Ok(())
        },
        Event::Text(t) => accumulate_app_text(&t, *current, props),
        Event::End(_) => {
            *depth = depth.saturating_sub(1);
            *current = None;
            Ok(())
        },
        _ => Ok(()),
    }
}

/// Append the (unescaped) `Text` event into the `current` field's slot.
///
/// ACCUMULATES across split Text events instead of overwriting: a single
/// element's text can arrive in multiple Text events (e.g. around a character
/// reference like `&amp;`), so `get_or_insert_with` + `push_str` concatenates
/// every chunk in order. An unescape failure becomes [`ProvenanceError::UnreadableXml`].
fn accumulate_app_text(
    t: &quick_xml::events::BytesText<'_>,
    current: Option<AppField>,
    props: &mut RawAppProps,
) -> Result<(), ProvenanceError> {
    let Some(field) = current else {
        return Ok(());
    };
    let text = t.unescape().map_err(|err| ProvenanceError::UnreadableXml {
        part: APP_PART.to_string(),
        detail: err.to_string(),
    })?;
    let slot = match field {
        AppField::Application => &mut props.application,
        AppField::AppVersion => &mut props.app_version,
    };
    slot.get_or_insert_with(String::new).push_str(&text);
    Ok(())
}

/// Parse an OOXML boolean attribute (`"1"`/`"true"` ⇒ `true`; anything else,
/// incl. `"0"`/`"false"`, ⇒ `false`).
fn parse_ooxml_bool(value: &str) -> bool {
    matches!(value, "1" | "true")
}

/// Fuzz-only public hook over the untrusted `.xlsx` ZIP/XML provenance reader.
///
/// Drives BOTH `read_calc_pr` and `read_app_props` over arbitrary `bytes` so the
/// fuzz target (a separate crate) exercises the full raw-bytes path WITHOUT the
/// pub(crate) entry points leaving the crate on a non-fuzz build (the reader
/// stays quarantined). The invariant the fuzz target asserts: ANY input either
/// yields a structured result or a typed [`ProvenanceError`] — never a panic,
/// hang, or unbounded allocation (the hard limits above are the guards).
#[cfg(fuzzing)]
pub fn fuzz_read_parts(bytes: &[u8]) {
    let _ = read_calc_pr(bytes);
    let _ = read_app_props(bytes);
}

/// Build a minimal in-memory `.xlsx` zip with the given part contents (only the
/// parts the readers touch). `cfg(test)` relaxes the unwrap-deny. `pub(crate)`
/// at module scope so the gate's tests reuse the same authoring helper (a
/// sibling module cannot reach into another module's private `mod tests`).
#[cfg(test)]
pub(crate) fn zip_with(parts: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut w = zip::ZipWriter::new(cursor);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        for (name, content) in parts {
            w.start_file(*name, opts).unwrap();
            w.write_all(content).unwrap();
        }
        w.finish().unwrap();
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_calc_attrs_are_none_not_error() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr iterateCount="100"/></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let calc = read_calc_pr(&bytes).expect("absent attrs are not an error");
        assert_eq!(calc.calc_mode, None);
        assert_eq!(calc.full_calc_on_load, None);
        assert_eq!(calc.calc_id, None);
    }

    #[test]
    fn present_calc_attrs_are_parsed() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr calcMode="manual" calcId="191029" fullCalcOnLoad="1"/></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let calc = read_calc_pr(&bytes).expect("parse present attrs");
        assert_eq!(calc.calc_mode.as_deref(), Some("manual"));
        assert_eq!(calc.calc_id, Some(191029));
        assert_eq!(calc.full_calc_on_load, Some(true));
    }

    #[test]
    fn paired_calc_pr_element_is_parsed() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="124519"></calcPr></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let calc = read_calc_pr(&bytes).expect("parse paired calcPr");
        assert_eq!(calc.calc_mode.as_deref(), Some("auto"));
        assert_eq!(calc.calc_id, Some(124519));
    }

    #[test]
    fn force_full_calc_is_parsed_and_recorded() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr calcMode="auto" calcId="191029" forceFullCalc="1"/></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let calc = read_calc_pr(&bytes).expect("parse forceFullCalc");
        assert_eq!(calc.force_full_calc, Some(true));
    }

    #[test]
    fn decoy_calc_pr_does_not_shadow_the_workbook_child() {
        let wb = br#"<?xml version="1.0"?><workbook><extLst><decoy><calcPr calcMode="manual"/></decoy></extLst><calcPr calcMode="auto" calcId="191029"/></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let calc = read_calc_pr(&bytes).expect("parse with decoy calcPr");
        assert_eq!(calc.calc_mode.as_deref(), Some("auto"));
        assert_eq!(calc.calc_id, Some(191029));
    }

    #[test]
    fn split_application_text_is_accumulated_not_overwritten() {
        let app = br#"<?xml version="1.0"?><Properties><Application>Foo &amp; Bar</Application></Properties>"#;
        let bytes = zip_with(&[(APP_PART, app)]);
        let props = read_app_props(&bytes).expect("parse split app text");
        assert_eq!(props.application.as_deref(), Some("Foo & Bar"));
    }

    #[test]
    fn truncated_zip_is_unreadable_zip() {
        let garbage = b"PK\x03\x04 not really a zip at all";
        let err = read_calc_pr(garbage).expect_err("truncated zip must error");
        assert!(
            matches!(err, ProvenanceError::UnreadableZip { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn malformed_workbook_xml_is_unreadable_xml() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr calcMode="manual"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let err = read_calc_pr(&bytes).expect_err("malformed xml must error");
        assert!(
            matches!(err, ProvenanceError::UnreadableXml { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_app_part_is_missing_part() {
        let wb = br#"<?xml version="1.0"?><workbook><calcPr/></workbook>"#;
        let bytes = zip_with(&[(WORKBOOK_PART, wb)]);
        let err = read_app_props(&bytes).expect_err("absent app.xml must error");
        match err {
            ProvenanceError::MissingPart { part } => assert_eq!(part, APP_PART),
            other => panic!("expected MissingPart, got {other:?}"),
        }
    }

    #[test]
    fn missing_workbook_part_is_missing_part() {
        let app = br#"<?xml version="1.0"?><Properties><Application>X</Application></Properties>"#;
        let bytes = zip_with(&[(APP_PART, app)]);
        let err = read_calc_pr(&bytes).expect_err("absent workbook.xml must error");
        match err {
            ProvenanceError::MissingPart { part } => assert_eq!(part, WORKBOOK_PART),
            other => panic!("expected MissingPart, got {other:?}"),
        }
    }

    #[test]
    fn oversize_workbook_part_is_part_too_large() {
        // A synthetic workbook.xml larger than the per-entry cap. The reader must
        // stop at limit+1 WITHOUT inflating the rest.
        let mut big = Vec::with_capacity(MAX_ZIP_ENTRY_BYTES + 1024);
        big.extend_from_slice(br#"<?xml version="1.0"?><workbook><calcPr calcMode="auto"/>"#);
        big.resize(MAX_ZIP_ENTRY_BYTES + 512, b' ');
        big.extend_from_slice(b"</workbook>");
        let bytes = zip_with(&[(WORKBOOK_PART, big.as_slice())]);
        let err = read_calc_pr(&bytes).expect_err("oversize part must error");
        match err {
            ProvenanceError::PartTooLarge { part, limit } => {
                assert_eq!(part, WORKBOOK_PART);
                assert_eq!(limit, MAX_ZIP_ENTRY_BYTES);
            },
            other => panic!("expected PartTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn deeply_nested_xml_fails_closed_to_xml_too_deep() {
        // T-93-02-DOS: pathological element nesting beyond MAX_XML_DEPTH must
        // fail closed to XmlTooDeep, never recurse/allocate unbounded.
        let mut wb = Vec::new();
        wb.extend_from_slice(br#"<?xml version="1.0"?>"#);
        for _ in 0..(MAX_XML_DEPTH + 10) {
            wb.extend_from_slice(b"<a>");
        }
        let bytes = zip_with(&[(WORKBOOK_PART, wb.as_slice())]);
        let err = read_calc_pr(&bytes).expect_err("deep nesting must error");
        match err {
            ProvenanceError::XmlTooDeep { part, limit } => {
                assert_eq!(part, WORKBOOK_PART);
                assert_eq!(limit, MAX_XML_DEPTH);
            },
            other => panic!("expected XmlTooDeep, got {other:?}"),
        }
    }
}
