//! Variable reference detection (port of `variable.go`).

use super::token::{LexerState, Span, Token};

/// A variable reference ending the current word.
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// The variable name typed so far (empty directly after the sigil).
    pub name: String,

    /// The `${` form (as opposed to the plain `$` form).
    pub(crate) brace: bool,

    /// The raw text the shell's completion interface replaces on insertion,
    /// ending in the sigil and the typed name.
    pub(crate) replacement: String,

    /// The absolute rune span of the replaced region.
    pub(crate) span: Span,
}

impl Variable {
    /// The replacement word with the completed variable reference in
    /// place, preserving the reference's form (`$HOME` or `${HOME}`).
    /// Consumers prefix it with everything before the replaced region.
    pub fn insert(&self, completed: &str) -> String {
        let (sigil, closer) = if self.brace { ("${", "}") } else { ("$", "") };
        let keep = self.replacement.len() - self.name.len() - sigil.len();
        format!("{}{sigil}{completed}{closer}", &self.replacement[..keep])
    }

    /// The rune span of the replaced region in the input.
    pub fn span(&self) -> Span {
        self.span
    }
}

/// Variable expansion rules per format.
struct VariableRules {
    /// The `${name` form exists (bash, zsh, tcsh, hilbish, xonsh).
    brace: bool,
    /// `$` expands in the given lexer state (nushell/xonsh: barewords only).
    expands: fn(LexerState) -> bool,
    /// The escape character escapes `$` in barewords (elvish: no).
    bareword_escapes: bool,
    name_start: fn(char) -> bool,
    name_rune: fn(char) -> bool,
}

fn posix_name_start(r: char) -> bool {
    r.is_ascii_lowercase() || r.is_ascii_uppercase() || r == '_'
}

fn posix_name_rune(r: char) -> bool {
    posix_name_start(r) || r.is_ascii_digit()
}

fn nushell_name_rune(r: char) -> bool {
    posix_name_rune(r) || r == '-'
}

/// Detect a variable reference ending the given word (per-format rules).
pub(crate) fn variable_ref(
    word: &Token,
    format: &dyn super::format::FormatImpl,
) -> Option<Variable> {
    // per-format rules
    let rules = match format.name() {
        // no `${` form: fish, elvish
        "fish" | "elvish" => VariableRules {
            brace: false,
            expands: |state| state != LexerState::Quoting,
            bareword_escapes: format.name() != "elvish",
            name_start: posix_name_start,
            name_rune: posix_name_rune,
        },
        // barewords only: nushell (names with hyphens), xonsh (brace form)
        "nushell" => VariableRules {
            brace: false,
            expands: |state| state == LexerState::InWord,
            bareword_escapes: true,
            name_start: posix_name_start,
            name_rune: nushell_name_rune,
        },
        "xonsh" => VariableRules {
            brace: true,
            expands: |state| state == LexerState::InWord,
            bareword_escapes: true,
            name_start: posix_name_start,
            name_rune: posix_name_rune,
        },
        // posix-ish: bash, zsh, tcsh, hilbish, cmd(no $ — but harmless), oil
        _ => VariableRules {
            brace: true,
            expands: |state| state != LexerState::Quoting,
            bareword_escapes: true,
            name_start: posix_name_start,
            name_rune: posix_name_rune,
        },
    };
    let _ = format;
    variable_ref_impl(word, &rules)
}

fn variable_ref_impl(word: &Token, rules: &VariableRules) -> Option<Variable> {
    if !(rules.expands)(word.state) {
        return None;
    }

    let runes: Vec<char> = word.raw_value.chars().collect();
    let mut opener: Option<usize> = None;
    let mut index = 0;
    while index < runes.len() {
        if runes[index] != '$' {
            index += 1;
            continue;
        }
        // count preceding backslashes: an odd count means an escaped sigil
        let mut escaped = false;
        let mut k = index;
        while k > 0 && runes[k - 1] == '\\' {
            escaped = !escaped;
            k -= 1;
        }
        if escaped && (rules.bareword_escapes || word.state == LexerState::QuotingEscaping) {
            index += 1;
            continue;
        }
        if index == runes.len() - 1 {
            opener = Some(index); // trailing sigil
            break;
        }
        let next = runes[index + 1];
        if next == '{' && rules.brace {
            match closed_brace_end(&runes, index + 1) {
                Some(end) => {
                    index = end + 1;
                    continue;
                }
                None => {
                    opener = Some(index); // unclosed brace: the rest is the name region
                    break;
                }
            }
        } else if (rules.name_rune)(next) {
            let mut j = index + 1;
            while j < runes.len() && (rules.name_rune)(runes[j]) {
                j += 1;
            }
            if j == runes.len() {
                opener = Some(index); // name runs to the end: trailing opener
            }
            index = j;
        } else {
            index += 2; // special parameter: consume `$` and its character
        }
    }
    let opener = opener?;

    let mut rest = &runes[opener + 1..];
    let brace = rest.first() == Some(&'{');
    if brace {
        rest = &rest[1..];
    }
    if rest.first().is_some_and(|r| !(rules.name_start)(*r)) {
        return None;
    }
    if rest.iter().any(|r| !(rules.name_rune)(*r)) {
        return None;
    }

    Some(Variable {
        name: rest.iter().collect(),
        brace,
        replacement: String::new(),
        span: Span::default(),
    })
}

