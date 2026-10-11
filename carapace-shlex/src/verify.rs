//! Verification tests cross-checking the lexer against shell source
//! constants (ports of `verify_*_test.go`). The authoritative sources
//! per shell are documented in the reference repository; each test
//! names the constant or function it verifies.

use super::format::FormatImpl;
use super::token::RuneClass;
use super::{Format, TokenSliceExt, TokenType, WordbreakType, complete, split};

// --- bash (verify_bash_test.go) ---

#[test]
fn bash_meta_chars() {
    // syntax.h: shell_meta_chars = "()<>;&|"
    let classifier = super::formats::BashFormat.classifier();
    for r in "()<>;&|".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "meta char {r}"
        );
    }
}

#[test]
fn bash_break_chars() {
    // syntax.h: shell_break_chars = "()<>;&| \t\n"
    let classifier = super::formats::BashFormat.classifier();
    for r in "()<>;&| \t\n".chars() {
        let got = classifier.classify(r);
        if " \t\n".contains(r) {
            assert_eq!(got, RuneClass::Space, "break char {r}");
        } else {
            assert_eq!(got, RuneClass::Wordbreak, "break char {r}");
        }
    }
}

#[test]
fn bash_quote_chars() {
    // syntax.h: shell_quote_chars = "\"`'"
    let classifier = super::formats::BashFormat.classifier();
    assert_eq!(classifier.classify('"'), RuneClass::EscapingQuote);
    assert_eq!(classifier.classify('\''), RuneClass::NonEscapingQuote);
}

#[test]
fn bash_slashify_in_quotes() {
    // syntax.h: slashify_in_quotes = "\\`$\"\n" (the CBSDQUOTE set)
    let chars = super::formats::BashFormat.escaping_quote_escape_chars();
    for r in "\\`$\"\n".chars() {
        assert!(chars.contains(&r), "CBSDQUOTE missing {r:?}");
    }
    for r in "abc123".chars() {
        assert!(!chars.contains(&r), "CBSDQUOTE should not contain {r:?}");
    }
}

#[test]
fn bash_completion_word_breaks() {
    // bashline.c: bash_completer_word_break_characters = " \t\n\"'@><=;|&(:"
    // shlex's BASH_WORDBREAKS adds \r and both parens.
    let classifier = super::formats::BashFormat.classifier();
    for r in " \t\n\"'@><=;|&(: ".chars() {
        let got = classifier.classify(r);
        match r {
            ' ' | '\t' | '\n' | '\r' => assert_eq!(got, RuneClass::Space, "{r:?}"),
            '"' => assert_eq!(got, RuneClass::EscapingQuote, "{r:?}"),
            '\'' => assert_eq!(got, RuneClass::NonEscapingQuote, "{r:?}"),
            _ => assert_eq!(got, RuneClass::Wordbreak, "{r:?}"),
        }
    }
}

#[test]
fn bash_operator_grammar() {
    // parse.y operator tokenization
    let tests = [
        ("|", WordbreakType::Pipe),
        ("||", WordbreakType::ListOr),
        ("&", WordbreakType::ListAsync),
        ("&&", WordbreakType::ListAnd),
        (";", WordbreakType::ListSequential),
        (";;", WordbreakType::ListSequentialDouble),
        (";&", WordbreakType::ListFallthrough),
        (";;&", WordbreakType::ListCaseNext),
        (">", WordbreakType::RedirectOutput),
        (">>", WordbreakType::RedirectOutputAppend),
        (">|", WordbreakType::RedirectOutputForce),
        ("<<", WordbreakType::RedirectHereDoc),
        ("<<<", WordbreakType::RedirectInputString),
        ("<", WordbreakType::RedirectInput),
        ("<&", WordbreakType::RedirectInputDuplicate),
        ("<>", WordbreakType::RedirectInputOutput),
        ("&>", WordbreakType::RedirectOutputBoth),
        ("&>>", WordbreakType::RedirectOutputBothAppend),
        ("|&", WordbreakType::PipeWithStderr),
    ];
    for (input, want) in tests {
        let tokens = split(&format!("echo {input} foo"), Format::Bash).unwrap();
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == TokenType::Wordbreak && t.wordbreak_type == want),
            "operator {input}: expected {want:?}"
        );
    }
}

#[test]
fn bash_check_redir_redirects() {
    // bashline.c check_redir: >& <& >| are redirects, not pipeline delimiters
    for op in [">&", "<&", ">|"] {
        let wb = super::wordbreak::bash_wordbreak_type(op);
        assert!(wb.is_redirect(), "{op} should be a redirect");
        assert!(
            !wb.is_pipeline_delimiter(),
            "{op} should not split pipelines"
        );
    }
}

#[test]
fn bash_pipeline_delimiters() {
    for op in ["|", "||", "&", "&&", ";", ";;", ";&", ";;&", "|&"] {
        assert!(
            super::wordbreak::bash_wordbreak_type(op).is_pipeline_delimiter(),
            "{op} should be a pipeline delimiter"
        );
    }
}

#[test]
fn bash_redirect_operators() {
    for op in ["<", ">", ">>", ">|", "<<", "<<<", "<&", "<>", "&>", "&>>"] {
        assert!(
            super::wordbreak::bash_wordbreak_type(op).is_redirect(),
            "{op} should be a redirect"
        );
    }
}

