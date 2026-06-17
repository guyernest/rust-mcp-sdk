//! The Excel-formula tokenizer (WBCO-03).
//!
//! Input is a formula string WITHOUT the leading `=`. Output is an owned
//! `Vec<Token>` over the constrained dialect grammar. The lexer handles the
//! known tokenizer landmines:
//!
//! - **Digit-leading sheet names.** A layered sheet is `<digit>_<Name>`
//!   (`1_Inputs` … `7_Quote`). A digit/letter run *followed by* `!` is a sheet
//!   QUALIFIER, not a number — so `2_Constants!$C$15` lexes as ONE
//!   [`Token::CellRef`], never `Number(2)` + `Name(_Constants)`.
//! - **`""`-doubled string literals.** A `"…"` literal is lexed FIRST so an
//!   embedded `,`/`)`/`#` inside `"#,##0.00"` is plain content, and `""`
//!   un-escapes to one literal `"`.
//! - **Sheet-qualified ranges.** `Sheet1!A1:B2` lexes as `CellRef("Sheet1!A1")`
//!   + `Colon` + `CellRef("B2")` — the sheet qualifier appears EXACTLY ONCE and
//!   the range bounds stay recoverable; the PARSER folds the three into one
//!   `Expr::Range(RangeRef{ sheet, start, end })`. The lexer never bakes the
//!   whole range into one opaque token (which would lose the bounds) nor
//!   duplicates the sheet onto the right operand.
//! - `:` is the range operator, `&` concat, `%` postfix, `1.5E3` scientific.
//! - `$`-anchors are PRESERVED in the token text (stripping is the DAG's job).
//!
//! Panic-freedom (lib.rs `#![deny(unwrap_used, expect_used, panic)]`): every
//! fallible step returns a [`LexError`], never `.unwrap()`. A pathologically
//! long input is rejected with [`LexError::InputTooLong`] (DoS guard).

use serde::Serialize;

/// The maximum formula-text length the lexer will accept (DoS guard). 64 KiB is
/// a generous cap that still rejects a pathologically long adversarial string.
pub const MAX_FORMULA_LEN: usize = 64 * 1024;

/// An owned lexer token over the constrained Excel-formula grammar.
///
/// Anchors (`$`) are preserved verbatim in [`Token::CellRef`]; the sheet
/// qualifier (`Sheet!`, `'Quoted Sheet'!`) is folded into the CellRef text but a
/// `:` range is left as a separate [`Token::Colon`] so the parser can recover
/// the start/end bounds with the sheet recorded once.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub enum Token {
    /// A numeric literal (incl. scientific notation `1.5E3`).
    Number(f64),
    /// A string literal with `""`-doubling already un-escaped to one `"`.
    Str(String),
    /// An A1-style cell reference, possibly `$`-anchored and possibly
    /// `Sheet!`-qualified (e.g. `"$C$15"`, `"2_Constants!$C$15"`,
    /// `"'Quoted Sheet'!A1"`). Anchors preserved; sheet folded in ONCE.
    CellRef(String),
    /// A bare identifier / defined-name or a boolean keyword carrier
    /// (`TRUE`/`FALSE` surface here; the parser lifts them to `Expr::Bool`).
    Name(String),
    /// An identifier immediately followed by `(` — a function-call opener
    /// (e.g. `CEILING(` lexes `FuncOpen("CEILING")`). The `(` is consumed.
    FuncOpen(String),
    /// An external/workbook-qualified reference token (`[Book.xlsx]Sheet1!A1`
    /// or `[1]Sheet1!A1`). Surfaced as its OWN token so the parser rejects it
    /// rather than mis-lexing it as a CellRef.
    ExternalRef(String),
    /// A literal Excel error parsed from text (`#REF!`, `#N/A`, …), carried as
    /// the raw `#…` lexeme; the parser maps it to `Expr::ErrorLit`.
    ErrorLit(String),
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `^`
    Caret,
    /// `&` (concat)
    Amp,
    /// `%` (postfix percent)
    Percent,
    /// `:` (range operator)
    Colon,
    /// `,` (argument separator / union)
    Comma,
    /// `=`
    Eq,
    /// `<>`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{` — an array-formula brace (the parser surfaces a rejection, never a panic).
    LBrace,
    /// `}`
    RBrace,
}

