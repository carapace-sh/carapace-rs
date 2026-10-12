//! Per-shell format implementations (ports of `format_*.go`).

use super::format::FormatImpl;
use super::quote::{
    cmd_quote_word, elvish_quote_word, fish_quote_word, hilbish_quote_word, nushell_quote_word,
    posix_quote_word, powershell_quote_word, xonsh_quote_word,
};
use super::token::{LexerState, RuneClassifier, Token};
use super::wordbreak::{self, WordbreakType};

/// `COMP_WORDBREAKS` from the environment (bash/zsh classifiers read it at
/// call time), falling back to bash's default set.
fn bash_wordbreak_runes() -> String {
    let wordbreaks = std::env::var("COMP_WORDBREAKS").unwrap_or_default();
    if wordbreaks.is_empty() {
        super::BASH_WORDBREAKS.to_owned()
    } else {
        wordbreaks
    }
}

fn base_classifier() -> RuneClassifier {
    RuneClassifier::base("\\")
}

/// Reclassify `(` / `)` wordbreak tokens as substitution delimiters.
fn reclassify_substitution(tokens: &mut [Token]) {
    for token in &mut *tokens {
        if token.kind == super::token::TokenType::Wordbreak {
            if token.value == "(" {
                token.wordbreak_type = WordbreakType::SubstitutionOpen;
            } else if token.value == ")" {
                token.wordbreak_type = WordbreakType::SubstitutionClose;
            }
        }
    }
}

/// Merge `$` + `(` adjacency into a substitution opener and reclassify
/// `(`/`)` as substitution delimiters (POSIX shells), ported from
/// `substitution_posix.go`:
///
/// 1. `$` word + adjacent `(` wordbreak merges into a single
///    `WORDBREAK_SUBSTITUTION_OPEN` token with raw value `$(`.
/// 2. `$((` arithmetic: a second adjacent `(` merges into one opener.
/// 3. Process substitution: adjacent redirect operator + `(` merges into
///    an opener (`<(`, `>(`, `>>(`).
/// 4. Standalone `(` reclassifies as `WORDBREAK_SUBSTITUTION_OPEN`.
/// 5. `)` reclassifies as `WORDBREAK_SUBSTITUTION_CLOSE`; with arithmetic
///    open, two adjacent `)` merge into one closer.
fn posix_substitution_post_process(tokens: Vec<Token>) -> Vec<Token> {
    use super::token::TokenType;

    let mut result = Vec::with_capacity(tokens.len());
    let mut depth: usize = 0;
    let mut arithmetic_depth: usize = 0;

    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];

        // $ + ( adjacency → merge into WORDBREAK_SUBSTITUTION_OPEN
        if token.kind == TokenType::Word && token.value == "$" && index + 1 < tokens.len() {
            let next = &tokens[index + 1];
            if next.kind == TokenType::Wordbreak && next.value == "(" && token.adjoins(next) {
                // $(( arithmetic
                if index + 2 < tokens.len()
                    && tokens[index + 2].kind == TokenType::Wordbreak
                    && tokens[index + 2].value == "("
                    && next.adjoins(&tokens[index + 2])
                {
                    result.push(Token {
                        kind: TokenType::Wordbreak,
                        value: "$(".to_owned(),
                        raw_value: token.raw_value.clone()
                            + &next.raw_value
                            + &tokens[index + 2].raw_value,
                        span: super::token::Span {
                            start: token.span.start,
                            end: tokens[index + 2].span.end,
                        },
                        state: tokens[index + 2].state,
                        wordbreak_type: WordbreakType::SubstitutionOpen,
                        ..Default::default()
                    });
                    depth += 1;
                    arithmetic_depth += 1;
                    index += 3;
                    continue;
                }
                // Regular $( command substitution
                result.push(Token {
                    kind: TokenType::Wordbreak,
                    value: "$(".to_owned(),
                    raw_value: token.raw_value.clone() + &next.raw_value,
                    span: super::token::Span {
                        start: token.span.start,
                        end: next.span.end,
                    },
                    state: next.state,
                    wordbreak_type: WordbreakType::SubstitutionOpen,
                    ..Default::default()
                });
                depth += 1;
                index += 2;
                continue;
            }
        }

        // Process substitution: <(, >(, or >>(
        if token.kind == TokenType::Wordbreak
            && token.wordbreak_type.is_redirect()
            && matches!(token.value.as_str(), "<" | ">" | ">>")
            && index + 1 < tokens.len()
        {
            let next = &tokens[index + 1];
            if next.kind == TokenType::Wordbreak && next.value == "(" && token.adjoins(next) {
                result.push(Token {
                    kind: TokenType::Wordbreak,
                    value: token.value.clone() + "(",
                    raw_value: token.raw_value.clone() + &next.raw_value,
                    span: super::token::Span {
                        start: token.span.start,
                        end: next.span.end,
                    },
                    state: next.state,
                    wordbreak_type: WordbreakType::SubstitutionOpen,
                    ..Default::default()
                });
                depth += 1;
                index += 2;
                continue;
            }
        }

        // Reclassify standalone ( as WORDBREAK_SUBSTITUTION_OPEN
        if token.kind == TokenType::Wordbreak && token.value == "(" {
            result.push(Token {
                wordbreak_type: WordbreakType::SubstitutionOpen,
                ..token.clone()
            });
            depth += 1;
            index += 1;
            continue;
        }

        // Reclassify ) as WORDBREAK_SUBSTITUTION_CLOSE
        if token.kind == TokenType::Wordbreak && token.value == ")" {
            // With arithmetic open, two adjacent ) merge into one closer
            if arithmetic_depth > 0
                && index + 1 < tokens.len()
                && tokens[index + 1].kind == TokenType::Wordbreak
                && tokens[index + 1].value == ")"
                && token.adjoins(&tokens[index + 1])
            {
                result.push(Token {
                    kind: TokenType::Wordbreak,
                    value: "))".to_owned(),
                    raw_value: token.raw_value.clone() + &tokens[index + 1].raw_value,
                    span: super::token::Span {
                        start: token.span.start,
                        end: tokens[index + 1].span.end,
                    },
                    state: tokens[index + 1].state,
                    wordbreak_type: WordbreakType::SubstitutionClose,
                    ..Default::default()
                });
                depth = depth.saturating_sub(1);
                arithmetic_depth -= 1;
                index += 2;
                continue;
            }
            result.push(Token {
                wordbreak_type: WordbreakType::SubstitutionClose,
                ..token.clone()
            });
            depth = depth.saturating_sub(1);
            index += 1;
            continue;
        }

        result.push(token.clone());
        index += 1;
    }

    result
}

