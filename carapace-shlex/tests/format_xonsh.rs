mod common;

use carapace_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, split,
};
use common::{pipeline_count, split_words};

fn words_of(input: &str) -> Vec<String> {
    split_words(input, Format::Xonsh)
}

#[test]
fn single_quote() {
    assert_eq!(words_of("echo 'hello world'"), vec!["echo", "hello world"]);
}

#[test]
fn raw_string() {
    // r'...' — r prefix + single quote, Words() merges
    assert_eq!(words_of(r"echo r'C:\path'"), vec!["echo", r"rC:\path"]);
}

#[test]
fn raw_double_quoted() {
    // r"..." — r prefix merges with double-quoted segment; backslashes literal
    assert_eq!(words_of(r#"echo r"C:\path""#), vec!["echo", r"rC:\path"]);
}

#[test]
fn double_quote_escape() {
    let tokens = split(r#"echo "say \"hello\"""#, Format::Xonsh).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.value, r#"say "hello""#);
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Xonsh), 2);
}

#[test]
fn background() {
    // Xonsh uses & for background (POSIX-like)
    let tokens = split("echo foo &", Format::Xonsh).unwrap();
    let amp = tokens.iter().rfind(|t| t.raw_value == "&").unwrap();
    assert_eq!(amp.kind, TokenType::Wordbreak);
    assert_eq!(amp.wordbreak_type, WordbreakType::ListAsync);
}

#[test]
fn double_and() {
    assert_eq!(pipeline_count("echo foo && echo bar", Format::Xonsh), 2);
}

#[test]
fn double_or() {
    assert_eq!(pipeline_count("echo foo || echo bar", Format::Xonsh), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Xonsh), 2);
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Xonsh).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn keyword_and() {
    assert_eq!(pipeline_count("echo foo and echo bar", Format::Xonsh), 2);
}

#[test]
fn keyword_or() {
    assert_eq!(pipeline_count("echo foo or echo bar", Format::Xonsh), 2);
}

fn wordbreak_type_of(input: &str, raw: &str) -> WordbreakType {
    let tokens = split(input, Format::Xonsh).unwrap();
    let found = tokens
        .iter()
        .rfind(|t| t.kind == TokenType::Wordbreak && t.raw_value == raw);
    assert!(found.is_some(), "{input}: no {raw} wordbreak token found");
    found.unwrap().wordbreak_type
}

#[test]
fn stream_redirect_stderr() {
    assert_eq!(
        wordbreak_type_of("echo foo e> bar", "e>"),
        WordbreakType::RedirectOutput
    );
}

#[test]
fn stream_redirect_stdout() {
    assert_eq!(
        wordbreak_type_of("echo foo o> bar", "o>"),
        WordbreakType::RedirectOutput
    );
}

#[test]
fn stream_redirect_all() {
    assert_eq!(
        wordbreak_type_of("echo foo a> bar", "a>"),
        WordbreakType::RedirectOutputBoth
    );
}

#[test]
fn stream_redirect_stderr_append() {
    assert_eq!(
        wordbreak_type_of("echo foo e>> bar", "e>>"),
        WordbreakType::RedirectOutput
    );
}

#[test]
fn stream_redirect_long_form() {
    assert_eq!(
        wordbreak_type_of("echo foo err> bar", "err>"),
        WordbreakType::RedirectOutput
    );
}

#[test]
fn stream_redirect_pipe_channel() {
    // e>p exists as a wordbreak token (pipe channel redirect)
    let tokens = split("echo foo e>p bar", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenType::Wordbreak && t.raw_value == "e>p"),
        "no e>p wordbreak token found"
    );
}

#[test]
fn triple_single_quote() {
    assert_eq!(
        words_of("echo '''hello world'''"),
        vec!["echo", "hello world"]
    );
}

