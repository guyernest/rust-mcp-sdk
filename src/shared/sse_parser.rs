//! Server-Sent Events (SSE) parser for MCP HTTP transport.
//!
//! This module provides a robust SSE parser compatible with the
//! `EventSource` specification, similar to eventsource-parser in TypeScript.

use std::collections::HashMap;
use std::fmt;

/// SSE event parsed from the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Event ID for resumption
    pub id: Option<String>,
    /// Event type/name
    pub event: Option<String>,
    /// Event data
    pub data: String,
    /// Retry interval in milliseconds
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Create a new SSE event with data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("Hello, world!");
    /// assert_eq!(event.data, "Hello, world!");
    /// assert!(event.id.is_none());
    /// assert!(event.event.is_none());
    /// ```
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            id: None,
            event: None,
            data: data.into(),
            retry: None,
        }
    }

    /// Set the event ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_id("msg-123");
    /// assert_eq!(event.id, Some("msg-123".to_string()));
    /// ```
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the event type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_event("custom");
    /// assert_eq!(event.event, Some("custom".to_string()));
    /// ```
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the retry interval.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseEvent;
    ///
    /// let event = SseEvent::new("data")
    ///     .with_retry(3000);
    /// assert_eq!(event.retry, Some(3000));
    /// ```
    pub fn with_retry(mut self, retry: u64) -> Self {
        self.retry = Some(retry);
        self
    }
}

impl fmt::Display for SseEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(id) = &self.id {
            writeln!(f, "id: {}", id)?;
        }
        if let Some(event) = &self.event {
            writeln!(f, "event: {}", event)?;
        }
        if let Some(retry) = self.retry {
            writeln!(f, "retry: {}", retry)?;
        }

        // Split data by newlines and write each line
        for line in self.data.lines() {
            writeln!(f, "data: {}", line)?;
        }

        writeln!(f)?; // Empty line to end event
        Ok(())
    }
}

/// SSE parser state machine.
#[derive(Debug)]
pub struct SseParser {
    buffer: String,
    current_event: EventBuilder,
    last_event_id: Option<String>,
    /// Upper bound on `buffer`, i.e. on ONE unterminated SSE line.
    ///
    /// The bytes fed to a parser are chosen by a REMOTE peer, and `buffer` only
    /// ever drains as far as a `\n`. Without a bound, a peer that never emits
    /// one grows this process's heap for as long as it holds the stream open.
    max_buffer_size: usize,
    /// Latched once an oversized line has been discarded.
    overflowed: bool,
}

/// The default bound on ONE unterminated SSE line, in bytes (1 MiB).
///
/// Single source of truth for [`SseParser::new`] and [`SseConfig::default()`],
/// so the two can never disagree. Reading it through `SseConfig::default()`
/// would allocate that struct's `HashMap` and four `String`s just to fetch one
/// `usize`, on a path taken once per SSE response.
pub const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024;

/// Split the longest decodable UTF-8 prefix off `buffer`, leaving the rest.
///
/// The companion every INCREMENTAL feeder of [`SseParser`] needs, and the reason
/// it lives beside the parser rather than beside one of them: a body chunk
/// boundary can fall in the MIDDLE of a multi-byte character, so a per-chunk
/// `String::from_utf8_lossy` corrupts any non-ASCII payload (a `file:///café.txt`
/// resource URI, a non-Latin tool argument) that happens to straddle two frames.
/// An INCOMPLETE tail is retained for the next chunk; genuinely INVALID bytes are
/// replaced with U+FFFD immediately, because retaining those forever would wedge
/// the stream on hostile input (T-113-67).
///
/// The two cases are handled INDEPENDENTLY rather than "any invalid byte means
/// decode the whole buffer lossily": a chunk that carries both an invalid byte
/// AND a trailing incomplete character would otherwise have that trailing
/// character replaced too, corrupting a legitimate multi-byte character that the
/// next chunk was about to complete.
///
/// The retained tail is at most 3 bytes, so this cannot grow without bound.
pub(crate) fn take_utf8_prefix(buffer: &mut Vec<u8>) -> String {
    let mut text = String::new();
    loop {
        let error = match std::str::from_utf8(buffer) {
            Ok(valid) => {
                text.push_str(valid);
                buffer.clear();
                return text;
            },
            Err(error) => error,
        };
        let valid_up_to = error.valid_up_to();
        if let Ok(valid) = std::str::from_utf8(&buffer[..valid_up_to]) {
            text.push_str(valid);
        }
        let Some(invalid_len) = error.error_len() else {
            // "Unexpected end of input": an incomplete character the next chunk
            // will finish. Keep exactly those bytes and yield what decoded.
            buffer.drain(..valid_up_to);
            return text;
        };
        // Never completable — emit the replacement character and skip past it.
        text.push('\u{FFFD}');
        buffer.drain(..valid_up_to + invalid_len);
    }
}

