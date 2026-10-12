use carapace_rs_shlex::{Format, join};

fn j(words: &[&str], format: Format) -> String {
    join(
        &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        format,
    )
}

#[test]
fn posix() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("", vec![]),
        ("echo hello", vec!["echo", "hello"]),
        (r#"echo "hello world""#, vec!["echo", "hello world"]),
        (r#"echo "\$(ls)""#, vec!["echo", "$(ls)"]),
        (r#"echo "'ls'""#, vec!["echo", "'ls'"]),
        (r#"echo "\"ls\"""#, vec!["echo", "\"ls\""]),
        (r#"echo "\`ls\`""#, vec!["echo", "`ls`"]),
        (
            r#"echo "with\"doubleQuote""#,
            vec!["echo", "with\"doubleQuote"],
        ),
        (
            r#"echo "with'singleQuote""#,
            vec!["echo", "with'singleQuote"],
        ),
        (r#"echo "with\$dollar""#, vec!["echo", "with$dollar"]),
        ("echo \"with\\\nlinefeed\"", vec!["echo", "with\nlinefeed"]),
        (
            "echo \"with\rcarriageReturn\"",
            vec!["echo", "with\rcarriageReturn"],
        ),
        ("echo \"with\ttab\"", vec!["echo", "with\ttab"]),
        (
            "ls /tmp | xargs -n 1 echo",
            vec!["ls", "/tmp", "|", "xargs", "-n", "1", "echo"],
        ),
    ];
    for (expected, words) in tests {
        assert_eq!(j(&words, Format::Bash), expected, "Join(bash) {words:?}");
    }
}

#[test]
fn fish() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo hello", vec!["echo", "hello"]),
        (r#"echo "hello world""#, vec!["echo", "hello world"]),
        (r#"echo "say \"hello\"""#, vec!["echo", "say \"hello\""]),
        (r#"echo "cost \$5""#, vec!["echo", "cost $5"]),
    ];
    for (expected, words) in tests {
        assert_eq!(j(&words, Format::Fish), expected, "Join(fish) {words:?}");
    }
}

#[test]
fn elvish() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo ''", vec!["echo", ""]),
        ("echo hello", vec!["echo", "hello"]),
        ("echo 'hello world'", vec!["echo", "hello world"]),
        ("echo 'it''s'", vec!["echo", "it's"]),
        (r"echo C:\path", vec!["echo", r"C:\path"]),
        (r#"echo 'say "hi"'"#, vec!["echo", r#"say "hi""#]),
        ("echo '$var'", vec!["echo", "$var"]),
        ("echo '*glob'", vec!["echo", "*glob"]),
        ("echo '|pipe'", vec!["echo", "|pipe"]),
        ("echo ';semi'", vec!["echo", ";semi"]),
        ("echo '(cap'", vec!["echo", "(cap"]),
        ("echo '[list'", vec!["echo", "[list"]),
        ("echo '{brace'", vec!["echo", "{brace"]),
        ("echo '&amp'", vec!["echo", "&amp"]),
        ("echo '#comment'", vec!["echo", "#comment"]),
        ("echo 'a=b'", vec!["echo", "a=b"]),
        ("echo 'a,b'", vec!["echo", "a,b"]),
        ("echo a~b", vec!["echo", "a~b"]),
    ];
    for (expected, words) in tests {
        assert_eq!(
            j(&words, Format::Elvish),
            expected,
            "Join(elvish) {words:?}"
        );
    }
}

#[test]
fn powershell() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo hello", vec!["echo", "hello"]),
        ("echo 'hello world'", vec!["echo", "hello world"]),
        ("echo 'don''t'", vec!["echo", "don't"]),
    ];
    for (expected, words) in tests {
        assert_eq!(
            j(&words, Format::Powershell),
            expected,
            "Join(powershell) {words:?}"
        );
    }
}

#[test]
fn nushell() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo hello", vec!["echo", "hello"]),
        (r#"echo "hello world""#, vec!["echo", "hello world"]),
        (r#"echo "say \"hello\"""#, vec!["echo", "say \"hello\""]),
    ];
    for (expected, words) in tests {
        assert_eq!(
            j(&words, Format::Nushell),
            expected,
            "Join(nushell) {words:?}"
        );
    }
}

#[test]
fn cmd() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo hello", vec!["echo", "hello"]),
        (r#"echo "hello world""#, vec!["echo", "hello world"]),
        (r#"echo "say "^"hello"^"""#, vec!["echo", "say \"hello\""]),
    ];
    for (expected, words) in tests {
        assert_eq!(j(&words, Format::Cmd), expected, "Join(cmd) {words:?}");
    }
}

#[test]
fn xonsh() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("echo hello", vec!["echo", "hello"]),
        ("echo 'hello world'", vec!["echo", "hello world"]),
        (r"echo 'it\'s'", vec!["echo", "it's"]),
    ];
    for (expected, words) in tests {
        assert_eq!(j(&words, Format::Xonsh), expected, "Join(xonsh) {words:?}");
    }
}

#[test]
fn backward_compat() {
    // Join() with the default format produces the same results as the v1 Join
    let tests: Vec<(&str, Vec<&str>)> = vec![
        (r#"echo "\$(ls)""#, vec!["echo", "$(ls)"]),
        (r#"echo "'ls'""#, vec!["echo", "'ls'"]),
        (r#"echo "\"ls\"""#, vec!["echo", "\"ls\""]),
        (
            "ls /tmp | xargs -n 1 echo",
            vec!["ls", "/tmp", "|", "xargs", "-n", "1", "echo"],
        ),
    ];
    for (expected, words) in tests {
        assert_eq!(
            j(&words, Format::Default),
            expected,
            "Join(Default) {words:?}"
        );
    }
}
