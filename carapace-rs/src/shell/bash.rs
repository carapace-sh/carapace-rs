//! Bash value formatting (port of Go `internal/shell/bash/action.go`).

use crate::meta::Meta;
use crate::value::{RawValue, RawValues};

use super::{
    Flags, common_display_prefix, common_value_prefix, matches_nospace, trimmed_description,
};

/// Bash-specific inputs collected during patching.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// Prefix stripped by bash's COMP_WORDBREAKS splitting.
    pub wordbreak_prefix: String,
    /// Bash COMP_TYPE value ("9", "63", ...).
    pub comp_type: String,
    /// COMP_WORDBREAKS contents affecting quoting.
    pub wordbreaks: String,
}

/// COMP_TYPE value for successive-tab listing.
const COMP_TYPE_LIST_SUCCESSIVE_TABS: &str = "63";

/// Remove newline, carriage return and tab (would break the line format).
fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
}

/// Escape `${` sequences so bash list output is not expanded.
fn display_replacer(value: &str) -> String {
    value.replace("${", "\\\\$\\{")
}

/// Escape for unquoted bash context.
fn escaping_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(
            c,
            '\\' | '&'
                | '<'
                | '>'
                | '`'
                | '\''
                | '"'
                | '{'
                | '}'
                | '$'
                | '#'
                | '|'
                | '?'
                | '('
                | ')'
                | ';'
                | ' '
                | '['
                | ']'
                | '*'
        ) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

/// Escape inside double quotes.
fn escaping_quoted_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | '"' | '$' | '`') {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn requires_quoting(value: &str, wordbreaks: &str) -> bool {
    let special = " \t\r\n`[]{}()<>;|$&:*#'\"\\";
    value.chars().any(|c| special.contains(c)) || wordbreaks.chars().any(|c| value.contains(c))
}

/// Format values for bash consumption: `nospace\001value\nvalue...`.
pub fn action_raw_values(
    current_word: &str,
    meta: &Meta,
    values: &RawValues,
    context: &Context,
    flags: &Flags,
) -> String {
    let mut values: Vec<RawValue> = values
        .iter()
        .map(|value| RawValue {
            value: value
                .value
                .strip_prefix(&context.wordbreak_prefix)
                .unwrap_or(&value.value)
                .to_owned(),
            ..value.clone()
        })
        .collect();

    let last_segment = current_word
        .strip_prefix(&context.wordbreak_prefix)
        .unwrap_or(current_word);

    if values.len() > 1 {
        let display_prefix = common_display_prefix(&values);
        if !display_prefix.is_empty() {
            let value_prefix = common_value_prefix(&values);
            if last_segment != value_prefix {
                // bash would insert the common prefix as partial completion:
                // replace values with the common value prefix
                values = vec![RawValue::new(value_prefix)];
            } else {
                // prevent insertion of partial display values by spacing one
                values[0].display = format!(" {}", values[0].display);
            }
            return render(meta, &values, context, true, flags);
        }
    }
    render(meta, &values, context, false, flags)
}

fn render(
    meta: &Meta,
    values: &[RawValue],
    context: &Context,
    forced_nospace: bool,
    flags: &Flags,
) -> String {
    let mut nospace = forced_nospace;
    let list_mode = context.comp_type == COMP_TYPE_LIST_SUCCESSIVE_TABS && values.len() > 1;
    let rendered: Vec<String> = values
        .iter()
        .map(|value| {
            if !list_mode {
                nospace = nospace || matches_nospace(meta, value);
                let sanitized = sanitize(&value.value);
                if sanitized.starts_with('~') {
                    escaping_replacer(&sanitized)
                } else if requires_quoting(&sanitized, &context.wordbreaks) {
                    format!("\"{}\"", escaping_quoted_replacer(&sanitized))
                } else {
                    sanitized
                }
            } else {
                nospace = true;
                let display = display_replacer(&sanitize(&value.display));
                if value.description.is_empty() {
                    display
                } else {
                    format!(
                        "{} ({})",
                        display,
                        display_replacer(&sanitize(&trimmed_description(
                            &value.description,
                            flags.description_length,
                        )))
                    )
                }
            }
        })
        .collect();
    let nospace_flag = if nospace { "true" } else { "false" };
    format!("{nospace_flag}\u{1}{}", rendered.join("\n"))
}

/// Generate the bash completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"#!/bin/bash
_{binary}_completion() {{
  export COMP_LINE
  export COMP_POINT
  export COMP_TYPE
  export COMP_WORDBREAKS

  local nospace data compline="${{COMP_LINE:0:${{COMP_POINT}}}}"

  data=$(echo "${{compline}}''" | xargs {binary} _carapace bash 2>/dev/null)
  if [ $? -eq 1 ]; then
    data=$(echo "${{compline}}'" | xargs {binary} _carapace bash 2>/dev/null)
    if [ $? -eq 1 ]; then
      data=$(echo "${{compline}}\"" | xargs {binary} _carapace bash 2>/dev/null)
    fi
  fi

  IFS=$'\001' read -r -d '' nospace data <<<"${{data}}"
  mapfile -t COMPREPLY < <(echo "${{data}}")
  unset COMPREPLY[-1]

  [ "${{nospace}}" = true ] && compopt -o nospace
  local IFS=$'\n'
  [[ "${{COMPREPLY[*]}}" == "" ]] && COMPREPLY=()
}}

complete -o noquote -F _{binary}_completion {binary}
"#
    )
}

