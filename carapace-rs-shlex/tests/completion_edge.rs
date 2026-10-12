//! Port of Go `completion_edge_test.go`.

use carapace_rs_shlex::{Format, LexerState, complete};

#[test]
fn multiple_wordbreaks() {
    let ctx = complete("echo a=b=c", Format::Bash);
    assert_eq!(ctx.current_word, "a=b=c");
    assert_eq!(ctx.prefix, "a=b=");
}

#[test]
fn trailing_wordbreak() {
    let ctx = complete("echo a=b=", Format::Bash);
    assert_eq!(ctx.current_word, "a=b=");
    assert_eq!(ctx.prefix, "a=b=");
}

#[test]
fn wordbreak_with_space() {
    let ctx = complete("echo a= b=c", Format::Bash);
    assert_eq!(ctx.current_word, "b=c");
    assert_eq!(ctx.prefix, "b=");
}

#[test]
fn multiple_at_wordbreaks() {
    // all @ should be skipped
    let ctx = complete("echo foo@@bar", Format::Bash);
    assert_eq!(ctx.prefix, "foo");
}

#[test]
fn at_after_other_wordbreak() {
    // @ should be skipped, but : is a regular wordbreak and included
    let ctx = complete("echo foo@:bar", Format::Bash);
    assert_eq!(ctx.prefix, "foo:");
}

#[test]
fn colon_wordbreak() {
    let ctx = complete("echo foo:bar", Format::Bash);
    assert_eq!(ctx.prefix, "foo:");
}

#[test]
fn fd_prefix() {
    let ctx = complete("echo 2> file", Format::Bash);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "file");
}

#[test]
fn multi_digit_fd() {
    let ctx = complete("echo 10> file", Format::Bash);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "file");
}

#[test]
fn fd_redirect_no_space() {
    let ctx = complete("echo 2>file", Format::Bash);
    assert!(ctx.is_redirect);
}

#[test]
fn redirect_at_eof() {
    let ctx = complete("echo foo >", Format::Bash);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "");
}

#[test]
fn fd_redirect_at_eof() {
    let ctx = complete("echo 2>", Format::Bash);
    assert!(ctx.is_redirect);
}

#[test]
fn fd_duplication() {
    let ctx = complete("echo 2>&1", Format::Bash);
    assert!(ctx.is_redirect);
}

#[test]
fn wordbreak_before_double_quote() {
    let ctx = complete("echo foo=\"bar", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::QuotingEscaping);
    assert_eq!(ctx.prefix, "foo=");
}

#[test]
fn wordbreak_before_single_quote() {
    let ctx = complete("echo foo='bar", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::Quoting);
    assert_eq!(ctx.prefix, "foo=");
}

#[test]
fn mid_word_quote_opening() {
    let ctx = complete("echo foo=bar'baz", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::Quoting);
    assert_eq!(ctx.prefix, "foo=bar");
}

#[test]
fn wordbreak_char_inside_quotes() {
    // = inside quotes is not a wordbreak
    let ctx = complete("echo \"foo=bar", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::QuotingEscaping);
    assert_eq!(ctx.prefix, "");
}

#[test]
fn comment_only_input() {
    let ctx = complete("# comment", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::Start);
    assert!(ctx.words.is_empty());
}

#[test]
fn whitespace_only_input() {
    let ctx = complete("   ", Format::Bash);
    assert_eq!(ctx.quoting_state, LexerState::Start);
    assert_eq!(ctx.current_word, "");
}

#[test]
fn bare_redirect() {
    let ctx = complete(">", Format::Bash);
    assert!(ctx.is_redirect);
}

#[test]
fn bare_pipe() {
    let ctx = complete("|", Format::Bash);
    assert_eq!(ctx.current_word, "");
}

#[test]
fn at_between_words_then_wordbreak() {
    // user@host:bar: @ between two WORD tokens should be in prefix
    let ctx = complete("echo user@host:bar", Format::Bash);
    assert_eq!(ctx.prefix, "user@host:");
}

#[test]
fn at_after_wordbreak_then_word() {
    // foo:@bar: @ adjacent to : wordbreak should be skipped
    let ctx = complete("echo foo:@bar", Format::Bash);
    assert_eq!(ctx.prefix, "foo:");
}

#[test]
fn at_between_wordbreak_and_word() {
    // foo:=@bar: @ adjacent to : (wordbreak side) should be skipped
    let ctx = complete("echo foo:=@bar", Format::Bash);
    assert_eq!(ctx.prefix, "foo:=");
}
