//! Bash value formatting (port of Go `internal/shell/bash/action.go`).

use crate::meta::Meta;
use crate::value::{RawValue, RawValues};

use super::{common_display_prefix, common_value_prefix, matches_nospace, trimmed_description};

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
            return render(meta, &values, context, true);
        }
    }
    render(meta, &values, context, false)
}

fn render(meta: &Meta, values: &[RawValue], context: &Context, forced_nospace: bool) -> String {
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
                        display_replacer(&sanitize(&trimmed_description(&value.description)))
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
    let wordbreaks = std::env::var("COMP_WORDBREAKS").unwrap_or_default();
    let wordbreak_chars: Vec<char> = wordbreaks.chars().filter(|c| !c.is_whitespace()).collect();

    let truncated = truncate_at_byte(&compline, comp_point);

    let mut tokens: Vec<String> = shlex::Shlex::new(&truncated).collect();
    let tail = crate::lex::raw_last_token(&truncated);
    if !tail.is_empty() && !tail.complete {
        // shlex drops a trailing partial token (open quote or trailing
        // escape); recover the dequoted value so the completion sees the
        // word the cursor is in
        tokens.push(crate::lex::dequote_partial(tail.token));
    }

    let result = if tokens.is_empty() {
        Patch::Patched {
            args: Vec::new(),
            wordbreak_prefix: String::new(),
            comp_type: comp_type.clone(),
        }
    } else {
        // drop redirects: operator plus its target
        let mut cleaned: Vec<String> = Vec::with_capacity(tokens.len());
        let mut iter = tokens.iter().peekable();
        let mut redirect_target = false;
        while let Some(token) = iter.next() {
            if is_redirect(token) {
                if iter.peek().is_none() {
                    redirect_target = true; // completing the redirect target
                } else {
                    iter.next(); // skip the target
                }
                continue;
            }
            cleaned.push(token.clone());
        }
        if redirect_target {
            return Patch::Redirect;
        }
        // drop the binary name: traverse expects post-binary words
        // (Go skips shell name and binary via args[2:])
        if !cleaned.is_empty() {
            cleaned.remove(0);
        }
        if tail.is_empty() && !truncated.is_empty() {
            // the cursor starts a new word (Go's lexer emits it)
            cleaned.push(String::new());
        }
        let wordbreak_prefix = tail
            .token
            .rfind(|c: char| wordbreak_chars.contains(&c))
            .map(|index| tail.token[..=index].to_owned())
            .unwrap_or_default();
        Patch::Patched {
            args: cleaned,
            wordbreak_prefix,
            comp_type: comp_type.clone(),
        }
    };

    unset_bash_env();
    result
}

fn truncate_at_byte(line: &str, point: usize) -> String {
    let mut end = point.min(line.len());
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_owned()
}

/// Whether the token is a shell redirect operator (e.g. `>`, `2>`, `>>`, `&>`).
fn is_redirect(token: &str) -> bool {
    let trimmed = token.trim_start_matches(['0', '1', '2', '&']);
    matches!(trimmed, ">" | ">>" | "<" | "<<") || token == ">&" || token == "<&"
}

fn unset_bash_env() {
    for variable in ["COMP_LINE", "COMP_POINT", "COMP_TYPE", "COMP_WORDBREAKS"] {
        unsafe { std::env::remove_var(variable) };
    }
}