impl SseParser {
    /// Create a new SSE parser bounded by [`DEFAULT_MAX_BUFFER_SIZE`] (1 MiB),
    /// the same value [`SseConfig::default()`] carries.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// assert!(parser.last_event_id().is_none());
    /// assert!(!parser.overflowed());
    /// ```
    pub fn new() -> Self {
        Self::with_max_buffer_size(DEFAULT_MAX_BUFFER_SIZE)
    }

    /// Create a new SSE parser with an explicit line-buffer bound.
    ///
    /// [`SseParser::new`] takes its bound from [`SseConfig::default()`]'s
    /// `max_buffer_size`. Use this constructor when a caller needs a TIGHTER
    /// one — a long-lived stream of small frames read from an untrusted remote
    /// peer, for example — or a looser one.
    ///
    /// The bound applies to ONE unterminated line. A chunk that would push the
    /// buffer past it while completing no line at all is DISCARDED, not
    /// truncated-and-emitted, and [`Self::overflowed`] latches: a silently
    /// truncated line would surface later as a misleading JSON parse failure,
    /// which is strictly worse for an operator than a named one.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::with_max_buffer_size(64);
    ///
    /// // A peer that never sends a newline cannot grow this parser's heap.
    /// assert!(parser.feed(&"x".repeat(1024)).is_empty());
    /// assert!(parser.overflowed());
    ///
    /// // The parser keeps working — but the flag stays latched.
    /// let events = parser.feed("data: ok\n\n");
    /// assert_eq!(events[0].data, "ok");
    /// assert!(parser.overflowed());
    /// ```
    #[must_use]
    pub fn with_max_buffer_size(max_buffer_size: usize) -> Self {
        Self {
            buffer: String::new(),
            current_event: EventBuilder::new(),
            last_event_id: None,
            max_buffer_size,
            overflowed: false,
        }
    }

    /// Whether this parser has DISCARDED an oversized line.
    ///
    /// LATCHING: once set it stays set for the parser's lifetime — including
    /// across [`Self::reset`] — so a caller that polls once per chunk cannot
    /// miss the event. An overflowed parser has lost bytes a remote peer sent,
    /// so its stream should be considered CORRUPT: the recommended response is
    /// to end the stream with an error naming the limit, not to keep parsing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    /// let _ = parser.feed("data: a well-formed event\n\n");
    /// assert!(!parser.overflowed());
    /// ```
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// The bound THIS parser was built with, in bytes.
    ///
    /// A caller reporting an overflow should name this rather than re-deriving
    /// a bound from config: parsers on different paths are deliberately built
    /// with different bounds, so a re-derived number can name a limit the
    /// parser never had.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// assert_eq!(SseParser::with_max_buffer_size(64).max_buffer_size(), 64);
    /// ```
    #[must_use]
    pub fn max_buffer_size(&self) -> usize {
        self.max_buffer_size
    }