/// A location-free lexing error. The parser folds it into its typed
/// [`super::parser::ParseError`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum LexError {
    /// The input exceeded [`MAX_FORMULA_LEN`] (DoS guard).
    InputTooLong {
        /// The offending length.
        len: usize,
    },
    /// A string literal was opened with `"` but never closed.
    UnterminatedString,
    /// A character the dialect grammar does not recognize.
    UnexpectedChar {
        /// The offending character.
        ch: char,
    },
    /// An external/workbook ref `[` bracket was opened but never closed.
    UnterminatedExternalRef,
    /// A quoted sheet name `'` was opened but never closed.
    UnterminatedQuotedSheet,
}

/// Tokenize an Excel formula (text WITHOUT the leading `=`) into owned tokens.
///
/// Returns [`LexError`] on a malformed or oversized input — NEVER panics
/// (lib.rs value-path gate). The sheet qualifier of a `Sheet!A1:B2` range lands
/// on the FIRST `CellRef` only; the `:` is a separate [`Token::Colon`] so the
/// parser folds the range with the sheet recorded once.
///
/// # Errors
/// Returns [`LexError`] for an oversized input, an unterminated string / quoted
/// sheet / external ref, or an unsupported character.
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    if input.len() > MAX_FORMULA_LEN {
        return Err(LexError::InputTooLong { len: input.len() });
    }

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    let mut tokens: Vec<Token> = Vec::new();

    while i < chars.len() {
        i = lex_next(&chars, i, &mut tokens)?;
    }

    Ok(tokens)
}

/// Lex the ONE token (or skipped whitespace) starting at `i`, pushing any
/// produced token onto `tokens` and returning the next index. Dispatches on the
/// lead char to the per-token-kind scan helper; operators fall through to
/// [`lex_operator`]. Preserves the exact token sequence and error positions.
fn lex_next(chars: &[char], i: usize, tokens: &mut Vec<Token>) -> Result<usize, LexError> {
    let c = chars[i];

    // Whitespace — skipped (no intersection-space operator in the dialect).
    if c.is_whitespace() {
        return Ok(i + 1);
    }

    // A leading delimiter dispatches to a dedicated scan helper that returns
    // `(Token, next_index)`; `lex_delimited` returns `None` for non-delimiters.
    if let Some(result) = lex_delimited(chars, i, c) {
        let (tok, next) = result?;
        tokens.push(tok);
        return Ok(next);
    }

    // A `[A-Za-z0-9_.$]` run: classify into Number / CellRef / Name / FuncOpen
    // by context. A leading `$` also starts a ref atom.
    if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
        let (tok, next) = lex_atom(chars, i)?;
        tokens.push(tok);
        return Ok(next);
    }

    // Operators / punctuation.
    lex_operator(chars, i, tokens)
}

/// Dispatch a leading delimiter char (`"`, `[`, `'`, `#`) to its scan helper,
/// returning `Some((Token, next_index))` (or `Some(Err)` on a lex error). Returns
/// `None` when `c` does not open a delimited lexeme, so the caller falls through
/// to atom/operator handling.
fn lex_delimited(chars: &[char], i: usize, c: char) -> Option<Result<(Token, usize), LexError>> {
    match c {
        // String literal FIRST: consume `""` as one escaped quote.
        '"' => Some(lex_string(chars, i)),
        // External/workbook ref: `[Book.xlsx]Sheet1!A1` or `[1]Sheet1!A1`.
        '[' => Some(lex_external_ref(chars, i)),
        // Quoted sheet name: `'Quoted Sheet'!A1` (with `''` escaping inside).
        '\'' => Some(lex_quoted_sheet_ref(chars, i)),
        // Excel error literal: `#REF!`, `#N/A`, `#VALUE!`, `#DIV/0!`, `#NAME?`,
        // `#NUM!`, `#NULL!`.
        '#' => Some(Ok(lex_error_literal(chars, i))),
        _ => None,
    }
}

