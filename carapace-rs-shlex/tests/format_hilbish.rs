mod common;

use carapace_rs_shlex::{
    Format, LexerState, TokenSliceExt, TokenSliceInternals, TokenType, WordbreakType, complete,
    join, quote_word, split,
};
use common::{pipeline_count, split_words};

fn words_of(input: &str) -> Vec<String> {
    split_words(input, Format::Hilbish)
}

fn wordbreak_of(input: &str, raw: &str) -> (TokenType, WordbreakType) {
    let tokens = split(input, Format::Hilbish).unwrap();
    let found = tokens
        .iter()
        .rfind(|t| t.raw_value == raw)
        .unwrap_or_else(|| panic!("{input}: no {raw} token"));
    (found.kind, found.wordbreak_type)
}

#[test]
fn basic_pipe() {
    assert_eq!(pipeline_count("echo foo | grep bar", Format::Hilbish), 2);
}

#[test]
fn single_quote_literal() {
    assert_eq!(words_of("echo '$HOME'"), vec!["echo", "$HOME"]);
}

#[test]
fn backtick_literal_in_single_quotes() {
    assert_eq!(words_of("echo '`cmd`'"), vec!["echo", "`cmd`"]);
}

#[test]
fn escaped_double_quote_outside() {
    assert_eq!(words_of(r#"echo \"hello\""#), vec!["echo", "\"hello\""]);
}

#[test]
fn open_single_quote() {
    let tokens = split("echo 'hel", Format::Hilbish).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Quoting);
}

#[test]
fn comment() {
    assert_eq!(words_of("echo foo # bar"), vec!["echo", "foo"]);
}

#[test]
fn pipe_with_stderr() {
    assert_eq!(
        wordbreak_of("echo foo |& grep bar", "|&"),
        (TokenType::Wordbreak, WordbreakType::PipeWithStderr)
    );
    assert_eq!(pipeline_count("echo foo |& grep bar", Format::Hilbish), 2);
}

#[test]
fn redirect_both() {
    assert_eq!(
        wordbreak_of("echo foo &> /tmp/bar", "&>"),
        (TokenType::Wordbreak, WordbreakType::RedirectOutputBoth)
    );
}

#[test]
fn here_doc() {
    assert_eq!(
        wordbreak_of("cat << EOF", "<<"),
        (TokenType::Wordbreak, WordbreakType::RedirectHereDoc)
    );
}

#[test]
fn here_string() {
    assert_eq!(
        wordbreak_of("cat <<< foo", "<<<"),
        (TokenType::Wordbreak, WordbreakType::RedirectInputString)
    );
}

#[test]
fn case_list() {
    assert_eq!(
        wordbreak_of("case foo in a) echo one ;; b) echo two ;; esac", ";;"),
        (TokenType::Wordbreak, WordbreakType::ListSequentialDouble)
    );
}

#[test]
fn substitution() {
    assert_eq!(
        wordbreak_of("echo $(date)", "$("),
        (TokenType::Wordbreak, WordbreakType::SubstitutionOpen)
    );
}

#[test]
fn variable() {
    let ctx = complete(r#"echo "text$HO"#, Format::Hilbish);
    assert_eq!(ctx.variable.as_ref().map(|v| v.name.as_str()), Some("HO"));
}

#[test]
fn line_continuation() {
    assert_eq!(words_of("echo foo\\\nbar"), vec!["echo", "foobar"]);
}

#[test]
fn join_hilbish() {
    let words = vec!["echo".to_owned(), "hello world".to_owned()];
    assert_eq!(join(&words, Format::Hilbish), r"echo hello\ world");
}

#[test]
fn join_roundtrip() {
    let words = vec![
        "echo".to_owned(),
        r#"it's "tricky"; really"#.to_owned(),
        "foo|bar".to_owned(),
        "a\tb".to_owned(),
    ];
    let joined = join(&words, Format::Hilbish);
    let got = split(&joined, Format::Hilbish).unwrap().words().strings();
    assert_eq!(got, words);
}

#[test]
fn adjacent_quote_segments() {
    let words = words_of(r#"echo a''b "foo"bar"#);
    assert_eq!(words, vec!["echo", "ab", "foobar"]);
}

#[test]
fn empty_quotes() {
    assert_eq!(words_of(r#"echo """#), vec!["echo", ""]);
}

#[test]
fn open_double_quote_at_eof() {
    let tokens = split(r#"echo "hel"#, Format::Hilbish).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::QuotingEscaping);
}

#[test]
fn escape_at_eof() {
    let tokens = split(r"echo hel\", Format::Hilbish).unwrap();
    let last = tokens.words().current_token().unwrap();
    assert_eq!(last.state, LexerState::Escaping);
}

#[test]
fn comment_only_at_word_start() {
    // # only starts a comment at word start
    assert_eq!(words_of("echo a#b #c"), vec!["echo", "a#b"]);
}

#[test]
fn redirect_filter_numeric_fd() {
    let tokens = split("echo foo 2>&1 | grep -", Format::Hilbish).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert_eq!(words, vec!["grep".to_owned(), "-".to_owned()]);
}

#[test]
fn variable_detection() {
    for (input, name) in [
        (r#"echo "text$HO"#, "HO"),
        ("echo ${HO", "HO"),
        ("echo $HO", "HO"),
    ] {
        let ctx = complete(input, Format::Hilbish);
        assert_eq!(
            ctx.variable.as_ref().map(|v| v.name.as_str()),
            Some(name),
            "{input}"
        );
    }
}

#[test]
fn variable_literal() {
    for input in [r"echo '$HO'", r"echo \$HO"] {
        let ctx = complete(input, Format::Hilbish);
        assert!(ctx.variable.is_none(), "{input}");
    }
}

#[test]
fn span_rune_offsets() {
    let ctx = complete("echo café", Format::Hilbish);
    // "echo " is 5 runes; café starts at rune offset 5, length 4
    assert_eq!(ctx.span.start, 5);
    assert_eq!(ctx.span.end, 9);
}

#[test]
fn crlf_line_continuation() {
    assert_eq!(words_of("echo foo\\\r\nbar"), vec!["echo", "foobar"]);
}

#[test]
fn modifier_words() {
    assert_eq!(words_of("@priv echo hi"), vec!["@priv", "echo", "hi"]);
}

// Real-world command lines from hilbish docs, hilbish's own completion
// tests, and community configs.

#[test]
fn modifier_with_quoted_arg() {
    let ctx = complete(
        r#"@priv curl -H "Authorization: Bearer secret" https://example.com"#,
        Format::Hilbish,
    );
    assert_eq!(ctx.words.len(), 5);
    assert_eq!(ctx.words[3], "Authorization: Bearer secret");
    assert_eq!(ctx.words[4], "https://example.com");
}

#[test]
fn equals_not_a_wordbreak() {
    // Unlike bash, = is not a wordbreak for hilbish: long flags stay one
    // word and get no wordbreak prefix.
    let ctx = complete("bat --paging=never --style=plain file.txt", Format::Hilbish);
    assert_eq!(ctx.words.len(), 4);
    assert_eq!(ctx.words[1], "--paging=never");
    assert_eq!(ctx.words[2], "--style=plain");
    assert_eq!(ctx.prefix, "");
}

#[test]
fn redirect_without_space() {
    let ctx = complete("chafa --view-size=80x25 pic.jpg >pic.bin", Format::Hilbish);
    assert!(ctx.is_redirect);
    assert_eq!(ctx.current_word, "pic.bin");
    assert_eq!(ctx.prefix, ">");
}

#[test]
fn escaped_unicode_filename() {
    // hilbish's own completion tests offer filenames like
    // [2021.07.14] ツユ 2ndアルバム as escaped barewords with \[ \] \ .
    let raw = r#"cd \[2021.07.14\]\ ツユ\ 2ndアルバム"#;
    let words = words_of(raw);
    assert_eq!(words.len(), 2);
    assert_eq!(words[1], "[2021.07.14] ツユ 2ndアルバム");
    let joined = join(&words, Format::Hilbish);
    assert_eq!(joined, raw);
}

#[test]
fn variable_in_flag_value() {
    let ctx = complete("nix-shell --command $SHELL", Format::Hilbish);
    assert_eq!(
        ctx.variable.as_ref().map(|v| v.name.as_str()),
        Some("SHELL")
    );
}

#[test]
fn pipeline_from_alias_body() {
    // Alias bodies like 'ls %1 | wc -l': % placeholders are plain word
    // chars, pipeline splits before wc.
    let ctx = complete("git status --porcelain | wc -l", Format::Hilbish);
    assert_eq!(ctx.words, vec!["wc".to_owned(), "-l".to_owned()]);
}

#[test]
fn quote_word_matches_join() {
    assert_eq!(quote_word("hello world", Format::Hilbish), r"hello\ world");
}
