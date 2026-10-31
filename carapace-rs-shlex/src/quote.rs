//! Quoting primitives for insertion and joining (port of `quote.go`).

use super::format::FormatImpl;
use super::token::LexerState;

/// Quote a word for tcsh, the only remaining consumer of this encoding:
/// double-quote wrapping with escapes for `$` `` ` `` `"` `\` and newline.
/// Newlines are emitted as backslash-newline, which tcsh preserves inside
/// double quotes (unlike bash, which drops it). Tab and CR are emitted
/// literally inside double quotes since they are safe there and not
/// backslash-escaped by the shell. Safe words returned as-is.
pub(crate) fn posix_quote_word(s: &str) -> String {
    quote_wrapped(s, posix_string_escape)
}

/// Escape the content of a tcsh double-quoted string (the characters `$`,
/// `` ` `` and `"`) without supplying the quotes. Newlines are emitted as
/// backslash-newline, which tcsh preserves inside double quotes. bash, zsh
/// and oil instead treat it as a line continuation and drop the newline;
/// use [`bash_string_escape`] for those.
pub(crate) fn posix_string_escape(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '$' => b.push_str("\\$"),
            '`' => b.push_str("\\`"),
            '\n' => b.push_str("\\\n"),
            _ => b.push(r),
        }
    }
    b
}

/// Escape the content of a bash/zsh/oil double-quoted string (the
/// characters `$`, `` ` ``, `"`, and `\`) without supplying the quotes.
/// Newlines are emitted as a quote close/reopen sequence (`"'\n'"`) because
/// bash treats backslash-newline inside double quotes as a line
/// continuation and drops it; this requires the caller to wrap the result
/// in double quotes.
pub(crate) fn bash_string_escape(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '$' => b.push_str("\\$"),
            '`' => b.push_str("\\`"),
            '\n' => {
                b.push_str("\"'\n'\"");
            }
            _ => b.push(r),
        }
    }
    b
}

/// Quote a word for bash/zsh/oil using [`bash_string_escape`]; safe words
/// returned as-is.
pub(crate) fn bash_quote_word(s: &str) -> String {
    quote_wrapped(s, bash_string_escape)
}

/// Wrap the output of the given escaper in double quotes, leaving safe
/// words as-is.
pub(crate) fn quote_wrapped(s: &str, escape: fn(&str) -> String) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['\"', '\'', ' ', '`', '$', '\n', '\r', '\t', '\\']) {
        return s.to_owned();
    }
    format!("\"{}\"", escape(s))
}

/// Quote a word for fish: fish double quotes escape only ", $, and \; a raw
/// newline is emitted verbatim (fish treats backslash-newline inside double
/// quotes as a line continuation and drops it); backtick is a regular
/// character in fish (not command substitution).
pub(crate) fn fish_quote_word(s: &str) -> String {
    quote_wrapped_fish(s)
}

fn quote_wrapped_fish(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['"', '\'', ' ', '$', '\n', '\r', '\t', '\\']) {
        return s.to_owned();
    }
    format!("\"{}\"", fish_string_escape(s))
}

/// Escape the content of a fish double-quoted string (the characters `"`,
/// `$`, and `\`) without supplying the quotes. Backtick is a regular
/// fish (not command substitution). A raw newline is emitted verbatim:
/// fish treats backslash-newline inside double quotes as a line
/// continuation and drops it.
pub(crate) fn fish_string_escape(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '$' => b.push_str("\\$"),
            _ => b.push(r),
        }
    }
    b
}

/// Quote a word for hilbish: metacharacters are backslash-escaped in
/// barewords rather than the word being quoted.
pub(crate) fn hilbish_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(|c| {
        matches!(
            c,
            '"' | '\''
                | '`'
                | ' '
                | '('
                | ')'
                | '['
                | ']'
                | '$'
                | '&'
                | '*'
                | '>'
                | '<'
                | '|'
                | '\\'
                | ';'
                | '\t'
                | '\r'
                | '\n'
        )
    }) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    for r in s.chars() {
        match r {
            '"' | '\'' | '`' | ' ' | '(' | ')' | '[' | ']' | '$' | '&' | '*' | '>' | '<' | '|'
            | '\\' | ';' | '\t' | '\r' => {
                b.push('\\');
                b.push(r);
            }
            '\n' => b.push_str("\\\n"),
            _ => b.push(r),
        }
    }
    b
}

