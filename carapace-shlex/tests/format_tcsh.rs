mod common;

use carapace_shlex::{Format, LexerState, TokenSliceExt, TokenType, WordbreakType, split};
use common::{pipeline_count, split_words};

fn words_of(input: &str) -> Vec<String> {
    split_words(input, Format::Tcsh)
}

fn wordbreak_of(input: &str, raw: &str) -> (TokenType, WordbreakType) {
    let tokens = split(input, Format::Tcsh).unwrap();
    let found = tokens
        .iter()
        .rfind(|t| t.raw_value == raw)
        .unwrap_or_else(|| panic!("{input}: no {raw} token"));
    (found.kind, found.wordbreak_type)
}

#[test]
fn basic_pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Tcsh), 2);
}

#[test]
fn backslash_quote() {
    // $'...' — the backslash is a bareword char, quote literal
    assert_eq!(words_of("echo $'hello'"), vec!["echo", "$hello"]);
}

#[test]
fn single_quote_literal() {
    assert_eq!(words_of("echo '$HOME'"), vec!["echo", "$HOME"]);
}

#[test]
fn backtick_literal_in_single_quotes() {
    assert_eq!(words_of("echo '`cmd`'"), vec!["echo", "`cmd`"]);
}

#[test]
fn escaped_double_quote_outside() {
    assert_eq!(words_of(r#"echo \"hello\""#), vec!["echo", "\"hello\""]);
}

#[test]
fn double_and() {
    assert_eq!(pipeline_count("echo foo && echo bar", Format::Tcsh), 2);
}

#[test]
fn double_or() {
    assert_eq!(pipeline_count("echo foo || echo bar", Format::Tcsh), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Tcsh), 2);
}

#[test]
fn background() {
    assert_eq!(
        wordbreak_of("echo foo &", "&"),
        (TokenType::Wordbreak, WordbreakType::ListAsync)
    );
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Tcsh).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn bang_not_wordbreak() {
    assert_eq!(words_of("echo !$"), vec!["echo", "!$"]);
}

#[test]
fn bang_in_word() {
    assert_eq!(words_of("echo foo!bar"), vec!["echo", "foo!bar"]);
}

#[test]
fn redirect_both_stdout_stderr() {
    // tcsh uses >& for combined redirect
    assert_eq!(
        wordbreak_of("echo foo >& /tmp/bar", ">&"),
        (TokenType::Wordbreak, WordbreakType::RedirectOutputBoth)
    );
}

#[test]
fn pipe_with_stderr() {
    assert_eq!(
        wordbreak_of("echo foo |& grep bar", "|&"),
        (TokenType::Wordbreak, WordbreakType::PipeWithStderr)
    );
    assert_eq!(pipeline_count("echo foo |& grep bar", Format::Tcsh), 2);
}

#[test]
fn here_doc() {
    assert_eq!(
        wordbreak_of("cat << EOF", "<<"),
        (TokenType::Wordbreak, WordbreakType::RedirectHereDoc)
    );
}

#[test]
fn input_duplicate() {
    assert_eq!(
        wordbreak_of("cmd <& 0", "<&"),
        (TokenType::Wordbreak, WordbreakType::RedirectInputDuplicate)
    );
}

#[test]
fn equals_not_wordbreak() {
    assert_eq!(words_of("set foo=bar"), vec!["set", "foo=bar"]);
}

#[test]
fn at_not_wordbreak() {
    assert_eq!(words_of("echo @foo"), vec!["echo", "@foo"]);
}

#[test]
fn greater_bang_is_redirect_plus_word() {
    // >!bar: > is the redirect, !bar is a separate word
    let tokens = split("echo foo >!bar", Format::Tcsh).unwrap();
    let mut redirect = None;
    let mut word = None;
    for tok in &tokens {
        if tok.raw_value == ">" {
            redirect = Some(tok);
        }
        if tok.kind == TokenType::Word && tok.raw_value == "!bar" {
            word = Some(tok);
        }
    }
    let redirect = redirect.expect("no > redirect");
    assert_eq!(redirect.kind, TokenType::Wordbreak);
    assert_eq!(redirect.wordbreak_type, WordbreakType::RedirectOutput);
    assert!(word.is_some(), "expected !bar as WORD_TOKEN");
}

#[test]
fn no_bash_pipe_force_operator() {
    // tcsh has no >| noclobber override
    let tokens = split("echo foo >| /tmp/bar", Format::Tcsh).unwrap();
    assert!(!tokens.iter().any(|t| t.kind == TokenType::Wordbreak
        && t.wordbreak_type == WordbreakType::RedirectOutputForce));
}

#[test]
fn no_here_string_operator() {
    let tokens = split("cmd <<< foo", Format::Tcsh).unwrap();
    assert!(!tokens.iter().any(|t| t.kind == TokenType::Wordbreak
        && t.wordbreak_type == WordbreakType::RedirectInputString));
}

#[test]
fn no_bash_both_redirect() {
    let tokens = split("cmd &> /tmp/out", Format::Tcsh).unwrap();
    assert!(!tokens.iter().any(|t| t.kind == TokenType::Wordbreak
        && t.wordbreak_type == WordbreakType::RedirectOutputBoth
        && t.raw_value == "&>"));
}

#[test]
fn line_continuation_outside_quotes() {
    let words = split_words("echo foo\\\nbar", Format::Tcsh);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_in_double_quotes() {
    let words = split_words("echo \"line1\\\nline2\"", Format::Tcsh);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}