#[test]
fn triple_double_quote() {
    assert_eq!(
        words_of(r#"echo """hello world""""#),
        vec!["echo", "hello world"]
    );
}

#[test]
fn triple_double_quote_escape() {
    let tokens = split(r#"echo """say \"hello\"""""#, Format::Xonsh).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.value, r#"say "hello""#);
}

#[test]
fn triple_single_quote_with_embedded_quotes() {
    assert_eq!(
        words_of(r#"echo '''he said "hi"'''"#),
        vec!["echo", r#"he said "hi""#]
    );
}

#[test]
fn triple_quote_unclosed() {
    let tokens = split("echo '''unclosed", Format::Xonsh).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::QuotingTriple);
}

#[test]
fn raw_triple_double_quote() {
    assert_eq!(
        words_of(r#"echo r"""C:\new\path""""#),
        vec!["echo", r"rC:\new\path"]
    );
}

#[test]
fn raw_double_quoted_with_escaped_quote() {
    // The " in r"...\..." closes the string (rawQuote only affects backslash)
    assert_eq!(words_of(r#"echo r"C:\path""#), vec!["echo", r"rC:\path"]);
}

#[test]
fn two_quotes_inside_triple() {
    // '' inside '''...''' — two single quotes should NOT close triple-single
    assert_eq!(
        words_of(r"echo '''hello''there'''"),
        vec!["echo", "hello''there"]
    );
}

#[test]
fn two_double_quotes_inside_triple_double() {
    assert_eq!(
        words_of(r#"echo """hello""there""""#),
        vec!["echo", r#"hello""there"#]
    );
}

#[test]
fn triple_quote_then_more() {
    assert_eq!(
        words_of(r#"echo """hello""" world"#),
        vec!["echo", "hello", "world"]
    );
}

#[test]
fn triple_quote_adjacent_word() {
    assert_eq!(words_of(r#"echo foo"""bar""""#), vec!["echo", "foobar"]);
}

#[test]
fn raw_prefix_false_positive() {
    // xr"hello\nworld" — 'x' is not a valid prefix char, no raw prefix
    assert_eq!(
        words_of(r#"echo xr"hello\nworld""#),
        vec!["echo", "xrhellonworld"]
    );
}

#[test]
fn valid_br_prefix() {
    // br"hello\nworld" — 'br' is a valid Python raw prefix
    assert_eq!(
        words_of(r#"echo br"hello\nworld""#),
        vec!["echo", r"brhello\nworld"]
    );
}

#[test]
fn keyword_and_inside_word() {
    // 'fooand' should NOT be a keyword operator — only exact match counts
    assert_eq!(pipeline_count("echo fooand bar", Format::Xonsh), 1);
}

#[test]
fn quoted_stream_redirect_not_merged() {
    // 'e'> bar — quoted 'e' should NOT be merged with > as stream redirect
    let tokens = split("echo foo 'e'> bar", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .all(|t| !(t.kind == TokenType::Wordbreak && t.raw_value == "e>"))
    );
}

#[test]
fn line_continuation_in_double_quotes() {
    let words = split_words("echo \"line1\\\nline2\"", Format::Xonsh);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}

// --- edge cases (format_xonsh_edge_test.go) ---

#[test]
fn raw_prefix_rb() {
    assert_eq!(
        words_of(r#"echo rb"hello\nworld""#),
        vec!["echo", r"rbhello\nworld"]
    );
}

#[test]
fn raw_prefix_rf() {
    assert_eq!(
        words_of(r#"echo rf"hello\nworld""#),
        vec!["echo", r"rfhello\nworld"]
    );
}

#[test]
fn raw_prefix_rbf() {
    assert_eq!(
        words_of(r#"echo rbf"hello\nworld""#),
        vec!["echo", r"rbfhello\nworld"]
    );
}

#[test]
fn raw_prefix_uppercase_r() {
    assert_eq!(
        words_of(r#"echo R"hello\nworld""#),
        vec!["echo", r"Rhello\nworld"]
    );
}

#[test]
fn raw_prefix_uppercase_br() {
    assert_eq!(
        words_of(r#"echo BR"hello\nworld""#),
        vec!["echo", r"BRhello\nworld"]
    );
}

#[test]
fn raw_prefix_false_positive_rr() {
    // rr is NOT a valid Python prefix (duplicate r)
    assert_eq!(
        words_of(r#"echo rr"hello\nworld""#),
        vec!["echo", "rrhellonworld"]
    );
}

#[test]
fn raw_prefix_false_positive_bb() {
    assert_eq!(
        words_of(r#"echo bb"hello\nworld""#),
        vec!["echo", "bbhellonworld"]
    );
}

#[test]
fn non_raw_prefix_f() {
    // f"hello\nworld" — f-string, NOT raw, escapes are processed
    assert_eq!(
        words_of(r#"echo f"hello\nworld""#),
        vec!["echo", "fhellonworld"]
    );
}

#[test]
fn non_raw_prefix_b() {
    assert_eq!(
        words_of(r#"echo b"hello\nworld""#),
        vec!["echo", "bhellonworld"]
    );
}

#[test]
fn raw_triple_rb() {
    assert_eq!(
        words_of(r#"echo rb"""hello\nworld""""#),
        vec!["echo", r"rbhello\nworld"]
    );
}

#[test]
fn empty_triple_double() {
    // """""" — six double quotes = empty triple-quoted string
    assert_eq!(words_of(r#"echo """""""#), vec!["echo", ""]);
}

#[test]
fn empty_triple_single() {
    assert_eq!(words_of("echo ''''''"), vec!["echo", ""]);
}

#[test]
fn triple_quote_at_eof_no_content() {
    let tokens = split(r#"echo """"#, Format::Xonsh).unwrap();
    let words = tokens.words();
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].state, LexerState::QuotingTripleEscaping);
}

#[test]
fn triple_single_at_eof_no_content() {
    let tokens = split("echo '''", Format::Xonsh).unwrap();
    let words = tokens.words();
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].state, LexerState::QuotingTriple);
}

#[test]
fn keyword_uppercase() {
    // AND/OR uppercase — Python keywords are case-sensitive, should NOT split
    assert_eq!(pipeline_count("echo foo AND echo bar", Format::Xonsh), 1);
}

#[test]
fn keyword_at_eof() {
    // "and" at EOF should split (no trailing-whitespace requirement)
    assert_eq!(pipeline_count("echo foo and", Format::Xonsh), 2);
}

#[test]
fn stream_redirect_at_eof() {
    let ctx = complete("echo foo e>", Format::Xonsh);
    assert!(ctx.is_redirect, "e> at EOF");
}

#[test]
fn append_pipe_channel() {
    // e>>p — append pipe channel, classified as redirect
    let tokens = split("echo foo e>>p bar", Format::Xonsh).unwrap();
    let found = tokens
        .iter()
        .rfind(|t| t.kind == TokenType::Wordbreak && t.raw_value == "e>>p");
    assert!(found.is_some(), "no e>>p wordbreak token found");
    assert!(found.unwrap().wordbreak_type.is_redirect());
}

#[test]
fn long_form_pipe_channel() {
    // err>p — long-form pipe channel, classified as redirect
    let tokens = split("echo foo err>p bar", Format::Xonsh).unwrap();
    let found = tokens
        .iter()
        .rfind(|t| t.kind == TokenType::Wordbreak && t.raw_value == "err>p");
    assert!(found.is_some(), "no err>p wordbreak token found");
    assert!(found.unwrap().wordbreak_type.is_redirect());
}
