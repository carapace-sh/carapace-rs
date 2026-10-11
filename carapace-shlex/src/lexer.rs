//! The lexer state machine (port of `shlex.go`'s tokenizer/lexer).

use super::format::FormatImpl;
use super::token::{LexerState, RuneClass, Token, TokenType};
use super::wordbreak::WordbreakType;

/// Turns an input string into a sequence of typed tokens. Whitespace and
/// comments are skipped by the [`Lexer`](crate::Lexer) wrapper.
pub(crate) struct Tokenizer {
    chars: Vec<char>,
    index: usize,
    classifier: RuneClassifierAlias,
    format: &'static dyn FormatImpl,
    state: LexerState,
    raw_quote: bool,
    triple_quote_rune: Option<char>,
    block_closer: String,
    block_closer_index: usize,
}

type RuneClassifierAlias = super::token::RuneClassifier;

impl Tokenizer {
    pub(crate) fn new(input: &str, format: &'static dyn FormatImpl) -> Self {
        Self {
            chars: input.chars().collect(),
            index: 0,
            classifier: format.classifier(),
            format,
            state: LexerState::Start,
            raw_quote: false,
            triple_quote_rune: None,
            block_closer: String::new(),
            block_closer_index: 0,
        }
    }

    /// The rune at the cursor (None at end of input).
    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn read(&mut self) -> Option<char> {
        let r = self.peek();
        if r.is_some() {
            self.index += 1;
        }
        r
    }

    /// Returns the next token, or None at the end of input.
    pub(crate) fn next(&mut self) -> Option<Token> {
        if self.state == LexerState::StopParsing {
            if let Some(mut token) = self.scan_stop_parsing() {
                token.state = self.state;
                if token.span.end == 0 && token.span.start != 0 {
                    token.span.end = token.span.start + token.raw_value.chars().count();
                } else if token.span.end == 0
                    && token.span.start == 0
                    && !token.raw_value.is_empty()
                {
                    token.span.end = token.raw_value.chars().count();
                }
                return Some(token);
            }
            return None;
        }

        let mut token = self.scan_stream()?;
        token.state = self.state;
        if token.span.end == 0 && token.span.start != 0 {
            token.span.end = token.span.start + token.raw_value.chars().count();
        } else if token.span.end == 0
            && token.span.start == 0
            && !token.raw_value.is_empty()
            && token.kind == TokenType::Word
        {
            token.span.end = token.raw_value.chars().count();
        }
        if token.kind == TokenType::Wordbreak {
            token.wordbreak_type = self.format.classify_operator(&token.raw_value);
        }
        if token.kind == TokenType::Word {
            for (word, wb_type) in self.format.keyword_operators() {
                if token.raw_value == *word {
                    token.kind = TokenType::Wordbreak;
                    token.wordbreak_type = *wb_type;
                    break;
                }
            }
            if let Some(word) = self.format.stop_parsing_word() {
                if token.value == word && token.value == token.raw_value {
                    self.state = LexerState::StopParsing;
                }
            }
        }
        Some(token)
    }

    fn check_triple_quote(&mut self, quote: char) -> TripleQuote {
        if !self.format.triple_quote_support() {
            return TripleQuote::No { consumed: None };
        }
        let Some(peek1) = self.read() else {
            return TripleQuote::No { consumed: None };
        };
        if peek1 != quote {
            self.index -= 1;
            return TripleQuote::No { consumed: None };
        }
        let Some(peek2) = self.read() else {
            // peek1 matched but EOF after — can't unread, return it consumed
            return TripleQuote::No {
                consumed: Some(peek1),
            };
        };
        if peek2 != quote {
            self.index -= 1;
            return TripleQuote::No {
                consumed: Some(peek1),
            };
        }
        TripleQuote::Yes
    }

    /// Examine the current token's value for a raw string prefix (r/R with
    /// valid Python prefix characters, no duplicates).
    fn check_raw_prefix(&self, token: &Token) -> bool {
        if !self.format.raw_prefix_support() {
            return false;
        }
        let value = token.value.as_bytes();
        if value.is_empty() {
            return false;
        }
        let mut seen = [false; 26];
        for &c in value {
            let lower = c.to_ascii_lowercase();
            if !matches!(lower, b'b' | b'p' | b'r' | b'u' | b'f') {
                return false;
            }
            let slot = (lower - b'a') as usize;
            if seen[slot] {
                return false;
            }
            seen[slot] = true;
        }
        value.contains(&b'r') || value.contains(&b'R')
    }