#[test]
fn bash_double_quote_backslash_escaping() {
    // syntax.h slashify_in_quotes + lib/sh/shquote.c sh_backslash_quote:
    // \ only escapes \ ` $ " \n inside double quotes.
    for r in ['\\', '`', '$', '"'] {
        let input = format!("echo \"\\{r}\"");
        let tokens = split(&input, Format::Bash).unwrap();
        let last = tokens.words().current_token().unwrap();
        assert_eq!(last.value, r.to_string(), "escaped {r:?}");
    }
    // Line continuation: both \ and \n removed
    let tokens = split("echo \"\\\n\"", Format::Bash).unwrap();
    assert_eq!(tokens.words().current_token().unwrap().value, "");

    // Non-escapable chars keep both \ and the char
    for r in ['a', 'b', 'c', '1', '2', '3'] {
        let input = format!("echo \"\\{r}\"");
        let tokens = split(&input, Format::Bash).unwrap();
        let last = tokens.words().current_token().unwrap();
        assert_eq!(last.value, format!("\\{r}"), "non-escaped {r:?}");
    }
}

#[test]
fn bash_single_quote_no_escape() {
    for input in [r"echo 'hello\nworld'", r"echo '$HOME'", "echo '`cmd`'"] {
        let tokens = split(input, Format::Bash).unwrap();
        let value = tokens.words().current_token().unwrap().value.clone();
        let expected = input
            .trim_start_matches("echo ")
            .trim_matches('\'')
            .to_owned();
        assert_eq!(value, expected, "{input}");
    }
}

#[test]
fn bash_line_continuation() {
    for (input, want) in [
        ("echo foo\\\nbar", "foobar"),
        ("echo \"foo\\\nbar\"", "foobar"),
        ("echo foo\\\r\nbar", "foobar"),
    ] {
        let tokens = split(input, Format::Bash).unwrap();
        let words = tokens.words().strings();
        assert_eq!(words, vec!["echo".to_owned(), want.to_owned()], "{input}");
    }
}

#[test]
fn bash_comment() {
    let tokens = split("echo hello # this is a comment", Format::Bash).unwrap();
    let words = tokens.words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

#[test]
fn bash_quote_word() {
    // lib/sh/shquote.c sh_backslash_quote
    let tests = [
        ("hello", "hello"),
        ("abc123", "abc123"),
        ("hello world", "\"hello world\""),
        (r#"with"quote"#, "\"with\\\"quote\""),
        (r"with\backslash", "\"with\\\\backslash\""),
        (r"with$dollar", "\"with\\$dollar\""),
        ("with`backtick", "\"with\\`backtick\""),
        ("with\nnewline", "\"with\\\nnewline\""),
        ("", "\"\""),
    ];
    for (input, want) in tests {
        assert_eq!(super::quote::posix_quote_word(input), want, "{input:?}");
    }
}

#[test]
fn bash_command_separators() {
    for sep in [";", "|", "&"] {
        let input = format!("echo foo{sep} bar");
        let tokens = split(&input, Format::Bash).unwrap();
        assert!(tokens.pipelines().len() >= 2, "{sep}");
    }
}

#[test]
fn bash_fd_prefix_in_redirects() {
    let tokens = split("echo 2> file", Format::Bash).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert!(!words.contains(&"2".to_owned()), "{words:?}");
}

#[test]
fn bash_process_substitution() {
    for (input, expect_open) in [
        ("echo <(", true),
        ("echo >(", true),
        ("echo <(cat) )", false),
    ] {
        let tokens = split(input, Format::Bash).unwrap();
        let found = tokens.iter().any(|t| match expect_open {
            true => t.wordbreak_type == WordbreakType::SubstitutionOpen,
            false => t.wordbreak_type == WordbreakType::SubstitutionClose,
        });
        assert!(found, "process substitution {input}");
    }
}

#[test]
fn bash_command_substitution() {
    let tokens = split("echo $(ls)", Format::Bash).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.value == "$(")
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );
}

#[test]
fn bash_arithmetic_expansion() {
    let tokens = split("echo $((1+2))", Format::Bash).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen
                && t.raw_value.ends_with("((")),
        "$(( not detected as arithmetic opener"
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose
                && t.raw_value.starts_with("))")),
        ")) not detected as arithmetic closer"
    );
}

#[test]
fn bash_dollar_not_wordbreak() {
    let classifier = super::formats::BashFormat.classifier();
    assert_ne!(classifier.classify('$'), RuneClass::Wordbreak);
}

#[test]
fn bash_glob_chars_not_wordbreak() {
    let classifier = super::formats::BashFormat.classifier();
    for r in "*?[]^".chars() {
        assert_ne!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "glob char {r}"
        );
    }
}

#[test]
fn bash_ext_glob_chars_not_shell_break() {
    let classifier = super::formats::BashFormat.classifier();
    assert_eq!(classifier.classify('@'), RuneClass::Wordbreak);
    for r in "+!".chars() {
        assert_ne!(classifier.classify(r), RuneClass::Wordbreak, "ext glob {r}");
    }
}

// --- fish (verify_fish_test.go) ---

#[test]
fn fish_word_break_chars() {
    // tokenizer.rs tok_is_string_character: unconditional separators
    let classifier = super::formats::FishFormat.classifier();
    for r in "|;<>".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "separator {r}"
        );
    }
    for r in " \t\r\n".chars() {
        assert_eq!(classifier.classify(r), RuneClass::Space, "whitespace {r}");
    }
}

#[test]
fn fish_string_characters() {
    let classifier = super::formats::FishFormat.classifier();
    // '?' is a string character in fish but classified as wordbreak for
    // noclobber operators; () for command substitution — deliberate.
    for r in "?()".chars() {
        assert_eq!(classifier.classify(r), RuneClass::Wordbreak, "{r}");
    }
    for r in "abcXYZ012_-.@/%+".chars() {
        assert_ne!(classifier.classify(r), RuneClass::Wordbreak, "{r}");
    }
}

#[test]
fn fish_quote_escape_comment_chars() {
    let classifier = super::formats::FishFormat.classifier();
    assert_eq!(classifier.classify('"'), RuneClass::EscapingQuote);
    assert_eq!(classifier.classify('\''), RuneClass::NonEscapingQuote);
    assert_eq!(classifier.classify('\\'), RuneClass::Escape);
    assert_eq!(classifier.classify('#'), RuneClass::Comment);
}

