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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split;
    use crate::tokenslice::TokenSliceExt;
    use crate::{Format, complete};

    fn scope(s: &str) -> Option<usize> {
        innermost_unclosed_command_scope(&split(s, Format::Bash).unwrap())
    }

    #[test]
    fn nested_scope_last_open_tracking() {
        // $(( opens (arithmetic, skipped), then ( opens (depth=1, command
        // scope), then ) closes (depth=0). But the $(( arithmetic is still
        // open. Since arithmetic is not a command scope, the innermost
        // unclosed command scope is None.
        assert_eq!(scope("echo $(( (foo) bar"), None);
    }

    #[test]
    fn nested_command_sub_last_open_tracking() {
        // $( (test) foo: $( opens (depth=1), ( opens (depth=2), ) closes
        // (depth=1). The innermost unclosed command scope is the $(, not
        // the already-closed (.
        let tokens = split("echo $( (test) foo", Format::Bash).unwrap();
        let index = innermost_unclosed_command_scope(&tokens).expect("unclosed $()");
        assert_eq!(tokens[index].value, "$(");
    }

    #[test]
    fn stray_close_does_not_mask_open() {
        // ) $(git ch: the stray ) must not make depth negative, so the
        // unclosed $( is still detected.
        let tokens = split("echo ) $(git ch", Format::Bash).unwrap();
        assert!(innermost_unclosed_command_scope(&tokens).is_some());
        assert_eq!(count_unclosed_command_scopes(&tokens), 1);
    }

    #[test]
    fn arithmetic_inside_command_sub() {
        // $(echo $((1+2)): $( opens (depth=1, arith=0), $(( opens
        // (depth=2, arith=1), )) closes arithmetic (depth=1, arith=0).
        // The $( is still unclosed.
        let tokens = split("echo $(echo $((1+2))", Format::Bash).unwrap();
        assert!(innermost_unclosed_command_scope(&tokens).is_some());
        assert_eq!(count_unclosed_command_scopes(&tokens), 1);
    }

    #[test]
    fn arithmetic_closed() {
        let tokens = split("echo $((1+2))", Format::Bash).unwrap();
        assert_eq!(innermost_unclosed_command_scope(&tokens), None);
        assert_eq!(count_unclosed_command_scopes(&tokens), 0);
    }

    #[test]
    fn nested_arithmetic() {
        assert_eq!(scope("echo $(( $((1+2)) + 3 ))"), None);
    }

    #[test]
    fn empty_closed() {
        let tokens = split("echo $()", Format::Bash).unwrap();
        let words = tokens.words_with_substitutions().strings();
        assert_eq!(words, ["echo", "$()"]);
        assert_eq!(innermost_unclosed_command_scope(&tokens), None);
    }

    #[test]
    fn open_at_eof() {
        let tokens = split("echo $(", Format::Bash).unwrap();
        assert!(innermost_unclosed_command_scope(&tokens).is_some());
        assert_eq!(complete("echo $(", Format::Bash).substitution_depth, 1);
    }

    #[test]
    fn stray_close_pipeline() {
        // ) echo | test: the stray ) must not mask the pipe in pipelines().
        let pipelines = split("echo ) | grep foo", Format::Bash)
            .unwrap()
            .pipelines();
        assert_eq!(pipelines.len(), 2);
    }

    #[test]
    fn command_sub_in_arithmetic_at_eof() {
        // $(( $(echo test): arithmetic open, command sub closed. The
        // command sub is closed and arithmetic is not a command scope.
        assert_eq!(scope("echo $(( $(echo test)"), None);
    }

    #[test]
    fn nested_command_sub_with_arithmetic() {
        // $( $((1+2)) ): command sub containing arithmetic, both closed.
        let tokens = split("echo $( $((1+2)) )", Format::Bash).unwrap();
        assert_eq!(innermost_unclosed_command_scope(&tokens), None);
        let words = tokens.words_with_substitutions().strings();
        assert_eq!(words, ["echo", "$( $((1+2)) )"]);
    }

    #[test]
    fn append_process_substitution() {
        // >>(cmd): the >> redirect operator followed by ( is a process
        // substitution opener.
        let tokens = split("echo >>(grep foo)", Format::Bash).unwrap();
        assert!(tokens.iter().any(
            |token| token.wordbreak_type == WordbreakType::SubstitutionOpen && token.value == ">>("
        ));
    }
}