    fn check_triple_close(&mut self) -> TripleClose {
        let Some(peek1) = self.read() else {
            return TripleClose::NotClosed { consumed: None };
        };
        let Some(triple_quote) = self.triple_quote_rune else {
            return TripleClose::NotClosed { consumed: None };
        };
        if peek1 != triple_quote {
            self.index -= 1;
            return TripleClose::NotClosed { consumed: None };
        }
        let Some(peek2) = self.read() else {
            return TripleClose::NotClosed {
                consumed: Some(peek1),
            };
        };
        if peek2 != triple_quote {
            self.index -= 1;
            return TripleClose::NotClosed {
                consumed: Some(peek1),
            };
        }
        TripleClose::Closed
    }

    fn check_block_comment_opener(&mut self, first_rune: char, token: &mut Token) -> bool {
        let Some((opener, closer)) = self.format.block_comment() else {
            return false;
        };
        if opener.is_empty() || !opener.starts_with(first_rune) {
            return false;
        }
        if opener.len() > 2 {
            return false;
        }
        let entry_index = self.index;
        let entry_raw_length = token.raw_value.len();
        for expected in opener.chars().skip(1) {
            match self.read() {
                Some(r) if r == expected => token.raw_value.push(r),
                _ => {
                    // restore the cursor and raw value on mismatch
                    self.index = entry_index;
                    token.raw_value.truncate(entry_raw_length);
                    return false;
                }
            }
        }
        token.raw_value.push_str(&opener[1..]);
        self.block_closer = closer.to_owned();
        self.block_closer_index = 0;
        true
    }

