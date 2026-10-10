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

/// Shell format for lexing/quoting a value (Go `shlex.Format`).
///
/// `Default`, `Bash` and `Fish` share POSIX quoting (Go's `fishQuoteWord`
/// is `posixQuoteWord`); `Cmd` and `Nushell` have their own escape and
/// quote rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Default,
    Bash,
    Fish,
    Cmd,
    Nushell,
}

/// Lexing rules of a format: which characters quote and where the escape
/// character is honored.
#[derive(Clone, Copy)]
struct FormatSpec {
    /// Characters that open/close a quoted region.
    quotes: &'static [char],
    /// The escape character, if the format has one.
    escape: Option<char>,
    /// The escape character is honored outside quotes.
    escape_outside_quotes: bool,
    /// The escape character is honored inside double quotes.
    escape_inside_double_quote: bool,
}

impl Format {
    fn spec(self) -> FormatSpec {
        match self {
            Self::Default | Self::Bash | Self::Fish => FormatSpec {
                quotes: &['\'', '"'],
                escape: Some('\\'),
                escape_outside_quotes: true,
                escape_inside_double_quote: true,
            },
            Self::Cmd => FormatSpec {
                quotes: &['"'],
                escape: Some('^'),
                escape_outside_quotes: true,
                escape_inside_double_quote: false,
            },
            Self::Nushell => FormatSpec {
                quotes: &['\'', '"', '`'],
                escape: Some('\\'),
                escape_outside_quotes: false,
                escape_inside_double_quote: true,
            },
        }
    }
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
pub fn raw_last_token(line: &str, format: Format) -> RawTail<'_> {
    let spec = format.spec();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        let escape_applies = spec.escape.is_some()
            && match quote {
                Some(q) => q == '"' && spec.escape_inside_double_quote,
                None => spec.escape_outside_quotes,
            };
        if escaped {
            escaped = false;
        } else if Some(c) == spec.escape && escape_applies {
            escaped = true;
        } else if quote.is_none() && spec.quotes.contains(&c) {
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
pub fn complete_tokens(line: &str, format: Format) -> (Vec<String>, RawTail<'_>) {
    let tail = raw_last_token(line, format);
    let mut tokens = match format {
        Format::Cmd => tokenize_cmd(line),
        _ => Shlex::new(line).collect(),
    };
    if !tail.is_empty() && !tail.complete {
        // the tokenizer lost the trailing partial token (open quote,
        // trailing escape): shlex drops it, the cmd tokenizer truncates it
        if matches!(format, Format::Cmd) && tail.open_quote.is_none() {
            tokens.pop(); // the truncated prefix is already the last word
        }
        tokens.push(dequote_partial(tail.token, format));
    }
    (tokens, tail)
}

/// Tokenize a cmd.exe command line: whitespace-delimited words, `"..."`
/// quoting (quotes stripped) and `^` escaping outside quotes.
fn tokenize_cmd(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut in_quotes = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '^' && !in_quotes {
            escaped = true;
        } else if c == '"' {
            in_quotes = !in_quotes;
            started = true;
        } else if c.is_whitespace() && !in_quotes {
            if started {
                tokens.push(std::mem::take(&mut current));
                started = false;
            }
        } else {
            current.push(c);
            started = true;
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

/// Lenient dequote of a partial shell token: open quotes and a trailing
/// escape are tolerated.
pub fn dequote_partial(token: &str, format: Format) -> String {
    let spec = format.spec();
    let mut result = String::new();
    let mut quote: Option<char> = None;
    let mut chars = token.chars();
    while let Some(c) = chars.next() {
        let escape_applies = spec.escape.is_some()
            && match quote {
                Some(q) => q == '"' && spec.escape_inside_double_quote,
                None => spec.escape_outside_quotes,
            };
        match c {
            c if Some(c) == spec.escape && escape_applies => match chars.next() {
                Some(next) => result.push(next),
                None => break,
            },
            c if quote.is_none() && spec.quotes.contains(&c) => quote = Some(c),
            c if quote == Some(c) => quote = None,
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
        if c == '\n' || c == '\r' {
            // line continuation: the newline is consumed by the shell
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Quote and join words for insertion into a command line of the format
/// (port of Go `shlex.Join`): each word quoted only when needed, empty
/// words as `""`.
pub fn join(args: &[String], format: Format) -> String {
    let _ = format; // Default, Bash and Fish share POSIX quoting so far
    args.iter()
        .map(|arg| quote_word(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_tokens(line: &str) -> (Vec<String>, RawTail<'_>) {
        super::complete_tokens(line, Format::default())
    }

    fn raw_last_token(line: &str) -> RawTail<'_> {
        super::raw_last_token(line, Format::default())
    }

    fn dequote_partial(token: &str) -> String {
        super::dequote_partial(token, Format::default())
    }

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

#[test]
fn complete_tokens_cmd_caret_edge_cases() {
    use Format::Cmd;
    assert_eq!(complete_tokens("one^ two", Cmd).0, vec!["one two"]);
    assert_eq!(complete_tokens("myapp one^", Cmd).0, vec!["myapp", "one"]);
    assert_eq!(complete_tokens("\"a\" \"b\"", Cmd).0, vec!["a", "b"]);
    assert_eq!(complete_tokens("myapp \"\" ", Cmd).0, vec!["myapp", ""]);
}