/// Bash/POSIX lexing — the default format. The classifier reads
/// `COMP_WORDBREAKS` at call time.
pub(crate) struct BashFormat;

impl FormatImpl for BashFormat {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks(&bash_wordbreak_runes());
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        wordbreak::bash_wordbreak_type(raw)
    }

    fn quote_word(&self, s: &str) -> String {
        posix_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn naive_split_word_support(&self) -> bool {
        true
    }

    fn naive_split_word(&self, raw: &str) -> String {
        super::variable::naive_split_word(&self.classifier(), raw)
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        posix_substitution_post_process(tokens)
    }
}

/// Zsh lexing: extends bash with RC_QUOTES and zsh-specific operators.
pub(crate) struct ZshFormat;

impl FormatImpl for ZshFormat {
    fn name(&self) -> &'static str {
        "zsh"
    }

    fn classifier(&self) -> RuneClassifier {
        BashFormat.classifier()
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            ">>|" => WordbreakType::RedirectOutputAppendForce,
            ";&" => WordbreakType::ListFallthrough,
            ";|" => WordbreakType::ListFallthroughRetry,
            "&|" => WordbreakType::ListAsyncErrcheck,
            _ => wordbreak::bash_wordbreak_type(raw),
        }
    }

    fn non_escaping_quote_escapes(&self) -> bool {
        true // RC_QUOTES: '' → '
    }

    fn quote_word(&self, s: &str) -> String {
        posix_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        posix_substitution_post_process(tokens)
    }
}

/// Tcsh lexing: `>&` combined redirect, no `<<<`, no `;;` case operators.
pub(crate) struct TcshFormat;

const TCSH_WORDBREAKS: &str = "><;|&()";

impl FormatImpl for TcshFormat {
    fn name(&self) -> &'static str {
        "tcsh"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks(TCSH_WORDBREAKS);
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        wordbreak::tcsh_wordbreak_type(raw)
    }

    fn quote_word(&self, s: &str) -> String {
        posix_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        posix_substitution_post_process(tokens)
    }
}

/// Hilbish lexing: the POSIX sh grammar of its interpreter, without
/// bash's COMP_WORDBREAKS customization.
pub(crate) struct HilbishFormat;