    /// Scan the stream for the next token using the state machine.
    fn scan_stream(&mut self) -> Option<Token> {
        let previous_state = self.state;
        self.state = LexerState::Start;
        self.raw_quote = false;
        self.triple_quote_rune = None;
        let mut token = Token::default();
        let mut saw_space = false;

        loop {
            let read = self.read();
            let next_rune = read; // None at EOF
            let next_rune_type = match next_rune {
                Some(r) => self.classifier.classify(r),
                None => RuneClass::Eof,
            };
            if let Some(r) = next_rune {
                token.raw_value.push(r);
            }

            match self.state {
                LexerState::Start => {
                    if next_rune_type != RuneClass::Space {
                        token.span.start = self.index.saturating_sub(1);
                    }
                    if let Some(r) = next_rune {
                        if self.check_block_comment_opener(r, &mut token) {
                            token.kind = TokenType::Comment;
                            self.state = LexerState::BlockComment;
                            continue;
                        }
                    }
                    match next_rune_type {
                        RuneClass::Eof => {
                            if self.index == 0 {
                                // empty input: an empty token for the cursor
                                token.kind = TokenType::Word;
                                token.span.start = self.index;
                                token.span.end = self.index;
                                self.index += 1;
                                return Some(token);
                            }
                            if previous_state == LexerState::Wordbreak || saw_space {
                                token.kind = TokenType::Word;
                                token.span.start = self.index;
                                token.span.end = self.index;
                                return Some(token);
                            }
                            return None;
                        }
                        RuneClass::Space => {
                            token.remove_last_raw();
                            saw_space = true;
                        }
                        RuneClass::EscapingQuote => {
                            token.kind = TokenType::Word;
                            token.wordbreak_index = token.value.len();
                            match self.check_triple_quote(next_rune.unwrap()) {
                                TripleQuote::Yes => {
                                    self.triple_quote_rune = next_rune;
                                    if self.check_raw_prefix(&token) {
                                        self.raw_quote = true;
                                        self.state = LexerState::QuotingTripleEscaping;
                                    } else {
                                        self.state = LexerState::QuotingTripleEscaping;
                                    }
                                }
                                TripleQuote::No { consumed: Some(c) } => {
                                    token.raw_value.push(c);
                                    self.state = LexerState::InWord;
                                }
                                TripleQuote::No { consumed: None } => {
                                    if self.check_raw_prefix(&token) {
                                        self.raw_quote = true;
                                    }
                                    self.state = LexerState::QuotingEscaping;
                                }
                            }
                        }
                        RuneClass::NonEscapingQuote => {
                            token.kind = TokenType::Word;
                            token.wordbreak_index = token.value.len();
                            match self.check_triple_quote(next_rune.unwrap()) {
                                TripleQuote::Yes => {
                                    self.triple_quote_rune = next_rune;
                                    self.state = LexerState::QuotingTriple;
                                }
                                TripleQuote::No { consumed: Some(c) } => {
                                    token.raw_value.push(c);
                                    self.state = LexerState::InWord;
                                }
                                TripleQuote::No { consumed: None } => {
                                    self.state = LexerState::Quoting;
                                }
                            }
                        }
                        RuneClass::Escape => {
                            token.kind = TokenType::Word;
                            if self.format.escape_not_bareword() {
                                if self.handle_line_continuation(&mut token, true) {
                                    continue;
                                }
                                self.state = LexerState::Escaping;
                            } else {
                                token.add(next_rune.unwrap());
                                self.state = LexerState::InWord;
                            }
                        }
                        RuneClass::Comment => {
                            token.kind = TokenType::Comment;
                            self.state = LexerState::Comment;
                        }
                        RuneClass::Wordbreak => {
                            token.kind = TokenType::Wordbreak;
                            token.add(next_rune.unwrap());
                            self.state = LexerState::Wordbreak;
                        }
                        _ => {
                            if self.handle_line_continuation_whitespace(&mut token, next_rune) {
                                continue;
                            }
                            token.kind = TokenType::Word;
                            token.add(next_rune.unwrap());
                            self.state = LexerState::InWord;
                        }
                    }
                }
                LexerState::Wordbreak => {
                    match next_rune_type {
                        RuneClass::Wordbreak => {
                            // extend the operator; a new operator starts when
                            // the extended raw value is no longer known. The
                            // raw value already includes the rune (appended
                            // at the top of the loop).
                            if self.format.classify_operator(&token.raw_value)
                                == WordbreakType::Unknown
                            {
                                token.remove_last_raw();
                                self.index -= 1;
                                return Some(token);
                            }
                            token.add(next_rune.unwrap());
                            self.state = LexerState::Wordbreak;
                        }
                        _ => {
                            if next_rune.is_some() {
                                token.remove_last_raw();
                                self.index -= 1;
                            }
                            // at EOF nothing was appended: the raw value
                            // keeps the operator intact
                            return Some(token);
                        }
                    }
                }
                LexerState::InWord => {
                    match next_rune_type {
                        RuneClass::Wordbreak | RuneClass::Space => {
                            token.remove_last_raw();
                            self.index -= 1;
                            return Some(token);
                        }
                        RuneClass::Eof => {
                            return Some(token);
                        }
                        RuneClass::EscapingQuote => {
                            token.wordbreak_index = token.value.len();
                            match self.check_triple_quote(next_rune.unwrap()) {
                                TripleQuote::Yes => {
                                    self.triple_quote_rune = next_rune;
                                    if self.check_raw_prefix(&token) {
                                        self.raw_quote = true;
                                    }
                                    self.state = LexerState::QuotingTripleEscaping;
                                }
                                TripleQuote::No { consumed: Some(c) } => {
                                    token.raw_value.push(c);
                                    self.state = LexerState::InWord;
                                }
                                TripleQuote::No { consumed: None } => {
                                    if self.check_raw_prefix(&token) {
                                        self.raw_quote = true;
                                        self.state = LexerState::QuotingEscaping;
                                    } else {
                                        self.state = LexerState::QuotingEscaping;
                                    }
                                }
                            }
                        }
                        RuneClass::NonEscapingQuote => {
                            token.wordbreak_index = token.value.len();
                            match self.check_triple_quote(next_rune.unwrap()) {
                                TripleQuote::Yes => {
                                    self.triple_quote_rune = next_rune;
                                    self.state = LexerState::QuotingTriple;
                                }
                                TripleQuote::No { consumed: Some(c) } => {
                                    token.raw_value.push(c);
                                    self.state = LexerState::InWord;
                                }
                                TripleQuote::No { consumed: None } => {
                                    self.state = LexerState::Quoting;
                                }
                            }
                        }
                        RuneClass::Escape => {
                            if self.format.escape_not_bareword() {
                                if self.handle_line_continuation(&mut token, true) {
                                    continue;
                                }
                                self.state = LexerState::Escaping;
                            } else {
                                token.add(next_rune.unwrap()); // elvish: \ is a bareword char
                            }
                        }
                        _ => {
                            if self.handle_line_continuation_whitespace(&mut token, next_rune) {
                                return Some(token);
                            }
                            token.add(next_rune.unwrap());
                        }
                    }
                }
                LexerState::Escaping => {
                    match next_rune_type {
                        RuneClass::Eof => {
                            return Some(token);
                        }
                        _ => {
                            if let Some(r) = next_rune {
                                if self.format.is_line_continuation(r) {
                                    if r == '\r' && self.peek() == Some('\n') {
                                        self.index += 1; // consume \n after \r
                                    }
                                    token.raw_value.pop();
                                    token.raw_value.pop(); // newline + escape char
                                    self.state = if !token.value.is_empty() {
                                        LexerState::InWord
                                    } else {
                                        LexerState::Start
                                    };
                                    continue;
                                }
                            }
                            self.state = LexerState::InWord;
                            token.add(next_rune.unwrap());
                        }
                    }
                }
                LexerState::EscapingQuoted => match next_rune_type {
                    RuneClass::Eof => {
                        return Some(token);
                    }
                    _ => {
                        if let Some(r) = next_rune {
                            if self.format.is_line_continuation(r) {
                                if r == '\r' && self.peek() == Some('\n') {
                                    self.index += 1; // consume \n after \r
                                }
                                token.raw_value.pop();
                                token.raw_value.pop(); // newline + escape char
                                self.state = LexerState::QuotingEscaping;
                                continue;
                            }
                            self.state = LexerState::QuotingEscaping;
                            if let Some(replacement) = self.format.escaping_quote_unescape(r) {
                                token.value.push_str(&replacement);
                            } else if self
                                .format
                                .escaping_quote_escape_chars()
                                .contains(&r)
                            {
                                token.add(r);
                            } else {
                                token.add('\\');
                                token.add(r);
                            }
                        }
                    }
                },
                LexerState::QuotingEscaping => {
                    match next_rune_type {
                        RuneClass::Eof => {
                            return Some(token);
                        }
                        RuneClass::EscapingQuote => {
                            if self.format.non_escaping_quote_escapes() {
                                // PowerShell: "" → literal " (doubled), else close
                                match self.read() {
                                    Some(peek)
                                        if self.classifier.classify(peek)
                                            == RuneClass::EscapingQuote =>
                                    {
                                        token.raw_value.push(peek);
                                        token.add(next_rune.unwrap());
                                        // stay in QuotingEscaping
                                    }
                                    peek => {
                                        if peek.is_some() {
                                            self.index -= 1;
                                        }
                                        self.raw_quote = false;
                                        self.state = LexerState::InWord;
                                    }
                                }
                            } else {
                                self.raw_quote = false;
                                self.state = LexerState::InWord;
                            }
                        }
                        RuneClass::Escape => {
                            if self.raw_quote {
                                token.add(next_rune.unwrap()); // raw string: backslash is literal
                            } else if self.format.escape_not_in_escaping_quote() {
                                token.add(next_rune.unwrap()); // cmd: caret literal inside quotes
                            } else {
                                self.state = LexerState::EscapingQuoted;
                            }
                        }
                        _ => {
                            token.add(next_rune.unwrap());
                        }
                    }
                }
                LexerState::Quoting => {
                    match next_rune_type {
                        RuneClass::Eof => {
                            return Some(token);
                        }
                        RuneClass::NonEscapingQuote => {
                            self.raw_quote = false;
                            if self.format.non_escaping_quote_escapes() {
                                // '' → literal ' (stay in quote), else close
                                match self.read() {
                                    Some(peek)
                                        if self.classifier.classify(peek)
                                            == RuneClass::NonEscapingQuote =>
                                    {
                                        token.raw_value.push(peek);
                                        token.add(next_rune.unwrap());
                                    }
                                    peek => {
                                        if peek.is_some() {
                                            self.index -= 1;
                                        }
                                        self.state = LexerState::InWord;
                                    }
                                }
                            } else {
                                self.state = LexerState::InWord;
                            }
                        }
                        RuneClass::Escape => {
                            if self.format.non_escaping_quote_backslash_escapes() {
                                // fish: only \' and \\ are escapes; other \X literal
                                match self.read() {
                                    Some(peek) => {
                                        token.raw_value.push(peek);
                                        if peek == '\'' || peek == '\\' {
                                            token.add(peek);
                                        } else {
                                            token.add(next_rune.unwrap());
                                            token.add(peek);
                                        }
                                    }
                                    None => {
                                        token.add(next_rune.unwrap());
                                    }
                                }
                            } else {
                                token.add(next_rune.unwrap()); // literal backslash
                            }
                        }
                        _ => {
                            token.add(next_rune.unwrap());
                        }
                    }
                }
                LexerState::QuotingTriple => {
                    match next_rune_type {
                        RuneClass::Eof => {
                            return Some(token);
                        }
                        RuneClass::NonEscapingQuote | RuneClass::EscapingQuote
                            if next_rune == self.triple_quote_rune =>
                        {
                            match self.check_triple_close() {
                                TripleClose::Closed => {
                                    // the two closing runes already appended
                                    self.raw_quote = false;
                                    self.triple_quote_rune = None;
                                    self.state = LexerState::InWord;
                                }
                                TripleClose::NotClosed { consumed: Some(c) } => {
                                    token.raw_value.push(c);
                                    token.add(next_rune.unwrap());
                                    token.add(c);
                                }
                                TripleClose::NotClosed { consumed: None } => {
                                    token.add(next_rune.unwrap());
                                }
                            }
                        }
                        _ => {
                            token.add(next_rune.unwrap());
                        }
                    }
                }
                LexerState::QuotingTripleEscaping => match next_rune_type {
                    RuneClass::Eof => {
                        return Some(token);
                    }
                    RuneClass::EscapingQuote | RuneClass::NonEscapingQuote
                        if next_rune == self.triple_quote_rune =>
                    {
                        match self.check_triple_close() {
                            TripleClose::Closed => {
                                self.raw_quote = false;
                                self.triple_quote_rune = None;
                                self.state = LexerState::InWord;
                            }
                            TripleClose::NotClosed { consumed: Some(c) } => {
                                token.raw_value.push(c);
                                token.add(next_rune.unwrap());
                                token.add(c);
                            }
                            TripleClose::NotClosed { consumed: None } => {
                                token.add(next_rune.unwrap());
                            }
                        }
                    }
                    RuneClass::Escape => {
                        if self.raw_quote {
                            token.add(next_rune.unwrap());
                        } else {
                            self.state = LexerState::EscapingQuoted;
                        }
                    }
                    _ => {
                        token.add(next_rune.unwrap());
                    }
                },
                LexerState::Comment => match next_rune_type {
                    RuneClass::Eof => {
                        return Some(token);
                    }
                    RuneClass::Space => {
                        if next_rune == Some('\n') {
                            self.state = LexerState::Start;
                            return Some(token);
                        }
                        token.add(next_rune.unwrap());
                    }
                    _ => {
                        token.add(next_rune.unwrap());
                    }
                },
                LexerState::BlockComment => {
                    let closer = self.block_closer.clone();
                    if Some(closer.as_bytes()[self.block_closer_index] as char) == next_rune {
                        self.block_closer_index += 1;
                        if self.block_closer_index >= closer.len() {
                            self.block_closer = String::new();
                            self.block_closer_index = 0;
                            self.state = LexerState::Start;
                            return Some(token);
                        }
                    } else {
                        self.block_closer_index = 0;
                        if Some(closer.as_bytes()[0] as char) == next_rune {
                            self.block_closer_index = 1;
                        }
                    }
                    if next_rune_type == RuneClass::Eof {
                        return Some(token);
                    }
                }
                LexerState::StopParsing => unreachable!("handled by scan_stop_parsing"),
            }
        }
    }

