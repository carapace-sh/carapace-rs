//! Shell lexing for completion — Rust port of the
//! [carapace-shlex](https://github.com/carapace-sh/carapace-shlex) library.
//!
//! The host shell usually does the word splitting and hands the completer
//! its words plus context (`COMP_LINE`, `COMP_POINT`, ...). Re-lexing is
//! needed where the shell's view is not enough: bash's wordbreak-split
//! words (`bash::patch`), and values that are themselves command lines
//! (`Action::split`).

mod completion;
mod format;
mod formats;
mod lexer;
mod quote;
mod substitution;
mod token;
mod tokenslice;
mod variable;
mod wordbreak;

pub use completion::{CompletionContext, complete};
pub use token::{LexerState, Span, Token, TokenType};
pub use tokenslice::TokenSliceExt;
pub use wordbreak::WordbreakType;

/// The bash COMP_WORDBREAKS default.
pub const BASH_WORDBREAKS: &str = " \t\r\n\"'@><=;|&():";

/// Shell format for lexing/quoting a value (Go `shlex.Format`): the name
/// of one of the supported shells. The lexing behavior per name lives in
/// the format implementations; unknown names are rejected by [`split`],
/// yield an empty context in [`complete`], and fall back to the default
/// format in [`join`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// The empty format name, resolving to Bash. Spec macros and callers
    /// that omit the shell use this.
    #[default]
    Default,
    Bash,
    Cmd,
    Elvish,
    Fish,
    Hilbish,
    Nushell,
    Oil,
    Powershell,
    Tcsh,
    Xonsh,
    Zsh,
}

impl Format {
    /// The format's name string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Bash => "bash",
            Self::Cmd => "cmd",
            Self::Elvish => "elvish",
            Self::Fish => "fish",
            Self::Hilbish => "hilbish",
            Self::Nushell => "nushell",
            Self::Oil => "oil",
            Self::Powershell => "powershell",
            Self::Tcsh => "tcsh",
            Self::Xonsh => "xonsh",
            Self::Zsh => "zsh",
        }
    }

    /// Parse a format name; the empty name is Default. Unknown names are
    /// rejected.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "" => Some(Self::Default),
            "bash" => Some(Self::Bash),
            "cmd" => Some(Self::Cmd),
            "elvish" => Some(Self::Elvish),
            "fish" => Some(Self::Fish),
            "hilbish" => Some(Self::Hilbish),
            "nushell" => Some(Self::Nushell),
            "oil" => Some(Self::Oil),
            "powershell" => Some(Self::Powershell),
            "tcsh" => Some(Self::Tcsh),
            "xonsh" => Some(Self::Xonsh),
            "zsh" => Some(Self::Zsh),
            _ => None,
        }
    }
}

/// Partition a string into tokens using the given format. Word and
/// wordbreak tokens are yielded; whitespace and comments are skipped.
/// Unknown format names are rejected.
pub fn split(s: &str, format: Format) -> Option<Vec<Token>> {
    let impl_ = format::format_impl_for(format)?;
    let mut lexer = lexer::Lexer::new(s, impl_);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next() {
        tokens.push(token);
    }
    Some(impl_.post_process(tokens))
}

