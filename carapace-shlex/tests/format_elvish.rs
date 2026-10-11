mod common;

use carapace_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, split,
};
use common::{pipeline_count, split_words};

fn pipe_classification(input: &str, want_lambda: usize, want_real: usize) {
    let tokens = split(input, Format::Elvish).unwrap();
    let mut lambda_pipes = 0;
    let mut real_pipes = 0;
    for tok in &tokens {
        if tok.kind == TokenType::Wordbreak && tok.value == "|" {
            match tok.wordbreak_type {
                WordbreakType::LambdaPipe => lambda_pipes += 1,
                WordbreakType::Pipe => real_pipes += 1,
                _ => {}
            }
        }
    }
    assert_eq!(lambda_pipes, want_lambda, "{input}: lambda pipes");
    assert_eq!(real_pipes, want_real, "{input}: real pipes");
}

#[test]
fn bareword_backslash() {
    // Elvish: \ is a bareword character outside quotes (not an escape)
    let words = split_words(r"echo C:\path", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), r"C:\path".to_owned()]);
}

#[test]
fn double_quote_escape() {
    // Elvish: \ IS an escape inside double quotes
    let tokens = split(r#"echo "hello\nworld""#, Format::Elvish).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.state, LexerState::InWord);
}

#[test]
fn doubled_quote_split() {
    let words = split_words("echo 'it''s a test'", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "it's a test".to_owned()]);
}

#[test]
fn double_quote_value() {
    let words = split_words(r#"echo "say \"hello\"""#, Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), r#"say "hello""#.to_owned()]);
}

#[test]
fn amp_not_list_operator() {
    // & is for map literals in elvish, not a list operator
    assert_eq!(pipeline_count("echo foo & echo bar", Format::Elvish), 1);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Elvish), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Elvish), 2);
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Elvish).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn lambda_pipe() {
    // {| — the | after { is a lambda parameter delimiter, not a pipeline pipe
    pipe_classification("bat | {|", 1, 1);
}

#[test]
fn lambda_pipe_params() {
    // {|a b| — both | are lambda parameter delimiters
    pipe_classification("bat | {|a b|", 2, 1);
}

#[test]
fn lambda_body_pipe() {
    // {|a| cmd1 | cmd2} — first two | are lambda delimiters, third is real
    pipe_classification("{|a| cmd1 | cmd2}", 2, 1);
}

#[test]
fn braced_list_not_lambda() {
    // {a,b} is a braced list, not a lambda — no | to reclassify
    pipe_classification("echo {a,b}", 0, 0);
}

#[test]
fn lambda_no_params() {
    // { body } — lambda with no params (space after {), no | at all
    pipe_classification("var f = { echo hi }", 0, 0);
}

#[test]
fn nested_lambda() {
    // {|a| {|b| echo $a $b }} — nested lambdas
    pipe_classification("{|a| {|b| echo $a $b }}", 4, 0);
}

#[test]
fn lambda_pipe_does_not_split_pipeline() {
    let tokens = split("bat | {|a", Format::Elvish).unwrap();
    let pipelines = tokens.pipelines();
    assert_eq!(pipelines.len(), 2);
    let current = tokens.current_pipeline();
    assert!(!current.is_empty());
    assert!(
        current.iter().any(
            |t| t.kind == TokenType::Wordbreak && t.wordbreak_type == WordbreakType::LambdaPipe
        )
    );
}

#[test]
fn completion_in_lambda_params() {
    let ctx = complete("bat | {|", Format::Elvish);
    assert!(ctx.in_lambda_params);
}

#[test]
fn completion_in_lambda_params_with_arg() {
    let ctx = complete("bat | {|a", Format::Elvish);
    assert!(ctx.in_lambda_params);
}

#[test]
fn completion_after_lambda_params() {
    let ctx = complete("bat | {|a|", Format::Elvish);
    assert!(!ctx.in_lambda_params);
}

#[test]
fn completion_not_in_lambda() {
    let ctx = complete("bat | grep ", Format::Elvish);
    assert!(!ctx.in_lambda_params);
}

