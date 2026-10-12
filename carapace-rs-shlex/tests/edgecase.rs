mod common;

use carapace_rs_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, quote_word, split,
};
use common::{pipeline_count, pipeline_words, split_words};

// Stop-parsing mode edge cases (PowerShell --%).

#[test]
fn stop_parsing_eof() {
    // EOF immediately after --% with no content — empty word merges with --%
    let words = split_words("echo --%", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "--%".to_owned()]);
}

#[test]
fn stop_parsing_newline_only() {
    // Newline after —% — newline is whitespace, next line continues in same pipeline
    let words = split_words("echo --%\nfoo", Format::Powershell);
    assert_eq!(
        words,
        vec!["echo".to_owned(), "--%".to_owned(), "foo".to_owned()]
    );
    assert_eq!(pipeline_count("echo --%\nfoo", Format::Powershell), 1);
}

#[test]
fn stop_parsing_pipe_in_double_quotes() {
    // After --%, | inside double quotes should NOT split the pipeline
    assert_eq!(
        pipeline_count(r#"echo --% "foo | bar" baz"#, Format::Powershell),
        1
    );
}

#[test]
fn stop_parsing_double_quote_toggling() {
    // After --%, double quotes toggle in-quotes state — raw content is one word
    let tokens = split(r#"echo --% "hello" world"#, Format::Powershell).unwrap();
    let words = tokens.words().strings();
    assert_eq!(words.len(), 3);
    assert_eq!(words[0], "echo");
    assert_eq!(words[1], "--%");
    assert_eq!(words[2], r#""hello" world"#);
}

// Block comment edge cases (PowerShell <# ... #>).

#[test]
fn block_comment_unclosed_eof() {
    let words = split_words("echo <# unclosed comment", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned()]);
}

#[test]
fn block_comment_hash_inside() {
    // A lone # inside a block comment does NOT close it — only #> closes
    let words = split_words("echo <# has # and # inside #> foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn block_comment_closer_restart() {
    // The closer #> starts with # — a lone # restarts the match but doesn't close
    let words = split_words("echo <# # not closer #> foo", Format::Powershell);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn block_comment_after_wordbreak() {
    // Block comment opener after a pipe — pipe splits, comment consumed
    assert_eq!(
        pipeline_count("echo foo |<# comment #> bar", Format::Powershell),
        2
    );
}

#[test]
fn block_comment_after_wordbreak_trailing_words() {
    // the words after the block comment must survive (a naive fix starts a
    // line comment that swallows the rest of the line)
    assert_eq!(
        pipeline_words("echo foo |<# comment #> bar", Format::Powershell),
        vec!["bar".to_owned()]
    );
}

#[test]
fn line_continuation_mid_word() {
    // Go stays in IN_WORD_STATE across `\<LF>` inside a word: the word
    // continues on the next line and a following space still splits
    assert_eq!(
        split_words(
            "foo\\
bar baz",
            Format::Bash
        ),
        vec!["foobar".to_owned(), "baz".to_owned()]
    );
}

#[test]
fn no_variables_for_cmd_and_powershell() {
    // Go's variableExpander interface: cmd and powershell do not implement
    // it, so `$name` stays a plain word
    let ctx = complete("echo $HO", Format::Cmd);
    assert!(ctx.variable.is_none());
    let ctx = complete("echo $HO", Format::Powershell);
    assert!(ctx.variable.is_none());
}

// Unicode / multi-byte character tests.

#[test]
fn unicode_cjk() {
    let tokens = split("echo 日本語", Format::Bash).unwrap();
    let words = tokens.words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "日本語".to_owned()]);
    // Span should be rune-based: "echo " is 5 runes, "日本語" is 3 runes
    assert_eq!(tokens[1].span.start, 5);
    assert_eq!(tokens[1].span.end, 8);
}

#[test]
fn unicode_accented() {
    let tokens = split("echo café", Format::Bash).unwrap();
    let words = tokens.words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "café".to_owned()]);
    assert_eq!(tokens[1].span.start, 5);
    assert_eq!(tokens[1].span.end, 9);
}

#[test]
fn unicode_in_quotes() {
    let words = split_words("echo \"日本語\"", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "日本語".to_owned()]);
}

#[test]
fn unicode_emoji() {
    let words = split_words("echo 🎉", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "🎉".to_owned()]);
}

#[test]
fn unicode_elvish_bareword() {
    // Elvish allows non-ASCII printable in barewords
    let words = split_words("echo naïve", Format::Elvish);
    assert_eq!(words, vec!["echo".to_owned(), "naïve".to_owned()]);
}

// Triple-quote opening "consumed" path (xonsh).

#[test]
fn triple_quote_opening_two_quotes_then_word() {
    // Two double quotes followed by a word — not a triple quote
    let words = split_words(r#"echo ""hello"#, Format::Xonsh);
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

#[test]
fn triple_quote_opening_two_quotes_eof() {
    // Two double quotes at EOF — not a triple quote, empty string
    let words = split_words(r#"echo """#, Format::Xonsh);
    assert_eq!(words, vec!["echo".to_owned(), String::new()]);
}

#[test]
fn triple_quote_opening_two_single_quotes_then_word() {
    let words = split_words("echo ''hello", Format::Xonsh);
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

// FilterRedirects fd prefix edge cases.

#[test]
fn filter_redirects_fd3() {
    let tokens = split("echo 3> file", Format::Bash).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned()]);
}

#[test]
fn filter_redirects_fd10() {
    let tokens = split("echo 10> file", Format::Bash).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned()]);
}

