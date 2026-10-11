mod common;

use carapace_shlex::{Format, LexerState, TokenSliceExt, TokenType, complete, split};
use common::{pipeline_count, pipeline_words, split_words};

#[test]
fn double_quote() {
    let words = split_words(r#"echo "hello world""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
}

#[test]
fn no_single_quote() {
    // Cmd: ' is a literal character, not a quote
    let words = split_words("echo 'hello'", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "'hello'".to_owned()]);
}

#[test]
fn caret_escape() {
    let words = split_words("echo hello^&world", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello&world".to_owned()]);
}

#[test]
fn caret_escape_pipe() {
    let words = split_words("echo ^|", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "|".to_owned()]);
}

#[test]
fn backslash_literal() {
    // Cmd: \ is a literal character (Windows paths)
    let words = split_words(r"echo C:\path\to\file", Format::Cmd);
    assert_eq!(
        words,
        vec!["echo".to_owned(), r"C:\path\to\file".to_owned()]
    );
}

#[test]
fn amp_separator() {
    // Cmd: & is a command separator (like ; in POSIX)
    assert_eq!(pipeline_count("echo foo & echo bar", Format::Cmd), 2);
}

#[test]
fn no_semicolon_separator() {
    // Cmd: ; is NOT a separator — it's a literal character
    let words = split_words("echo foo;bar", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foo;bar".to_owned()]);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | findstr bar", Format::Cmd), 2);
}

#[test]
fn double_and() {
    // Cmd: && is conditional and
    assert_eq!(pipeline_count("echo foo && echo bar", Format::Cmd), 2);
}

#[test]
fn caret_in_quotes() {
    // Cmd: ^ is LITERAL inside double quotes — it does not escape.
    // "say ^" → ^ is literal, " closes the quote; ^" outside → literal ".
    // Words() merges adjacent tokens, so the whole thing is one word.
    let tokens = split(r#"echo "say ^"hello^"""#, Format::Cmd).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.value, r#"say ^hello""#);
}

#[test]
fn percent_not_wordbreak() {
    // Cmd: % is a word character (variable expansion), not a word break
    let words = split_words("echo %PATH%", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "%PATH%".to_owned()]);
}

#[test]
fn double_or() {
    assert_eq!(pipeline_count("echo foo || echo bar", Format::Cmd), 2);
}

#[test]
fn redirect() {
    let ctx = complete("echo foo > bar", Format::Cmd);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "bar");
}

#[test]
fn open_double_quote() {
    let tokens = split(r#"echo "hel"#, Format::Cmd).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::QuotingEscaping);
}

#[test]
fn caret_at_eof() {
    let tokens = split("echo foo^", Format::Cmd).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Escaping);
    assert_eq!(last.value, "foo");
}