#[test]
fn fish_single_quote_escapes() {
    // Only \' and \\ are escapes; other \X keep both characters
    let words = |input: &str| {
        split(input, Format::Fish)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words("echo 'it\\'s'"), "it's");
    assert_eq!(words(r"echo 'C:\\path'"), r"C:\path");
    assert_eq!(words(r"echo 'cost: \$5'"), r"cost: \$5");
    assert_eq!(words(r"echo 'hello\nworld'"), r"hello\nworld");
}

#[test]
fn fish_double_quote_escape_chars() {
    // Inside "..." only \" \$ \\ and \<newline> are escapes
    let chars = super::formats::FishFormat.escaping_quote_escape_chars();
    for r in "\"$\\\n".chars() {
        assert!(chars.contains(&r), "fish escape set missing {r:?}");
    }
    for r in "abc123`".chars() {
        assert!(
            !chars.contains(&r),
            "fish escape set should not contain {r:?}"
        );
    }
}

#[test]
fn fish_double_quote_escaping() {
    let words = |input: &str| {
        split(input, Format::Fish)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words(r#"echo "say \"hello\"""#), r#"say "hello""#);
    assert_eq!(words(r#"echo "cost: \$5""#), "cost: $5");
    assert_eq!(words(r#"echo "C:\\path""#), r"C:\path");
    assert_eq!(words(r#"echo "hello\nworld""#), r"hello\nworld");
    assert_eq!(words(r#"echo "a\tb""#), r"a\tb");
}

#[test]
fn fish_line_continuation() {
    for (input, want) in [
        ("echo foo\\\nbar", "foobar"),
        ("echo \"foo\\\nbar\"", "foobar"),
        ("echo foo\\\r\nbar", "foobar"),
    ] {
        let tokens = split(input, Format::Fish).unwrap();
        let words = tokens.words().strings();
        assert_eq!(words, vec!["echo".to_owned(), want.to_owned()], "{input}");
    }
}

#[test]
fn fish_operator_grammar() {
    // tokenizer.rs PipeOrRedir::try_from; >| is a PIPE in fish (not a redirect)
    let tests = [
        ("|", WordbreakType::Pipe, true, false),
        ("|&", WordbreakType::PipeWithStderr, true, false),
        ("&|", WordbreakType::PipeWithStderr, true, false),
        (">|", WordbreakType::Pipe, true, false),
        ("&&", WordbreakType::ListAnd, true, false),
        ("||", WordbreakType::ListOr, true, false),
        ("&", WordbreakType::ListAsync, true, false),
        (";", WordbreakType::ListSequential, true, false),
        (">", WordbreakType::RedirectOutput, false, true),
        (">>", WordbreakType::RedirectOutputAppend, false, true),
        (">?", WordbreakType::RedirectOutput, false, true),
        (">>?", WordbreakType::RedirectOutputAppend, false, true),
        ("&>", WordbreakType::RedirectOutputBoth, false, true),
        ("&>>", WordbreakType::RedirectOutputBothAppend, false, true),
        ("&>?", WordbreakType::RedirectOutputBoth, false, true),
        ("&>>?", WordbreakType::RedirectOutputBothAppend, false, true),
        ("<", WordbreakType::RedirectInput, false, true),
        ("<?", WordbreakType::RedirectInput, false, true),
        ("<>", WordbreakType::RedirectInputOutput, false, true),
        (">&", WordbreakType::RedirectInputDuplicate, false, true),
    ];
    for (input, want, is_pipe, is_redir) in tests {
        let got = super::formats::FishFormat.classify_operator(input);
        assert_eq!(got, want, "ClassifyOperator({input})");
        assert_eq!(got.is_pipeline_delimiter(), is_pipe, "{input}");
        assert_eq!(got.is_redirect(), is_redir, "{input}");
    }
}

#[test]
fn fish_keyword_operators() {
    // parser_keywords.rs: "and" and "or" split pipelines; "not" and "!" don't
    let ops = super::formats::FishFormat.keyword_operators();
    assert!(
        ops.iter()
            .any(|(w, t)| *w == "and" && *t == WordbreakType::ListAnd)
    );
    assert!(
        ops.iter()
            .any(|(w, t)| *w == "or" && *t == WordbreakType::ListOr)
    );
    assert!(!ops.iter().any(|(w, _)| *w == "not"));
    assert!(!ops.iter().any(|(w, _)| *w == "!"));

    assert_eq!(
        split("echo foo and echo bar", Format::Fish)
            .unwrap()
            .pipelines()
            .len(),
        2
    );
    assert_eq!(
        split("echo foo or echo bar", Format::Fish)
            .unwrap()
            .pipelines()
            .len(),
        2
    );
    assert_eq!(
        split("echo foo not echo bar", Format::Fish)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
}

#[test]
fn fish_command_substitution() {
    // fish uses bare () for command substitution
    let tokens = split("echo (ls)", Format::Fish).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen)
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );
    assert_eq!(
        split("echo (ls | grep foo)", Format::Fish)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
}

#[test]
fn fish_quote_word() {
    let tests = [
        ("hello", "hello"),
        ("abc123", "abc123"),
        ("hello-world", "hello-world"),
        ("hello world", "\"hello world\""),
        (r#"with"quote"#, "\"with\\\"quote\""),
        (r"with\backslash", "\"with\\\\backslash\""),
        (r"with$dollar", "\"with\\$dollar\""),
        // Backtick is NOT special in fish (unlike bash)
        ("hello`world", "hello`world"),
        ("", "\"\""),
    ];
    for (input, want) in tests {
        assert_eq!(super::quote::fish_quote_word(input), want, "{input:?}");
    }
}

#[test]
fn fish_no_word_splitting_and_escape_outside() {
    // spaces inside double quotes don't split words
    let words = split(r#"echo "hello world""#, Format::Fish)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
    // escaped space joins words; escaped | and > stay literal
    let words = split(r"echo a\ b", Format::Fish).unwrap().words().strings();
    assert_eq!(words, vec!["echo".to_owned(), "a b".to_owned()]);
    let words = split(r"echo foo\|bar", Format::Fish)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "foo|bar".to_owned()]);
    let words = split(r"echo foo\>bar", Format::Fish)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "foo>bar".to_owned()]);
}

#[test]
fn fish_comment() {
    let words = split("echo hello # this is a comment", Format::Fish)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

// --- zsh (verify_zsh_test.go) ---

#[test]
fn zsh_word_break_and_quote_chars() {
    let classifier = super::formats::ZshFormat.classifier();
    for r in "()<>;&|".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "zsh separator {r}"
        );
    }
    for r in " \t\r\n".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Space,
            "zsh whitespace {r}"
        );
    }
    assert_eq!(classifier.classify('"'), RuneClass::EscapingQuote);
    assert_eq!(classifier.classify('\''), RuneClass::NonEscapingQuote);
}

#[test]
fn zsh_rc_quotes() {
    // lex.c gettokstr LX2_QUOTE + RCQUOTES option (set by default)
    assert!(super::formats::ZshFormat.non_escaping_quote_escapes());
    let words = |input: &str| split(input, Format::Zsh).unwrap().words().strings();
    assert_eq!(
        words("echo 'it''s'"),
        vec!["echo".to_owned(), "it's".to_owned()]
    );
    assert_eq!(
        words("echo '''hello'''"),
        vec!["echo".to_owned(), "'hello'".to_owned()]
    );
    assert_eq!(words("echo ''"), vec!["echo".to_owned(), String::new()]);
}

#[test]
fn zsh_single_quote_no_backslash_escape() {
    assert!(!super::formats::ZshFormat.non_escaping_quote_backslash_escapes());
    let words = |input: &str| {
        split(input, Format::Zsh)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words(r"echo 'hello\nworld'"), r"hello\nworld");
    assert_eq!(words(r"echo '$HOME'"), "$HOME");
    assert_eq!(words("echo '`cmd`'"), "`cmd`");
}

#[test]
fn zsh_double_quote_escape_chars() {
    // lex.c dquote_parse: \ escapes $ \ " ` and \n (CBSDQUOTE, same as bash)
    let chars = super::formats::ZshFormat.escaping_quote_escape_chars();
    for r in "\\`$\"\n".chars() {
        assert!(chars.contains(&r), "zsh CBSDQUOTE missing {r:?}");
    }
    for r in "abc123".chars() {
        assert!(
            !chars.contains(&r),
            "zsh CBSDQUOTE should not contain {r:?}"
        );
    }
}

#[test]
fn zsh_double_quote_escaping() {
    let words = |input: &str| {
        split(input, Format::Zsh)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words(r#"echo "say \"hello\"""#), r#"say "hello""#);
    assert_eq!(words(r#"echo "cost: \$5""#), "cost: $5");
    assert_eq!(words(r#"echo "C:\\path""#), r"C:\path");
    assert_eq!(words("echo \"cmd \\`whoami\\`\""), "cmd `whoami`");
    assert_eq!(words(r#"echo "hello\nworld""#), r"hello\nworld");
    assert_eq!(words(r#"echo "a\tb""#), r"a\tb");
}

#[test]
fn zsh_line_continuation_and_comment() {
    for (input, want) in [
        ("echo foo\\\nbar", "foobar"),
        ("echo \"foo\\\nbar\"", "foobar"),
        ("echo foo\\\r\nbar", "foobar"),
    ] {
        let words = split(input, Format::Zsh).unwrap().words().strings();
        assert_eq!(words, vec!["echo".to_owned(), want.to_owned()], "{input}");
    }
    let words = split("echo hello # this is a comment", Format::Zsh)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

#[test]
fn zsh_operator_grammar() {
    // lex.c tokstrings[]
    let tests = [
        ("|", WordbreakType::Pipe),
        ("||", WordbreakType::ListOr),
        ("&", WordbreakType::ListAsync),
        ("&&", WordbreakType::ListAnd),
        (";", WordbreakType::ListSequential),
        (";;", WordbreakType::ListSequentialDouble),
        (";&", WordbreakType::ListFallthrough),
        (";|", WordbreakType::ListFallthroughRetry),
        ("&|", WordbreakType::ListAsyncErrcheck),
        ("|&", WordbreakType::PipeWithStderr),
        (">", WordbreakType::RedirectOutput),
        (">>", WordbreakType::RedirectOutputAppend),
        (">|", WordbreakType::RedirectOutputForce),
        (">>|", WordbreakType::RedirectOutputAppendForce),
        ("<", WordbreakType::RedirectInput),
        ("<<", WordbreakType::RedirectHereDoc),
        ("<<<", WordbreakType::RedirectInputString),
        ("<&", WordbreakType::RedirectInputDuplicate),
        ("<>", WordbreakType::RedirectInputOutput),
        (">&", WordbreakType::RedirectOutputBoth),
        ("&>", WordbreakType::RedirectOutputBoth),
        ("&>>", WordbreakType::RedirectOutputBothAppend),
    ];
    for (input, want) in tests {
        let tokens = split(&format!("echo {input} foo"), Format::Zsh).unwrap();
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == TokenType::Wordbreak && t.wordbreak_type == want),
            "zsh operator {input}: expected {want:?}"
        );
    }
}

#[test]
fn zsh_pipeline_and_redirect_delimiters() {
    for op in ["|", "||", "&", "&&", ";", ";;", ";&", ";|", "&|", "|&"] {
        assert!(
            super::formats::ZshFormat
                .classify_operator(op)
                .is_pipeline_delimiter(),
            "zsh {op} should be a pipeline delimiter"
        );
    }
    for op in [
        "<", ">", ">>", ">|", ">>|", "<<", "<<<", "<&", "<>", ">&", "&>", "&>>",
    ] {
        let wb = super::formats::ZshFormat.classify_operator(op);
        assert!(wb.is_redirect(), "zsh {op} should be a redirect");
        assert!(
            !wb.is_pipeline_delimiter(),
            "zsh {op} should not split pipelines"
        );
    }
}

#[test]
fn zsh_missing_operators_known_limitation() {
    // <<- &>| >>& >>&| are not recognized as single operators (known
    // limitation of the flat state machine, shared with Go)
    for op in ["<<-", "&>|", ">>&", ">>&|"] {
        assert_eq!(
            super::formats::ZshFormat.classify_operator(op),
            WordbreakType::Unknown,
            "{op}"
        );
    }
}

#[test]
fn zsh_dollar_and_glob_not_wordbreak() {
    let classifier = super::formats::ZshFormat.classifier();
    assert_ne!(classifier.classify('$'), RuneClass::Wordbreak);
    for r in "*?[]^".chars() {
        assert_ne!(classifier.classify(r), RuneClass::Wordbreak, "glob {r}");
    }
}

#[test]
fn zsh_substitutions() {
    let tokens = split("echo $(ls)", Format::Zsh).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.value == "$(")
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );

    let tokens = split("echo $((1+2))", Format::Zsh).unwrap();
    assert!(tokens.iter().any(
        |t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.raw_value.ends_with("((")
    ));
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose
                && t.raw_value.starts_with("))"))
    );

    for input in ["echo <(", "echo >(", "echo <(cat) )"] {
        let tokens = split(input, Format::Zsh).unwrap();
        assert!(
            tokens.iter().any(|t| matches!(
                t.wordbreak_type,
                WordbreakType::SubstitutionOpen | WordbreakType::SubstitutionClose
            )),
            "process substitution {input}"
        );
    }

    assert_eq!(
        split("echo $(ls | grep foo)", Format::Zsh)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
}

#[test]
fn zsh_fd_prefix_in_redirects() {
    let tokens = split("echo 2> file", Format::Zsh).unwrap();
    let words = tokens
        .current_pipeline()
        .filter_redirects()
        .words()
        .strings();
    assert!(!words.contains(&"2".to_owned()), "{words:?}");
}

#[test]
fn zsh_quote_word() {
    // zsh quotestring() with QT_DOUBLE style (posixQuoteWord)
    let tests = [
        ("hello", "hello"),
        ("abc123", "abc123"),
        ("hello-world", "hello-world"),
        ("hello world", "\"hello world\""),
        (r#"with"quote"#, "\"with\\\"quote\""),
        (r"with\backslash", "\"with\\\\backslash\""),
        (r"with$dollar", "\"with\\$dollar\""),
        ("with`backtick", "\"with\\`backtick\""),
        ("with\nnewline", "\"with\\\nnewline\""),
        ("", "\"\""),
    ];
    for (input, want) in tests {
        assert_eq!(
            super::formats::ZshFormat.quote_word(input),
            want,
            "{input:?}"
        );
    }
}

#[test]
fn zsh_escape_outside_quotes() {
    let words = |input: &str| split(input, Format::Zsh).unwrap().words().strings();
    assert_eq!(
        words(r"echo a\ b"),
        vec!["echo".to_owned(), "a b".to_owned()]
    );
    assert_eq!(
        words(r"echo \$HOME"),
        vec!["echo".to_owned(), "$HOME".to_owned()]
    );
    assert_eq!(
        words(r"echo foo\|bar"),
        vec!["echo".to_owned(), "foo|bar".to_owned()]
    );
    assert_eq!(
        words(r"echo foo\>bar"),
        vec!["echo".to_owned(), "foo>bar".to_owned()]
    );
}

#[test]
fn zsh_no_word_splitting() {
    let words = split(r#"echo "hello world""#, Format::Zsh)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
    assert_eq!(
        split("echo hello world", Format::Zsh)
            .unwrap()
            .words()
            .strings()
            .len(),
        3
    );
}

// --- xonsh (verify_xonsh_test.go) ---

#[test]
fn xonsh_word_break_quote_escape_comment_chars() {
    let classifier = super::formats::XonshFormat.classifier();
    for r in "|<>&;()".chars() {
        assert_eq!(classifier.classify(r), RuneClass::Wordbreak, "xonsh {r}");
    }
    for r in " \t\r\n".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Space,
            "xonsh whitespace {r}"
        );
    }
    for r in "abcXYZ012_-.@/%+".chars() {
        assert_ne!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "xonsh regular {r}"
        );
    }
    assert_eq!(classifier.classify('"'), RuneClass::EscapingQuote);
    assert_eq!(classifier.classify('\''), RuneClass::NonEscapingQuote);
    assert_eq!(classifier.classify('\\'), RuneClass::Escape);
    assert_eq!(classifier.classify('#'), RuneClass::Comment);
}

#[test]
fn xonsh_operator_grammar() {
    // tokenize.py Operator + lexer.py _op_map (xonsh uses POSIX operators)
    let tests = [
        ("|", WordbreakType::Pipe, true, false),
        ("||", WordbreakType::ListOr, true, false),
        ("&&", WordbreakType::ListAnd, true, false),
        ("&", WordbreakType::ListAsync, true, false),
        (";", WordbreakType::ListSequential, true, false),
        (">", WordbreakType::RedirectOutput, false, true),
        (">>", WordbreakType::RedirectOutputAppend, false, true),
        ("<", WordbreakType::RedirectInput, false, true),
    ];
    for (input, want, is_pipe, is_redir) in tests {
        let got = super::formats::XonshFormat.classify_operator(input);
        assert_eq!(got, want, "ClassifyOperator({input})");
        assert_eq!(got.is_pipeline_delimiter(), is_pipe, "{input}");
        assert_eq!(got.is_redirect(), is_redir, "{input}");
    }
}

#[test]
fn xonsh_keyword_operators() {
    // lexer.py NEED_WHITESPACE = {"and", "or"}
    let ops = super::formats::XonshFormat.keyword_operators();
    assert!(
        ops.iter()
            .any(|(w, t)| *w == "and" && *t == WordbreakType::ListAnd)
    );
    assert!(
        ops.iter()
            .any(|(w, t)| *w == "or" && *t == WordbreakType::ListOr)
    );
    assert_eq!(
        split("echo foo and echo bar", Format::Xonsh)
            .unwrap()
            .pipelines()
            .len(),
        2
    );
    assert_eq!(
        split("echo foo or echo bar", Format::Xonsh)
            .unwrap()
            .pipelines()
            .len(),
        2
    );
    assert_eq!(
        split("echo foo not echo bar", Format::Xonsh)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
    assert_eq!(
        split("echo fooand bar", Format::Xonsh)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
}

#[test]
fn xonsh_double_quote_escape_chars() {
    // Python semantics: the backslash escapes ANY character
    assert!(
        super::formats::XonshFormat
            .escaping_quote_escape_chars()
            .is_empty()
    );
    let words = |input: &str| {
        split(input, Format::Xonsh)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words(r#"echo "say \"hello\"""#), r#"say "hello""#);
    assert_eq!(words(r#"echo "C:\\path""#), r"C:\path");
    assert_eq!(words(r#"echo "hello\nworld""#), "hellonworld");
    assert_eq!(words(r#"echo "a\tb""#), "atb");
    assert_eq!(words(r#"echo "cost: \$5""#), "cost: $5");
}

#[test]
fn xonsh_single_quote_escapes() {
    // tokenize.py Single regex: \' and \\ are escapes; other \X literal
    assert!(super::formats::XonshFormat.non_escaping_quote_backslash_escapes());
    let words = |input: &str| {
        split(input, Format::Xonsh)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words("echo 'it\\'s'"), "it's");
    assert_eq!(words(r"echo 'C:\\path'"), r"C:\path");
    assert_eq!(words(r"echo 'cost: \$5'"), r"cost: \$5");
    assert_eq!(words(r"echo 'hello\nworld'"), r"hello\nworld");
}

#[test]
fn xonsh_line_continuation_and_comment() {
    for (input, want) in [
        ("echo foo\\\nbar", "foobar"),
        ("echo \"foo\\\nbar\"", "foobar"),
        ("echo foo\\\r\nbar", "foobar"),
    ] {
        let words = split(input, Format::Xonsh).unwrap().words().strings();
        assert_eq!(words, vec!["echo".to_owned(), want.to_owned()], "{input}");
    }
    let words = split("echo hello # this is a comment", Format::Xonsh)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello".to_owned()]);
}

#[test]
fn xonsh_triple_quotes_and_raw_prefixes() {
    assert!(super::formats::XonshFormat.triple_quote_support());
    assert!(super::formats::XonshFormat.raw_prefix_support());
    let words = |input: &str| {
        split(input, Format::Xonsh)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words("echo '''hello world'''"), "hello world");
    assert_eq!(words(r#"echo """hello world""""#), "hello world");
    assert_eq!(words(r"echo '''hello''there'''"), "hello''there");
    assert_eq!(words(r#"echo """hello""there""""#), r#"hello""there"#);
    assert_eq!(words(r#"echo """say \"hello\"""""#), r#"say "hello""#);
    assert_eq!(words(r"echo r'C:\path'"), r"rC:\path");
    assert_eq!(words(r#"echo r"C:\path""#), r"rC:\path");
    assert_eq!(words(r#"echo br"hello\nworld""#), r"brhello\nworld");
    // invalid prefix does not suppress escapes
    assert_eq!(words(r#"echo xr"hello\nworld""#), "xrhellonworld");
}

#[test]
fn xonsh_stream_redirects() {
    // lexer.py _redir_names / _redir_map
    let tests = [
        ("echo foo e> bar", "e>", WordbreakType::RedirectOutput, true),
        (
            "echo foo err> bar",
            "err>",
            WordbreakType::RedirectOutput,
            true,
        ),
        (
            "echo foo e>> bar",
            "e>>",
            WordbreakType::RedirectOutput,
            true,
        ),
        ("echo foo o> bar", "o>", WordbreakType::RedirectOutput, true),
        (
            "echo foo out> bar",
            "out>",
            WordbreakType::RedirectOutput,
            true,
        ),
        (
            "echo foo a> bar",
            "a>",
            WordbreakType::RedirectOutputBoth,
            true,
        ),
        (
            "echo foo all> bar",
            "all>",
            WordbreakType::RedirectOutputBoth,
            true,
        ),
    ];
    for (input, raw, want, is_redir) in tests {
        let tokens = split(input, Format::Xonsh).unwrap();
        let found = tokens
            .iter()
            .find(|t| t.kind == TokenType::Wordbreak && t.raw_value == raw);
        let found = found.unwrap_or_else(|| panic!("{input}: no {raw} wordbreak token"));
        assert_eq!(found.wordbreak_type, want, "{raw}");
        assert_eq!(found.wordbreak_type.is_redirect(), is_redir, "{raw}");
    }
    // pipe-channel redirects
    for op in ["e>p", "o>p", "a>p"] {
        let tokens = split(&format!("echo foo {op} bar"), Format::Xonsh).unwrap();
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == TokenType::Wordbreak && t.raw_value == op),
            "pipe-channel {op}"
        );
    }
    // quoted stream redirect not merged
    let tokens = split("echo foo 'e'> bar", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .all(|t| !(t.kind == TokenType::Wordbreak && t.raw_value == "e>"))
    );
}

#[test]
fn xonsh_substitutions() {
    let tokens = split("echo $(ls)", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.value == "$(")
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );

    // @() — Python expression substitution
    let tokens = split("echo @(1+2)", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.value == "@(")
    );

    // bare () — subprocess grouping
    let tokens = split("echo (ls)", Format::Xonsh).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen)
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );

    assert_eq!(
        split("echo $(ls | grep foo)", Format::Xonsh)
            .unwrap()
            .pipelines()
            .len(),
        1
    );
}

#[test]
fn xonsh_quote_word() {
    // Python single-quote wrapping with \' and \\ escapes
    let tests = [
        ("hello", "hello"),
        ("abc123", "abc123"),
        ("hello-world", "hello-world"),
        ("hello world", "'hello world'"),
        ("with'quote", r"'with\'quote'"),
        (r"with\backslash", r"'with\\backslash'"),
        (r#"with"quote"#, r#"'with"quote'"#),
        ("", "\"\""),
    ];
    for (input, want) in tests {
        assert_eq!(super::quote::xonsh_quote_word(input), want, "{input:?}");
    }
}

#[test]
fn xonsh_escape_outside_quotes_and_line_continuation_interface() {
    let words = |input: &str| split(input, Format::Xonsh).unwrap().words().strings();
    assert_eq!(
        words(r"echo a\ b"),
        vec!["echo".to_owned(), "a b".to_owned()]
    );
    assert_eq!(
        words(r"echo foo\|bar"),
        vec!["echo".to_owned(), "foo|bar".to_owned()]
    );

    assert!(super::formats::XonshFormat.is_line_continuation('\n'));
    assert!(super::formats::XonshFormat.is_line_continuation('\r'));
    assert!(!super::formats::XonshFormat.is_line_continuation('a'));
}

#[test]
fn xonsh_no_word_splitting() {
    let words = split(r#"echo "hello world""#, Format::Xonsh)
        .unwrap()
        .words()
        .strings();
    assert_eq!(words, vec!["echo".to_owned(), "hello world".to_owned()]);
    assert_eq!(
        split("echo hello world", Format::Xonsh)
            .unwrap()
            .words()
            .strings()
            .len(),
        3
    );
}

// --- powershell (verify_powershell_test.go) ---

#[test]
fn powershell_force_start_new_token() {
    // CharTraits.cs ForceStartNewToken; , { } are deliberately NOT wordbreaks
    let classifier = super::formats::PowershellFormat.classifier();
    for r in "|;&><()".chars() {
        assert_eq!(classifier.classify(r), RuneClass::Wordbreak, "ps {r}");
    }
    for r in " \t\r\n".chars() {
        assert_eq!(
            classifier.classify(r),
            RuneClass::Space,
            "ps whitespace {r}"
        );
    }
    for r in ",{}".chars() {
        assert_ne!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "ps separator {r}"
        );
    }
    for r in "abcXYZ012_-.@/%+".chars() {
        assert_ne!(
            classifier.classify(r),
            RuneClass::Wordbreak,
            "ps regular {r}"
        );
    }
}

#[test]
fn powershell_quote_escape_comment_chars() {
    let classifier = super::formats::PowershellFormat.classifier();
    assert_eq!(classifier.classify('"'), RuneClass::EscapingQuote);
    assert_eq!(classifier.classify('\''), RuneClass::NonEscapingQuote);
    assert_eq!(classifier.classify('`'), RuneClass::Escape);
    // \ is NOT the escape character in PowerShell
    assert_ne!(classifier.classify('\\'), RuneClass::Escape);
    assert_ne!(classifier.classify('\\'), RuneClass::Wordbreak);
    assert_eq!(classifier.classify('#'), RuneClass::Comment);
}

#[test]
fn powershell_operator_grammar() {
    // tokenizer.cs dispatch: & is the call operator, NOT background
    let tests = [
        ("|", WordbreakType::Pipe, true, false),
        ("||", WordbreakType::ListOr, true, false),
        ("&&", WordbreakType::ListAnd, true, false),
        (";", WordbreakType::ListSequential, true, false),
        ("&", WordbreakType::Unknown, false, false),
        (">", WordbreakType::RedirectOutput, false, true),
        (">>", WordbreakType::RedirectOutput, false, true),
        ("<", WordbreakType::RedirectInput, false, true),
    ];
    for (input, want, is_pipe, is_redir) in tests {
        let got = super::formats::PowershellFormat.classify_operator(input);
        assert_eq!(got, want, "ClassifyOperator({input})");
        assert_eq!(got.is_pipeline_delimiter(), is_pipe, "{input}");
        assert_eq!(got.is_redirect(), is_redir, "{input}");
    }
}

#[test]
fn powershell_single_quote_no_escape() {
    let words = |input: &str| {
        split(input, Format::Powershell)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words(r"echo 'hello\nworld'"), r"hello\nworld");
    assert_eq!(words(r"echo '$HOME'"), "$HOME");
    assert_eq!(words("echo '`hello'"), "`hello");
    assert_eq!(words("echo 'don''t'"), "don't");
}

#[test]
fn powershell_double_quote_doubled_and_escapes() {
    // tokenizer.cs ScanStringExpandable: "" → literal ", backtick escapes any char
    assert!(
        super::formats::PowershellFormat
            .escaping_quote_escape_chars()
            .is_empty()
    );
    let words = |input: &str| {
        split(input, Format::Powershell)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    let tokens = split(r#"echo "say ""hello""""#, Format::Powershell).unwrap();
    assert_eq!(
        tokens.words().current_token().unwrap().value,
        r#"say "hello""#
    );
    assert_eq!(words("echo \"say `\"hello`\"\""), r#"say "hello""#);
    assert_eq!(words("echo \"cost: `$5\""), "cost: $5");
}

#[test]
fn powershell_escape_flags() {
    assert!(super::formats::PowershellFormat.escape_not_bareword());
    assert!(super::formats::PowershellFormat.non_escaping_quote_escapes());
    assert!(!super::formats::PowershellFormat.non_escaping_quote_backslash_escapes());
    assert!(!super::formats::PowershellFormat.escape_not_in_escaping_quote());
    assert!(super::formats::PowershellFormat.triple_quote_support() == false);
    assert!(super::formats::PowershellFormat.raw_prefix_support() == false);
    assert!(
        super::formats::PowershellFormat
            .keyword_operators()
            .is_empty()
    );
}

#[test]
fn powershell_backtick_escape_outside() {
    let words = |input: &str| {
        split(input, Format::Powershell)
            .unwrap()
            .words()
            .strings()
            .pop()
            .unwrap()
    };
    assert_eq!(words("echo `$HOME"), "$HOME");
    assert_eq!(words("echo foo`|bar"), "foo|bar");
}

#[test]
fn powershell_line_continuation_and_block_comment() {
    for (input, want) in [
        ("echo foo`\nbar", "foobar"),
        ("echo foo`\r\nbar", "foobar"),
        ("echo `\nbar", "bar"),
    ] {
        let words = split(input, Format::Powershell).unwrap().words().strings();
        assert_eq!(words, vec!["echo".to_owned(), want.to_owned()], "{input}");
    }
    assert_eq!(
        super::formats::PowershellFormat.block_comment(),
        Some(("<#", "#>"))
    );
    for input in [
        "echo <# multi\nline\ncomment #> foo",
        "echo <# inline comment #> foo",
    ] {
        let words = split(input, Format::Powershell).unwrap().words().strings();
        assert_eq!(words, vec!["echo".to_owned(), "foo".to_owned()], "{input}");
    }
}

#[test]
fn powershell_stop_parsing() {
    // Parser.cs VERBATIM_ARGUMENT + tokenizer.cs GetVerbatimCommandArgument
    assert_eq!(
        super::formats::PowershellFormat.stop_parsing_word(),
        Some("--%")
    );
    let tokens = split(
        "icacls X: --% /grant Dom\\HVAdmin:(CI)(OI)F",
        Format::Powershell,
    )
    .unwrap();
    let words = tokens.words().strings();
    assert_eq!(words.len(), 4);
    assert_eq!(words[2], "--%");
    assert_eq!(words[3], r"/grant Dom\HVAdmin:(CI)(OI)F");
    assert_eq!(
        split("echo --% foo | Select-String bar", Format::Powershell)
            .unwrap()
            .pipelines()
            .len(),
        2
    );
}

#[test]
fn powershell_stream_redirects() {
    // tokenizer.cs numeric/star redirections, merged in PostProcess
    for (input, raw) in [
        ("echo foo 2> file", "2>"),
        ("echo foo 2>> file", "2>>"),
        ("echo foo 2>&1", "2>&1"),
        ("echo foo 1>&2", "1>&2"),
        ("echo foo *> file", "*>"),
        ("echo foo *>> file", "*>>"),
    ] {
        let tokens = split(input, Format::Powershell).unwrap();
        let found = tokens
            .iter()
            .find(|t| t.kind == TokenType::Wordbreak && t.raw_value == raw);
        let found = found.unwrap_or_else(|| panic!("{input}: no merged {raw} token"));
        assert!(found.wordbreak_type.is_redirect(), "{raw}");
    }
}

#[test]
fn powershell_subexpression() {
    let tokens = split("echo $(ls)", Format::Powershell).unwrap();
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionOpen && t.value == "$(")
    );
    assert!(
        tokens
            .iter()
            .any(|t| t.wordbreak_type == WordbreakType::SubstitutionClose)
    );
}

#[test]
fn powershell_quote_word() {
    // PowerShell prefers single-quote wrapping with '' for literal '
    let tests = [
        ("hello", "hello"),
        ("abc123", "abc123"),
        (r"C:\path", r"C:\path"),
        ("hello world", "'hello world'"),
        ("with'quote", "'with''quote'"),
        ("with$dollar", "'with$dollar'"),
        (r#"with"quote"#, "'with\"quote'"),
        ("with`backtick", "'with`backtick'"),
        ("", "''"),
    ];
    for (input, want) in tests {
        assert_eq!(
            super::quote::powershell_quote_word(input),
            want,
            "{input:?}"
        );
    }
}

#[test]
fn powershell_command_separators_and_states() {
    for sep in ["|", ";", "&&", "||"] {
        let input = format!("echo foo{sep} echo bar");
        assert!(
            split(&input, Format::Powershell).unwrap().pipelines().len() >= 2,
            "{sep}"
        );
    }
    let tokens = split("echo 'hel", Format::Powershell).unwrap();
    assert_eq!(
        tokens.words().current_token().unwrap().state,
        super::token::LexerState::Quoting
    );
    let tokens = split(r#"echo "hel"#, Format::Powershell).unwrap();
    assert_eq!(
        tokens.words().current_token().unwrap().state,
        super::token::LexerState::QuotingEscaping
    );
    let words = split(r"echo C:\path\to\file", Format::Powershell)
        .unwrap()
        .words()
        .strings();
    assert_eq!(
        words,
        vec!["echo".to_owned(), r"C:\path\to\file".to_owned()]
    );
}

#[test]
fn complete_stop_parsing_stays_raw() {
    // STOP_PARSING_STATE survives to the completion context
    let ctx = complete("echo --% x", Format::Powershell);
    assert_eq!(ctx.quote("a|b"), "a|b");
}