    /// Feed data to the parser and get parsed events.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::sse_parser::SseParser;
    ///
    /// let mut parser = SseParser::new();
    ///
    /// // Simple event
    /// let events = parser.feed("data: Hello\n\n");
    /// assert_eq!(events.len(), 1);
    /// assert_eq!(events[0].data, "Hello");
    ///
    /// // Event with ID
    /// let events = parser.feed("id: 123\ndata: World\n\n");
    /// assert_eq!(events[0].id, Some("123".to_string()));
    /// assert_eq!(events[0].data, "World");
    ///
    /// // Multi-line data
    /// let events = parser.feed("data: Line 1\ndata: Line 2\n\n");
    /// assert_eq!(events[0].data, "Line 1\nLine 2");
    ///
    /// // Custom event type
    /// let events = parser.feed("event: ping\ndata: pong\n\n");
    /// assert_eq!(events[0].event, Some("ping".to_string()));
    /// ```
    pub fn feed(&mut self, data: &str) -> Vec<SseEvent> {
        // `buffer` holds ONE unterminated line, and a REMOTE peer decides when
        // (or whether) that line ends. Bound it: when appending `data` would
        // push the buffer past `max_buffer_size` AND this chunk can complete no
        // line at all, the line is unbounded by construction — so DISCARD it
        // and latch `overflowed` rather than grow.
        //
        // Only `data` has to be tested. The drain loop below runs to exhaustion
        // (`drain(..=line_end)` consumes through each `\n`), and `process_line`
        // never touches `buffer`, so every `feed` RETURNS with a newline-free
        // buffer and it starts empty — an unterminated line is all it can hold.
        // The question is therefore only whether THIS chunk can end that line.
        //
        // The `data` condition is what keeps a single legitimately large
        // COMPLETE body working: such a body carries newlines and drains in the
        // loop below, so it never trips the bound. The two whole-body `feed`
        // call sites in the streamable-HTTP transport are therefore
        // behaviourally unchanged.
        debug_assert!(
            !self.buffer.contains('\n'),
            "the drain loop leaves no newline in the buffer"
        );
        if self.buffer.len().saturating_add(data.len()) > self.max_buffer_size
            && !data.contains('\n')
        {
            self.overflowed = true;
            self.buffer.clear();
            // The in-progress event is now missing a line; anything built from
            // it would be a corrupted frame presented to the caller as genuine.
            self.current_event = EventBuilder::new();
            // `last_event_id` is deliberately untouched: it is stream-level
            // resumption state, not line state.
            return Vec::new();
        }

        self.buffer.push_str(data);
        let mut events = Vec::new();

        while let Some(line_end) = self.buffer.find('\n') {
            // `line_end` is a BYTE index (`str::find` returns one). The CRLF
            // check must therefore also be a BYTE check.
            //
            // It used to read `self.buffer.chars().nth(line_end - 1)`, which
            // indexes by CHARACTER. On any buffer containing a multi-byte
            // character the two disagree, so the check could report `'\r'` for
            // a position that is not byte `line_end - 1`, and the slice that
            // followed (`self.buffer[..line_end - 1]`) then cut INSIDE a
            // character and PANICKED — on bytes supplied by a remote server.
            // Found by `client::subscriptions`'s arbitrary-bytes property test
            // (Phase 113-13, T-113-67); `feed_never_panics_on_arbitrary_text`
            // below is the permanent guard.
            //
            // `\n` and `\r` are ASCII, so both `line_end` and `line_end - 1`
            // (taken only when that byte IS `\r`) are guaranteed char
            // boundaries.
            let line_start_len = if line_end > 0 && self.buffer.as_bytes()[line_end - 1] == b'\r' {
                line_end - 1
            } else {
                line_end
            };
            let line = self.buffer[..line_start_len].to_string();

            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }

            self.buffer.drain(..=line_end);
        }

        // The pre-check above can only refuse a chunk that completes NO line, so
        // a chunk that begins with `\n` and then carries megabytes of
        // newline-free bytes sails past it: the drain loop consumes through that
        // single `\n` and leaves the whole remainder as one unterminated line,
        // arbitrarily larger than `max_buffer_size`. Re-check the RESIDUAL so the
        // bound holds on what this call actually leaves behind, not only on what
        // it was asked to add.
        if self.buffer.len() > self.max_buffer_size {
            self.overflowed = true;
            self.buffer.clear();
            self.current_event = EventBuilder::new();
        }