/// Lex a single operator / punctuation char at `i`, pushing its token and
/// returning the next index. Two-char operators (`<=`/`<>`/`>=`) are folded.
fn lex_operator(chars: &[char], i: usize, tokens: &mut Vec<Token>) -> Result<usize, LexError> {
    let c = chars[i];
    let next = match c {
        '+' => {
            tokens.push(Token::Plus);
            i + 1
        },
        '-' => {
            tokens.push(Token::Minus);
            i + 1
        },
        '*' => {
            tokens.push(Token::Star);
            i + 1
        },
        '/' => {
            tokens.push(Token::Slash);
            i + 1
        },
        '^' => {
            tokens.push(Token::Caret);
            i + 1
        },
        '&' => {
            tokens.push(Token::Amp);
            i + 1
        },
        '%' => {
            tokens.push(Token::Percent);
            i + 1
        },
        ':' => {
            tokens.push(Token::Colon);
            i + 1
        },
        ',' => {
            tokens.push(Token::Comma);
            i + 1
        },
        '(' => {
            tokens.push(Token::LParen);
            i + 1
        },
        ')' => {
            tokens.push(Token::RParen);
            i + 1
        },
        '{' => {
            tokens.push(Token::LBrace);
            i + 1
        },
        '}' => {
            tokens.push(Token::RBrace);
            i + 1
        },
        '=' => {
            tokens.push(Token::Eq);
            i + 1
        },
        '<' => {
            if chars.get(i + 1) == Some(&'=') {
                tokens.push(Token::Le);
                i + 2
            } else if chars.get(i + 1) == Some(&'>') {
                tokens.push(Token::Ne);
                i + 2
            } else {
                tokens.push(Token::Lt);
                i + 1
            }
        },
        '>' => {
            if chars.get(i + 1) == Some(&'=') {
                tokens.push(Token::Ge);
                i + 2
            } else {
                tokens.push(Token::Gt);
                i + 1
            }
        },
        other => return Err(LexError::UnexpectedChar { ch: other }),
    };
    Ok(next)
}

/// Lex a `"…"` string literal starting at `start` (the opening quote). `""`
/// inside the literal un-escapes to one `"`. Returns the [`Token::Str`] and the
/// index just past the closing quote.
fn lex_string(chars: &[char], start: usize) -> Result<(Token, usize), LexError> {
    let mut i = start + 1; // past the opening quote
    let mut out = String::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            // A doubled `""` is one literal quote; a lone `"` closes the string.
            if i + 1 < chars.len() && chars[i + 1] == '"' {
                out.push('"');
                i += 2;
            } else {
                return Ok((Token::Str(out), i + 1));
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    Err(LexError::UnterminatedString)
}

/// Lex an external/workbook ref starting at the `[` bracket
/// (`[Book.xlsx]Sheet1!A1`, `[1]Sheet1!A1`). The whole lexeme up to the end of
/// the trailing ref run is captured so the parser can reject it.
fn lex_external_ref(chars: &[char], start: usize) -> Result<(Token, usize), LexError> {
    let mut i = start + 1; // past `[`
                           // Advance past the closing `]`; an absent `]` is unterminated.
    match chars[i..].iter().position(|&c| c == ']') {
        Some(offset) => i += offset + 1,
        None => return Err(LexError::UnterminatedExternalRef),
    }
    // Consume the trailing sheet!addr run (letters/digits/_/$/!/' and a quoted
    // sheet segment) so the whole external lexeme is one token.
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            i = skip_quoted_segment(chars, i);
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '!' || c == ':' {
            i += 1;
        } else {
            break;
        }
    }
    let lexeme: String = chars[start..i].iter().collect();
    Ok((Token::ExternalRef(lexeme), i))
}