const HILBISH_WORDBREAKS: &str = "><;|&()";

impl FormatImpl for HilbishFormat {
    fn name(&self) -> &'static str {
        "hilbish"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks(HILBISH_WORDBREAKS);
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        wordbreak::bash_wordbreak_type(raw) // hilbish uses the full bash operator grammar
    }

    fn quote_word(&self, s: &str) -> String {
        hilbish_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        posix_substitution_post_process(tokens)
    }
}

/// Fish lexing: `\'`/`\\` escapes inside single quotes, keyword operators
/// (`and`/`or`), fish-specific redirect operators.
pub(crate) struct FishFormat;

impl FormatImpl for FishFormat {
    fn name(&self) -> &'static str {
        "fish"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks("|;<>&?()");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            "|" => WordbreakType::Pipe,
            "|&" | "&|" => WordbreakType::PipeWithStderr,
            ">|" => WordbreakType::Pipe, // pipe with explicit fd (e.g. 2>|)
            ";" => WordbreakType::ListSequential,
            "&&" => WordbreakType::ListAnd,
            "||" => WordbreakType::ListOr,
            "&" => WordbreakType::ListAsync,
            ">" | ">?" => WordbreakType::RedirectOutput,
            ">>" | ">>?" => WordbreakType::RedirectOutputAppend,
            "&>" | "&>?" => WordbreakType::RedirectOutputBoth,
            "&>>" | "&>>?" => WordbreakType::RedirectOutputBothAppend,
            "<" | "<>&" | "<?" => WordbreakType::RedirectInput,
            "<>" => WordbreakType::RedirectInputOutput,
            ">&" => WordbreakType::RedirectInputDuplicate, // fd redirection (e.g. >&2)
            _ => WordbreakType::Unknown,
        }
    }

    fn keyword_operators(&self) -> &'static [(&'static str, WordbreakType)] {
        &[
            ("and", WordbreakType::ListAnd),
            ("or", WordbreakType::ListOr),
        ]
    }

    fn non_escaping_quote_escapes(&self) -> bool {
        true // ' and \\ inside single quotes
    }

    fn non_escaping_quote_backslash_escapes(&self) -> bool {
        true
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &['"', '$', '\\', '\n'] // no backtick in fish
    }

    fn quote_word(&self, s: &str) -> String {
        fish_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, mut tokens: Vec<Token>) -> Vec<Token> {
        reclassify_substitution(&mut tokens);
        tokens
    }
}

/// Elvish lexing: `\` is a bareword character (no escapes outside quotes),
/// `''` doubles a quote, `^`+newline is whitespace, lambda/brace/bracket
/// delimiters.
pub(crate) struct ElvishFormat;

impl FormatImpl for ElvishFormat {
    fn name(&self) -> &'static str {
        "elvish"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks("|><;()[]");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            "|" => WordbreakType::Pipe,
            ">" | ">>" | "<>" | "<" => WordbreakType::RedirectOutput, // simplified
            ";" => WordbreakType::ListSequential,
            "(" | ")" => WordbreakType::OutputCapture,
            "[" | "]" => WordbreakType::Bracket,
            _ => WordbreakType::Unknown,
        }
    }

    fn non_escaping_quote_escapes(&self) -> bool {
        true // '' → '
    }

    fn escape_not_bareword(&self) -> bool {
        false // \ is a bareword character in elvish
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &[] // backslash escapes any character
    }

    fn quote_word(&self, s: &str) -> String {
        elvish_quote_word(s)
    }

    fn line_continuation_char(&self) -> Option<char> {
        Some('^')
    }

    fn is_line_continuation_whitespace(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        elvish_post_process(tokens)
    }
}

/// Brace scopes for elvish post processing.
#[derive(Clone, Copy, PartialEq)]
enum BraceState {
    LambdaOpen,
    Params,
    LambdaBody,
    Braced,
}

