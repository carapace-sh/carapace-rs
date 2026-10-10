//! Command substitution scope tracking (port of `substitution.go`).

use super::token::Token;
use super::wordbreak::WordbreakType;

/// The token-slice index of the opener of the innermost unclosed command
/// substitution scope, or None at top level. Arithmetic (`$((...))`) and
/// backtick scopes are not command scopes.
pub(crate) fn innermost_unclosed_command_scope(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0;
    let mut open_stack: Vec<usize> = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        match token.wordbreak_type {
            WordbreakType::SubstitutionOpen => {
                if is_arithmetic_opener(token) {
                    continue;
                }
                depth += 1;
                open_stack.push(index);
            }
            WordbreakType::SubstitutionClose => {
                if is_arithmetic_closer(token) {
                    continue;
                }
                if depth > 0 {
                    depth -= 1;
                    open_stack.pop();
                }
            }
            _ => {}
        }
    }

    if depth > 0 {
        return open_stack.last().copied();
    }
    None
}

fn is_arithmetic_opener(token: &Token) -> bool {
    token.raw_value.len() >= 2 && token.raw_value.as_bytes()[token.raw_value.len() - 2] == b'('
}

fn is_arithmetic_closer(token: &Token) -> bool {
    token.raw_value.len() >= 2
        && token.raw_value.as_bytes()[0] == b')'
        && token.raw_value.as_bytes()[1] == b')'
}

/// The number of unclosed command substitution scopes in the token slice.
pub(crate) fn count_unclosed_command_scopes(tokens: &[Token]) -> usize {
    let mut depth: usize = 0;
    for token in tokens {
        match token.wordbreak_type {
            WordbreakType::SubstitutionOpen => {
                if is_arithmetic_opener(token) {
                    continue;
                }
                depth += 1;
            }
            WordbreakType::SubstitutionClose => {
                if is_arithmetic_closer(token) {
                    continue;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    depth
}