/// Skip a `'…'` quoted segment with `''` escaping, starting at the opening `'`.
/// Returns the index just past the closing quote (or end of input).
fn skip_quoted_segment(chars: &[char], start: usize) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\'' {
            if i + 1 < chars.len() && chars[i + 1] == '\'' {
                i += 2;
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    i
}

/// Lex a quoted-sheet-qualified ref starting at the `'` (`'Quoted Sheet'!$A$1`).
/// `''` inside the quotes is an escaped single quote. The trailing `!addr` (with
/// `$`-anchors) is folded into ONE [`Token::CellRef`] — the `:` of a range is
/// NOT consumed here.
fn lex_quoted_sheet_ref(chars: &[char], start: usize) -> Result<(Token, usize), LexError> {
    // Read the `'…'` sheet name (with `''` escaping) into the lexeme.
    let (mut lexeme, mut i) = read_quoted_sheet_name(chars, start)?;

    // Expect `!addr`; if absent, this is a bare quoted name (still emit CellRef
    // text so the parser can decide). Consume `!` + an address run (no `:`).
    if i < chars.len() && chars[i] == '!' {
        lexeme.push('!');
        i = scan_addr_run(chars, i + 1, &mut lexeme);
    }
    Ok((Token::CellRef(lexeme), i))
}

/// Read a `'…'` quoted sheet name starting at the opening `'`, treating `''` as
/// an escaped single quote. Returns the lexeme built so far (including BOTH
/// quotes) and the index just past the closing `'`. Errors if never closed.
fn read_quoted_sheet_name(chars: &[char], start: usize) -> Result<(String, usize), LexError> {
    let mut i = start + 1; // past opening `'`
    let mut sheet = String::from("'");
    while i < chars.len() {
        let c = chars[i];
        if c != '\'' {
            sheet.push(c);
            i += 1;
            continue;
        }
        // A doubled `''` is an escaped quote; a lone `'` closes the name.
        if i + 1 < chars.len() && chars[i + 1] == '\'' {
            sheet.push_str("''");
            i += 2;
        } else {
            sheet.push('\'');
            return Ok((sheet, i + 1));
        }
    }
    Err(LexError::UnterminatedQuotedSheet)
}

/// Append the `[A-Za-z0-9_$]` address run (no `:`) starting at `start` onto
/// `lexeme`, returning the index just past the run.
fn scan_addr_run(chars: &[char], start: usize, lexeme: &mut String) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            lexeme.push(c);
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Lex an Excel error literal `#…` (`#REF!`, `#N/A`, `#VALUE!`, `#DIV/0!`,
/// `#NAME?`, `#NUM!`, `#NULL!`). Consumes the `#` plus the contiguous error-name
/// run (including a trailing `!`/`?` and the `/0` of `#DIV/0!`).
fn lex_error_literal(chars: &[char], start: usize) -> (Token, usize) {
    let mut i = start + 1; // past `#`
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() || c == '/' || c == '0' {
            i += 1;
        } else if c == '!' || c == '?' {
            i += 1;
            break;
        } else {
            break;
        }
    }
    let lexeme: String = chars[start..i].iter().collect();
    (Token::ErrorLit(lexeme), i)
}

/// Lex a `[A-Za-z0-9_.$]` atom (plus a leading `$`) and classify it by context:
/// - run is all-numeric (with optional `.`/scientific `E`) AND NOT followed by
///   `!` → [`Token::Number`];
/// - run is followed by `!` (a sheet qualifier) → [`Token::CellRef`] (sheet
///   folded in ONCE);
/// - run immediately followed by `(` → [`Token::FuncOpen`];
/// - a `$`-bearing or A1-shaped run → [`Token::CellRef`];
/// - otherwise [`Token::Name`].
fn lex_atom(chars: &[char], start: usize) -> Result<(Token, usize), LexError> {
    let (i, has_dollar, all_numeric_shape) = scan_atom_run(chars, start);
    let run: String = chars[start..i].iter().collect();

    // Sheet qualifier: a run immediately followed by `!` is a sheet name — fold
    // the `!addr` into ONE CellRef (`2_Constants!$C$15`).
    if i < chars.len() && chars[i] == '!' {
        let (lexeme, next) = fold_sheet_qualified(chars, i, run);
        return Ok((Token::CellRef(lexeme), next));
    }

    // Function call: an identifier-shaped run immediately followed by `(`.
    if i < chars.len() && chars[i] == '(' && !all_numeric_shape && !has_dollar {
        return Ok((Token::FuncOpen(run), i + 1));
    }

    // Pure numeric (incl. scientific) and NOT a sheet qualifier → Number.
    if all_numeric_shape {
        return match run.parse::<f64>() {
            Ok(n) => Ok((Token::Number(n), i)),
            // A malformed numeric-shape run (e.g. lone `.`) — treat the first
            // char as unexpected so we never silently accept garbage.
            Err(_) => Err(LexError::UnexpectedChar { ch: chars[start] }),
        };
    }

    // A `$`-bearing run, or an A1-shaped run (letters then digits, e.g. `C16`),
    // is a CellRef; everything else is a bare Name / defined-name.
    if has_dollar || is_a1_shape(&run) {
        Ok((Token::CellRef(run), i))
    } else {
        Ok((Token::Name(run), i))
    }
}

/// Scan the `[A-Za-z0-9_.$]` atom run (supporting scientific `1.5E3`/`1.5E+3`),
/// returning the end index, whether a `$` appeared, and whether the run is a
/// pure numeric shape (only digits / `.` / scientific E, no letters/`_`).
fn scan_atom_run(chars: &[char], start: usize) -> (usize, bool, bool) {
    let mut i = start;
    let mut has_dollar = false;
    let mut all_numeric_shape = true;
    while i < chars.len() {
        match classify_atom_char(chars, i, start, all_numeric_shape) {
            AtomStep::Stop => break,
            AtomStep::Advance {
                step,
                dollar,
                breaks_numeric,
            } => {
                has_dollar |= dollar;
                all_numeric_shape &= !breaks_numeric;
                i += step;
            },
        }
    }
    (i, has_dollar, all_numeric_shape)
}

/// One step of the atom-run scan: stop, or advance `step` indices while
/// recording whether a `$` was seen and whether the pure-numeric shape is broken.
enum AtomStep {
    /// The char does not continue the atom run.
    Stop,
    /// Consume `step` indices; `dollar`/`breaks_numeric` update the run flags.
    Advance {
        /// How many indices this step consumes (2 for a signed sci exponent).
        step: usize,
        /// Whether this step saw a `$` anchor.
        dollar: bool,
        /// Whether this step disqualifies the run from being a pure number.
        breaks_numeric: bool,
    },
}

/// Classify the char at `i` within an atom run beginning at `start`, given
/// whether the run is still numeric-shaped. Mirrors the original accept ladder
/// (`$`, digit/`.`, scientific `E`, alnum/`_`, else stop) exactly.
fn classify_atom_char(chars: &[char], i: usize, start: usize, numeric: bool) -> AtomStep {
    let c = chars[i];
    if c == '$' {
        return AtomStep::Advance {
            step: 1,
            dollar: true,
            breaks_numeric: true,
        };
    }
    if c.is_ascii_digit() || c == '.' {
        return AtomStep::Advance {
            step: 1,
            dollar: false,
            breaks_numeric: false,
        };
    }
    if (c == 'E' || c == 'e') && numeric && i > start && is_scientific_exp(chars, i) {
        // Scientific exponent `1.5E3` / `1.5E+3` / `1.5e-2`: consume the `E` plus
        // an optional sign; the digit run continues on next iterations.
        let step = if chars[i + 1] == '+' || chars[i + 1] == '-' {
            2
        } else {
            1
        };
        return AtomStep::Advance {
            step,
            dollar: false,
            breaks_numeric: false,
        };
    }
    if c.is_ascii_alphanumeric() || c == '_' {
        return AtomStep::Advance {
            step: 1,
            dollar: false,
            breaks_numeric: true,
        };
    }
    AtomStep::Stop
}

/// Is the `E`/`e` at index `i` a scientific exponent marker (followed by a digit
/// or a sign+digit)?
fn is_scientific_exp(chars: &[char], i: usize) -> bool {
    (i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        || (i + 2 < chars.len()
            && (chars[i + 1] == '+' || chars[i + 1] == '-')
            && chars[i + 2].is_ascii_digit())
}

/// Fold a sheet-qualified ref: `run` is the sheet name and `chars[i]` is the `!`.
/// Consume `!addr` (letters/digits/`_`/`$`) into one CellRef lexeme.
fn fold_sheet_qualified(chars: &[char], bang: usize, run: String) -> (String, usize) {
    let mut lexeme = run;
    lexeme.push('!');
    let mut i = bang + 1;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
            lexeme.push(c);
            i += 1;
        } else {
            break;
        }
    }
    (lexeme, i)
}