fn elvish_post_process(mut tokens: Vec<Token>) -> Vec<Token> {
    let mut stack: Vec<BraceState> = Vec::new();

    for i in 0..tokens.len() {
        {
            let token = &mut tokens[i];

            if token.kind == super::token::TokenType::Wordbreak
                && token.wordbreak_type == WordbreakType::OutputCapture
            {
                if token.value == "(" {
                    token.wordbreak_type = WordbreakType::SubstitutionOpen;
                } else if token.value == ")" {
                    token.wordbreak_type = WordbreakType::SubstitutionClose;
                }
            }
        }

        if tokens
            .get(i)
            .is_some_and(|token| token.kind == super::token::TokenType::Word && token.value == "{")
        {
            let token = &tokens[i];
            let is_lambda = tokens.get(i + 1).is_some_and(|next| {
                next.kind == super::token::TokenType::Wordbreak && next.value == "|"
            }) || tokens.get(i + 1).is_none();
            let is_lambda_body =
                !is_lambda && tokens.get(i + 1).is_some_and(|next| !token.adjoins(next));
            if is_lambda {
                stack.push(BraceState::LambdaOpen);
            } else if is_lambda_body {
                stack.push(BraceState::LambdaBody);
            } else {
                stack.push(BraceState::Braced);
            }
            continue;
        }

        let token = &mut tokens[i];
        if token.kind == super::token::TokenType::Word {
            if token.value != "{" && token.value != "}" {
                for r in token.value.chars() {
                    match r {
                        '{' => stack.push(BraceState::Braced),
                        '}' => {
                            stack.pop();
                        }
                        _ => {}
                    }
                }
            } else if token.value == "}" {
                stack.pop();
                continue;
            }
        }

        if token.kind == super::token::TokenType::Wordbreak
            && token.value == "|"
            && !stack.is_empty()
        {
            let scope = stack.last_mut().unwrap();
            match scope {
                BraceState::LambdaOpen => {
                    *scope = BraceState::Params;
                    token.wordbreak_type = WordbreakType::LambdaPipe;
                }
                BraceState::Params => {
                    *scope = BraceState::LambdaBody;
                    token.wordbreak_type = WordbreakType::LambdaPipe;
                }
                BraceState::Braced => {
                    token.wordbreak_type = WordbreakType::LambdaPipe;
                }
                BraceState::LambdaBody => {}
            }
        }
    }

    tokens
}

/// cmd.exe lexing: `^` is the escape (literal inside quotes), `"` the only
/// quote, comma is a word delimiter, `&` a command separator, `(`/`)`
/// grouping operators.
pub(crate) struct CmdFormat;

impl FormatImpl for CmdFormat {
    fn name(&self) -> &'static str {
        "cmd"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = RuneClassifier::default();
        classifier.add_class(" \t\r\n", super::token::RuneClass::Space);
        // comma is a word delimiter (like space), but not a command
        // separator; `;` and `=` stay literal (safer for completion)
        classifier.add_class(",", super::token::RuneClass::Space);
        classifier.add_class("\"", super::token::RuneClass::EscapingQuote);
        classifier.add_class("^", super::token::RuneClass::Escape);
        // no comment classification (REM/:: keyword comments are deferred)
        classifier.add_wordbreaks("|&<>()");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            "|" => WordbreakType::Pipe,
            "&" => WordbreakType::ListSequential, // command separator
            "&&" => WordbreakType::ListAnd,
            "||" => WordbreakType::ListOr,
            ">" | ">>" => WordbreakType::RedirectOutput,
            "<" => WordbreakType::RedirectInput,
            "(" | ")" => WordbreakType::Unknown, // grouping operators
            _ => WordbreakType::Unknown,
        }
    }

    fn escape_not_in_escaping_quote(&self) -> bool {
        true // caret is completely literal inside double quotes
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &[] // backslash escapes any character — cmd uses `^` anyway
    }

    fn quote_word(&self, s: &str) -> String {
        cmd_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        cmd_post_process(tokens)
    }
}