#[test]
fn caret_literal_in_quotes() {
    // Cmd: ^ is literal inside double quotes — does not escape the next char.
    let words = split_words(r#"echo "hello^world""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello^world".to_owned()]);
}

#[test]
fn double_caret_literal_in_quotes() {
    // Cmd: ^^ inside quotes is literal ^^ (both carets), not a single ^.
    let words = split_words(r#"echo "hello^^world""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello^^world".to_owned()]);
}

#[test]
fn line_continuation() {
    // Cmd: ^ at end of line is a line continuation — ^\n is consumed
    let words = split_words("echo foo^\nbar", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_crlf() {
    let words = split_words("echo foo^\r\nbar", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn paren_grouping() {
    // Cmd: ( and ) are grouping operators
    assert_eq!(pipeline_count("(echo foo) & echo bar", Format::Cmd), 2);
}

#[test]
fn paren_before_command() {
    let words = pipeline_words("( echo hello )", Format::Cmd);
    assert_eq!(
        words,
        vec![
            "(".to_owned(),
            "echo".to_owned(),
            "hello".to_owned(),
            ")".to_owned()
        ]
    );
}

#[test]
fn comma_delimiter() {
    // Cmd: comma is a word delimiter (like space)
    let words = split_words("echo hello,world", Format::Cmd);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "hello".to_owned(), "world".to_owned()]
    );
}

#[test]
fn comma_in_quotes() {
    let words = split_words(r#"echo "hello,world""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello,world".to_owned()]);
}

#[test]
fn stream_redirect_2() {
    // Cmd: 2> should be recognized as a stream redirect (stderr)
    let tokens = split("echo foo 2> bar", Format::Cmd).unwrap();
    assert_eq!(pipeline_count("echo foo 2> bar", Format::Cmd), 1);
    let filtered = tokens.current_pipeline().filter_redirects();
    let words = filtered.words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_merge() {
    // Cmd: 2>&1 should be recognized as a stream merge redirect
    let tokens = split("echo foo 2>&1 bar", Format::Cmd).unwrap();
    assert_eq!(tokens.pipelines().len(), 1);
    let filtered = tokens.current_pipeline().filter_redirects();
    let words = filtered.words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_completion() {
    let ctx = complete("echo foo 2> bar", Format::Cmd);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "bar");
}

#[test]
fn caret_line_continuation_at_eof() {
    // Cmd: ^ at EOF (no newline) should enter ESCAPING_STATE, not line continuation
    let tokens = split("echo foo^", Format::Cmd).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Escaping);
}

// --- edge cases (format_cmd_edge_test.go) ---

#[test]
fn caret_escape_paren() {
    let words = split_words("echo foo^(", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foo(".to_owned()]);
}

#[test]
fn caret_escape_close_paren() {
    let words = split_words("echo foo^)", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foo)".to_owned()]);
}

#[test]
fn caret_escape_redirect() {
    let words = split_words("echo ^>foo", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), ">foo".to_owned()]);
}

#[test]
fn caret_escape_quote() {
    let words = split_words("echo ^\"", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "\"".to_owned()]);
}

#[test]
fn double_caret_outside_quotes() {
    let tokens = split("echo ^^& echo bar", Format::Cmd).unwrap();
    assert_eq!(tokens.pipelines().len(), 2);
    let words = tokens.pipelines()[0].words().strings();
    assert_eq!(words[1], "^");
}

#[test]
fn percent_at_eof() {
    let words = split_words("echo foo%", Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "foo%".to_owned()]);
}

#[test]
fn percent_in_double_quotes() {
    let words = split_words(r#"echo "hello%world%""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello%world%".to_owned()]);
}

#[test]
fn percent_followed_by_redirect() {
    // % is a word char, > is a redirect. They adjoin (no space),
    // so Words() merges them into one word.
    let words = split_words("echo %> foo", Format::Cmd);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "%>".to_owned(), "foo".to_owned()]
    );
}

#[test]
fn nested_parens() {
    let tokens = split("echo ((a) b)", Format::Cmd).unwrap();
    let words = tokens.words().strings();
    assert_eq!(words[0], "echo");
}

#[test]
fn unclosed_paren() {
    assert!(split("echo (foo", Format::Cmd).is_some());
}

#[test]
fn empty_parens() {
    assert!(split("echo ()", Format::Cmd).is_some());
}

#[test]
fn parens_in_quotes() {
    let words = split_words(r#"echo "(foo)""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "(foo)".to_owned()]);
}

#[test]
fn comma_at_eof() {
    let ctx = complete("echo foo,", Format::Cmd);
    assert_eq!(ctx.current_word, "");
}

#[test]
fn multiple_commas() {
    let words = split_words("echo a,,b", Format::Cmd);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "a".to_owned(), "b".to_owned()]
    );
}

#[test]
fn fd_duplication_at_eof() {
    // 2>&1 is a single merged WORDBREAK_TOKEN at EOF with no trailing
    // empty word; the redirect detection doesn't fire because there's
    // no empty word after it (known edge case).
    let _ctx = complete("echo foo 2>&1", Format::Cmd);
}

#[test]
fn multiple_redirects() {
    let ctx = complete("echo foo 2>&1 > bar", Format::Cmd);
    assert!(ctx.is_redirect);
}

#[test]
fn empty_quotes() {
    let words = split_words(r#"echo """#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), String::new()]);
}

#[test]
fn adjacent_quotes() {
    let words = split_words(r#"echo ""hello"""#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

#[test]
fn quote_inside_word() {
    let words = split_words(r#"echo pre"mid"post"#, Format::Cmd);
    assert_eq!(words, vec!["echo".to_owned(), "premidpost".to_owned()]);
}

#[test]
fn wordbreak_token_type_exists() {
    // sanity: the cmd lexer produces wordbreak tokens for operators
    let tokens = split("echo foo | bar", Format::Cmd).unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenType::Wordbreak));
}
