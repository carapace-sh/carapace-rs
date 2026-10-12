//! Token slice helpers: pipelines, words, redirect filtering, wordbreak
//! prefix (port of `tokenslice.go`).

use super::wordbreak::WordbreakType;
use crate::token::{LexerState, Token, TokenType};

/// Helpers over a token slice (port of the `TokenSlice` methods in
/// `tokenslice.go`).
pub trait TokenSliceExt {
    /// The tokens of the last (current) pipeline: pipeline delimiters split
    /// pipelines; substitution scopes protect their content.
    fn current_pipeline(&self) -> Vec<Token>;

    /// The pipelines of the token slice: pipeline delimiters split
    /// pipelines; substitution scopes protect their content.
    fn pipelines(&self) -> Vec<Vec<Token>>;

    /// Merge tokens into words: adjoining tokens concatenate.
    fn words(&self) -> Vec<Token>;

    /// The word values of the slice.
    fn strings(&self) -> Vec<String>;

    /// The last word of the slice.
    fn current_token(&self) -> Option<Token>;

    /// Remove redirect operators and their targets.
    fn filter_redirects(&self) -> Vec<Token>;

    /// Merge tokens into words, treating closed substitution scopes as
    /// single words; unclosed scopes are left as separate tokens.
    fn words_with_substitutions(&self) -> Vec<Token>;

    /// The wordbreak prefix of the last token: the raw text from the last
    /// word boundary (or opening quote) up to the cursor.
    fn wordbreak_prefix(&self) -> String;
}

impl TokenSliceExt for [Token] {
    fn pipelines(&self) -> Vec<Vec<Token>> {
        let mut pipelines: Vec<Vec<Token>> = Vec::new();
        let mut pipeline: Vec<Token> = Vec::new();
        let mut depth: usize = 0;
        for token in self {
            if token.wordbreak_type == WordbreakType::SubstitutionOpen {
                depth += 1;
                pipeline.push(token.clone());
            } else if token.wordbreak_type == WordbreakType::SubstitutionClose {
                depth = depth.saturating_sub(1);
                pipeline.push(token.clone());
            } else if depth > 0 {
                pipeline.push(token.clone());
            } else if token.kind == TokenType::Wordbreak
                && token.wordbreak_type.is_pipeline_delimiter()
            {
                pipelines.push(std::mem::take(&mut pipeline));
            } else {
                pipeline.push(token.clone());
            }
        }
        pipelines.push(pipeline);
        pipelines
    }

    fn current_pipeline(&self) -> Vec<Token> {
        let mut pipelines = self.pipelines();
        pipelines.pop().unwrap_or_default()
    }

    fn words(&self) -> Vec<Token> {
        let mut words: Vec<Token> = Vec::new();
        for (index, token) in self.iter().enumerate() {
            if index == 0 {
                words.push(token.clone());
            } else if self[index - 1].adjoins(token) {
                let last = words.last_mut().unwrap();
                last.value.push_str(&token.value);
                last.raw_value.push_str(&token.raw_value);
                last.span.end = token.span.end;
                last.state = token.state;
            } else {
                words.push(token.clone());
            }
        }
        words
    }

    fn strings(&self) -> Vec<String> {
        self.iter().map(|t| t.value.clone()).collect()
    }

    fn current_token(&self) -> Option<Token> {
        self.last().cloned()
    }

    fn filter_redirects(&self) -> Vec<Token> {
        let mut filtered = Vec::new();
        for (index, token) in self.iter().enumerate() {
            if token.kind == TokenType::Wordbreak && token.wordbreak_type.is_redirect() {
                continue;
            }
            if index > 0 && self[index - 1].wordbreak_type.is_redirect() {
                continue;
            }
            if index < self.len() - 1 {
                let next = &self[index + 1];
                if token.adjoins(next)
                    && token.raw_value.parse::<i64>().is_ok()
                    && next.wordbreak_type.is_redirect()
                {
                    continue;
                }
            }
            filtered.push(token.clone());
        }
        filtered
    }