fn cmd_post_process(tokens: Vec<Token>) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];

        // bare digit word (1/2) + adjacent redirect wordbreak: numeric
        // stream redirect (2>, 2>>); optional &N stream merge (2>&1, 1>&2)
        if token.kind == super::token::TokenType::Word
            && token.value == token.raw_value
            && token.value.len() == 1
            && matches!(token.value.as_str(), "1" | "2")
            && index + 1 < tokens.len()
        {
            let next = &tokens[index + 1];
            if next.kind == super::token::TokenType::Wordbreak
                && next.adjoins(token)
                && next.wordbreak_type.is_redirect()
                && next.raw_value.starts_with('>')
            {
                let mut merged = Token {
                    kind: super::token::TokenType::Wordbreak,
                    value: token.value.clone() + &next.value,
                    raw_value: token.raw_value.clone() + &next.raw_value,
                    span: super::token::Span {
                        start: token.span.start,
                        end: next.span.end,
                    },
                    state: next.state,
                    wordbreak_type: next.wordbreak_type,
                    ..Default::default()
                };

                if index + 3 < tokens.len()
                    && tokens[index + 2].kind == super::token::TokenType::Wordbreak
                    && tokens[index + 2].value == "&"
                    && tokens[index + 2].adjoins(next)
                    && tokens[index + 3].kind == super::token::TokenType::Word
                    && tokens[index + 3].value == tokens[index + 3].raw_value
                    && tokens[index + 3].adjoins(&tokens[index + 2])
                    && matches!(tokens[index + 3].value.as_str(), "1" | "2")
                {
                    // stream merge: 2>&1
                    merged.raw_value += &tokens[index + 2].raw_value;
                    merged.raw_value += &tokens[index + 3].raw_value;
                    merged.value += &tokens[index + 2].value;
                    merged.value += &tokens[index + 3].value;
                    merged.span.end = tokens[index + 3].span.end;
                    merged.wordbreak_type = WordbreakType::RedirectOutputBoth;
                    result.push(merged);
                    index += 4;
                    continue;
                }

                result.push(merged);
                index += 2;
                continue;
            }
        }

        result.push(tokens[index].clone());
        index += 1;
    }
    result
}

/// PowerShell lexing: backtick escapes, doubled quotes, block comments,
/// `--%` stop-parsing, `&&`/`||` lists.
pub(crate) struct PowershellFormat;

impl FormatImpl for PowershellFormat {
    fn name(&self) -> &'static str {
        "powershell"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = RuneClassifier::default();
        classifier.add_class(" \t\r\n", super::token::RuneClass::Space);
        classifier.add_class("\"", super::token::RuneClass::EscapingQuote);
        classifier.add_class("'", super::token::RuneClass::NonEscapingQuote);
        classifier.add_class("`", super::token::RuneClass::Escape);
        classifier.add_class("#", super::token::RuneClass::Comment);
        classifier.add_wordbreaks("|;&><()");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            "|" => WordbreakType::Pipe,
            ";" => WordbreakType::ListSequential,
            ">" | ">>" => WordbreakType::RedirectOutput,
            "<" => WordbreakType::RedirectInput,
            "&&" => WordbreakType::ListAnd,
            "||" => WordbreakType::ListOr,
            "&" => WordbreakType::Unknown, // call operator, not a list operator
            _ => WordbreakType::Unknown,
        }
    }

    fn non_escaping_quote_escapes(&self) -> bool {
        true // '' → '
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &[] // the backtick escapes any character
    }

    fn quote_word(&self, s: &str) -> String {
        powershell_quote_word(s)
    }

    fn quote_insertion_custom(&self, state: LexerState, value: &str) -> Option<String> {
        // Inside double quotes the escape character is the backtick, not
        // the backslash; other states use the default rules.
        if state != LexerState::QuotingEscaping {
            return None;
        }
        let mut b = String::with_capacity(value.len() + 2);
        b.push('"');
        for r in value.chars() {
            match r {
                '"' => b.push_str("`\""),
                '`' => b.push_str("``"),
                '$' => b.push_str("`$"),
                _ => b.push(r),
            }
        }
        b.push('"');
        Some(b)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn block_comment(&self) -> Option<(&'static str, &'static str)> {
        Some(("<#", "#>"))
    }

    fn stop_parsing_word(&self) -> Option<&'static str> {
        Some("--%")
    }

    fn escaping_quote_unescape(&self, r: char) -> Option<String> {
        // PowerShell doubled-quote form: "" → " is handled in the state
        // machine; the backtick escapes are not unescaped on lexing
        let _ = r;
        None
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        powershell_post_process(tokens)
    }
}

