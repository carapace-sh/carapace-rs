mod common;

use carapace_rs_shlex::{
    Format, LexerState, TokenSliceExt, TokenSliceInternals, TokenType, complete, quote_word, split,
};
use common::{pipeline_count, split_words};

#[test]
fn single_quote_escape() {
    // Fish: \' inside single quotes → literal '
    let tokens = split("echo 'it\\'s'", Format::Fish).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, "it's");
    assert_eq!(last.state, LexerState::InWord);
}

#[test]
fn single_quote_backslash_escape() {
    // Fish: \\ inside single quotes → literal \
    let tokens = split(r"echo 'C:\\path'", Format::Fish).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, r"C:\path");
}

#[test]
fn keyword_operators() {
    // Fish: "and" and "or" are keyword operators that split pipelines
    assert_eq!(pipeline_count("echo foo and echo bar", Format::Fish), 2);
}

#[test]
fn keyword_operator_and() {
    let tokens = split("echo foo and", Format::Fish).unwrap();
    let last = tokens.last().unwrap();
    assert_eq!(last.kind, TokenType::Wordbreak);
    assert_eq!(
        last.wordbreak_type,
        carapace_rs_shlex::WordbreakType::ListAnd
    );
}

#[test]
fn keyword_operator_or() {
    let tokens = split("echo foo or", Format::Fish).unwrap();
    let last = tokens.last().unwrap();
    assert_eq!(last.kind, TokenType::Wordbreak);
    assert_eq!(
        last.wordbreak_type,
        carapace_rs_shlex::WordbreakType::ListOr
    );
}

#[test]
fn not_not_keyword_operator() {
    // "not" is a prefix keyword but NOT a pipeline delimiter
    let tokens = split("echo foo not", Format::Fish).unwrap();
    let last = tokens.last().unwrap();
    assert_eq!(last.kind, TokenType::Word);
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Fish).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Fish), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Fish), 2);
}

#[test]
fn double_quote() {
    let words = split_words(r#"echo "hello world""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
}

#[test]
fn completion_context() {
    let ctx = complete("echo foo and grep hel", Format::Fish);
    assert_eq!(ctx.current_word, "hel");
    assert_eq!(ctx.words.len(), 2);
}

#[test]
fn dollar_not_escape_in_single_quotes() {
    // \$ is NOT an escape in fish single quotes — only \' and \\ are
    let words = split_words(r"echo 'cost: \$5'", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), r"cost: \$5".to_owned()]);
}

#[test]
fn escaped_space() {
    let words = split_words(r"echo a\ b", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "a b".to_owned()]);
}

#[test]
fn parens_not_wordbreak() {
    // Fish: () are command substitution, not word breaks.
    let words = split_words("echo (echo test)", Format::Fish);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "(echo".to_owned(), "test)".to_owned()]
    );
}

#[test]
fn and_and() {
    let tokens = split("echo foo && echo bar", Format::Fish).unwrap();
    assert_eq!(pipeline_count("echo foo && echo bar", Format::Fish), 2);
    let last = &tokens[tokens.len() - 3];
    assert_eq!(last.kind, TokenType::Wordbreak);
    assert_eq!(
        last.wordbreak_type,
        carapace_rs_shlex::WordbreakType::ListAnd
    );
}

#[test]
fn or_or() {
    assert_eq!(pipeline_count("echo foo || echo bar", Format::Fish), 2);
}

#[test]
fn background() {
    assert_eq!(pipeline_count("echo foo & echo bar", Format::Fish), 2);
}

#[test]
fn pipe_with_stderr_merge() {
    let tokens = split("echo foo |& cat bar", Format::Fish).unwrap();
    assert_eq!(pipeline_count("echo foo |& cat bar", Format::Fish), 2);
    let wb = &tokens[tokens.len() - 3];
    assert_eq!(wb.kind, TokenType::Wordbreak);
    assert_eq!(
        wb.wordbreak_type,
        carapace_rs_shlex::WordbreakType::PipeWithStderr
    );
}

#[test]
fn amp_pipe() {
    assert_eq!(pipeline_count("echo foo &| cat bar", Format::Fish), 2);
}

