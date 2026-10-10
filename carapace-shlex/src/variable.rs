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