#[test]
fn output_capture() {
    // ( and ) are word breaks — output capture delimiters
    let words = split_words("echo (ls)", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "(ls)".to_owned()]);
}

#[test]
fn output_capture_wordbreak_type() {
    let tokens = split("echo (ls)", Format::Elvish).unwrap();
    for tok in &tokens {
        if tok.kind == TokenType::Wordbreak && tok.value == "(" {
            assert_eq!(tok.wordbreak_type, WordbreakType::SubstitutionOpen);
        }
        if tok.kind == TokenType::Wordbreak && tok.value == ")" {
            assert_eq!(tok.wordbreak_type, WordbreakType::SubstitutionClose);
        }
    }
}

#[test]
fn output_capture_does_not_split_pipeline() {
    // ( and ) create a substitution scope — | inside does not split the outer pipeline
    assert_eq!(pipeline_count("echo (ls | grep foo)", Format::Elvish), 1);
}

#[test]
fn list_literal() {
    // [ and ] are word breaks at word start — list literal
    let words = split_words("[a b]", Format::Elvish);
    assert_eq!(words, vec!["[a".to_owned(), "b]".to_owned()]);
}

#[test]
fn list_literal_wordbreak_type() {
    let tokens = split("[a b]", Format::Elvish).unwrap();
    for tok in &tokens {
        if tok.kind == TokenType::Wordbreak && (tok.value == "[" || tok.value == "]") {
            assert_eq!(tok.wordbreak_type, WordbreakType::Bracket);
        }
    }
}

#[test]
fn indexing() {
    // [ after a word char is indexing — Words() should merge $var[0] back together
    let words = split_words("echo $var[0]", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "$var[0]".to_owned()]);
}

#[test]
fn indexing_range() {
    let words = split_words("echo $var[0:3]", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "$var[0:3]".to_owned()]);
}

#[test]
fn bracket_does_not_split_pipeline() {
    assert_eq!(pipeline_count("echo [a b]", Format::Elvish), 1);
}

#[test]
fn nested_output_capture() {
    let words = split_words("echo (echo (ls))", Format::Elvish);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "(echo".to_owned(), "(ls))".to_owned()]
    );
}

#[test]
fn output_capture_completion() {
    // Cursor inside output capture: the ( opens a substitution scope
    let ctx = complete("echo (ls", Format::Elvish);
    assert_eq!(ctx.words, vec!["ls".to_owned()]);
}

#[test]
fn closed_lambda_then_pipe() {
    pipe_classification("{|a| echo $a} | grep foo", 2, 1);
}

#[test]
fn lambda_no_params_body_pipe() {
    pipe_classification("{ echo foo | grep bar }", 0, 1);
}

#[test]
fn multiple_lambdas_with_pipe_between() {
    pipe_classification("{|a| echo $a} | {|b| echo $b}", 4, 1);
}

#[test]
fn nested_lambda_then_pipe_in_body() {
    pipe_classification("{|a| {|b| echo $b} | grep foo}", 4, 1);
}

#[test]
fn closed_lambda_completion() {
    let ctx = complete("{|a| echo $a} | grep foo", Format::Elvish);
    assert!(!ctx.in_lambda_params);
    assert_eq!(ctx.current_word, "foo");
}

#[test]
fn caret_line_continuation() {
    // elvish: ^<newline> is whitespace (word break) — like a space.
    let words = split_words("echo foo^\nbar", Format::Elvish);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "foo".to_owned(), "bar".to_owned()]
    );
}

#[test]
fn caret_line_continuation_crlf() {
    let words = split_words("echo foo^\r\nbar", Format::Elvish);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "foo".to_owned(), "bar".to_owned()]
    );
}

#[test]
fn caret_alone_is_bareword() {
    let words = split_words("echo foo^bar", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "foo^bar".to_owned()]);
}

#[test]
fn caret_newline_at_word_start() {
    let words = split_words("echo ^\nbar", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "bar".to_owned()]);
}
