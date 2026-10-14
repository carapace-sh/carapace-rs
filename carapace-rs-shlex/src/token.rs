//! Token types and rune classification (port of `shlex.go`).

/// A top-level token classification: word, space, comment, wordbreak, unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TokenType {
    #[default]
    Unknown,
    Word,
    Space,
    Comment,
    Wordbreak,
}

/// The internal state of the lexer state machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LexerState {
    #[default]
    /// No runes have been seen.
    Start,
    /// Processing regular runes in a word.
    InWord,
    /// The previous rune was an escape character.
    Escaping,
    /// The previous rune was an escape character within a quoted string.
    EscapingQuoted,
    /// Within a quoted string that supports escaping (`"..."`).
    QuotingEscaping,
    /// Within a string that does not support escaping (`'...'`).
    Quoting,
    /// Within a triple-quoted non-escaping string (`'''...'''`).
    QuotingTriple,
    /// Within a triple-quoted escaping string (`"""..."""`).
    QuotingTripleEscaping,
    /// Within a comment (everything after an unquoted `#`).
    Comment,
    /// Within a block comment (e.g. PowerShell `<# ... #>`).
    BlockComment,
    /// Raw mode after a stop-parsing token (e.g. PowerShell `--%`).
    StopParsing,
    /// The previous rune was a wordbreak.
    Wordbreak,
}

/// A rune-offset range in the input string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// Rune offset of the first character.
    pub start: usize,
    /// Rune offset after the last character.
    pub end: usize,
}

/// A lexicographic token: a (type, value) pair with source information.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Token {
    #[allow(dead_code)]
    pub kind: TokenType,
    /// Dequoted value.
    pub value: String,
    /// Raw source text (quotes and escapes included).
    pub raw_value: String,
    /// Rune-offset range in the input.
    pub span: Span,
    /// The lexer state the token ended in.
    pub state: LexerState,
    /// Wordbreak classification for wordbreak tokens.
    pub wordbreak_type: super::WordbreakType,
    /// Index of the last opening quote in `value` (correct while in a quoting state).
    pub wordbreak_index: usize,
}

impl Token {
    pub(crate) fn add(&mut self, r: char) {
        self.value.push(r);
    }

    pub(crate) fn remove_last_raw(&mut self) {
        self.raw_value.pop();
    }

    pub(crate) fn adjoins(&self, other: &Token) -> bool {
        self.span.end == other.span.start || self.span.start == other.span.end
    }
}

/// Named classes of runes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RuneClass {
    #[default]
    Unknown,
    Space,
    EscapingQuote,
    NonEscapingQuote,
    Escape,
    Comment,
    Wordbreak,
    Eof,
}

/// Classifies runes for a format.
#[derive(Clone, Debug)]
pub(crate) struct RuneClassifier {
    classes: [RuneClass; 128],
}

impl Default for RuneClassifier {
    fn default() -> Self {
        Self {
            classes: [RuneClass::Unknown; 128],
        }
    }
}

impl RuneClassifier {
    /// The standard POSIX classes (space, quotes, escape, comment) without
    /// wordbreaks; formats add their own wordbreaks on top.
    pub(crate) fn base(escape_runes: &str) -> Self {
        let mut classifier = Self::default();
        classifier.add_class(" \t\r\n", RuneClass::Space);
        classifier.add_class("\"", RuneClass::EscapingQuote);
        classifier.add_class("'", RuneClass::NonEscapingQuote);
        classifier.add_class(escape_runes, RuneClass::Escape);
        classifier.add_class("#", RuneClass::Comment);
        classifier
    }

    pub(crate) fn add_class(&mut self, runes: &str, class: RuneClass) {
        for r in runes.chars() {
            if (r as usize) < 128 {
                self.classes[r as usize] = class;
            }
        }
    }

    /// Add wordbreak runes, filtering out runes with a stronger class.
    pub(crate) fn add_wordbreaks(&mut self, wordbreak_runes: &str) {
        let filtered: String = wordbreak_runes
            .chars()
            .filter(|r| matches!(self.classify(*r), RuneClass::Unknown))
            .collect();
        self.add_class(&filtered, RuneClass::Wordbreak);
    }

    pub(crate) fn classify(&self, r: char) -> RuneClass {
        let idx = r as usize;
        if idx < 128 {
            self.classes[idx]
        } else {
            RuneClass::Unknown
        }
    }
}