#[test]
fn explicit_fd_pipe() {
    // >| is a pipe with explicit fd in fish (e.g. echo foo >| bar)
    let tokens = split("echo foo >| cat bar", Format::Fish).unwrap();
    assert_eq!(pipeline_count("echo foo >| cat bar", Format::Fish), 2);
    let wb = &tokens[tokens.len() - 3];
    assert_eq!(wb.kind, TokenType::Wordbreak);
    assert_eq!(wb.wordbreak_type, carapace_rs_shlex::WordbreakType::Pipe);
}

#[test]
fn amp_redirect() {
    assert!(split("echo foo &> file.txt", Format::Fish).is_some());
    let ctx = complete("echo foo &> file.txt", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn amp_redirect_append() {
    assert!(split("echo foo &>> file.txt", Format::Fish).is_some());
    let ctx = complete("echo foo &>> file.txt", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn fd_redirect() {
    // >&2 is a fd redirect: >& is the operator, 2 is the fd number.
    let tokens = split("echo foo >&2", Format::Fish).unwrap();
    assert!(tokens.iter().any(|t| t.kind == TokenType::Wordbreak
        && t.wordbreak_type == carapace_rs_shlex::WordbreakType::RedirectInputDuplicate));
}

#[test]
fn input_output_redirect() {
    let ctx = complete("echo foo <> ", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn noclobber_redirect() {
    let ctx = complete("echo foo >? ", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn noclobber_append_redirect() {
    let ctx = complete("echo foo >>? ", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn try_input_redirect() {
    let ctx = complete("echo foo <? ", Format::Fish);
    assert!(ctx.is_redirect);
}

#[test]
fn double_quote_escaped_quote() {
    let words = split_words(r#"echo "say \"hello\"""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), r#"say "hello""#.to_owned()]);
}

#[test]
fn double_quote_escaped_dollar() {
    let words = split_words(r#"echo "cost: \$5""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "cost: $5".to_owned()]);
}

#[test]
fn double_quote_escaped_backslash() {
    let words = split_words(r#"echo "C:\\path""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), r"C:\path".to_owned()]);
}

#[test]
fn double_quote_non_escape_backslash() {
    // \n inside fish double quotes is NOT an escape — both \ and n are literal
    let words = split_words(r#"echo "hello\nworld""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), r"hello\nworld".to_owned()]);
}

#[test]
fn double_quote_non_escape_backslash_other() {
    // \t inside fish double quotes is NOT an escape — both \ and t are literal
    let words = split_words(r#"echo "a\tb""#, Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), r"a\tb".to_owned()]);
}

#[test]
fn double_quote_escaped_newline() {
    // \<newline> is a line continuation escape inside fish double quotes.
    // Both the backslash and the newline are consumed (removed).
    let words = split_words("echo \"hello\\\nworld\"", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "helloworld".to_owned()]);
}

#[test]
fn quote_word_backtick() {
    // Backtick is a regular character in fish — should not trigger quoting
    assert_eq!(quote_word("hello`world", Format::Fish), "hello`world");
}

#[test]
fn quote_word_dollar() {
    assert_eq!(quote_word("hello$world", Format::Fish), r#""hello\$world""#);
}

#[test]
fn quote_word_safe() {
    assert_eq!(quote_word("hello-world", Format::Fish), "hello-world");
}

#[test]
fn completion_and_and() {
    let ctx = complete("echo foo && echo bar hel", Format::Fish);
    assert_eq!(ctx.current_word, "hel");
    assert_eq!(ctx.words.len(), 3);
}

#[test]
fn completion_background() {
    let ctx = complete("echo foo & echo bar hel", Format::Fish);
    assert_eq!(ctx.current_word, "hel");
    assert_eq!(ctx.words.len(), 3);
}

#[test]
fn line_continuation_outside_quotes() {
    let words = split_words("echo foo\\\nbar", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_in_double_quotes() {
    let words = split_words("echo \"line1\\\nline2\"", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}

#[test]
fn line_continuation_crlf_in_double_quotes() {
    let words = split_words("echo \"line1\\\r\nline2\"", Format::Fish);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}