/// Concatenate words using the format's quoting rules. Unknown format
/// names fall back to the default format.
pub fn join(s: &[String], format: Format) -> String {
    let Some(impl_) = format::format_impl_for(format) else {
        return join(s, Format::Default);
    };
    s.iter()
        .map(|arg| impl_.quote_word(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Quote a single word using the format's quoting rules.
pub fn quote_word(s: &str, format: Format) -> String {
    match format::format_impl_for(format) {
        Some(impl_) => impl_.quote_word(s),
        None => quote_word(s, Format::Default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_bash_words_and_wordbreaks() {
        let tokens = split("echo 'a b' c", Format::Bash).unwrap();
        let values: Vec<&str> = tokens.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, vec!["echo", "a b", "c"]);
    }

    #[test]
    fn split_skips_comments() {
        let tokens = split("echo hi # note", Format::Bash).unwrap();
        let values: Vec<&str> = tokens.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, vec!["echo", "hi"]);
    }

    #[test]
    fn split_classifies_operators() {
        let tokens = split("echo foo > out.txt", Format::Bash).unwrap();
        assert!(
            tokens.iter().any(|t| t.kind == TokenType::Wordbreak
                && t.wordbreak_type == WordbreakType::RedirectOutput)
        );
    }

    #[test]
    fn split_recovers_partial_tail() {
        // the trailing partial token (open quote) is preserved
        let tokens = split("myapp \"ab", Format::Bash).unwrap();
        let values: Vec<&str> = tokens.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, vec!["myapp", "ab"]);
    }

    #[test]
    fn split_trailing_space_yields_empty_word() {
        let tokens = split("myapp ", Format::Bash).unwrap();
        let values: Vec<&str> = tokens.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(values, vec!["myapp", ""]);
    }

    #[test]
    fn split_multibyte_spans_are_rune_based() {
        let tokens = split("café", Format::Bash).unwrap();
        assert_eq!(tokens[0].span, Span { start: 0, end: 4 }); // 4 runes
    }

    #[test]
    fn join_quotes_only_when_needed() {
        let words = vec!["plain".to_owned(), "a b".to_owned(), String::new()];
        assert_eq!(join(&words, Format::Bash), "plain \"a b\" \"\"");
        // `=` does not force quoting (posixQuoteWord's trigger set)
        assert_eq!(
            join(&["--bool=true".to_owned()], Format::Bash),
            "--bool=true"
        );
    }

    #[test]
    fn complete_reports_wordbreak_prefix() {
        let context = complete("myapp --profile=dev", Format::Bash);
        assert_eq!(context.current_word, "--profile=dev");
        assert_eq!(context.prefix, "--profile=");
    }

    #[test]
    fn complete_detects_redirect() {
        let context = complete("myapp > out", Format::Bash);
        assert!(context.is_redirect);
    }

    #[test]
    fn complete_closes_open_quote_on_insertion() {
        let context = complete("myapp 'a b", Format::Bash);
        assert_eq!(context.quoting_state, LexerState::Quoting);
        assert_eq!(context.quote("a c"), "'a c'");
    }

    #[test]
    fn complete_detects_variables() {
        let context = complete("echo café | grep $HO", Format::Bash);
        let variable = context.variable.as_ref().expect("variable expected");
        assert_eq!(variable.name, "HO");
        assert_eq!(variable.insert("HOME"), "$HOME");
    }

    #[test]
    fn complete_trailing_space_starts_new_word() {
        let context = complete("myapp ", Format::Bash);
        assert_eq!(context.current_word, "");
        assert_eq!(context.words, vec!["myapp", ""]);
        assert_eq!(context.span.start, 6);
    }
}

// ported from Go completion_test.go (TestComplete)
#[cfg(test)]
mod completion_tests {
    use super::tokenslice::TokenSliceExt;
    use super::*;

    struct Case {
        input: &'static str,
        want_word: &'static str,
        want_prefix: &'static str,
        want_state: LexerState,
        want_redirect: bool,
        want_words: &'static [&'static str],
    }

    #[test]
    fn complete_matches_go_reference() {
        let cases = vec![
            Case {
                input: "echo hel",
                want_word: "hel",
                want_prefix: "",
                want_state: LexerState::InWord,
                want_redirect: false,
                want_words: &["echo", "hel"],
            },
            Case {
                input: "echo \"hel",
                want_word: "hel",
                want_prefix: "",
                want_state: LexerState::QuotingEscaping,
                want_redirect: false,
                want_words: &["echo", "hel"],
            },
            Case {
                input: "echo 'hel",
                want_word: "hel",
                want_prefix: "",
                want_state: LexerState::Quoting,
                want_redirect: false,
                want_words: &["echo", "hel"],
            },
            Case {
                input: "echo foo | grep bar",
                want_word: "bar",
                want_prefix: "",
                want_state: LexerState::InWord,
                want_redirect: false,
                want_words: &["grep", "bar"],
            },
            Case {
                input: "echo foo > bar",
                want_word: "bar",
                want_prefix: "",
                want_state: LexerState::InWord,
                want_redirect: true,
                want_words: &["echo", "foo"],
            },
            Case {
                input: "echo foo=bar",
                want_word: "foo=bar",
                want_prefix: "foo=",
                want_state: LexerState::InWord,
                want_redirect: false,
                want_words: &["echo", "foo=bar"],
            },
            Case {
                input: "",
                want_word: "",
                want_prefix: "",
                want_state: LexerState::Start,
                want_redirect: false,
                want_words: &[""],
            },
            Case {
                input: "echo foo\\",
                want_word: "foo",
                want_prefix: "",
                want_state: LexerState::Escaping,
                want_redirect: false,
                want_words: &["echo", "foo"],
            },
        ];
        for case in &cases {
            let context = complete(case.input, Format::Bash);
            assert_eq!(context.current_word, case.want_word, "{}: word", case.input);
            assert_eq!(context.prefix, case.want_prefix, "{}: prefix", case.input);
            assert_eq!(
                context.quoting_state, case.want_state,
                "{}: state",
                case.input
            );
            assert_eq!(
                context.is_redirect, case.want_redirect,
                "{}: redirect",
                case.input
            );
            assert_eq!(context.words, case.want_words, "{}: words", case.input);
        }
    }

    #[test]
    fn unknown_format_is_rejected() {
        assert!(
            split(
                "echo hi",
                Format::from_name("nosuch").unwrap_or(Format::Bash)
            )
            .is_some()
        );
        // from_name rejects unknown names
        assert!(Format::from_name("nosuch").is_none());
    }

    #[test]
    fn substitution_words_roundtrip_unquoted() {
        // port of substitution_test.go: closed substitution scopes are words
        let tokens = split("echo $(echo test)", Format::Bash).unwrap();
        let words = tokens.words_with_substitutions();
        let words: Vec<&str> = words.iter().map(|t| t.value.as_str()).collect();
        assert_eq!(words, vec!["echo", "$(echo test)"]);
    }
}
