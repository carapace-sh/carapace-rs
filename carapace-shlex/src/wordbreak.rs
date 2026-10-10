//! Operator classification for wordbreak tokens (port of `wordbreak.go`).

/// Classification of a wordbreak token: redirects, pipelines, lists,
/// substitution delimiters and custom (COMP_WORDBREAKS) characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WordbreakType {
    #[default]
    Unknown,
    // https://www.gnu.org/software/bash/manual/html_node/Redirections.html
    RedirectInput,
    RedirectOutput,
    RedirectOutputAppend,
    /// `>|` (noclobber override)
    RedirectOutputForce,
    /// `>>|` (noclobber override for append)
    RedirectOutputAppendForce,
    RedirectOutputBoth,
    RedirectOutputBothAppend,
    RedirectInputString,
    /// `<<` (here-document)
    RedirectHereDoc,
    RedirectInputDuplicate,
    RedirectInputOutput,
    // https://www.gnu.org/software/bash/manual/html_node/Pipelines.html
    Pipe,
    PipeWithStderr,
    /// Elvish lambda parameter delimiter: `|` inside `{|params| ...}`
    LambdaPipe,
    // https://www.gnu.org/software/bash/manual/html_node/Lists.html
    ListAsync,
    ListSequential,
    ListAnd,
    ListOr,
    ListSequentialDouble,
    ListFallthrough,
    ListFallthroughRetry,
    ListCaseNext,
    ListAsyncErrcheck,
    /// COMP_WORDBREAKS
    Custom,
    /// Elvish output capture delimiters `(` and `)`
    OutputCapture,
    /// Elvish list literal / indexing delimiters `[` and `]`
    Bracket,
    /// Substitution delimiters opening/closing command or arithmetic
    /// substitution scopes (`$(...)`, `` `...` ``, `(...)`, `$((...))`)
    SubstitutionOpen,
    SubstitutionClose,
}

impl WordbreakType {
    pub fn is_pipeline_delimiter(self) -> bool {
        matches!(
            self,
            Self::Pipe
                | Self::PipeWithStderr
                | Self::ListAsync
                | Self::ListSequential
                | Self::ListAnd
                | Self::ListOr
                | Self::ListSequentialDouble
                | Self::ListFallthrough
                | Self::ListFallthroughRetry
                | Self::ListCaseNext
                | Self::ListAsyncErrcheck
        )
    }

    pub fn is_redirect(self) -> bool {
        matches!(
            self,
            Self::RedirectInput
                | Self::RedirectOutput
                | Self::RedirectOutputAppend
                | Self::RedirectOutputForce
                | Self::RedirectOutputAppendForce
                | Self::RedirectOutputBoth
                | Self::RedirectOutputBothAppend
                | Self::RedirectInputString
                | Self::RedirectHereDoc
                | Self::RedirectInputDuplicate
                | Self::RedirectInputOutput
        )
    }
}

/// The POSIX/bash operator grammar.
pub(crate) fn bash_wordbreak_type(raw: &str) -> WordbreakType {
    match raw {
        "<" => WordbreakType::RedirectInput,
        ">" => WordbreakType::RedirectOutput,
        ">>" => WordbreakType::RedirectOutputAppend,
        ">|" => WordbreakType::RedirectOutputForce,
        "&>" | ">&" => WordbreakType::RedirectOutputBoth,
        "&>>" => WordbreakType::RedirectOutputBothAppend,
        "<<" => WordbreakType::RedirectHereDoc,
        "<<<" => WordbreakType::RedirectInputString,
        "<&" => WordbreakType::RedirectInputDuplicate,
        "<>" => WordbreakType::RedirectInputOutput,
        "|" => WordbreakType::Pipe,
        "|&" => WordbreakType::PipeWithStderr,
        "&" => WordbreakType::ListAsync,
        ";" => WordbreakType::ListSequential,
        "&&" => WordbreakType::ListAnd,
        "||" => WordbreakType::ListOr,
        ";;" => WordbreakType::ListSequentialDouble,
        ";;&" => WordbreakType::ListCaseNext,
        ";&" => WordbreakType::ListFallthrough,
        _ => WordbreakType::Unknown,
    }
}

/// The tcsh operator grammar: `>&` for combined redirect, no `<<<`, no `;;`
/// case operators, no `&>` / `&>>`.
pub(crate) fn tcsh_wordbreak_type(raw: &str) -> WordbreakType {
    match raw {
        "<" => WordbreakType::RedirectInput,
        ">" => WordbreakType::RedirectOutput,
        ">>" => WordbreakType::RedirectOutputAppend,
        ">&" => WordbreakType::RedirectOutputBoth,
        "<<" => WordbreakType::RedirectHereDoc,
        "<&" => WordbreakType::RedirectInputDuplicate,
        "<>" => WordbreakType::RedirectInputOutput,
        "|" => WordbreakType::Pipe,
        "|&" => WordbreakType::PipeWithStderr,
        "&" => WordbreakType::ListAsync,
        ";" => WordbreakType::ListSequential,
        "&&" => WordbreakType::ListAnd,
        "||" => WordbreakType::ListOr,
        _ => WordbreakType::Unknown,
    }
}

/// The POSIX sh operator grammar handled by hilbish's interpreter: no `<<<`,
/// no `|&`, no `&>` / `&>>`; case lists use `;;`.
pub(crate) fn hilbish_wordbreak_type(raw: &str) -> WordbreakType {
    match raw {
        "<" => WordbreakType::RedirectInput,
        ">" => WordbreakType::RedirectOutput,
        ">>" => WordbreakType::RedirectOutputAppend,
        "<<" => WordbreakType::RedirectHereDoc,
        "<&" => WordbreakType::RedirectInputDuplicate,
        "<>" => WordbreakType::RedirectInputOutput,
        "|" => WordbreakType::Pipe,
        "&" => WordbreakType::ListAsync,
        ";" => WordbreakType::ListSequential,
        "&&" => WordbreakType::ListAnd,
        "||" => WordbreakType::ListOr,
        ";;" => WordbreakType::ListSequentialDouble,
        _ => WordbreakType::Unknown,
    }
}
