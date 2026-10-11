//! Port of Go `shlex_test.go` (the `testString` tokenizer corpus).

use carapace_shlex::{
    Format, LexerState, Span, Token, TokenSliceExt, TokenType, WordbreakType, split,
};

const TEST_STRING: &str = "one two \"three four\" \"five \\\"six\\\"\" seven#eight # nine # ten\n eleven 'twelve\\' thirteen=13 fourteen/14 | || |after before| & ;";

fn token(
    kind: TokenType,
    value: &str,
    raw_value: &str,
    span: Span,
    state: LexerState,
    wordbreak_type: WordbreakType,
) -> Token {
    Token {
        kind,
        value: value.to_owned(),
        raw_value: raw_value.to_owned(),
        span,
        state,
        wordbreak_type,
        ..Default::default()
    }
}

#[test]
fn tokenizer() {
    // the Go tokenizer yields a comment token ("# nine # ten"); `split`
    // skips comments, so it is absent here. Spans still refer to the
    // source positions.
    let expected = vec![
        token(
            TokenType::Word,
            "one",
            "one",
            Span { start: 0, end: 3 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "two",
            "two",
            Span { start: 4, end: 7 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "three four",
            "\"three four\"",
            Span { start: 8, end: 20 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "five \"six\"",
            "\"five \\\"six\\\"\"",
            Span { start: 21, end: 35 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "seven#eight",
            "seven#eight",
            Span { start: 36, end: 47 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "eleven",
            "eleven",
            Span { start: 62, end: 68 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "twelve\\",
            "'twelve\\'",
            Span { start: 69, end: 78 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "thirteen",
            "thirteen",
            Span { start: 79, end: 87 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Wordbreak,
            "=",
            "=",
            Span { start: 87, end: 88 },
            LexerState::Wordbreak,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "13",
            "13",
            Span { start: 88, end: 90 },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "fourteen/14",
            "fourteen/14",
            Span {
                start: 91,
                end: 102,
            },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Wordbreak,
            "|",
            "|",
            Span {
                start: 103,
                end: 104,
            },
            LexerState::Wordbreak,
            WordbreakType::Pipe,
        ),
        token(
            TokenType::Wordbreak,
            "||",
            "||",
            Span {
                start: 105,
                end: 107,
            },
            LexerState::Wordbreak,
            WordbreakType::ListOr,
        ),
        token(
            TokenType::Wordbreak,
            "|",
            "|",
            Span {
                start: 108,
                end: 109,
            },
            LexerState::Wordbreak,
            WordbreakType::Pipe,
        ),
        token(
            TokenType::Word,
            "after",
            "after",
            Span {
                start: 109,
                end: 114,
            },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Word,
            "before",
            "before",
            Span {
                start: 115,
                end: 121,
            },
            LexerState::InWord,
            WordbreakType::Unknown,
        ),
        token(
            TokenType::Wordbreak,
            "|",
            "|",
            Span {
                start: 121,
                end: 122,
            },
            LexerState::Wordbreak,
            WordbreakType::Pipe,
        ),
        token(
            TokenType::Wordbreak,
            "&",
            "&",
            Span {
                start: 123,
                end: 124,
            },
            LexerState::Wordbreak,
            WordbreakType::ListAsync,
        ),
        token(
            TokenType::Wordbreak,
            ";",
            ";",
            Span {
                start: 125,
                end: 126,
            },
            LexerState::Wordbreak,
            WordbreakType::ListSequential,
        ),
        token(
            TokenType::Word,
            "",
            "",
            Span {
                start: 126,
                end: 126,
            },
            LexerState::Start,
            WordbreakType::Unknown,
        ),
    ];

    let got = split(TEST_STRING, Format::Bash).unwrap();
    for (index, want) in expected.iter().enumerate() {
        assert_eq!(
            got.get(index),
            Some(want),
            "token[{index}] of {TEST_STRING:?}\ngot : {:?}\nwant: {:?}",
            got.get(index),
            want
        );
    }
    assert_eq!(got.len(), expected.len(), "token count of {TEST_STRING:?}");
}

#[test]
fn lexer() {
    // the lexer yields word tokens only (wordbreaks and comments skipped);
    // the Go test stops consuming after these 11
    let expected = [
        "one",
        "two",
        "three four",
        "five \"six\"",
        "seven#eight",
        "eleven",
        "twelve\\",
        "thirteen",
        "13",
        "fourteen/14",
    ];
    let got: Vec<String> = split(TEST_STRING, Format::Bash)
        .unwrap()
        .into_iter()
        .filter(|token| token.kind == TokenType::Word)
        .map(|token| token.value)
        .take(expected.len())
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn split_test_string() {
    let want = [
        "one",
        "two",
        "three four",
        "five \"six\"",
        "seven#eight",
        "eleven",
        "twelve\\",
        "thirteen",
        "=",
        "13",
        "fourteen/14",
        "|",
        "||",
        "|",
        "after",
        "before",
        "|",
        "&",
        ";",
        "",
    ];
    let got = split(TEST_STRING, Format::Default).unwrap().strings();
    assert_eq!(got, want);
}