    /// Handle the escape + line-continuation sequence (consumed, discarded).
    /// Returns true when a continuation was consumed.
    fn handle_line_continuation(&mut self, token: &mut Token, at_start: bool) -> bool {
        let Some(peek) = self.peek() else {
            return false;
        };
        if !self.format.is_line_continuation(peek) {
            return false;
        }
        self.index += 1; // consume the newline
        // consume optional \n after \r
        if peek == '\r' && self.peek() == Some('\n') {
            self.index += 1;
        }
        token.raw_value.pop(); // the escape char
        if let Some(r) = self.peek() {
            let _ = r;
        }
        if at_start {
            // no word content yet: stay in Start
            self.state = LexerState::Start;
        }
        true
    }

    /// Handle elvish `^`+newline acting as whitespace. Returns true when
    /// consumed.
    fn handle_line_continuation_whitespace(
        &mut self,
        token: &mut Token,
        next_rune: Option<char>,
    ) -> bool {
        let Some(line_char) = self.format.line_continuation_char() else {
            return false;
        };
        if next_rune != Some(line_char) {
            return false;
        }
        match self.peek() {
            Some(peek) if self.format.is_line_continuation_whitespace(peek) => {
                self.index += 1;
                if peek == '\r' && self.peek() == Some('\n') {
                    self.index += 1;
                }
                token.raw_value.pop(); // the ^ char
                true
            }
            _ => false,
        }
    }

