//! Shell lexing for completion (Rust port of the
//! [carapace-shlex](https://github.com/carapace-sh/carapace-shlex) completion
//! primitives, POSIX quoting only so far).
//!
//! The host shell usually does the word splitting and hands the completer
//! its words plus context (`COMP_LINE`, `COMP_POINT`, ...). Re-lexing is
//! needed where the shell's view is not enough: bash's wordbreak-split
//! words (`bash::patch`), and values that are themselves command lines
//! (`Action::split`).

/// Shell format for lexing a value (Go `shlex.Format`).
///
/// `Default` and `Bash` share POSIX lexing so far; other formats fall back
/// to `Default` until their quoting machines are ported.
use shlex::Shlex;

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

/// Tokenize a command line like the shell would, recovering a trailing
/// partial token.
///
/// The shlex crate drops a trailing partial token (open quote or trailing
/// escape); the recovered dequoted word is appended so the completion sees
/// the word the cursor sits in. The [`RawTail`] describes that tail for
/// prefix computation and insertion quoting.
pub fn complete_tokens(line: &str) -> (Vec<String>, RawTail<'_>) {
    let mut tokens: Vec<String> = Shlex::new(line).collect();
    let tail = raw_last_token(line);
    if !tail.is_empty() && !tail.complete {
        tokens.push(dequote_partial(tail.token));
    }
    (tokens, tail)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_tokens_recovers_partial_tail() {
        assert_eq!(complete_tokens("myapp \"ab").0, vec!["myapp", "ab"]);
        assert_eq!(complete_tokens("myapp 'ab").0, vec!["myapp", "ab"]);
        assert_eq!(complete_tokens("myapp a\\").0, vec!["myapp", "a"]);
        assert_eq!(complete_tokens("myapp a b ").0, vec!["myapp", "a", "b"]);
        assert_eq!(complete_tokens("myapp 'ab c").0, vec!["myapp", "ab c"]);
        assert_eq!(complete_tokens("myapp foo:ba").0, vec!["myapp", "foo:ba"]);
    }

    #[test]
    fn raw_last_token_reports_state() {
        assert_eq!(raw_last_token("foo bar").token, "bar");
        assert!(raw_last_token("foo bar").complete);
        assert_eq!(raw_last_token("foo ").token, "");
        assert!(raw_last_token("foo ").complete);
        assert_eq!(raw_last_token("foo \"ab").open_quote, Some('"'));
        assert!(!raw_last_token("foo \"ab").complete);
        assert_eq!(raw_last_token("'a b' rest").token, "rest");
        assert_eq!(raw_last_token("'a b c").token, "'a b c");
        assert_eq!(raw_last_token("it\\").open_quote, None);
        assert!(!raw_last_token("it\\").complete);
    }

    #[test]
    fn dequote_partial_strips_quotes_and_escapes() {
        assert_eq!(dequote_partial("\"ab"), "ab");
        assert_eq!(dequote_partial("'a b c"), "a b c");
        assert_eq!(dequote_partial("a\\"), "a");
        assert_eq!(dequote_partial("--bool="), "--bool=");
    }

    #[test]
    fn quote_insertion_closes_open_quotes() {
        assert_eq!(quote_insertion("a b", Some('\'')), "'a b'");
        assert_eq!(quote_insertion("it' end", Some('\'')), "'it'\\'' end'");
        assert_eq!(quote_insertion("a\"b", Some('"')), "\"a\\\"b\"");
        assert_eq!(quote_insertion("plain", None), "plain");
        assert_eq!(quote_insertion("--bool=true", None), "--bool=true");
        assert_eq!(quote_insertion("a b", None), "\"a b\"");
        assert_eq!(quote_insertion("", None), "\"\"");
    }
}
