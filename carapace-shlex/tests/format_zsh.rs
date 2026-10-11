mod common;

use carapace_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, split,
};
use common::{pipeline_count, split_words};

#[test]
fn rc_quotes() {
    // With RC_QUOTES, '' inside single quotes → literal '
    let tokens = split("echo 'it''s'", Format::Zsh).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, "it's");
    assert_eq!(last.state, LexerState::InWord);
}

#[test]
fn no_rc_quotes() {
    // Without RC_QUOTES (bash), '' closes then reopens → words merge to "its"
    let tokens = split("echo 'it''s'", Format::Bash).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.value, "its");
}

#[test]
fn open_quote() {
    let tokens = split("echo 'hel", Format::Zsh).unwrap();
    let words = tokens.words();
    let last = words.current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn double_quote_escape() {
    let words = split_words(r#"echo "say \"hello\"""#, Format::Zsh);
    assert_eq!(words, vec!["echo".to_owned(), r#"say "hello""#.to_owned()]);
}

#[test]
fn rc_quotes_longer() {
    let words = split_words("echo 'it''s a test'", Format::Zsh);
    assert_eq!(words, vec!["echo".to_owned(), "it's a test".to_owned()]);
}

#[test]
fn operators() {
    let tests = [
        ("echo foo >| bar", WordbreakType::RedirectOutputForce, ">|"),
        (
            "echo foo >>| bar",
            WordbreakType::RedirectOutputAppendForce,
            ">>|",
        ),
        ("echo foo ;& bar", WordbreakType::ListFallthrough, ";&"),
        ("echo foo ;| bar", WordbreakType::ListFallthroughRetry, ";|"),
        ("echo foo &| bar", WordbreakType::ListAsyncErrcheck, "&|"),
        ("echo foo ;; bar", WordbreakType::ListSequentialDouble, ";;"),
    ];
    for (input, want_type, want_raw) in tests {
        let tokens = split(input, Format::Zsh).unwrap();
        let found = tokens
            .iter()
            .find(|t| t.kind == TokenType::Wordbreak && t.raw_value == want_raw)
            .unwrap_or_else(|| panic!("zsh operator {want_raw}: not found"));
        assert_eq!(found.wordbreak_type, want_type, "zsh operator {want_raw}");
    }
}

#[test]
fn force_redirect_is_redirect() {
    let ctx = complete("echo foo >| bar", Format::Zsh);
    assert!(ctx.is_redirect);
}

#[test]
fn force_append_redirect_is_redirect() {
    let ctx = complete("echo foo >>| bar", Format::Zsh);
    assert!(ctx.is_redirect);
}

#[test]
fn fallthrough_is_pipeline_delimiter() {
    assert_eq!(pipeline_count("echo foo ;& bar", Format::Zsh), 2);
}

#[test]
fn line_continuation_outside_quotes() {
    let words = split_words("echo foo\\\nbar", Format::Zsh);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_in_double_quotes() {
    let words = split_words("echo \"line1\\\nline2\"", Format::Zsh);
    assert_eq!(words, vec!["echo".to_owned(), "line1line2".to_owned()]);
}
