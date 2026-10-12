//! Quoting primitives for insertion and joining (port of `quote.go`).

use super::format::FormatImpl;
use super::token::LexerState;

/// Quote a word for POSIX shells (bash, zsh, oil, tcsh): double-quote
/// wrapping with escapes for `$` `` ` `` `"` `\` and newline; safe words
/// returned as-is.
pub(crate) fn posix_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['\"', '\'', ' ', '`', '$', '\n', '\r', '\t', '\\']) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
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
    b.push('"');
    b
}

/// Quote a word for fish: fish double quotes escape only ", $, \, and
/// newline; backtick is a regular character in fish.
pub(crate) fn fish_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['"', '\'', ' ', '$', '\n', '\r', '\t', '\\']) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('"');
    for r in s.chars() {
        match r {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '$' => b.push_str("\\$"),
            '\n' => b.push_str("\\\n"),
            _ => b.push(r),
        }
    }
    b.push('"');
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
    let mut b = String::with_capacity(s.len() + 2);
    b.push('\'');
    for r in s.chars() {
        if r == '\'' {
            b.push_str("''");
        } else {
            b.push(r);
        }
    }
    b.push('\'');
    b
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
    let mut b = String::with_capacity(s.len() + 2);
    b.push('\'');
    for r in s.chars() {
        if r == '\'' {
            b.push_str("''");
        } else {
            b.push(r);
        }
    }
    b.push('\'');
    b
}

/// Quote a word for xonsh: Python single-quote wrapping with `\` escapes.
pub(crate) fn xonsh_quote_word(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_owned();
    }
    if !s.contains(['\'', '"', ' ', '\t', '\r', '\n', '\\']) {
        return s.to_owned();
    }
    let mut b = String::with_capacity(s.len() + 2);
    b.push('\'');
    for r in s.chars() {
        match r {
            '\'' => b.push_str("\\'"),
            '\\' => b.push_str("\\\\"),
            _ => b.push(r),
        }
    }
    b.push('\'');
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