fn powershell_post_process(tokens: Vec<Token>) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];

        // bare stream number/* + adjacent redirect: 2>, *>>, 2>&1, 1>&2
        if token.kind == super::token::TokenType::Word
            && token.value == token.raw_value
            && !token.value.is_empty()
            && index + 1 < tokens.len()
        {
            let next = &tokens[index + 1];
            if next.kind == super::token::TokenType::Wordbreak
                && next.adjoins(token)
                && next.wordbreak_type.is_redirect()
                && next.raw_value.starts_with('>')
                && (token.value == "*"
                    || (token.value.len() == 1
                        && token.value.as_bytes()[0].is_ascii_digit()
                        && (b'1'..=b'6').contains(&token.value.as_bytes()[0])))
            {
                let mut merged = Token {
                    kind: super::token::TokenType::Wordbreak,
                    value: token.value.clone() + &next.value,
                    raw_value: token.raw_value.clone() + &next.raw_value,
                    span: super::token::Span {
                        start: token.span.start,
                        end: next.span.end,
                    },
                    state: next.state,
                    wordbreak_type: next.wordbreak_type,
                    ..Default::default()
                };

                if index + 3 < tokens.len()
                    && tokens[index + 2].kind == super::token::TokenType::Wordbreak
                    && tokens[index + 2].value == "&"
                    && tokens[index + 2].adjoins(next)
                    && tokens[index + 3].kind == super::token::TokenType::Word
                    && tokens[index + 3].value == tokens[index + 3].raw_value
                    && tokens[index + 3].adjoins(&tokens[index + 2])
                    && matches!(tokens[index + 3].value.as_str(), "1" | "2")
                {
                    merged.raw_value += &tokens[index + 2].raw_value;
                    merged.raw_value += &tokens[index + 3].raw_value;
                    merged.value += &tokens[index + 2].value;
                    merged.value += &tokens[index + 3].value;
                    merged.span.end = tokens[index + 3].span.end;
                    result.push(merged);
                    index += 4;
                    continue;
                }

                result.push(merged);
                index += 2;
                continue;
            }
        }

        result.push(tokens[index].clone());
        index += 1;
    }

    // merge $ + ( into substitution opener; reclassify ( and )
    let mut final_tokens: Vec<Token> = Vec::with_capacity(result.len());
    let mut iter = result.into_iter().peekable();
    while let Some(token) = iter.next() {
        let merged = {
            let next = iter.peek();
            match next {
                Some(next)
                    if token.kind == super::token::TokenType::Word
                        && token.value == "$"
                        && next.kind == super::token::TokenType::Wordbreak
                        && next.value == "("
                        && token.adjoins(next) =>
                {
                    Some(Token {
                        kind: super::token::TokenType::Wordbreak,
                        value: "$(".to_owned(),
                        raw_value: token.raw_value.clone() + &next.raw_value,
                        span: super::token::Span {
                            start: token.span.start,
                            end: next.span.end,
                        },
                        state: next.state,
                        wordbreak_type: WordbreakType::SubstitutionOpen,
                        ..Default::default()
                    })
                }
                _ => None,
            }
        };
        if let Some(merged) = merged {
            iter.next();
            final_tokens.push(merged);
            continue;
        }
        let mut token = token;
        if token.kind == super::token::TokenType::Wordbreak {
            if token.value == "(" {
                token.wordbreak_type = WordbreakType::SubstitutionOpen;
            } else if token.value == ")" {
                token.wordbreak_type = WordbreakType::SubstitutionClose;
            }
        }
        final_tokens.push(token);
    }
    final_tokens
}

/// Nushell lexing: backtick quotes, `\` outside quotes is a word character,
/// C-style escapes in double quotes, no `<` redirect, stream redirects
/// (`out>`, `err>`, `o+e>`, `e>|`, ...).
pub(crate) struct NushellFormat;

impl FormatImpl for NushellFormat {
    fn name(&self) -> &'static str {
        "nushell"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = RuneClassifier::default();
        classifier.add_class(" \t\r\n", super::token::RuneClass::Space);
        classifier.add_class("\"", super::token::RuneClass::EscapingQuote);
        classifier.add_class("'`", super::token::RuneClass::NonEscapingQuote);
        classifier.add_class("\\", super::token::RuneClass::Escape);
        classifier.add_class("#", super::token::RuneClass::Comment);
        classifier.add_wordbreaks("|;>()");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        match raw {
            "|" => WordbreakType::Pipe,
            ";" => WordbreakType::ListSequential,
            ">" => WordbreakType::RedirectOutput,
            ">>" => WordbreakType::RedirectOutputAppend,
            _ => WordbreakType::Unknown,
        }
    }

    fn escape_not_bareword(&self) -> bool {
        false // backslash outside quotes is a regular word character
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &[] // the C-style escape table applies (EscapingQuoteUnescape)
    }

    fn escaping_quote_unescaper(&self) -> bool {
        true
    }

    fn escaping_quote_unescape(&self, r: char) -> Option<String> {
        let replacement = match r {
            '"' => "\"",
            '\'' => "'",
            '\\' => "\\",
            '/' => "/",
            'b' => "\u{8}",
            'f' => "\u{c}",
            'r' => "\r",
            'n' => "\n",
            't' => "\t",
            '0' => "\0",
            'a' => "\u{7}",
            'e' => "\u{1b}",
            '(' => "(",
            ')' => ")",
            '{' => "{",
            '}' => "}",
            '$' => "$",
            '^' => "^",
            '#' => "#",
            '|' => "|",
            '~' => "~",
            _ => return None,
        };
        Some(replacement.to_owned())
    }

    fn quote_word(&self, s: &str) -> String {
        nushell_quote_word(s)
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        nushell_post_process(tokens)
    }
}