    /// Raw mode after a stop-parsing token: everything literal until
    /// newline or pipeline delimiter.
    fn scan_stop_parsing(&mut self) -> Option<Token> {
        let mut token = Token {
            kind: TokenType::Word,
            ..Default::default()
        };
        self.state = LexerState::StopParsing;
        let mut in_quotes = false;

        // skip leading whitespace
        loop {
            match self.read() {
                None => {
                    token.span.start = self.index;
                    token.span.end = self.index;
                    self.state = LexerState::Start;
                    return Some(token);
                }
                Some(r) if matches!(r, ' ' | '\t' | '\r' | '\n') => {
                    if matches!(r, '\r' | '\n') {
                        token.span.start = self.index;
                        token.span.end = self.index;
                        self.state = LexerState::Start;
                        return Some(token);
                    }
                }
                Some(_) => {
                    self.index -= 1;
                    break;
                }
            }
        }

        loop {
            match self.read() {
                None => {
                    if token.raw_value.is_empty() {
                        return None;
                    }
                    token.span.end = token.span.start + token.raw_value.chars().count();
                    token.state = LexerState::StopParsing;
                    self.state = LexerState::Start;
                    return Some(token);
                }
                Some(r) => {
                    if token.raw_value.is_empty() {
                        token.span.start = self.index - 1;
                    }

                    if r == '\r' || r == '\n' {
                        self.index -= 1;
                        if token.raw_value.is_empty() {
                            token.span.start = self.index;
                            token.span.end = self.index;
                            self.state = LexerState::Start;
                            return Some(token);
                        }
                        token.span.end = token.span.start + token.raw_value.chars().count();
                        token.state = LexerState::StopParsing;
                        self.state = LexerState::Start;
                        return Some(token);
                    }

                    token.raw_value.push(r);

                    if r == '"' {
                        in_quotes = !in_quotes;
                        token.add(r);
                        continue;
                    }

                    if !in_quotes && r == '|' {
                        // pipeline delimiter — return the word before it
                        token.raw_value.pop();
                        self.index -= 1;
                        if token.raw_value.is_empty() {
                            token.span.start = self.index;
                            token.span.end = self.index;
                        } else {
                            token.span.end = token.span.start + token.raw_value.chars().count();
                        }
                        token.state = LexerState::StopParsing;
                        self.state = LexerState::Start;
                        return Some(token);
                    }

                    token.add(r);
                }
            }
        }
    }
}

enum TripleQuote {
    Yes,
    No { consumed: Option<char> },
}

enum TripleClose {
    Closed,
    NotClosed { consumed: Option<char> },
}

/// Lexer skipping whitespace and comments, yielding word and wordbreak
/// tokens (port of `lexer`).
pub(crate) struct Lexer {
    tokenizer: Tokenizer,
}

impl Lexer {
    pub(crate) fn new(input: &str, format: &'static dyn FormatImpl) -> Self {
        Self {
            tokenizer: Tokenizer::new(input, format),
        }
    }

    /// The next word or wordbreak token, or None at the end of input.
    pub(crate) fn next(&mut self) -> Option<Token> {
        loop {
            let token = self.tokenizer.next()?;
            match token.kind {
                TokenType::Word | TokenType::Wordbreak => return Some(token),
                TokenType::Comment => continue, // skip comments
                _ => return None,
            }
        }
    }
}
