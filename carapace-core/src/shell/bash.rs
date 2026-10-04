//! Bash value formatting (port of Go `internal/shell/bash/action.go`).

use crate::meta::Meta;
use crate::value::RawValue;
use crate::value::RawValues;

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

/// Escape for unquoted bash context.
fn escaping_replacer(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(
            c,
            '\\' | '&' | '<' | '>' | '`' | '\'' | '"' | '{' | '}' | '$' | '#' | '|' | '?' | '('
                | ')' | ';' | ' ' | '[' | ']' | '*'
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

fn matches_nospace(meta: &Meta, value: &RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}

/// Longest prefix shared by all strings.
fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let len = a
        .char_indices()
        .zip(b.chars())
        .take_while(|((_, a_char), b_char)| a_char == b_char)
        .map(|((index, a_char), _)| index + a_char.len_utf8())
        .last()
        .unwrap_or(0);
    &a[..len]
}

fn common_prefix_of(values: &[&str]) -> String {
    let mut iter = values.iter().copied();
    match iter.next() {
        Some(first) => iter.fold(first.to_owned(), |acc, value| {
            common_prefix(&acc, value).to_owned()
        }),
        None => String::new(),
    }
}

fn common_display_prefix(values: &[RawValue]) -> String {
    common_prefix_of(&values.iter().map(|v| v.display.as_str()).collect::<Vec<_>>())
}

fn common_value_prefix(values: &[RawValue]) -> String {
    common_prefix_of(&values.iter().map(|v| v.value.as_str()).collect::<Vec<_>>())
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
                let display = sanitize(&value.display);
                if value.description.is_empty() {
                    display
                } else {
                    format!("{} ({})", display, sanitize(&value.description))
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
        r#"#compdef {binary}
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
