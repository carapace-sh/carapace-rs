use carapace_rs_shlex::Format;

#[test]
fn escape() {
    let tests: Vec<(&str, Format, &str, &str)> = vec![
        ("posix dollar/quote", Format::Bash, "a$()b'c", r"a\$()b'c"),
        (
            "posix doublequote/backslash",
            Format::Bash,
            r#"a"b\c"#,
            r#"a\"b\\c"#,
        ),
        ("fish", Format::Fish, r#"a$()b"c\d"#, r#"a\$()b\"c\\d"#),
        ("elvish", Format::Elvish, "a'b", r"a''b"),
        (
            "powershell",
            Format::Powershell,
            r#"demo'; IEX 'x"#,
            r#"demo''; IEX ''x"#,
        ),
        (
            "xonsh",
            Format::Xonsh,
            r#"demo'; touch 'x"#,
            r#"demo\'; touch \'x"#,
        ),
        (
            "lua",
            Format::Lua,
            r#"demo'; touch 'x"#,
            r#"demo\'; touch \'x"#,
        ),
        (
            "lua passes metacharacters",
            Format::Lua,
            "a$(touch x)",
            "a$(touch x)",
        ),
        ("lua newline", Format::Lua, "a\nb", r"a\nb"),
        ("bash newline", Format::Bash, "a\nb", "a\"'\n'\"b"),
        ("tcsh newline", Format::Tcsh, "a\nb", "a\\\nb"),
        ("fish newline stays raw", Format::Fish, "a\nb", "a\nb"),
        ("xonsh newline escape", Format::Xonsh, "a\nb", r"a\nb"),
    ];
    for (name, format, input, expected) in tests {
        assert_eq!(
            carapace_rs_shlex::escape(input, format),
            expected,
            "Escape({input:?}, {name})"
        );
    }
}
