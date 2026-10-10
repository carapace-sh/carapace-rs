//! Shell lexing helpers shared by bash patching and `Action::split`
//! (a small port of `carapace-shlex`'s completion primitives, POSIX
//! quoting only so far).

use crate::meta::Meta;
use crate::value::RawValue;

/// Shell format for lexing a value (Go `shlex.Format`).
///
/// `Default` and `Bash` share POSIX lexing so far; other formats fall back
/// to `Default` until their quoting machines are ported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Default,
    Bash,
}

/// The raw (still quoted) trailing token of a command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawTail<'a> {
    /// The token the cursor sits in (quotes included).
    pub token: &'a str,
    /// The quote character if the token ends inside an open quote.
    pub open_quote: Option<char>,
    /// Whether the token is complete (a shlex tokenizer would have yielded it).
    pub complete: bool,
}

impl<'a> RawTail<'a> {
    /// Empty tail: the cursor starts a new word.
    pub fn is_empty(&self) -> bool {
        self.token.is_empty()
    }
}

/// The raw trailing token of the command line, bounded by whitespace
/// outside quotes.
///
/// `split_whitespace` would both split inside quoted whitespace and return
/// the previous word for a trailing space; neither matches bash's word the
/// cursor sits in.
pub fn raw_last_token(line: &str) -> RawTail<'_> {
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' && quote != Some('\'') {
            escaped = true;
        } else if quote.is_none() && (c == '\'' || c == '"') {
            quote = Some(c);
        } else if quote == Some(c) {
            quote = None;
        } else if quote.is_none() && c.is_whitespace() {
            start = index + c.len_utf8();
        }
    }
    RawTail {
        token: &line[start..],
        open_quote: quote,
        complete: quote.is_none() && !escaped,
    }
}

/// Lenient dequote of a partial shell token: open quotes and a trailing
/// escape are tolerated.
pub fn dequote_partial(token: &str) -> String {
    let mut result = String::new();
    let mut quote: Option<char> = None;
    let mut chars = token.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' if quote.is_none() => quote = Some(c),
            c if quote == Some(c) => quote = None,
            '\\' if quote != Some('\'') => match chars.next() {
                Some(next) => result.push(next),
                None => break,
            },
            c => result.push(c),
        }
    }
    result
}

/// Quote a completion candidate for insertion at the cursor described by
/// `open_quote` (port of `quoteInsertionDefault` for POSIX formats): an
/// open quote is closed with the format's escape rules, a bareword is
/// quoted only when needed (`posixQuoteWord`).
pub fn quote_insertion(value: &str, open_quote: Option<char>) -> String {
    match open_quote {
        Some('\'') => format!("'{}'", value.replace('\'', "'\\''")),
        Some('"') => format!("\"{}\"", escape_double_quote(value)),
        _ => quote_word(value),
    }
}

/// Quote a bareword as a complete word (port of `posixQuoteWord`): only
/// `"` `'` whitespace, `` ` `` `$` and `\\` force quoting.
fn quote_word(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_owned();
    }
    if !value
        .chars()
        .any(|c| matches!(c, '"' | '\'' | ' ' | '`' | '$' | '\n' | '\r' | '\t' | '\\'))
    {
        return value.to_owned();
    }
    format!("\"{}\"", escape_double_quote(value))
}

/// Escape for insertion inside double quotes.
fn escape_double_quote(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | '"' | '$' | '`') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Whether completion of `value` suppresses the trailing space.
pub fn matches_nospace(meta: &Meta, value: &RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}