/// The index of the `}` closing the brace expansion whose `{` is at
/// `start`, or None when unclosed.
fn closed_brace_end(runes: &[char], start: usize) -> Option<usize> {
    (start + 1..runes.len()).find(|&i| runes[i] == '}')
}

/// The raw text of `raw` after its last delimiter character, where
/// delimiters are every rune the classifier treats as space, wordbreak or
/// quote (bash's COMP_WORDS model).
pub(crate) fn naive_split_word(classifier: &super::token::RuneClassifier, raw: &str) -> String {
    for (index, r) in raw.char_indices().rev() {
        if matches!(
            classifier.classify(r),
            super::token::RuneClass::Space
                | super::token::RuneClass::Wordbreak
                | super::token::RuneClass::EscapingQuote
                | super::token::RuneClass::NonEscapingQuote
        ) {
            return raw[index + r.len_utf8()..].to_owned();
        }
    }
    raw.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Format, complete};

    #[test]
    fn variable_ref() {
        struct Case {
            name: &'static str,
            input: &'static str,
            format: Format,
            want_name: &'static str,
            want_brace: bool,
            want_ref: bool,
        }
        let cases = [
            Case {
                name: "empty input",
                input: "",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "no dollar",
                input: "echo text",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "dollar only",
                input: "echo $",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "dollar with name",
                input: "echo $HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "brace only",
                input: "echo ${",
                format: Format::Bash,
                want_name: "",
                want_brace: true,
                want_ref: true,
            },
            Case {
                name: "brace with name",
                input: "echo ${HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: true,
                want_ref: true,
            },
            Case {
                name: "quoted text then dollar",
                input: "echo \"text$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "quoted text then brace",
                input: "echo \"test${",
                format: Format::Bash,
                want_name: "",
                want_brace: true,
                want_ref: true,
            },
            // the lexer word is `text $HO`; expansion semantics are the
            // same as the unsplit case
            Case {
                name: "quoted text with space",
                input: "echo \"text $HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "single quotes keep dollar literal",
                input: "echo 'text$HO",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "closed quote then dollar",
                input: "echo \"text\"$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "closed brace is not a reference",
                input: "echo ${HOME}",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "dollar before non-name char",
                input: "echo a$-x",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "escaped dollar is literal",
                input: "echo \"\\$HO",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "escaped backslash then dollar",
                input: "echo \"\\\\$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "command substitution is not a variable",
                input: "echo $(",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "fish has no brace form",
                input: "echo ${",
                format: Format::Fish,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "fish detects variables",
                input: "echo $HO",
                format: Format::Fish,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "fish quoted dollar expands",
                input: "echo \"text$HO",
                format: Format::Fish,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "elvish detects variables",
                input: "echo $HO",
                format: Format::Elvish,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            // backslash is a literal bareword character in elvish:
            // `\$HO` is a backslash followed by the expansion
            Case {
                name: "elvish bareword backslash does not escape",
                input: "echo \\$HO",
                format: Format::Elvish,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "elvish quoted backslash escapes",
                input: "echo \"\\$HO",
                format: Format::Elvish,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "elvish single quotes keep dollar literal",
                input: "echo 'text$HO",
                format: Format::Elvish,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "nushell detects variables",
                input: "echo $HO",
                format: Format::Nushell,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "nushell hyphenated name",
                input: "echo $my-var",
                format: Format::Nushell,
                want_name: "my-var",
                want_brace: false,
                want_ref: true,
            },
            // nushell quotes are literal strings, not interpolations
            Case {
                name: "nushell double quotes keep dollar literal",
                input: "echo \"text$HO",
                format: Format::Nushell,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "nushell single quotes keep dollar literal",
                input: "echo 'text$HO",
                format: Format::Nushell,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "nushell has no brace form",
                input: "echo ${",
                format: Format::Nushell,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "xonsh detects variables",
                input: "echo $HO",
                format: Format::Xonsh,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "xonsh brace form",
                input: "echo ${HO",
                format: Format::Xonsh,
                want_name: "HO",
                want_brace: true,
                want_ref: true,
            },
            // xonsh strings are Python literals: `$` is literal everywhere
            Case {
                name: "xonsh double quotes keep dollar literal",
                input: "echo \"text$HO",
                format: Format::Xonsh,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "xonsh triple quotes keep dollar literal",
                input: "echo '''text$HO'''",
                format: Format::Xonsh,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "xonsh command substitution is not a variable",
                input: "echo $(",
                format: Format::Xonsh,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "tcsh detects variables",
                input: "echo $HO",
                format: Format::Tcsh,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "tcsh brace form",
                input: "echo ${HO",
                format: Format::Tcsh,
                want_name: "HO",
                want_brace: true,
                want_ref: true,
            },
            Case {
                name: "tcsh single quotes keep dollar literal",
                input: "echo 'text$HO",
                format: Format::Tcsh,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "zsh quoted",
                input: "echo \"text$HO",
                format: Format::Zsh,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "redirect target",
                input: "echo >$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "positional parameter",
                input: "echo $1",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "positional parameter plus literal",
                input: "echo $9x",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "brace positional parameter",
                input: "echo ${1",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "positional parameter then reference",
                input: "echo $1x$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "inside substitution uses absolute span",
                input: "echo $(echo $HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            // `#` mid-word is not a bash wordbreak; the reference is
            // still detected and the whole word is replaced
            Case {
                name: "hash mid-word",
                input: "echo a#c$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "pid parameter is not a reference",
                input: "echo $$",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "pid parameter mid-word is not a reference",
                input: "echo a$$",
                format: Format::Bash,
                want_name: "",
                want_brace: false,
                want_ref: false,
            },
            Case {
                name: "pid parameter then reference",
                input: "echo $$$HO",
                format: Format::Bash,
                want_name: "HO",
                want_brace: false,
                want_ref: true,
            },
            Case {
                name: "closed brace then reference",
                input: "echo ${HO}x$NE",
                format: Format::Bash,
                want_name: "NE",
                want_brace: false,
                want_ref: true,
            },
        ];

        for case in &cases {
            let ctx = complete(case.input, case.format);
            if case.want_ref {
                let variable = ctx
                    .variable
                    .as_ref()
                    .unwrap_or_else(|| panic!("{}: variable = None", case.name));
                assert_eq!(variable.name, case.want_name, "{}: name", case.name);
                assert_eq!(variable.brace, case.want_brace, "{}: brace", case.name);
            } else {
                assert!(ctx.variable.is_none(), "{}: unexpected ref", case.name);
            }
        }
    }

    #[test]
    fn variable_replacement() {
        // bash's naive COMP_WORDS split keeps quoted text before a
        // wordbreak in the replaced word, but not text after it
        let cases = [
            ("echo \"text$HO", "text$HO"),
            ("echo \"text $HO", "$HO"),
            ("echo $HO", "$HO"),
            ("echo \"test${HO", "test${HO"),
            ("echo a#c$HO", "a#c$HO"),
            ("echo >$HO", "$HO"),
        ];
        for (input, want) in &cases {
            let ctx = complete(input, Format::Bash);
            let variable = ctx.variable.as_ref().expect("variable");
            assert_eq!(variable.replacement, *want, "input {input:?}");
        }
    }

    #[test]
    fn zsh_keeps_whole_raw_word() {
        let ctx = complete("echo \"text $HO", Format::Zsh);
        let variable = ctx.variable.as_ref().expect("variable");
        assert_eq!(variable.replacement, ctx.raw_current_word);
    }

    #[test]
    fn variable_insert() {
        let cases = [
            ("echo $HO", Format::Bash, "$HOME"),
            ("echo \"text$HO", Format::Bash, "text$HOME"),
            ("echo \"text $HO", Format::Bash, "$HOME"),
            ("echo \"test${HO", Format::Bash, "test${HOME}"),
            ("echo ${", Format::Bash, "${HOME}"),
            ("echo $", Format::Bash, "$HOME"),
            ("echo $my-var", Format::Nushell, "$HOME"),
            ("echo \\$HO", Format::Elvish, "\\$HOME"),
            ("echo >$HO", Format::Bash, "$HOME"),
        ];
        for (input, format, want) in &cases {
            let ctx = complete(input, *format);
            let variable = ctx.variable.as_ref().expect("variable");
            assert_eq!(variable.insert("HOME"), *want, "input {input:?}");
        }
    }

    #[test]
    fn variable_span() {
        let ctx = complete("echo \"text $HO", Format::Bash);
        let variable = ctx.variable.as_ref().expect("variable");
        // the naive replacement region `$HO` ends at the word's end
        assert_eq!(
            variable.span(),
            Span {
                start: ctx.span.end - 3,
                end: ctx.span.end,
            }
        );

        // the naive replacement region excludes the opening quote
        let ctx = complete("echo \"text$HO", Format::Bash);
        let variable = ctx.variable.as_ref().expect("variable");
        assert_eq!(
            variable.span(),
            Span {
                start: ctx.span.end - 7,
                end: ctx.span.end,
            }
        );
    }
}
