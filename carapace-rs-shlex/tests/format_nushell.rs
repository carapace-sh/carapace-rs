mod common;

use carapace_rs_shlex::{Format, LexerState, TokenSliceExt, complete, split};
use common::{filtered_pipeline_words, pipeline_count, split_words};

fn escape_test(input: &str, want: &str) {
    let tokens = split(&format!("echo {input}"), Format::Nushell).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, want, "nushell escape {input}");
}

#[test]
fn single_quote() {
    let words = split_words("echo 'hello world'", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
}

#[test]
fn double_quote() {
    let tokens = split(r#"echo "hello\nworld""#, Format::Nushell).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, "hello\nworld");
}

#[test]
fn backtick_quote() {
    // Nushell: backtick is a quote char (not escape)
    let words = split_words("echo `hello world`", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
}

#[test]
fn interpolated_prefix() {
    // $'...' — $ prefix + single quote, Words() merges
    let words = split_words("echo $'hello'", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "$hello".to_owned()]);
}

#[test]
fn interpolated_double() {
    let words = split_words(r#"echo $"hello""#, Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "$hello".to_owned()]);
}

#[test]
fn open_backtick() {
    let tokens = split("echo `hel", Format::Nushell).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Nushell), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Nushell), 2);
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Nushell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn open_double_quote() {
    let tokens = split(r#"echo "hel"#, Format::Nushell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::QuotingEscaping);
}

// --- Escape sequence tests ---

#[test]
fn escape_sequences() {
    escape_test(r#""hello\nworld""#, "hello\nworld");
    escape_test(r#""col1\tcol2""#, "col1\tcol2");
    escape_test(r#""test\rmore""#, "test\rmore");
    escape_test(r#""path\\file""#, r"path\file");
    escape_test(r#""say \"hello\"""#, r#"say "hello""#);
    escape_test(r#""it's \"quoted\"""#, r#"it's "quoted""#);
    escape_test(r#""x\ay""#, "x\u{7}y");
    escape_test(r#""x\by""#, "x\u{8}y");
    escape_test(r#""x\ey""#, "x\u{1b}y");
    escape_test(r#""x\fy""#, "x\u{c}y");
    escape_test(r#""x\0y""#, "x\u{0}y");
    escape_test(r#""C:\/Users""#, "C:/Users");
    escape_test(r#""\$""#, "$");
    escape_test(r#""\^""#, "^");
    escape_test(r#""\#""#, "#");
    escape_test(r#""\|""#, "|");
    escape_test(r#""\~""#, "~");
    escape_test(r#""\("""#, "(");
    escape_test(r#""\)""#, ")");
    escape_test(r#""\{""#, "{");
    escape_test(r#""\}""#, "}");
    escape_test(r#""\q""#, r"\q"); // unrecognized keeps the backslash
}

#[test]
fn escape_in_single_quote() {
    // Single-quoted strings have no escape processing
    let words = split_words(r"echo 'hello\nworld'", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), r"hello\nworld".to_owned()]);
}

#[test]
fn escape_in_backtick() {
    // Backtick strings have no escape processing
    let words = split_words(r"echo `hello\nworld`", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), r"hello\nworld".to_owned()]);
}

#[test]
fn open_double_quote_with_escape() {
    // Open double quote with escape at EOF — stays in ESCAPING_QUOTED_STATE
    let tokens = split(r#"echo "hello\"#, Format::Nushell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::EscapingQuoted);
}

// --- Stream redirect operator tests ---

#[test]
fn stream_redirect_out() {
    let words = filtered_pipeline_words("cat foo out> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_err() {
    let words = filtered_pipeline_words("cat foo err> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_oe() {
    let words = filtered_pipeline_words("cat foo o+e> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_out_err() {
    let words = filtered_pipeline_words("cat foo out+err> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_short() {
    let words = filtered_pipeline_words("cat foo o> bar e> baz", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_pipe_epipe() {
    assert_eq!(pipeline_count("cat foo e>| bar", Format::Nushell), 2);
}

#[test]
fn stream_pipe_oepipe() {
    assert_eq!(pipeline_count("cat foo o+e>| bar", Format::Nushell), 2);
}

#[test]
fn stream_pipe_errpipe() {
    assert_eq!(pipeline_count("cat foo err>| bar", Format::Nushell), 2);
}

#[test]
fn stream_redirect_is_redirect() {
    let ctx = complete("cat foo out> ", Format::Nushell);
    assert!(ctx.is_redirect);
    let tokens = split("cat foo out> ", Format::Nushell).unwrap();
    assert_eq!(
        tokens
            .current_pipeline()
            .filter_redirects()
            .words()
            .strings()
            .len(),
        2
    );
}

#[test]
fn non_stream_word_not_merged() {
    let words = filtered_pipeline_words("echo foo > bar", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_with_append() {
    // out>> should be recognized (out + >>)
    let words = filtered_pipeline_words("cat foo out>> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn quoted_stream_word_not_merged() {
    // A quoted word like 'out' before > should NOT be merged as a stream
    // redirect — it's a string literal, not an operator.
    for input in ["echo 'out'>bar", r#"echo "out">bar"#, "echo `out`>bar"] {
        let words = filtered_pipeline_words(input, Format::Nushell);
        assert_eq!(words, vec!["echo".to_owned(), "out".to_owned()], "{input}");
    }
}

// --- Backslash outside quotes (bareword) tests ---

#[test]
fn backslash_bareword() {
    // Nushell: backslash outside quotes is a regular word character
    let words = split_words(r"echo foo\bar", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), r"foo\bar".to_owned()]);
}

#[test]
fn backslash_space_not_escape() {
    // foo\ bar should be two words: foo\ and bar
    let words = split_words(r"echo foo\ bar", Format::Nushell);
    assert_eq!(
        words,
        vec!["echo".to_owned(), r"foo\".to_owned(), "bar".to_owned()]
    );
}

// --- < not a wordbreak test ---

#[test]
fn less_than_not_wordbreak() {
    // Nushell does not support input redirection with <.
    let words = split_words("echo foo<bar", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "foo<bar".to_owned()]);
}

// --- Additional stream redirect alias tests ---

#[test]
fn stream_redirect_err_out() {
    let words = filtered_pipeline_words("cat foo err+out> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_eo() {
    let words = filtered_pipeline_words("cat foo e+o> bar", Format::Nushell);
    assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()]);
}

// --- Append redirect classification tests ---

#[test]
fn append_redirect() {
    let words = filtered_pipeline_words("echo foo >> bar", Format::Nushell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stream_redirect_append_both() {
    for input in [
        "cat foo out+err>> bar",
        "cat foo o+e>> bar",
        "cat foo err+out>> bar",
        "cat foo e+o>> bar",
    ] {
        let words = filtered_pipeline_words(input, Format::Nushell);
        assert_eq!(words, vec!["cat".to_owned(), "foo".to_owned()], "{input}");
    }
}

// --- Pipe variant classification tests ---

#[test]
fn stream_pipe_out_pipe_is_plain_pipe() {
    // out>| in nushell is a plain Pipe (stdout to pipe = normal piping)
    assert_eq!(pipeline_count("echo foo out>| bar", Format::Nushell), 2);
}

#[test]
fn stream_pipe_all_pipe_variants() {
    for input in [
        "cat foo err>| bar",
        "cat foo e>| bar",
        "cat foo o+e>| bar",
        "cat foo e+o>| bar",
        "cat foo err+out>| bar",
        "cat foo out+err>| bar",
        "cat foo out>| bar",
        "cat foo o>| bar",
    ] {
        assert_eq!(pipeline_count(input, Format::Nushell), 2, "{input}");
    }
}

// --- Completion context redirect detection ---

#[test]
fn completion_redirect_after_err() {
    let ctx = complete("cat foo err> ", Format::Nushell);
    assert!(ctx.is_redirect);
}

#[test]
fn completion_redirect_after_out_append() {
    let ctx = complete("cat foo out>> ", Format::Nushell);
    assert!(ctx.is_redirect);
}
