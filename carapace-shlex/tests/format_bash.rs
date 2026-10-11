mod common;

use carapace_shlex::{Format, TokenSliceExt, TokenType, WordbreakType, complete, split};
use common::{pipeline_count, split_words};

#[test]
fn close_quote_escape_reopen() {
    // The POSIX idiom for embedding a single quote: 'it'\''s
    let words = split_words("echo 'it'\\''s", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "it's".to_owned()]);
}

#[test]
fn escaped_space() {
    let words = split_words(r"echo a\ b", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "a b".to_owned()]);
}

#[test]
fn backslash_n_in_double_quotes() {
    // In bash, \n inside "..." is literal (backslash not special before n).
    // Both backslash and n should be preserved in the value.
    let tokens = split(r#"echo "hello\nworld""#, Format::Bash).unwrap();
    let words = tokens.words();
    let last = words.last().unwrap();
    assert_eq!(last.state, carapace_shlex::LexerState::InWord);
    assert_eq!(last.value, r"hello\nworld");
}

#[test]
fn adjacent_quoted_segments() {
    let words = split_words(r#"echo a"b"'c'"#, Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "abc".to_owned()]);
}

#[test]
fn single_quote_literal() {
    let words = split_words("echo '$HOME \\n \\t'", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "$HOME \\n \\t".to_owned()]);
}

#[test]
fn at_wordbreak_prefix() {
    // @ is a wordbreak but WordbreakPrefix skips it
    let ctx = complete("echo foo@bar", Format::Bash);
    assert_eq!(ctx.current_word, "foo@bar");
    // @ is skipped as a wordbreak boundary, so prefix should be "foo"
    assert_eq!(ctx.prefix, "foo");
}

#[test]
fn escape_at_eof() {
    let tokens = split(r"echo foo\", Format::Bash).unwrap();
    let words = tokens.words();
    let last = words.last().unwrap();
    assert_eq!(last.state, carapace_shlex::LexerState::Escaping);
    assert_eq!(last.value, "foo");
}

#[test]
fn comment() {
    // Lexer skips comments, so only "echo" and "hello" are returned
    let words = split_words("echo hello # comment", Format::Bash);
    assert_eq!(words.len(), 2);
}

#[test]
fn force_output_redirect() {
    let ctx = complete("echo foo >| bar", Format::Bash);
    assert!(ctx.is_redirect);
}

#[test]
fn case_terminator() {
    let tokens = split("echo foo ;; bar", Format::Bash).unwrap();
    assert_eq!(pipeline_count("echo foo ;; bar", Format::Bash), 2);
    let found = tokens
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak)
        .expect("no wordbreak token found");
    assert_eq!(found.wordbreak_type, WordbreakType::ListSequentialDouble);
}

#[test]
fn backslash_dollar_in_double_quotes() {
    // \$ inside "..." should drop the backslash — $ is a CBSDQUOTE char
    let words = split_words(r#"echo "hello\$world""#, Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "hello$world".to_owned()]);
}

#[test]
fn backslash_quote_in_double_quotes() {
    let words = split_words(r#"echo "say \"hello\"""#, Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), r#"say "hello""#.to_owned()]);
}

#[test]
fn backslash_backtick_in_double_quotes() {
    let words = split_words("echo \"hello\\`world\"", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "hello`world".to_owned()]);
}

#[test]
fn backslash_backslash_in_double_quotes() {
    let words = split_words(r#"echo "hello\\world""#, Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), r"hello\world".to_owned()]);
}

#[test]
fn operator_boundary() {
    // >; should be two separate tokens: > and ; not a single unknown operator >;
    let tokens = split("echo foo >; bar", Format::Bash).unwrap();
    let wordbreaks: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenType::Wordbreak)
        .collect();
    assert_eq!(wordbreaks.len(), 2);
    assert_eq!(wordbreaks[0].raw_value, ">");
    assert_eq!(wordbreaks[1].raw_value, ";");
}

#[test]
fn here_doc_operator() {
    let tokens = split("echo foo << bar", Format::Bash).unwrap();
    let found = tokens
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak)
        .expect("no wordbreak token found");
    assert_eq!(found.wordbreak_type, WordbreakType::RedirectHereDoc);
    assert!(found.wordbreak_type.is_redirect());
}

#[test]
fn here_string_operator() {
    let tokens = split("echo foo <<< bar", Format::Bash).unwrap();
    let found = tokens
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak)
        .expect("no wordbreak token found");
    assert_eq!(found.wordbreak_type, WordbreakType::RedirectInputString);
}

#[test]
fn case_fallthrough() {
    // ;& is bash 4+ case fall-through
    let tokens = split("echo foo ;& bar", Format::Bash).unwrap();
    let found = tokens
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak)
        .expect("no wordbreak token found");
    assert_eq!(found.wordbreak_type, WordbreakType::ListFallthrough);
}

#[test]
fn case_next_pattern() {
    // ;;& is bash 4+ case next pattern
    let tokens = split("echo foo ;;& bar", Format::Bash).unwrap();
    let found = tokens
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak)
        .expect("no wordbreak token found");
    assert_eq!(found.wordbreak_type, WordbreakType::ListCaseNext);
    assert!(found.wordbreak_type.is_pipeline_delimiter());
}

#[test]
fn close_paren_as_wordbreak() {
    // ) is a shell break character in bash
    let tokens = split("echo (foo) bar", Format::Bash).unwrap();
    let wordbreaks: Vec<&str> = tokens
        .iter()
        .filter(|t| t.kind == TokenType::Wordbreak)
        .map(|t| t.raw_value.as_str())
        .collect();
    assert!(wordbreaks.len() >= 2);
    assert!(wordbreaks.contains(&"("));
    assert!(wordbreaks.contains(&")"));
}

#[test]
fn operator_sequence_pipe_semicolon() {
    // |; should be two separate tokens: | and ;
    let tokens = split("echo foo |; bar", Format::Bash).unwrap();
    let wordbreaks: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenType::Wordbreak)
        .collect();
    assert_eq!(wordbreaks.len(), 2);
    assert_eq!(wordbreaks[0].raw_value, "|");
    assert_eq!(wordbreaks[1].raw_value, ";");
}

#[test]
fn line_continuation_outside_quotes() {
    let words = split_words("echo foo\\\nbar", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_in_double_quotes() {
    let words = split_words("echo \"line1\\\nline2\"", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}

#[test]
fn line_continuation_crlf_in_double_quotes() {
    let words = split_words("echo \"line1\\\r\nline2\"", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}

#[test]
fn line_continuation_at_word_start() {
    let words = split_words("echo \\\nbar", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "bar".to_owned()]);
}