/// Is `run` an A1-style cell address shape (`C16`, `AA100`)? One-or-more letters
/// followed by one-or-more digits, nothing else.
fn is_a1_shape(run: &str) -> bool {
    let mut chars = run.chars().peekable();
    let mut saw_letter = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            saw_letter = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_letter {
        return false;
    }
    let mut saw_digit = false;
    for c in chars {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else {
            return false;
        }
    }
    saw_digit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_leading_sheet_name_is_one_cellref_not_number_plus_name() {
        let toks = tokenize("2_Constants!$C$15").expect("lex");
        assert_eq!(toks, vec![Token::CellRef("2_Constants!$C$15".to_string())]);
    }

    #[test]
    fn quoted_sheet_ref_is_one_qualifier() {
        let toks = tokenize("'Quoted Sheet'!A1").expect("lex");
        assert_eq!(toks, vec![Token::CellRef("'Quoted Sheet'!A1".to_string())]);
    }

    #[test]
    fn sheet_qualified_range_carries_sheet_exactly_once() {
        let toks = tokenize("Sheet1!A1:B2").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("Sheet1!A1".to_string()),
                Token::Colon,
                Token::CellRef("B2".to_string()),
            ]
        );
        let with_sheet = toks
            .iter()
            .filter(|t| matches!(t, Token::CellRef(s) if s.contains('!')))
            .count();
        assert_eq!(with_sheet, 1, "sheet qualifier must appear exactly once");
    }

    #[test]
    fn quoted_sheet_anchored_range() {
        let toks = tokenize("'Quoted Sheet'!$A$1:$B$2").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("'Quoted Sheet'!$A$1".to_string()),
                Token::Colon,
                Token::CellRef("$B$2".to_string()),
            ]
        );
    }

    #[test]
    fn format_string_keeps_comma_and_hash_inside_one_str() {
        let toks = tokenize("TEXT(C11,\"#,##0.00\")").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::FuncOpen("TEXT".to_string()),
                Token::CellRef("C11".to_string()),
                Token::Comma,
                Token::Str("#,##0.00".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn doubled_quote_unescapes_to_single_quote() {
        let toks = tokenize("\"a\"\"b\"").expect("lex");
        assert_eq!(toks, vec![Token::Str("a\"b".to_string())]);
    }

    #[test]
    fn range_colon_between_bare_refs() {
        let toks = tokenize("A1:B2").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("A1".to_string()),
                Token::Colon,
                Token::CellRef("B2".to_string()),
            ]
        );
    }

    #[test]
    fn concat_amp_and_postfix_percent_and_scientific() {
        let toks = tokenize("A1&C7%").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("A1".to_string()),
                Token::Amp,
                Token::CellRef("C7".to_string()),
                Token::Percent,
            ]
        );

        assert_eq!(
            tokenize("1.5E3").expect("lex sci"),
            vec![Token::Number(1500.0)]
        );
        assert_eq!(
            tokenize("1.5E+3").expect("lex sci signed"),
            vec![Token::Number(1500.0)]
        );
    }

    #[test]
    fn all_anchor_forms_lex_to_cellref_with_anchors_preserved() {
        for raw in ["$C$16", "C16", "$C16", "C$16"] {
            let toks = tokenize(raw).expect("lex anchor form");
            assert_eq!(
                toks,
                vec![Token::CellRef(raw.to_string())],
                "{raw} should lex to a single CellRef with anchors preserved"
            );
        }
    }

    #[test]
    fn external_workbook_ref_is_its_own_token() {
        assert_eq!(
            tokenize("[Book.xlsx]Sheet1!A1").expect("lex"),
            vec![Token::ExternalRef("[Book.xlsx]Sheet1!A1".to_string())]
        );
        assert_eq!(
            tokenize("[1]Sheet1!A1").expect("lex"),
            vec![Token::ExternalRef("[1]Sheet1!A1".to_string())]
        );
    }

    #[test]
    fn error_literal_lexes_as_one_token() {
        assert_eq!(
            tokenize("#REF!").expect("lex"),
            vec![Token::ErrorLit("#REF!".to_string())]
        );
        assert_eq!(
            tokenize("#N/A").expect("lex"),
            vec![Token::ErrorLit("#N/A".to_string())]
        );
        assert_eq!(
            tokenize("#DIV/0!").expect("lex"),
            vec![Token::ErrorLit("#DIV/0!".to_string())]
        );
    }

    #[test]
    fn comparison_operators_lex() {
        let toks = tokenize("A1<=B1<>C1>=D1").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("A1".to_string()),
                Token::Le,
                Token::CellRef("B1".to_string()),
                Token::Ne,
                Token::CellRef("C1".to_string()),
                Token::Ge,
                Token::CellRef("D1".to_string()),
            ]
        );
    }

    #[test]
    fn bare_name_lexes_as_name() {
        assert_eq!(
            tokenize("FooBar").expect("lex"),
            vec![Token::Name("FooBar".to_string())]
        );
    }

    #[test]
    fn oversized_input_is_rejected_not_panicked() {
        let big = "1".repeat(MAX_FORMULA_LEN + 1);
        let err = tokenize(&big).expect_err("must reject oversized input");
        assert_eq!(
            err,
            LexError::InputTooLong {
                len: MAX_FORMULA_LEN + 1
            }
        );
    }

    #[test]
    fn nested_function_formula_lexes_without_error() {
        // A representative formula exercising several landmines in one string.
        let f = "IF(ROUND(C11,2)=1594.93,\"OK\",\"FAIL: \"&TEXT(C11,\"#,##0.00\"))";
        let toks = tokenize(f).expect("lex");
        assert!(toks.contains(&Token::FuncOpen("IF".to_string())));
        assert!(toks.contains(&Token::FuncOpen("ROUND".to_string())));
        assert!(toks.contains(&Token::FuncOpen("TEXT".to_string())));
        assert!(toks.contains(&Token::Amp));
        assert!(toks.contains(&Token::Str("#,##0.00".to_string())));
    }

    #[test]
    fn cross_sheet_arithmetic_lexes() {
        let toks = tokenize("C6*2_Constants!$C$17+2_Constants!$C$18").expect("lex");
        assert_eq!(
            toks,
            vec![
                Token::CellRef("C6".to_string()),
                Token::Star,
                Token::CellRef("2_Constants!$C$17".to_string()),
                Token::Plus,
                Token::CellRef("2_Constants!$C$18".to_string()),
            ]
        );

        let ceil = tokenize("CEILING(C7,2_Constants!$C$15)").expect("lex");
        assert_eq!(
            ceil,
            vec![
                Token::FuncOpen("CEILING".to_string()),
                Token::CellRef("C7".to_string()),
                Token::Comma,
                Token::CellRef("2_Constants!$C$15".to_string()),
                Token::RParen,
            ]
        );
    }
}