/// The word portion of nushell stream-redirect operators mapped to their
/// type (merged with a following `>`/`>>`/`>|` wordbreak in post processing).
fn nushell_stream_redirect(word: &str) -> Option<WordbreakType> {
    match word {
        "out" | "err" | "o" | "e" => Some(WordbreakType::RedirectOutput),
        "out+err" | "err+out" | "o+e" | "e+o" => Some(WordbreakType::RedirectOutputBoth),
        _ => None,
    }
}

fn nushell_post_process(tokens: Vec<Token>) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];

        // bare stream word + adjacent `>`/`>>`/`>|` wordbreak(s)
        if token.kind == super::token::TokenType::Word
            && token.value == token.raw_value
            && index + 1 < tokens.len()
        {
            let next = &tokens[index + 1];
            if next.kind == super::token::TokenType::Wordbreak
                && next.adjoins(token)
                && next.raw_value.starts_with('>')
            {
                if let Some(base_type) = nushell_stream_redirect(&token.value) {
                    let mut wb_type = base_type;
                    let is_append = next.raw_value.starts_with(">>");

                    // pipe variant: an adjacent `|` wordbreak after the `>`
                    let mut pipe_index = None;
                    if let Some(maybe_pipe) = tokens.get(index + 2) {
                        if maybe_pipe.kind == super::token::TokenType::Wordbreak
                            && maybe_pipe.adjoins(next)
                            && maybe_pipe.raw_value == "|"
                        {
                            pipe_index = Some(index + 2);
                        }
                    }

                    match pipe_index {
                        Some(pipe) => {
                            // out>| is a plain pipe (stdout to pipe);
                            // err>| and o+e>| pipe stderr/both
                            wb_type = if base_type == WordbreakType::RedirectOutput
                                && token.value != "err"
                                && token.value != "e"
                            {
                                WordbreakType::Pipe
                            } else {
                                WordbreakType::PipeWithStderr
                            };
                            let end = &tokens[pipe];
                            let mut merged = Token {
                                kind: super::token::TokenType::Wordbreak,
                                value: token.value.clone() + &next.value + &end.value,
                                raw_value: token.raw_value.clone()
                                    + &next.raw_value
                                    + &end.raw_value,
                                span: super::token::Span {
                                    start: token.span.start,
                                    end: end.span.end,
                                },
                                state: end.state,
                                wordbreak_type: wb_type,
                                ..Default::default()
                            };
                            let _ = &mut merged;
                            result.push(merged);
                            index = pipe + 1;
                            continue;
                        }
                        None => {
                            if is_append {
                                wb_type = if base_type == WordbreakType::RedirectOutputBoth {
                                    WordbreakType::RedirectOutputBothAppend
                                } else {
                                    WordbreakType::RedirectOutputAppend
                                };
                            }
                            let mut merged = Token {
                                kind: super::token::TokenType::Wordbreak,
                                value: token.value.clone() + &next.value,
                                raw_value: token.raw_value.clone() + &next.raw_value,
                                span: super::token::Span {
                                    start: token.span.start,
                                    end: next.span.end,
                                },
                                state: next.state,
                                wordbreak_type: wb_type,
                                ..Default::default()
                            };
                            reclassify_substitution(std::slice::from_mut(&mut merged));
                            result.push(merged);
                            index += 2;
                            continue;
                        }
                    }
                }
            }
        }

        result.push(tokens[index].clone());
        index += 1;
    }

    reclassify_substitution(&mut result);
    result
}

/// Xonsh lexing: Python string rules (triple quotes, raw prefixes, `\'`
/// escapes inside single quotes), POSIX operators, keyword `and`/`or`,
/// stream redirects (`e>`, `o>>`, `a>`, `e>p`, ...).
pub(crate) struct XonshFormat;