/// Quote a word for elvish: barewords allow letters, digits, `-_:~./\@%+!`
/// and non-ASCII printable; single quotes doubled inside single quotes.
pub(crate) fn elvish_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "''".to_owned();
    }
    if is_elvish_bareword(s) {
        return s.to_owned();
    }
    format!("'{}'", elvish_string_escape(s))
}

/// Escape the content of an elvish single-quoted string (two consecutive
/// single-quote characters produce one literal quote) without supplying
/// the quotes.
pub(crate) fn elvish_string_escape(s: &str) -> String {
    s.replace('\'', "''")
}

fn is_elvish_bareword(s: &str) -> bool {
    if s.starts_with('~') {
        return false;
    }
    s.chars().all(is_elvish_bareword_rune)
}

fn is_elvish_bareword_rune(r: char) -> bool {
    is_elvish_variable_name_rune(r) || matches!(r, '.' | '/' | '\\' | '@' | '%' | '+' | '!')
}

fn is_elvish_variable_name_rune(r: char) -> bool {
    ((r as u32) >= 0x80 && !r.is_control())
        || r.is_ascii_digit()
        || r.is_ascii_lowercase()
        || r.is_ascii_uppercase()
        || matches!(r, '-' | '_' | ':' | '~')
}

/// Quote a word for nushell: double-quote wrapping with C-style escapes
/// for `\` and `"`.
pub(crate) fn nushell_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(|c: char| {
        matches!(
            c,
            ' ' | '{'
                | '}'
                | '('
                | ')'
                | '['
                | ']'
                | '<'
                | '>'
                | '$'
                | '&'
                | '"'
                | '\''
                | '|'
                | ';'
                | '#'
                | '`'
                | '\n'
                | '\r'
                | '\t'
                | '\\'
        )
    }) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            _ => b.push(r),
        }
    }
    b.push('"');
    b
}

/// Quote a word for PowerShell: single-quote wrapping (verbatim, `''`
/// for a literal `'`).
pub(crate) fn powershell_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "''".to_owned();
    }
    if !s.contains(|c: char| {
        matches!(
            c,
            ' ' | '\''
                | '"'
                | '`'
                | '$'
                | '&'
                | '|'
                | ';'
                | '<'
                | '>'
                | '('
                | ')'
                | '{'
                | '}'
                | '\n'
                | '\r'
                | '\t'
        )
    }) {
        return s.to_owned();
    }
    format!("'{}'", powershell_string_escape(s))
}

/// Escape the content of a PowerShell single-quoted string (a doubled quote
/// produces one literal quote) without supplying the quotes.
pub(crate) fn powershell_string_escape(s: &str) -> String {
    s.replace('\'', "''")
}

/// Quote a word for xonsh: Python single-quote wrapping with `\` escapes.
pub(crate) fn xonsh_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['\'', '"', ' ', '\t', '\r', '\n', '\\']) {
        return s.to_owned();
    }
    format!("'{}'", xonsh_string_escape(s))
}

/// Escape the content of a xonsh (Python) single-quoted string with
/// backslash escapes for quote, backslash and newline (a raw newline is a
/// syntax error in a Python string literal) without supplying the quotes.
/// Note that the shlex lexer does not unescape `\n` (Value differs from the
/// actual xonsh value).
pub(crate) fn xonsh_string_escape(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for r in s.chars() {
        match r {
            '\'' => b.push_str("\\'"),
            '\\' => b.push_str("\\\\"),
            '\n' => b.push_str("\\n"),
            _ => b.push(r),
        }
    }
    b
}

/// Quote a word for cmd.exe: double-quote wrapping for spaces; a literal
/// `"` is handled by closing the quote, escaping it with `^`, reopening.
pub(crate) fn cmd_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(|c| {
        matches!(
            c,
            ' ' | '"' | '&' | '|' | '<' | '>' | '(' | ')' | '^' | ',' | '\t' | '\n' | '\r'
        )
    }) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
    for r in s.chars() {
        if r == '"' {
            b.push_str("\"^\"");
        } else {
            b.push(r);
        }
    }
    b.push('"');
    b
}

/// Quote a word for Lua (as used by cmd-clink snippets): Lua identifiers
/// stay bare; anything else is wrapped in double quotes with backslash
/// escapes for backslash, quote, and newline.
pub(crate) fn lua_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if is_lua_bareword(s) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '\n' => b.push_str("\\n"),
            _ => b.push(r),
        }
    }
    b.push('"');
    b
}