#[test]
fn filter_redirects_non_numeric_not_filtered() {
    let tokens = split("echo foo> file", Format::Bash).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

// WordbreakPrefix with multiple wordbreaks.

#[test]
fn wordbreak_prefix_multiple_equals() {
    let ctx = complete("echo foo=bar=baz", Format::Bash);
    assert_eq!(ctx.prefix, "foo=bar=");
}

#[test]
fn wordbreak_prefix_multiple_at() {
    let ctx = complete("echo foo@bar@baz", Format::Bash);
    assert_eq!(ctx.prefix, "foo@bar");
}

#[test]
fn wordbreak_prefix_mixed_equals_at() {
    let ctx = complete("echo foo=bar@baz", Format::Bash);
    assert_eq!(ctx.prefix, "foo=bar");
}

// Whitespace-only input.

#[test]
fn whitespace_only() {
    for input in ["   ", "\t\t", " \t \n"] {
        let tokens = split(input, Format::Bash).unwrap();
        assert_eq!(tokens.len(), 1, "{input}");
        assert_eq!(tokens[0].value, "", "{input}");
        // Completion context: one empty word (the current cursor position)
        let ctx = complete(input, Format::Bash);
        assert_eq!(ctx.quoting_state, LexerState::Start, "{input}");
        assert_eq!(ctx.words, vec![String::new()], "{input}");
    }
}

// Pipeline with no space after delimiter.

#[test]
fn pipe_no_space_after() {
    assert_eq!(pipeline_count("echo foo|grep", Format::Bash), 2);
}

#[test]
fn pipe_no_space_completion() {
    let ctx = complete("echo foo|grep bar", Format::Bash);
    assert_eq!(ctx.words, vec!["grep".to_owned(), "bar".to_owned()]);
}

#[test]
fn pipe_eof_completion() {
    let ctx = complete("echo foo |", Format::Bash);
    assert_eq!(ctx.current_word, "");
    assert_eq!(ctx.quoting_state, LexerState::Start);
}

// Comment edge cases.

#[test]
fn comment_at_eof() {
    let words = split_words("echo foo # comment", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn comment_then_newline() {
    let words = split_words("echo foo # comment\necho bar", Format::Bash);
    assert_eq!(
        words,
        vec![
            "echo".to_owned(),
            "foo".to_owned(),
            "echo".to_owned(),
            "bar".to_owned()
        ]
    );
}

#[test]
fn comment_at_start() {
    let words = split_words("# comment\necho foo", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn hash_in_double_quotes_not_comment() {
    let words = split_words(r#"echo "foo#bar""#, Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foo#bar".to_owned()]);
}

// Line continuation edge cases.

#[test]
fn line_continuation_at_eof() {
    // Dangling escape at EOF — escape char is dropped, word returned without it
    let words = split_words(r"echo foo\", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()]);
}

#[test]
fn line_continuation_joins() {
    let words = split_words("echo foo\\\nbar", Format::Bash);
    assert_eq!(words, vec!["echo".to_owned(), "foobar".to_owned()]);
}

#[test]
fn line_continuation_in_single_quotes_literal() {
    // Backslash-newline inside single quotes is literal (no line continuation)
    let words = split_words("echo 'foo\\\nbar'", Format::Bash);
    assert_eq!(words.len(), 2);
    assert_eq!(words[1], "foo\\\nbar");
}

// Empty token after wordbreak at EOF.

#[test]
fn empty_token_after_pipe_eof() {
    let tokens = split("echo foo |", Format::Bash).unwrap();
    assert!(tokens.len() >= 4);
    assert_eq!(tokens.last().unwrap().value, "");
}

#[test]
fn empty_token_after_redirect_eof() {
    let tokens = split("echo foo >", Format::Bash).unwrap();
    assert!(tokens.len() >= 4);
    assert_eq!(tokens.last().unwrap().value, "");
}

#[test]
fn empty_token_after_case_terminator_eof() {
    let tokens = split("echo foo ;;", Format::Bash).unwrap();
    assert!(tokens.len() >= 4);
    assert_eq!(tokens.last().unwrap().value, "");
}

// Nushell wordbreak type verification for ( and ).

#[test]
fn nushell_substitution_wordbreak_type() {
    let tokens = split("echo (ls)", Format::Nushell).unwrap();
    for tok in &tokens {
        if tok.kind == TokenType::Wordbreak && tok.raw_value == "(" {
            assert_eq!(tok.wordbreak_type, WordbreakType::SubstitutionOpen);
        }
        if tok.kind == TokenType::Wordbreak && tok.raw_value == ")" {
            assert_eq!(tok.wordbreak_type, WordbreakType::SubstitutionClose);
        }
    }
}

// posixQuoteWord edge cases (via quote_word with the default format).

#[test]
fn posix_quote_word_tab() {
    // Tab is emitted literally inside double quotes
    assert_eq!(
        quote_word("hello\tworld", Format::Default),
        "\"hello\tworld\""
    );
}

#[test]
fn posix_quote_word_cr() {
    // CR is emitted literally inside double quotes
    assert_eq!(
        format!("echo {}", quote_word("hello\rworld", Format::Default)),
        "echo \"hello\rworld\""
    );
}

#[test]
fn posix_quote_word_backtick() {
    assert_eq!(
        quote_word("hello`world", Format::Default),
        "\"hello\\`world\""
    );
}

// cmdQuoteWord edge cases.

#[test]
fn cmd_quote_word_caret() {
    // Caret triggers quoting but is literal inside double quotes
    assert_eq!(quote_word("hello^world", Format::Cmd), "\"hello^world\"");
}

#[test]
fn cmd_quote_word_multiple_quotes() {
    assert_eq!(quote_word(r#"a"b"c"#, Format::Cmd), "\"a\"^\"b\"^\"c\"");
}
