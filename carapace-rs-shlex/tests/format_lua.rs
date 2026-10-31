use carapace_rs_shlex::{
    Format, LexerState, TokenSliceExt, TokenType, WordbreakType, complete, join, split,
};

fn strings_of(input: &str) -> Vec<String> {
    split(input, Format::Lua).unwrap().words().strings()
}

#[test]
fn split_words() {
    assert_eq!(
        strings_of(r"argmatcher 'don\'t fail'"),
        vec!["argmatcher".to_owned(), "don't fail".to_owned()],
    );

    let tokens = split(r#"argmatcher git :addarg(x, checkpoint)"#, Format::Lua).unwrap();
    assert_eq!(
        tokens.words().strings(),
        vec![
            "argmatcher".to_owned(),
            "git".to_owned(),
            ":addarg(x,".to_owned(),
            "checkpoint)".to_owned(),
        ],
    );
    for token in &tokens {
        if token.kind != TokenType::Wordbreak {
            continue;
        }
        match token.value.as_str() {
            "(" | ")" => {
                assert_eq!(token.wordbreak_type, WordbreakType::Bracket);
            }
            "," => {
                assert_eq!(token.wordbreak_type, WordbreakType::Unknown);
            }
            _ => {}
        }
    }

    let tokens = split("a\u{c}b c", Format::Lua).unwrap();
    assert_eq!(
        tokens.words().strings(),
        vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
    );

    let tokens = split(r"argmatcher 'don\'t fail'", Format::Lua).unwrap();
    assert_eq!(tokens.last().unwrap().state, LexerState::InWord);
}

#[test]
fn complete_words() {
    let context = complete(r#"argmatcher git :addarg(checkpoint"#, Format::Lua);
    assert_eq!(
        context.words,
        vec![
            "argmatcher".to_owned(),
            "git".to_owned(),
            ":addarg(checkpoint".to_owned(),
        ],
    );
    assert_eq!(context.current_word, ":addarg(checkpoint");
}

#[test]
fn join_words() {
    let tests: Vec<(&str, Vec<&str>)> = vec![
        ("argmatcher git", vec!["argmatcher", "git"]),
        (r#"x "a b""#, vec!["x", "a b"]),
        (r#"x "a\"b""#, vec!["x", r#"a"b"#]),
        (r#"x "a\\b""#, vec!["x", r"a\b"]),
        (r#"x "a$(touch x)""#, vec!["x", "a$(touch x)"]),
        (r#"x "a\nb""#, vec!["x", "a\nb"]),
        (r#"x "123abc""#, vec!["x", "123abc"]),
        (r#"x "café""#, vec!["x", "café"]),
    ];
    for (expected, words) in tests {
        assert_eq!(j(&words, Format::Lua), expected, "Join(lua) {words:?}");
    }
}

fn j(words: &[&str], format: Format) -> String {
    join(
        &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        format,
    )
}

#[test]
fn quote_insertion() {
    let tests: Vec<(&str, &str, &str)> = vec![
        (r#"x "a"#, "a b", "\"a b\""),
        (r#"x "a"#, r#"a"b"#, r#""a\"b""#),
        (r#"x "a"#, r"a\b", "\"a\\\\b\""),
        (r#"x "a"#, "a\nb", "\"a\\nb\""),
        (r"x 'a", "a'b", r#"'a\'b'"#),
        (r"x 'a", "a\nb", "'a\\nb'"),
    ];
    for (input, value, expected) in tests {
        let context = complete(input, Format::Lua);
        assert_eq!(
            context.quote(value),
            expected,
            "Quote({value:?}) for {input:?}"
        );
    }
}

#[test]
fn escape_sequences_are_lenient() {
    // Lua's read_string errors on these; the model deliberately treats any
    // backslash as escaping the next rune and drops the backslash, so the
    // letters and digits below are literal.
    let tests: Vec<(&str, &str)> = vec![
        (r#""a\099b""#, "a099b"),
        (r#""a\x41b""#, "ax41b"),
        (r#""a\u{65}b""#, "au{65}b"),
        (r#""a\g""#, "ag"),
        (r#""a\999b""#, "a999b"),
        (r#""abc\z   efg""#, "abcz   efg"),
    ];
    for (input, expected) in tests {
        assert_eq!(
            strings_of(&format!("x {input}")),
            vec!["x".to_owned(), expected.to_owned()],
            "{input}"
        );
    }
}

#[test]
fn unfinished_strings_stay_quoting() {
    let tests: Vec<(&str, LexerState)> = vec![
        (r"x 'alo", LexerState::Quoting),
        (r#"x "alo"#, LexerState::QuotingEscaping),
    ];
    for (input, expected) in tests {
        let tokens = split(input, Format::Lua).unwrap();
        assert_eq!(
            tokens.words().strings(),
            vec!["x".to_owned(), "alo".to_owned()],
            "{input}"
        );
        assert_eq!(tokens.last().unwrap().state, expected, "{input}");
    }
}

#[test]
fn long_brackets_split_on_whitespace_only() {
    // Long strings and their bracket levels are not lexed; bracket text
    // without raw spaces stays one word.
    let tests: Vec<(&str, Vec<&str>)> = vec![
        (r"x [=[al]o]]", vec!["x", "[=[al]o]]"]),
        (r"x [==[a b]==]", vec!["x", "[==[a", "b]==]"]),
        (r"x [===[[=[]]=]===]", vec!["x", "[===[[=[]]=]===]"]),
        (r"x [[alo] ", vec!["x", "[[alo]", ""]),
    ];
    for (input, expected) in tests {
        assert_eq!(
            strings_of(input),
            expected.iter().map(|s| s.to_owned()).collect::<Vec<_>>(),
            "{input}"
        );
    }
}

#[test]
fn line_ends_as_separators() {
    // literals.lua tests \n, \r, \n\r, \r\n as end-of-line sequences.
    for sep in ["\n", "\r", "\n\r", "\r\n"] {
        assert_eq!(
            strings_of(&format!("a{sep}b{sep}c")),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            "{sep:?}"
        );
    }
}

#[test]
fn nul_roundtrip() {
    // literals.lua: 'a\0a' with embedded zero bytes.
    let word = "a\0a\0b";
    let tokens = split(&format!("\"{word}\""), Format::Lua).unwrap();
    assert_eq!(tokens.words().strings(), vec![word.to_owned()], "{word:?}");
    assert!(j(&[word], Format::Lua).starts_with('"'), "{word:?}");
}
