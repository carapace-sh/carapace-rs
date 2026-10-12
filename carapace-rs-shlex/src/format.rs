//! The format trait: a shell's lexical rules (port of `format.go`).

use super::token::{LexerState, RuneClassifier, Token};
use super::wordbreak::WordbreakType;

/// The behavior behind a format name: which runes are quotes, escapes,
/// comments and word breaks, and how operators are classified.
///
/// Optional capabilities are defaulted methods (Go's optional interfaces:
/// line continuations, block comments, stop-parsing, triple quotes, raw
/// prefixes, post processing, variable expansion, naive word splitting).
pub(crate) trait FormatImpl: Sync {
    /// The format's name (Go's `Format` string).
    fn name(&self) -> &'static str;

    /// Rune classifier; built fresh per tokenizer (may read env vars).
    fn classifier(&self) -> RuneClassifier;

    /// Classify a wordbreak token's raw value (redirect vs pipeline vs other).
    fn classify_operator(&self, _raw: &str) -> WordbreakType {
        WordbreakType::Unknown
    }

    /// Bare-word operators (e.g. fish `and`/`or`) treated as wordbreaks.
    fn keyword_operators(&self) -> &'static [(&'static str, WordbreakType)] {
        &[]
    }

    /// The non-escaping quote (single quote) supports limited escapes
    /// (`''` doubled, `\'`/`\\`): fish, elvish, zsh, PowerShell.
    fn non_escaping_quote_escapes(&self) -> bool {
        false
    }

    /// Backslash is an escape inside single quotes (only fish and xonsh).
    fn non_escaping_quote_backslash_escapes(&self) -> bool {
        false
    }

    /// `false` when the escape character is a literal bareword character
    /// outside quotes (only elvish, nushell).
    fn escape_not_bareword(&self) -> bool {
        true
    }

    /// `true` when the escape character is literal inside double quotes
    /// (only cmd: `^` is completely literal inside quotes).
    fn escape_not_in_escaping_quote(&self) -> bool {
        false
    }

    /// The characters the escape character can escape inside double
    /// quotes; empty means any character.
    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &['\\', '`', '$', '"', '\n']
    }

    /// Triple-quoted strings (`'''...'''`, `"""..."""`): only xonsh.
    fn triple_quote_support(&self) -> bool {
        false
    }

    /// Raw string prefixes (`r'...'`, `r"..."`): only xonsh.
    fn raw_prefix_support(&self) -> bool {
        false
    }

    /// Quote a single word for safe insertion into a command line.
    fn quote_word(&self, s: &str) -> String;

    /// Transform the rune following the escape character inside double
    /// quotes (None: keep the escape and the rune). Only nushell implements
    /// the C-style escape table (Go's `escapingQuoteUnescaper`).
    fn escaping_quote_unescape(&self, _r: char) -> Option<String> {
        None
    }

    /// Whether the format transforms escapes inside double quotes through
    /// [`FormatImpl::escaping_quote_unescape`]; unrecognized escapes then
    /// keep their backslash instead of consulting the escape-chars table.
    fn escaping_quote_unescaper(&self) -> bool {
        false
    }

    /// Optional per-format insertion quoting for states whose escaping
    /// differs from the POSIX-style default (PowerShell escapes with the
    /// backtick); None falls back to the default rules.
    fn quote_insertion_custom(&self, _state: LexerState, _value: &str) -> Option<String> {
        None
    }

    /// Reclassify tokens after the main pass (elvish braces, nushell
    /// stream redirects, cmd/powershell numeric redirects).
    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        tokens
    }

    /// The escape character followed by this rune acts as a line
    /// continuation (consumed, discarded): bash/zsh/tcsh/fish/xonsh/hilbish
    /// (backslash+newline), PowerShell (backtick+newline), cmd (caret+newline).
    fn is_line_continuation(&self, _r: char) -> bool {
        false
    }

    /// This rune followed by a newline acts as whitespace (elvish `^`).
    fn line_continuation_char(&self) -> Option<char> {
        None
    }

    fn is_line_continuation_whitespace(&self, _r: char) -> bool {
        false
    }

    /// Block comment opener/closer (PowerShell `<#`/`#>`).
    fn block_comment(&self) -> Option<(&'static str, &'static str)> {
        None
    }

    /// The token triggering raw mode (PowerShell `--%`).
    fn stop_parsing_word(&self) -> Option<&'static str> {
        None
    }

    /// Bash's COMP_WORDS interface splits words naively on delimiter
    /// characters: the text the shell replaces on insertion is only the
    /// raw text after the last such character.
    fn naive_split_word_support(&self) -> bool {
        false
    }

    fn naive_split_word(&self, raw: &str) -> String {
        let _ = raw;
        String::new()
    }
}

/// The behavior for a format name. The empty name resolves to Bash (spec
/// macros and callers that omit the shell); unknown names are rejected.
pub(crate) fn format_impl_for(name: super::Format) -> Option<&'static dyn FormatImpl> {
    use super::formats::*;
    match name {
        super::Format::Default | super::Format::Bash | super::Format::Oil => Some(&BashFormat),
        super::Format::Cmd => Some(&CmdFormat),
        super::Format::Elvish => Some(&ElvishFormat),
        super::Format::Fish => Some(&FishFormat),
        super::Format::Hilbish => Some(&HilbishFormat),
        super::Format::Nushell => Some(&NushellFormat),
        super::Format::Powershell => Some(&PowershellFormat),
        super::Format::Tcsh => Some(&TcshFormat),
        super::Format::Xonsh => Some(&XonshFormat),
        super::Format::Zsh => Some(&ZshFormat),
    }
}