        events
    }

    /// Process a single line and potentially emit an event.
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line dispatches the event
        if line.is_empty() {
            return self.dispatch_event();
        }

        // Comment line (starts with :)
        if line.starts_with(':') {
            return None;
        }

        // Parse field and value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Remove leading space from value if present
            let value = value.strip_prefix(' ').unwrap_or(value);
            (field, value)
        } else {
            // Field without value
            (line, "")
        };

        // Process field
        match field {
            "event" => {
                self.current_event.event = Some(value.to_string());
            },
            "data" => {
                if self.current_event.data.is_empty() {
                    self.current_event.data = value.to_string();
                } else {
                    self.current_event.data.push('\n');
                    self.current_event.data.push_str(value);
                }
            },
            "id" if !value.contains('\0') => {
                self.current_event.id = Some(value.to_string());
                self.last_event_id = Some(value.to_string());
            },
            "retry" => {
                if let Ok(retry) = value.parse::<u64>() {
                    self.current_event.retry = Some(retry);
                }
            },
            _ => {
                // Unknown field, ignore
            },
        }

        None
    }

    /// Dispatch the current event if it has data.
    fn dispatch_event(&mut self) -> Option<SseEvent> {
        if self.current_event.data.is_empty() {
            // No data, don't dispatch
            self.current_event = EventBuilder::new();
            return None;
        }

        let event = SseEvent {
            id: self
                .current_event
                .id
                .clone()
                .or_else(|| self.last_event_id.clone()),
            event: self.current_event.event.clone(),
            data: self.current_event.data.clone(),
            retry: self.current_event.retry,
        };

        self.current_event = EventBuilder::new();
        Some(event)
    }

    /// Get the last event ID seen.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Reset the parser state.
    ///
    /// Clears the line buffer and any in-progress event. It deliberately does
    /// NOT clear [`Self::overflowed`], which records that bytes a peer sent were
    /// already LOST — a fact resetting the line state cannot undo.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.current_event = EventBuilder::new();
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SSE events during parsing.
#[derive(Debug, Clone)]
struct EventBuilder {
    id: Option<String>,
    event: Option<String>,
    data: String,
    retry: Option<u64>,
}

impl EventBuilder {
    fn new() -> Self {
        Self {
            id: None,
            event: None,
            data: String::new(),
            retry: None,
        }
    }
}

/// SSE stream builder for creating SSE responses.
#[derive(Debug)]
pub struct SseStream {
    events: Vec<SseEvent>,
}

impl SseStream {
    /// Create a new SSE stream builder.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the stream.
    pub fn event(mut self, event: SseEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add a simple data event.
    pub fn data(self, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data))
    }

    /// Add a typed event with data.
    pub fn typed_event(self, event_type: impl Into<String>, data: impl Into<String>) -> Self {
        self.event(SseEvent::new(data).with_event(event_type))
    }

    /// Add a comment line.
    pub fn comment(self, _comment: impl Into<String>) -> Self {
        // Comments are not stored as events, they're just for keep-alive
        // In a real implementation, we'd write this directly to the stream
        self
    }

    /// Build the SSE stream as a string.
    pub fn build(self) -> String {
        self.events
            .into_iter()
            .map(|e| e.to_string())
            .collect::<String>()
    }
}

impl Default for SseStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for SSE connections.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Reconnection retry interval in milliseconds
    pub retry: u64,
    /// Maximum buffer size for incomplete lines.
    ///
    /// [`SseParser::new`] takes its bound from this field's DEFAULT value, and
    /// [`SseParser::with_max_buffer_size`] overrides it per parser. A chunk that
    /// would push a parser's line buffer past the bound without completing any
    /// line is discarded and latches [`SseParser::overflowed`], so a peer that
    /// never emits a newline cannot grow the process's heap without limit.
    pub max_buffer_size: usize,
    /// Enable compression
    pub compression: bool,
    /// Custom headers
    pub headers: HashMap<String, String>,
}