impl FormatImpl for XonshFormat {
    fn name(&self) -> &'static str {
        "xonsh"
    }

    fn classifier(&self) -> RuneClassifier {
        let mut classifier = base_classifier();
        classifier.add_wordbreaks("|<>&;()");
        classifier
    }

    fn classify_operator(&self, raw: &str) -> WordbreakType {
        wordbreak::bash_wordbreak_type(raw) // xonsh uses POSIX operators
    }

    fn keyword_operators(&self) -> &'static [(&'static str, WordbreakType)] {
        &[
            ("and", WordbreakType::ListAnd),
            ("or", WordbreakType::ListOr),
        ]
    }

    fn non_escaping_quote_backslash_escapes(&self) -> bool {
        true // \' and \\ inside single quotes (Python syntax)
    }

    fn escaping_quote_escape_chars(&self) -> &'static [char] {
        &[] // Python: the backslash escapes any character inside double quotes
    }

    fn triple_quote_support(&self) -> bool {
        true
    }

    fn raw_prefix_support(&self) -> bool {
        true
    }

    fn quote_word(&self, s: &str) -> String {
        xonsh_quote_word(s)
    }

    fn is_line_continuation(&self, r: char) -> bool {
        r == '\n' || r == '\r'
    }

    fn post_process(&self, tokens: Vec<Token>) -> Vec<Token> {
        xonsh_post_process(tokens)
    }
}

fn xonsh_stream_redirect(word: &str) -> Option<WordbreakType> {
    match word {
        "e" | "err" | "o" | "out" => Some(WordbreakType::RedirectOutput),
        "a" | "all" => Some(WordbreakType::RedirectOutputBoth),
        _ => None,
    }
}

fn xonsh_post_process(tokens: Vec<Token>) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];

        // bare stream word + adjacent `>`/`>>`/`<` wordbreak
        if token.kind == super::token::TokenType::Word
            && token.value == token.raw_value
            && index + 1 < tokens.len()
        {
            let next = &tokens[index + 1];
            if next.kind == super::token::TokenType::Wordbreak
                && next.adjoins(token)
                && (next.raw_value.starts_with('>') || next.raw_value.starts_with('<'))
            {
                if let Some(wb_type) = xonsh_stream_redirect(&token.value) {
                    let mut merged = Token {
                        kind: super::token::TokenType::Wordbreak,
                        value: token.value.clone() + &next.value,
                        raw_value: token.raw_value.clone() + &next.raw_value,
                        span: super::token::Span {
                            start: token.span.start,
                            end: next.span.end,
                        },
                        state: next.state,
                        wordbreak_type: wb_type,
                        ..Default::default()
                    };

                    // pipe-channel suffix: e>p, o>p, a>p
                    if let Some(after_next) = tokens.get(index + 2) {
                        if after_next.kind == super::token::TokenType::Word
                            && after_next.value == "p"
                            && after_next.value == after_next.raw_value
                            && after_next.adjoins(next)
                        {
                            merged.value.push('p');
                            merged.raw_value.push('p');
                            merged.span.end = after_next.span.end;
                            result.push(merged);
                            index += 3;
                            continue;
                        }
                    }

                    result.push(merged);
                    index += 2;
                    continue;
                }
            }
        }

        result.push(tokens[index].clone());
        index += 1;
    }

    // merge $/@ + ( into substitution opener; reclassify ( and )
    let mut final_tokens: Vec<Token> = Vec::with_capacity(result.len());
    let mut iter = result.into_iter().peekable();
    while let Some(token) = iter.next() {
        let merged = {
            let next = iter.peek();
            match next {
                Some(next)
                    if token.kind == super::token::TokenType::Word
                        && (token.value == "$" || token.value == "@")
                        && next.kind == super::token::TokenType::Wordbreak
                        && next.value == "("
                        && token.adjoins(next) =>
                {
                    Some(Token {
                        kind: super::token::TokenType::Wordbreak,
                        value: token.value.clone() + "(",
                        raw_value: token.raw_value.clone() + &next.raw_value,
                        span: super::token::Span {
                            start: token.span.start,
                            end: next.span.end,
                        },
                        state: next.state,
                        wordbreak_type: WordbreakType::SubstitutionOpen,
                        ..Default::default()
                    })
                }
                _ => None,
            }
        };
        if let Some(merged) = merged {
            iter.next();
            final_tokens.push(merged);
            continue;
        }
        let mut token = token;
        if token.kind == super::token::TokenType::Wordbreak {
            if token.value == "(" {
                token.wordbreak_type = WordbreakType::SubstitutionOpen;
            } else if token.value == ")" {
                token.wordbreak_type = WordbreakType::SubstitutionClose;
            }
        }
        final_tokens.push(token);
    }
    final_tokens
}