    fn words_with_substitutions(&self) -> Vec<Token> {
        let mut words: Vec<Token> = Vec::new();
        let mut depth = 0;
        let mut sub: Option<Token> = None;

        for (index, token) in self.iter().enumerate() {
            if token.wordbreak_type == WordbreakType::SubstitutionOpen {
                if depth == 0 {
                    sub = Some(Token {
                        kind: TokenType::Word,
                        span: token.span,
                        ..Default::default()
                    });
                }
                depth += 1;
                if let Some(sub) = &mut sub {
                    if sub.span.end != token.span.start && !sub.raw_value.is_empty() {
                        sub.raw_value.push(' ');
                        sub.value.push(' ');
                    }
                    sub.raw_value.push_str(&token.raw_value);
                    sub.value.push_str(&token.raw_value);
                    sub.span.end = token.span.end;
                    sub.state = token.state;
                }
            } else if token.wordbreak_type == WordbreakType::SubstitutionClose {
                if depth > 0 {
                    if let Some(sub) = &mut sub {
                        if sub.span.end != token.span.start && !sub.raw_value.is_empty() {
                            sub.raw_value.push(' ');
                            sub.value.push(' ');
                        }
                        sub.raw_value.push_str(&token.raw_value);
                        sub.value.push_str(&token.raw_value);
                        sub.span.end = token.span.end;
                        sub.state = token.state;
                    }
                    depth -= 1;
                    if depth == 0 {
                        words.push(sub.take().unwrap());
                    }
                }
            } else if depth > 0 {
                if let Some(sub) = &mut sub {
                    // insert a space when tokens don't adjoin (gap = space
                    // tokens the lexer skipped)
                    if sub.span.end != token.span.start && !sub.raw_value.is_empty() {
                        sub.raw_value.push(' ');
                        sub.value.push(' ');
                    }
                    sub.raw_value.push_str(&token.raw_value);
                    sub.value.push_str(&token.raw_value);
                    sub.span.end = token.span.end;
                    sub.state = token.state;
                }
            } else if index == 0 {
                words.push(token.clone());
            } else if self[index - 1].adjoins(token) {
                let last = words.last_mut().unwrap();
                last.value.push_str(&token.value);
                last.raw_value.push_str(&token.raw_value);
                last.span.end = token.span.end;
                last.state = token.state;
            } else {
                words.push(token.clone());
            }
        }

        // unclosed substitution: don't append the partial word
        words
    }

    fn wordbreak_prefix(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut found = false;
        let mut prefix = String::new();

        let last = self.last().unwrap();
        match last.state {
            LexerState::Quoting
            | LexerState::QuotingEscaping
            | LexerState::EscapingQuoted
            | LexerState::QuotingTriple
            | LexerState::QuotingTripleEscaping => {
                // bash handles the last opening quote as a wordbreak when
                // in quoting state: add the value up to the opening quote
                found = true;
                prefix = last.value[..last.wordbreak_index.min(last.value.len())].to_owned();
            }
            _ => {}
        }

        for index in (0..self.len().saturating_sub(1)).rev() {
            let token = &self[index];
            if !token.adjoins(&self[index + 1]) {
                break;
            }

            if token.kind == TokenType::Wordbreak && token.value == "@" {
                if !found {
                    found = true;
                    continue;
                }
                // @ after a boundary: skip if adjacent to another wordbreak,
                // include if between two word tokens (e.g. user@host)
                let previous_is_wordbreak =
                    index > 0 && self[index - 1].kind == TokenType::Wordbreak;
                let next_is_wordbreak =
                    index + 1 < self.len() && self[index + 1].kind == TokenType::Wordbreak;
                if previous_is_wordbreak || next_is_wordbreak {
                    continue;
                }
            }

            if !found && token.kind == TokenType::Wordbreak {
                found = true;
            }

            if found {
                prefix = token.value.clone() + &prefix;
            }
        }
        prefix
    }
}

impl TokenSliceExt for Vec<Token> {
    fn pipelines(&self) -> Vec<Vec<Token>> {
        self.as_slice().pipelines()
    }
    fn current_pipeline(&self) -> Vec<Token> {
        self.as_slice().current_pipeline()
    }
    fn words(&self) -> Vec<Token> {
        self.as_slice().words()
    }
    fn strings(&self) -> Vec<String> {
        self.as_slice().strings()
    }
    fn current_token(&self) -> Option<Token> {
        self.as_slice().current_token()
    }
    fn filter_redirects(&self) -> Vec<Token> {
        self.as_slice().filter_redirects()
    }
    fn words_with_substitutions(&self) -> Vec<Token> {
        self.as_slice().words_with_substitutions()
    }
    fn wordbreak_prefix(&self) -> String {
        self.as_slice().wordbreak_prefix()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_redirects_removes_operators_and_targets() {
        fn word(value: &str, raw: &str, start: usize) -> Token {
            Token {
                kind: TokenType::Word,
                value: value.to_owned(),
                raw_value: raw.to_owned(),
                span: crate::token::Span {
                    start,
                    end: start + raw.chars().count(),
                },
                state: LexerState::InWord,
                ..Default::default()
            }
        }
        let redirect = Token {
            kind: TokenType::Wordbreak,
            value: ">".to_owned(),
            raw_value: ">".to_owned(),
            span: crate::token::Span { start: 5, end: 6 },
            state: crate::token::LexerState::Wordbreak,
            wordbreak_type: WordbreakType::RedirectOutput,
            ..Default::default()
        };
        let tokens = vec![
            word("echo", "echo", 0),
            redirect,
            word("out.txt", "out.txt", 6),
            word("fi", "fi", 14),
        ];
        let filtered = tokens.filter_redirects();
        let values: Vec<&str> = filtered.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, vec!["echo", "fi"]);
    }
}