impl Default for SseConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());

        Self {
            retry: 3000,
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
            compression: false,
            headers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parser_simple() {
        let mut parser = SseParser::new();

        let input = "data: hello world\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
        assert_eq!(events[0].event, None);
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn test_sse_parser_with_event_type() {
        let mut parser = SseParser::new();

        let input = "event: message\ndata: hello\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event, Some("message".to_string()));
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();

        let input = "data: line 1\ndata: line 2\ndata: line 3\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_sse_parser_with_id() {
        let mut parser = SseParser::new();

        let input = "id: 123\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("123".to_string()));
        assert_eq!(parser.last_event_id(), Some("123"));
    }

    #[test]
    fn test_sse_parser_with_retry() {
        let mut parser = SseParser::new();

        let input = "retry: 5000\ndata: test\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn test_sse_parser_comments() {
        let mut parser = SseParser::new();

        let input = ": this is a comment\ndata: actual data\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_sse_parser_incremental() {
        let mut parser = SseParser::new();

        // Feed data incrementally
        let events1 = parser.feed("data: par");
        assert_eq!(events1.len(), 0);

        let events2 = parser.feed("tial\ndata: more");
        assert_eq!(events2.len(), 0);

        let events3 = parser.feed("\n\n");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].data, "partial\nmore");
    }

    /// A remote peer that never emits a newline must not be able to grow a
    /// pmcp client's heap without limit.
    ///
    /// `feed` pushes every chunk into `buffer` and only ever drains as far as a
    /// `\n`, so before the bound existed a hostile or broken server could hold a
    /// `subscriptions/listen` stream open and stream newline-free bytes until
    /// the client ran out of memory (review CR-03, verification gap item 3,
    /// T-113-73).
    #[test]
    fn a_newlineless_flood_cannot_grow_the_buffer_past_the_bound() {
        let mut parser = SseParser::new();
        let chunk = "x".repeat(64 * 1024);
        // 2 MiB, with not one newline in it.
        for _ in 0..32 {
            assert!(
                parser.feed(&chunk).is_empty(),
                "an unterminated line completes no event"
            );
        }
        assert!(
            parser.buffer.len() <= 1024 * 1024,
            "the line buffer grew to {} bytes, past the 1 MiB bound",
            parser.buffer.len()
        );
    }

    /// The default bound is the one `SseConfig` already documented — the number
    /// is sourced from there, not re-typed, so there is exactly ONE of it.
    #[test]
    fn new_takes_its_bound_from_the_sse_config_default() {
        let parser = SseParser::new();
        assert_eq!(parser.max_buffer_size, SseConfig::default().max_buffer_size);
        assert_eq!(parser.max_buffer_size, 1024 * 1024, "1 MiB");
        assert!(!parser.overflowed(), "a fresh parser has lost nothing");
    }

    /// The limit is real CONFIG, not a constant baked into `feed`: a parser
    /// built with a tighter bound trips on bytes the default-bounded one
    /// swallows without complaint.
    #[test]
    fn with_max_buffer_size_bounds_at_the_value_given() {
        let flood = "x".repeat(256);

        let mut tight = SseParser::with_max_buffer_size(64);
        assert!(tight.feed(&flood).is_empty());
        assert!(tight.overflowed(), "256 bytes is past a 64-byte bound");
        assert!(tight.buffer.is_empty(), "the oversized line was discarded");

        let mut wide = SseParser::new();
        let _ = wide.feed(&flood);
        assert!(
            !wide.overflowed(),
            "the same bytes are nowhere near the 1 MiB default"
        );
    }

    /// The flag never auto-clears, so a caller polling once per chunk cannot
    /// miss the event even when well-formed frames follow the bad one.
    #[test]
    fn the_overflow_flag_latches() {
        let mut parser = SseParser::with_max_buffer_size(64);
        assert!(parser.feed(&"x".repeat(256)).is_empty());
        assert!(parser.overflowed());

        let events = parser.feed("data: ok\n\n");
        assert_eq!(events.len(), 1, "the parser keeps working");
        assert_eq!(events[0].data, "ok");
        assert!(parser.overflowed(), "and the flag stays set");

        parser.reset();
        assert!(
            parser.overflowed(),
            "reset cannot un-lose the discarded bytes"
        );
    }

    /// Events completed BEFORE the oversized line are still delivered, and the
    /// overflowing feed itself completes nothing rather than emitting a
    /// truncated frame that would fail JSON parsing with a misleading error.
    #[test]
    fn events_completed_before_an_oversized_line_are_still_returned() {
        let mut parser = SseParser::with_max_buffer_size(64);

        let events = parser.feed("data: first\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "first");
        assert!(!parser.overflowed());

        let events = parser.feed(&"x".repeat(256));
        assert!(events.is_empty(), "an overflowing feed completes nothing");
        assert!(parser.overflowed());
    }

    /// A chunk that CONTAINS a newline skips the pre-check, so the bound has to
    /// hold on the RESIDUAL the drain loop leaves behind. Without the residual
    /// check a single `"\n" + 256 bytes` chunk parks 256 bytes in a 64-byte
    /// parser — and a peer can repeat that with megabyte chunks.
    #[test]
    fn a_newline_prefixed_flood_still_trips_the_bound() {
        let mut parser = SseParser::with_max_buffer_size(64);

        let mut chunk = String::from("\n");
        chunk.push_str(&"x".repeat(256));
        let events = parser.feed(&chunk);

        assert!(
            events.is_empty(),
            "the leading blank line dispatches nothing"
        );
        assert!(
            parser.overflowed(),
            "the residual unterminated line exceeds the bound and is discarded"
        );

        // The parser keeps working afterwards, exactly as after a pre-check trip.
        let events = parser.feed("data: ok\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "ok");
    }

    /// A parser that never exceeds its bound behaves byte-identically to the
    /// pre-bound parser — every other test in this module is the rest of that
    /// proof; this one pins the flag itself.
    #[test]
    fn a_parser_under_its_bound_never_reports_overflow() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: 7\nevent: message\ndata: {\"a\":1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"a\":1}");
        assert_eq!(parser.last_event_id(), Some("7"));
        assert!(!parser.overflowed());
    }

    #[test]
    fn test_sse_stream_builder() {
        let stream = SseStream::new()
            .data("simple message")
            .typed_event("ping", "pong")
            .event(SseEvent::new("complex").with_id("42").with_retry(1000))
            .build();

        assert!(stream.contains("data: simple message"));
        assert!(stream.contains("event: ping"));
        assert!(stream.contains("data: pong"));
        assert!(stream.contains("id: 42"));
        assert!(stream.contains("retry: 1000"));
    }

    /// Regression: a multi-byte character before the first `\n`, with a `\r`
    /// later in the buffer, used to PANIC with "byte index N is not a char
    /// boundary".
    ///
    /// The old CRLF check indexed by CHARACTER (`chars().nth(line_end - 1)`)
    /// while `line_end` is a BYTE index, so it reported `'\r'` for a position
    /// that was actually inside `'\u{2602}'`, and the slice that followed cut
    /// mid-character. These bytes come off the wire from a remote server, so
    /// the panic was a remote-triggerable client crash (T-113-67).
    #[test]
    fn feed_does_not_panic_on_a_multibyte_char_before_a_later_cr() {
        let mut parser = SseParser::new();
        // bytes: 0..2 = '\u{2602}', 3 = '\n', 4 = '\r', 5 = 'X', 6 = '\n'.
        // `find('\n')` is 3 and `chars().nth(2)` was `'\r'` — the disagreement.
        let events = parser.feed("\u{2602}\n\rX\n");
        assert!(
            events.is_empty(),
            "neither line carries data, so nothing dispatches: {events:?}"
        );
    }

    /// A CRLF-terminated line still has its `\r` stripped, and a multi-byte
    /// payload survives intact.
    #[test]
    fn feed_strips_crlf_and_preserves_multibyte_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: \u{2602}-\u{4f60}\u{597d}\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "\u{2602}-\u{4f60}\u{597d}");
    }

    proptest::proptest! {
        /// `feed` runs on bytes a remote peer chose. It must never panic, for
        /// ANY text, at ANY chunk split.
        #[test]
        fn feed_never_panics_on_arbitrary_text(
            chunks in proptest::collection::vec(
                "(\\PC|\r|\n|\u{2602}|\u{4f60}){0,40}",
                0..4,
            ),
        ) {
            let mut parser = SseParser::new();
            for chunk in chunks {
                let _ = parser.feed(&chunk);
            }
        }

        /// And neither does a TIGHTLY bounded parser, whose enforcement branch
        /// discards a partial line mid-stream — including one cut in the middle
        /// of a multi-byte character (the 113-13 char-boundary guard, now
        /// exercised against the bound as well).
        #[test]
        fn a_bounded_feed_never_panics_on_arbitrary_text(
            chunks in proptest::collection::vec(
                "(\\PC|\r|\n|\u{2602}|\u{4f60}){0,40}",
                0..4,
            ),
        ) {
            let mut parser = SseParser::with_max_buffer_size(8);
            for chunk in chunks {
                let _ = parser.feed(&chunk);
                // The residual is either an unterminated line the bound
                // permitted (<= 8) or the tail of THIS chunk after its last
                // `\n`. Neither accumulates across chunks, which is the whole
                // point: memory is a function of one chunk, not of stream age.
                proptest::prop_assert!(
                    parser.buffer.len() <= std::cmp::max(8, chunk.len()),
                    "buffer {} outgrew both the bound and this chunk",
                    parser.buffer.len(),
                );
            }
        }
    }

    #[test]
    fn test_sse_event_display() {
        let event = SseEvent::new("test data")
            .with_id("123")
            .with_event("message")
            .with_retry(3000);

        let output = event.to_string();
        assert!(output.contains("id: 123"));
        assert!(output.contains("event: message"));
        assert!(output.contains("retry: 3000"));
        assert!(output.contains("data: test data"));
    }
}