/// Result of patching bash completion arguments.
#[derive(Clone, Debug, Default)]
pub enum Patch {
    #[default]
    None,
    /// Re-lexed arguments with metadata extracted from the COMP_* variables.
    Patched {
        /// Words from re-lexing COMP_LINE up to the cursor.
        args: Vec<String>,
        /// Portion of the current word before the last wordbreak character.
        wordbreak_prefix: String,
        /// COMP_TYPE value.
        comp_type: String,
    },
    /// The cursor completes a redirect target; complete files instead.
    Redirect,
}

/// Re-lex `COMP_LINE` and filter redirects (port of Go `bash.Patch`).
///
/// Only meaningful when the shell exported `COMP_LINE`; otherwise the
/// original arguments are passed through untouched.
pub fn patch(args: Vec<String>) -> Patch {
    crate::log::log(&format!(
        "COMP_LINE is {:?}",
        std::env::var("COMP_LINE").unwrap_or_default()
    ));
    crate::log::log(&format!(
        "COMP_POINT is {:?}",
        std::env::var("COMP_POINT").unwrap_or_default()
    ));
    crate::log::log(&format!(
        "COMP_WORDBREAKS is {:?}",
        std::env::var("COMP_WORDBREAKS").unwrap_or_default()
    ));
    let Ok(compline) = std::env::var("COMP_LINE") else {
        return Patch::Patched {
            args,
            wordbreak_prefix: String::new(),
            comp_type: String::new(),
        };
    };
    let comp_point: usize = std::env::var("COMP_POINT")
        .ok()
        .and_then(|point| point.parse().ok())
        .unwrap_or(compline.len());
    let comp_type = std::env::var("COMP_TYPE").unwrap_or_default();
    // COMP_WORDBREAKS is consumed by the lexer's classifier
    let truncated = truncate_at_byte(&compline, comp_point);
    let context = carapace_rs_shlex::complete(&truncated, carapace_rs_shlex::Format::Bash);
    unset_bash_env();

    if context.is_redirect {
        // completing a redirect target: files (Go's RedirectError)
        return Patch::Redirect;
    }

    // words are redirect-filtered and include the binary name plus the
    // trailing cursor word; traverse expects post-binary words
    let mut cleaned = context.words;
    if !cleaned.is_empty() {
        cleaned.remove(0);
    }
    crate::log::log(&format!("patching args to {cleaned:?}"));
    Patch::Patched {
        args: cleaned,
        wordbreak_prefix: context.prefix,
        comp_type: comp_type.clone(),
    }
}

fn truncate_at_byte(line: &str, point: usize) -> String {
    let mut end = point.min(line.len());
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_owned()
}

fn unset_bash_env() {
    // Go's `bash.Patch` unsets every `COMP_*` variable it may have been
    // started with so that child processes (mocked execs, git, …) don't
    // observe completion state.
    for variable in [
        "COMP_CWORD",
        "COMP_LINE",
        "COMP_POINT",
        "COMP_TYPE",
        "COMP_KEY",
        "COMP_WORDBREAKS",
        "COMP_WORDS",
    ] {
        unsafe { std::env::remove_var(variable) };
    }
}
