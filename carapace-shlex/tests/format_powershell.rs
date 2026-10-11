mod common;

use carapace_shlex::{Format, LexerState, TokenSliceExt, TokenType, split};
use common::{pipeline_count, split_words};

fn words_of(input: &str) -> Vec<String> {
    split_words(input, Format::Powershell)
}

#[test]
fn doubled_double_quote() {
    // PowerShell: "" inside double quotes → literal "
    let tokens = split(r#"echo "say ""hello""""#, Format::Powershell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.value, r#"say "hello""#);
}

#[test]
fn backtick_not_backslash() {
    // PowerShell: \ is NOT an escape (backtick is)
    assert_eq!(words_of(r"echo C:\path"), vec!["echo", r"C:\path"]);
}

#[test]
fn doubled_single_quote_split() {
    assert_eq!(words_of("echo 'don''t'"), vec!["echo", "don't"]);
}

#[test]
fn backtick_escape_outside() {
    assert_eq!(words_of("echo `$HOME"), vec!["echo", "$HOME"]);
}

#[test]
fn backtick_in_double_quotes() {
    assert_eq!(
        words_of("echo \"say `\"hello`\"\""),
        vec!["echo", r#"say "hello""#]
    );
}

#[test]
fn pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Powershell), 2);
}

#[test]
fn semicolon() {
    assert_eq!(pipeline_count("echo foo ; echo bar", Format::Powershell), 2);
}

#[test]
fn double_and() {
    assert_eq!(
        pipeline_count("echo foo && echo bar", Format::Powershell),
        2
    );
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Powershell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn open_double_quote() {
    let tokens = split(r#"echo "hel"#, Format::Powershell).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::QuotingEscaping);
}

#[test]
fn backtick_line_continuation() {
    let words = split_words("echo foo`\nbar", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn backtick_line_continuation_crlf() {
    let tokens = split("echo foo`\r\nbar", Format::Powershell).unwrap();
    let words = tokens.words();
    assert_eq!(words.strings()[1], "foobar");
    // RawValue should NOT contain the backtick or CRLF (line continuation
    // is consumed)
    assert_eq!(words[1].raw_value, "foobar");
}

#[test]
fn backtick_line_continuation_start_of_word() {
    let words = split_words("echo `\nbar", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "bar".to_owned()]);
}

#[test]
fn block_comment() {
    let words = split_words("echo <# multi\nline\ncomment #> foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn block_comment_single_line() {
    let words = split_words("echo <# inline comment #> foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stop_parsing_token() {
    // After --%, everything is literal until newline or |
    let words = split_words("echo --% /grant Dom\\HVAdmin:(CI)(OI)F", Format::Powershell);
    assert!(words.len() >= 3);
    assert_eq!(words[0], "echo");
    assert_eq!(words[1], "--%");
}

#[test]
fn stop_parsing_pipe_delim() {
    // After --%, | is still a pipeline delimiter
    assert_eq!(
        pipeline_count("echo --% foo | Select-String bar", Format::Powershell),
        2
    );
}

#[test]
fn stop_parsing_raw_content() {
    // After --%, content like (CI) should be literal, not split
    let words = split_words(
        "icacls X: --% /grant Dom\\HVAdmin:(CI)(OI)F",
        Format::Powershell,
    );
    assert_eq!(words.len(), 4);
    assert_eq!(words[2], "--%");
    assert_eq!(words[3], r"/grant Dom\HVAdmin:(CI)(OI)F");
}

fn wordbreak_raw_of(input: &str, raw: &str) -> Option<carapace_shlex::WordbreakType> {
    split(input, Format::Powershell)
        .unwrap()
        .iter()
        .find(|t| t.kind == TokenType::Wordbreak && t.raw_value == raw)
        .map(|t| t.wordbreak_type)
}

#[test]
fn stream_redirect_2() {
    let found = wordbreak_raw_of("echo foo 2> error.txt", "2>").expect("no merged 2> token");
    assert!(found.is_redirect());
}

#[test]
fn stream_redirect_2_append() {
    let found = wordbreak_raw_of("echo foo 2>> error.txt", "2>>").expect("no merged 2>> token");
    assert!(found.is_redirect());
}

#[test]
fn stream_redirect_merge() {
    // 2>&1 should be recognized as a merged stream redirect
    assert!(
        wordbreak_raw_of("echo foo 2>&1", "2>&1").is_some(),
        "no merged token found"
    );
}

#[test]
fn stream_redirect_star() {
    // *> should be recognized as all-streams redirect
    let found = wordbreak_raw_of("echo foo *> output.txt", "*>").expect("no merged *> token");
    assert!(found.is_redirect());
}

// --- edge cases (format_powershell_edge_test.go) ---

#[test]
fn unclosed_block_comment() {
    let words = split_words("echo <# unclosed comment", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned()]);
}

#[test]
fn block_comment_at_start() {
    let words = split_words("<# comment #> echo foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn zero_length_block_comment() {
    let words = split_words("echo <##> foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn stop_parsing_at_eof() {
    let words = split_words("echo --%", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "--%".to_owned()]);
}

#[test]
fn stop_parsing_pipe_in_quotes() {
    assert_eq!(
        pipeline_count(r#"echo --% "foo | bar""#, Format::Powershell),
        1
    );
}

#[test]
fn stop_parsing_unclosed_quote_pipe() {
    assert_eq!(
        pipeline_count(r#"echo --% "foo | bar"#, Format::Powershell),
        1
    );
}

#[test]
fn backtick_at_eof_in_double_quotes() {
    let tokens = split("echo \"hel`", Format::Powershell).unwrap();
    let words = tokens.words();
    assert_eq!(words.len(), 2);
    assert_eq!(words[1].value, "hel");
}

#[test]
fn backtick_escape_pipe() {
    let words = split_words("echo `|", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "|".to_owned()]);
}

#[test]
fn backtick_escape_operators() {
    for op in ["&", ";", ">", "<", "(", ")"] {
        let words = split_words(&format!("echo `{op}"), Format::Powershell);
        assert_eq!(
            words,
            vec!["echo".to_owned(), op.to_owned()],
            "backtick {op}"
        );
    }
}

#[test]
fn double_backtick() {
    let words = split_words("echo ``foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "`foo".to_owned()]);
}

#[test]
fn backtick_line_continuation_in_double_quotes() {
    let words = split_words("echo \"foo`\nbar\"", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn triple_double_quote_at_start() {
    // """ at start: first " opens, second peeks third → doubled literal ",
    // third closes, then hello, then " opens a new (unclosed) quote.
    let words = split_words(r#"echo """hello""#, Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "\"hello".to_owned()]);
}

#[test]
fn quadruple_double_quote() {
    // """" → 1st opens, 2nd peeks 3rd (match) → literal ", 3rd consumed,
    // 4th peeks EOF → close. Value = one literal ".
    let words = split_words(r#"echo """"#, Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "\"".to_owned()]);
}

#[test]
fn triple_single_quote_at_start() {
    let words = split_words("echo '''hello'", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "'hello".to_owned()]);
}

#[test]
fn double_or_pipeline_split() {
    assert_eq!(
        pipeline_count("echo foo || echo bar", Format::Powershell),
        2
    );
}