/// A valid Lua ASCII identifier (`[A-Za-z_][A-Za-z0-9_]*`, the fixed tables
/// of Lua's `lctype.h`); digits cannot lead a name, control characters and
/// bytes above 0x7F get quoted instead. Accepts the empty string like the
/// Go helper; callers that need a real bareword guard against it, see the
/// Lua format's `is_bare_word`.
pub(crate) fn is_lua_bareword(s: &str) -> bool {
    match s.chars().next() {
        Some(c) if c == '_' || c.is_ascii_lowercase() || c.is_ascii_uppercase() => {}
        Some(_) => return false,
        None => return true,
    }
    s.chars()
        .skip(1)
        .all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Escape the content of a Lua string literal (without the quotes) with
/// backslash escapes for the string's own quote character, the backslash,
/// and newlines.
pub(crate) fn lua_escape_interior(value: &str, quote: char) -> String {
    let mut b = String::with_capacity(value.len());
    for r in value.chars() {
        match r {
            c if c == quote => {
                b.push('\\');
                b.push(c);
            }
            '\\' => b.push_str("\\\\"),
            '\n' => b.push_str("\\n"),
            _ => b.push(r),
        }
    }
    b
}

/// Escape the content of a Lua single-quoted string (quote, backslash, and
/// newline).
pub(crate) fn lua_string_escape(s: &str) -> String {
    lua_escape_interior(s, '\'')
}

/// Screen whether `s` is a bareword for every currently supported format:
/// alphanumeric with the static characters `.`, `_`, `-`, `+` as
/// non-leading characters. Quotes, escapes, expansions (`` ` $ { ~ ( ``),
/// wordbreaks (`: @ = ; | &`) and history actives (`!`) are all rejected,
/// including ones that act above a shell's lexer, so a passing word stands
/// as a single inert token in unquoted position. Identifier validity is
/// not implied: dot-, dash-, plus- and digit-leading words are command
/// names, not identifiers. Formats may override with a sharper list
/// (Lua validates the whole identifier rule; xonsh and PowerShell reject
/// digit-leading words the shell would evaluate).
pub(crate) fn is_shared_bareword(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '+'))
}

/// Quote `value` for insertion at a cursor in the given lexer state using
/// the format's quoting primitives: quoting states are closed with the
/// format's own escape rules, and barewords are quoted as a complete word.
pub(crate) fn quote_insertion(f: &dyn FormatImpl, state: LexerState, value: &str) -> String {
    if let Some(quoted) = f.quote_insertion_custom(state, value) {
        return quoted;
    }
    if f.stop_parsing_word().is_some() && state == LexerState::StopParsing {
        return value.to_owned(); // raw mode: nothing is quoted
    }

    match state {
        LexerState::Quoting => {
            // inside single quotes: close them
            if f.non_escaping_quote_backslash_escapes() {
                format!("'{}'", value.replace('\'', "\\'"))
            } else if f.non_escaping_quote_escapes() {
                format!("'{}'", value.replace('\'', "''"))
            } else {
                format!("'{}'", value.replace('\'', "'\"'\"'"))
            }
        }
        LexerState::QuotingEscaping => {
            // inside double quotes: close them
            if f.escape_not_in_escaping_quote() {
                // the escape character is literal inside double quotes
                // (cmd): the quote itself is doubled instead
                format!("\"{}\"", value.replace('"', "\"\""))
            } else {
                format!("\"{}\"", escape_double_quote(f, value))
            }
        }
        LexerState::QuotingTriple => {
            format!("'''{}'''", value.replace('\'', "\\'"))
        }
        LexerState::QuotingTripleEscaping => {
            let escaped = value.replace('\\', "\\\\");
            format!("\"\"\"{}\"\"\"", escaped.replace('"', "\\\""))
        }
        _ => f.quote_word(value),
    }
}

/// Escape `value` for the inside of a double-quoted string: the quote
/// itself, the escape character, and the format's documented escape set.
fn escape_double_quote(f: &dyn FormatImpl, value: &str) -> String {
    let chars = f.escaping_quote_escape_chars();
    let mut b = String::with_capacity(value.len());
    for r in value.chars() {
        if r == '"' || r == '\\' || chars.contains(&r) {
            b.push('\\');
        }
        b.push(r);
    }
    b
}
