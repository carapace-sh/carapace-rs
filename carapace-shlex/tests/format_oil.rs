mod common;

use carapace_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, split,
};
use common::{pipeline_count, split_words};

fn words_of(input: &str) -> Vec<String> {
    split_words(input, Format::Oil)
}

fn substitution_words_of(input: &str) -> Vec<String> {
    split(input, Format::Oil)
        .unwrap()
        .words_with_substitutions()
        .strings()
}

#[test]
fn basic() {
    assert_eq!(
        words_of(r#"echo "hello" world"#),
        vec!["echo", "hello", "world"]
    );
}

#[test]
fn single_quote() {
    assert_eq!(words_of("echo 'hello world'"), vec!["echo", "hello world"]);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Oil), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Oil), 2);
}

#[test]
fn double_and() {
    assert_eq!(pipeline_count("echo foo && echo bar", Format::Oil), 2);
}

#[test]
fn double_or() {
    assert_eq!(pipeline_count("echo foo || echo bar", Format::Oil), 2);
}

#[test]
fn background() {
    assert_eq!(pipeline_count("echo foo & echo bar", Format::Oil), 2);
}

#[test]
fn pipe_with_stderr() {
    assert_eq!(pipeline_count("echo foo |& grep bar", Format::Oil), 2);
}

#[test]
fn redirect_output() {
    assert!(split("echo foo > file.txt", Format::Oil).is_some());
    let ctx = complete("echo foo > file.txt", Format::Oil);
    assert!(ctx.is_redirect);
}

#[test]
fn redirect_append() {
    let ctx = complete("echo foo >> file.txt", Format::Oil);
    assert!(ctx.is_redirect);
}

#[test]
fn here_doc() {
    let tokens = split("cat << EOF", Format::Oil).unwrap();
    assert!(
        tokens.iter().any(|t| t.kind == TokenType::Wordbreak
            && t.wordbreak_type == WordbreakType::RedirectHereDoc)
    );
}

#[test]
fn here_string() {
    let tokens = split(r#"cat <<< "string""#, Format::Oil).unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenType::Wordbreak
        && t.wordbreak_type == WordbreakType::RedirectInputString));
}

#[test]
fn command_substitution() {
    assert_eq!(
        substitution_words_of("echo $(date)"),
        vec!["echo".to_owned(), "$(date)".to_owned()]
    );
}

#[test]
fn arithmetic_expansion() {
    assert_eq!(
        substitution_words_of("echo $((1+2))"),
        vec!["echo".to_owned(), "$((1+2))".to_owned()]
    );
}

#[test]
fn process_substitution() {
    assert_eq!(
        substitution_words_of("echo <(grep foo)"),
        vec!["echo".to_owned(), "<(grep foo)".to_owned()]
    );
}

#[test]
fn escaped_space() {
    assert_eq!(words_of(r"echo foo\ bar"), vec!["echo", "foo bar"]);
}

#[test]
fn double_quote_backslash() {
    assert_eq!(words_of(r#"echo "a\$b""#), vec!["echo", "a$b"]);
}

#[test]
fn line_continuation() {
    // backslash-newline = line continuation. The space before the
    // backslash is kept, so words are "echo", "foo" and "bar".
    let words = split_words("echo foo \\\nbar", Format::Oil);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "foo".to_owned(), "bar".to_owned()]
    );
}

#[test]
fn empty_quotes() {
    assert_eq!(words_of(r#"echo "" ''"#), vec!["echo", "", ""]);
}

#[test]
fn adjacent_quotes() {
    assert_eq!(words_of(r#"echo a"b"c'd'"#), vec!["echo", "abcd"]);
}

#[test]
fn comment() {
    assert_eq!(words_of("echo foo # comment"), vec!["echo", "foo"]);
}

#[test]
fn escape_at_eof() {
    let tokens = split(r"echo foo\", Format::Oil).unwrap();
    let words = tokens.words();
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].state, LexerState::Escaping);
}
