//! The completion API: state at the cursor (port of `completion.go`).

use super::format::FormatImpl;
use super::substitution::{count_unclosed_command_scopes, innermost_unclosed_command_scope};
use super::token::{LexerState, Span, Token};
use super::tokenslice::{TokenSliceExt, TokenSliceInternals};
use super::variable::{Variable, variable_ref};
use super::{Format, format::format_impl_for};

/// The completion state at the end of the input: the primary API for
/// completion callers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompletionContext {
    /// The dequoted word values in the current pipeline (redirects
    /// filtered). When the cursor is inside a substitution scope
    /// (e.g. `$(...`), these are the inner command's words.
    pub words: Vec<String>,

    /// The word at the cursor position (dequoted value).
    pub current_word: String,

    /// The raw source of the current word (including quotes).
    pub raw_current_word: String,

    /// The wordbreak prefix up to the cursor.
    pub prefix: String,

    /// The lexer state of the current word.
    pub quoting_state: LexerState,

    /// The cursor completes a redirect target (e.g. after `>`, `>>`, `<`).
    pub is_redirect: bool,

    /// The cursor is inside a lambda parameter list (elvish `{|params|`).
    pub in_lambda_params: bool,

    /// A variable reference ending the current word, detected lexically.
    pub variable: Option<Variable>,

    /// The rune span of the current word in the input.
    pub span: Span,

    /// The raw token slice of the whole input.
    pub tokens: Vec<Token>,

    /// The number of unclosed substitution scopes at the cursor.
    pub substitution_depth: usize,

    /// The format the context was built with.
    pub(crate) format: Format,
}

impl CompletionContext {
    /// Quote `value` for insertion at the cursor position described by the
    /// context: an open quote is closed with the format's own escape rules
    /// and a bareword is quoted as a complete word, so the result can
    /// replace the whole raw current word.
    pub fn quote(&self, value: &str) -> String {
        match format_impl_for(self.format) {
            Some(impl_) => super::quote::quote_insertion(impl_, self.quoting_state, value),
            None => value.to_owned(),
        }
    }
}

impl CompletionContext {
    /// The quote character if the cursor sits inside an open quote.
    pub fn quoting_open_quote(&self) -> Option<char> {
        match self.quoting_state {
            LexerState::Quoting | LexerState::QuotingTriple => Some('\''),
            LexerState::QuotingEscaping | LexerState::QuotingTripleEscaping => Some('"'),
            _ => None,
        }
    }

    /// Whether the cursor sits in a complete word (no open quote, no
    /// dangling escape).
    pub fn tail_complete(&self) -> bool {
        !matches!(
            self.quoting_state,
            LexerState::Quoting
                | LexerState::QuotingEscaping
                | LexerState::QuotingTriple
                | LexerState::QuotingTripleEscaping
                | LexerState::Escaping
        )
    }
}

/// Parse `s` and return the completion state at the end of the string.
///
/// An empty or whitespace-terminated line yields an empty current word:
/// the cursor starts a new word. When the cursor is inside an unclosed
/// substitution scope (e.g. inside `$(...`), the context describes the
/// innermost substitution's command.
pub fn complete(s: &str, format: Format) -> CompletionContext {
    let Some(impl_) = format_impl_for(format) else {
        return CompletionContext {
            quoting_state: LexerState::Start,
            ..Default::default()
        };
    };
    let tokens = match super::split(s, format) {
        Some(tokens) if !tokens.is_empty() => tokens,
        _ => {
            return CompletionContext {
                quoting_state: LexerState::Start,
                ..Default::default()
            };
        }
    };

    // if the cursor is inside an unclosed command substitution, build the
    // context from the inner tokens
    if let Some(scope) = innermost_unclosed_command_scope(&tokens) {
        let mut context = build_completion_context(&tokens[scope + 1..], impl_);
        context.tokens = tokens;
        context.substitution_depth = count_unclosed_command_scopes(&context.tokens);
        context.format = format;
        return context;
    }

    let mut context = build_completion_context(&tokens, impl_);
    context.tokens = tokens;
    context.format = format;
    context
}

/// Derive the completion context fields from a token slice.
fn build_completion_context(
    tokens: &[Token],
    format: &'static dyn FormatImpl,
) -> CompletionContext {
    let pipeline = tokens.current_pipeline();
    let filtered = pipeline.filter_redirects();
    let words = filtered.words_with_substitutions();

    let mut context = CompletionContext {
        words: words.iter().map(|t| t.value.clone()).collect(),
        ..Default::default()
    };

    if pipeline.len() >= 2 {
        let previous = &pipeline[pipeline.len() - 2];
        if previous.wordbreak_type.is_redirect() {
            context.is_redirect = true;
        }
    }

    let current = if context.is_redirect {
        pipeline.last()
    } else {
        words.last()
    };
    if let Some(current) = current {
        context.current_word = current.value.clone();
        context.raw_current_word = current.raw_value.clone();
        context.quoting_state = current.state;
        context.span = current.span;

        context.variable = variable_ref(current, format);
    }

    if let (Some(variable), Some(current)) = (&mut context.variable, current) {
        variable.replacement = context.raw_current_word.clone();
        // naive word splitter: bash's COMP_WORDS interface replaces only
        // the raw text after the last delimiter
        if format.naive_split_word_support() {
            variable.replacement = format.naive_split_word(&context.raw_current_word);
        }
        variable.span = Span {
            start: current
                .span
                .end
                .saturating_sub(variable.replacement.chars().count()),
            end: current.span.end,
        };
    }

    context.prefix = pipeline.wordbreak_prefix();

    let lambda_pipes = pipeline
        .iter()
        .filter(|t| t.wordbreak_type == super::wordbreak::WordbreakType::LambdaPipe)
        .count();
    context.in_lambda_params = lambda_pipes % 2 == 1;

    context
}
