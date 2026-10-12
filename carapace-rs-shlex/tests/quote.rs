use carapace_rs_shlex::{Format, complete};

#[test]
fn quote() {
    let tests: Vec<(&str, Format, &str, &str, &str)> = vec![
        (
            "close double quote",
            Format::Bash,
            r#"echo "he"#,
            "llo world",
            "\"llo world\"",
        ),
        (
            "escape dollar inside double quotes",
            Format::Bash,
            r#"echo "he"#,
            "a$b",
            "\"a\\$b\"",
        ),
        (
            "close single quote posix",
            Format::Bash,
            "echo 'he",
            "a'b",
            "'a'\"'\"'b'",
        ),
        (
            "close single quote fish",
            Format::Fish,
            "echo 'he",
            "a'b",
            "'a\\'b'",
        ),
        (
            "close single quote zsh",
            Format::Zsh,
            "echo 'he",
            "a'b",
            "'a''b'",
        ),
        (
            "close single quote elvish",
            Format::Elvish,
            "echo 'he",
            "a'b",
            "'a''b'",
        ),
        (
            "close double quote cmd",
            Format::Cmd,
            r#"echo "he"#,
            "a\"b",
            "\"a\"\"b\"",
        ),
        (
            "close double quote powershell",
            Format::Powershell,
            r#"echo "he"#,
            "a\"b",
            "\"a`\"b\"",
        ),
        (
            "bareword quoted as complete word",
            Format::Bash,
            "echo he",
            "a b",
            "\"a b\"",
        ),
        (
            "bareword plain stays plain",
            Format::Bash,
            "echo he",
            "plain",
            "plain",
        ),
        (
            "bareword elvish single quotes",
            Format::Elvish,
            "echo he",
            "a b",
            "'a b'",
        ),
        (
            "stop parsing stays raw",
            Format::Powershell,
            "echo --% x",
            "a|b",
            "a|b",
        ),
        (
            "close triple single quote xonsh",
            Format::Xonsh,
            "echo '''he",
            "a'b",
            "'''a\\'b'''",
        ),
        (
            "close triple double quote xonsh",
            Format::Xonsh,
            r#"echo """he"#,
            "a\"b",
            "\"\"\"a\\\"b\"\"\"",
        ),
    ];

    for (name, format, input, value, want) in tests {
        let ctx = complete(input, format);
        assert_eq!(ctx.quote(value), want, "{name}");
    }
}
