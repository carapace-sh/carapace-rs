//! Port of Go `substitution_test.go` (the edge-case tests targeting the
//! internal scope functions live inline in `src/substitution.rs`).

mod common;

use carapace_shlex::{Format, TokenSliceExt, TokenType, WordbreakType, complete, split};

fn substitution_words(s: &str, format: Format) -> Vec<String> {
    split(s, format)
        .unwrap()
        .words_with_substitutions()
        .strings()
}

#[test]
fn bash_command_substitution() {
    let words = substitution_words("echo $(echo test)", Format::Bash);
    assert_eq!(words, ["echo", "$(echo test)"]);
}

#[test]
fn bash_arithmetic() {
    let words = substitution_words("echo $((1+2))", Format::Bash);
    assert_eq!(words, ["echo", "$((1+2))"]);
}

#[test]
fn bash_process_substitution() {
    let words = substitution_words("echo <(grep foo)", Format::Bash);
    assert_eq!(words, ["echo", "<(grep foo)"]);
}

#[test]
fn pipeline_does_not_split_inside_substitution() {
    let pipelines = split("echo foo $(bar | grep x) baz", Format::Bash)
        .unwrap()
        .pipelines();
    assert_eq!(pipelines.len(), 1);
    let words = pipelines[0].words_with_substitutions().strings();
    assert_eq!(words, ["echo", "foo", "$(bar | grep x)", "baz"]);
}

#[test]
fn nested_command_substitution() {
    let words = substitution_words("echo $(echo $(echo test))", Format::Bash);
    assert_eq!(words, ["echo", "$(echo $(echo test))"]);
}

#[test]
fn completion_inside_substitution() {
    let ctx = complete("echo $(git ch", Format::Bash);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.current_word, "ch");
}

#[test]
fn completion_inside_nested_substitution() {
    let ctx = complete("echo $(echo $(git ch", Format::Bash);
    assert_eq!(ctx.words, ["git", "ch"]);
}

#[test]
fn completion_inside_arithmetic() {
    let ctx = complete("echo $((1+2", Format::Bash);
    assert_eq!(ctx.words, ["echo"]);
}

#[test]
fn completion_closed_substitution() {
    let ctx = complete("echo $(echo test)", Format::Bash);
    assert_eq!(ctx.words, ["echo", "$(echo test)"]);
}

#[test]
fn completion_inside_substitution_with_inner_pipe() {
    let ctx = complete("echo foo $(bar | grep x", Format::Bash);
    assert_eq!(ctx.words, ["grep", "x"]);
    assert_eq!(ctx.current_word, "x");
}

#[test]
fn bash_backtick_substitution() {
    let words = substitution_words("echo `echo test`", Format::Bash);
    assert_eq!(words, ["echo", "`echo", "test`"]);
}

#[test]
fn elvish_output_capture() {
    let words = substitution_words("echo (echo test)", Format::Elvish);
    assert_eq!(words, ["echo", "(echo test)"]);
}

#[test]
fn elvish_output_capture_completion() {
    let ctx = complete("echo (ls", Format::Elvish);
    assert_eq!(ctx.words, ["ls"]);
}

#[test]
fn token_reclassification() {
    let tokens = split("echo $(test)", Format::Bash).unwrap();
    for token in &tokens {
        if token.kind == TokenType::Wordbreak {
            match token.value.as_str() {
                "$(" => assert_eq!(
                    token.wordbreak_type,
                    WordbreakType::SubstitutionOpen,
                    "token {:?}",
                    token.value
                ),
                ")" => assert_eq!(
                    token.wordbreak_type,
                    WordbreakType::SubstitutionClose,
                    "token {:?}",
                    token.value
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn completion_substitution_depth() {
    let ctx = complete("echo $(git ch", Format::Bash);
    assert_eq!(ctx.substitution_depth, 1);

    let ctx = complete("echo $(echo $(git ch", Format::Bash);
    assert_eq!(ctx.substitution_depth, 2);

    let ctx = complete("echo test", Format::Bash);
    assert_eq!(ctx.substitution_depth, 0);

    let ctx = complete("echo $(echo test)", Format::Bash);
    assert_eq!(ctx.substitution_depth, 0);
}

#[test]
fn completion_redirect_inside_substitution() {
    let ctx = complete("echo $(cat >", Format::Bash);
    assert_eq!(ctx.substitution_depth, 1);
    assert!(ctx.is_redirect);
}

#[test]
fn zsh_command_substitution() {
    let ctx = complete("echo $(git ch", Format::Zsh);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.substitution_depth, 1);
}

#[test]
fn fish_command_substitution() {
    let ctx = complete("echo (git ch", Format::Fish);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.substitution_depth, 1);
}

#[test]
fn nushell_subexpression() {
    let ctx = complete("echo (git ch", Format::Nushell);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.substitution_depth, 1);
}

#[test]
fn powershell_subexpression() {
    let ctx = complete("echo $(git ch", Format::Powershell);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.substitution_depth, 1);
}

#[test]
fn process_substitution() {
    let ctx = complete("echo <(git ch", Format::Bash);
    assert_eq!(ctx.words, ["git", "ch"]);
    assert_eq!(ctx.substitution_depth, 1);
}

#[test]
fn closed_substitution_does_not_affect_prefix() {
    let ctx = complete("echo $(echo test) foo", Format::Bash);
    assert_eq!(ctx.substitution_depth, 0);
    assert_eq!(ctx.words, ["echo", "$(echo test)", "foo"]);
}
