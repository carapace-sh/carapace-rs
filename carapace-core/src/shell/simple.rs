//! Simple line-based formatters: oil, tcsh, cmd-clink, ion, bash-ble.

use crate::meta::Meta;
use crate::value::{RawValue, RawValues};

/// Description trimmed per `CARAPACE_DESCRIPTION_LENGTH` (default 80).
pub(crate) fn trimmed_description(description: &str) -> String {
    let max_length = std::env::var("CARAPACE_DESCRIPTION_LENGTH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(80);
    let first_line = description.split('\n').next().unwrap_or_default();
    let trimmed = first_line.trim();
    let runes: Vec<char> = trimmed.chars().collect();
    if runes.len() > max_length {
        if max_length <= 3 {
            return runes[..max_length].iter().collect();
        }
        let mut result: String = runes[..max_length - 3].iter().collect();
        result.push_str("...");
        return result;
    }
    trimmed.to_owned()
}

fn matches_nospace(meta: &Meta, value: &RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}

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

/// Longest common display/value prefix across values.
fn common_display_prefix(values: &[RawValue]) -> String {
    common_prefix_of(&values.iter().map(|v| v.display.as_str()).collect::<Vec<_>>())
}

fn common_value_prefix(values: &[RawValue]) -> String {
    common_prefix_of(&values.iter().map(|v| v.value.as_str()).collect::<Vec<_>>())
}

/// Oil: values with inline `\u{1}` nospace indicator, single value plain.
pub mod oil {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\t'], "")
    }

    pub fn action_raw_values(current_word: &str, meta: &Meta, values: &RawValues) -> String {
        let _ = current_word;
        values
            .iter()
            .map(|value| {
                let mut value = value.clone();
                if matches_nospace(meta, &value) {
                    value.value.push('\u{1}');
                }
                if values.len() == 1 {
                    sanitize(&value.value)
                } else if !value.description.is_empty() {
                    format!("{} ({})", value.value, sanitize(&trimmed_description(&value.description)))
                } else {
                    value.value
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn snippet(binary: &str) -> String {
        format!(
            "complete -F _{binary}_completion {binary}\n\n_{binary}_completion() {{\n  local IFS=$'\\n'\n  local response\n  response=$(eval \"echo \"${{COMP_LINE:0:${{COMP_POINT}}}}\"''\" | xargs {binary} _carapace oil 2>/dev/null)\n  echo $response\n}}\n"
        )
    }
}

/// Tcsh: backslash-escaped values with `_(description)` annotation.
pub mod tcsh {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\r', '\t'], "")
    }

    fn quoter(value: &str) -> String {
        let mut result = String::with_capacity(value.len());
        for c in value.chars() {
            if matches!(
                c,
                '\\' | '&' | '<' | '>' | '`' | '\'' | '"' | '$' | '#' | '|' | '?' | '(' | ')'
                    | ';' | ' ' | '[' | ']' | '*'
            ) {
                result.push('\\');
            }
            result.push(c);
        }
        result
    }

    pub fn action_raw_values(current_word: &str, meta: &Meta, values: &RawValues) -> String {
        let _ = meta;
        let mut last_segment = current_word.to_owned();
        if let Ok(wordbreaks) = std::env::var("COMP_WORDBREAKS") {
            let wordbreaks = wordbreaks.replace(' ', "");
            if let Some(index) = current_word.rfind(|c: char| wordbreaks.contains(c)) {
                last_segment = current_word[index + 1..].to_owned();
            }
        }

        let mut values = values.clone();
        if values.len() > 1 {
            let display_prefix = common_display_prefix(&values);
            if !display_prefix.is_empty() {
                let value_prefix = common_value_prefix(&values);
                if last_segment != value_prefix {
                    values = vec![RawValue::new(value_prefix)];
                } else {
                    values[0].display = format!(" {}", values[0].display);
                }
            }
        }

        values
            .iter()
            .map(|value| {
                if values.len() == 1 || value.description.is_empty() {
                    quoter(&sanitize(&value.value))
                } else {
                    let description =
                        quoter(&sanitize(&trimmed_description(&value.description)).replace(' ', "_"));
                    format!("{}_({})", quoter(&sanitize(&value.value)), description)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn snippet(binary: &str) -> String {
        format!(
            "complete {binary} 'C@*,p@{binary} `echo $COMMAND_LINE[:]` _carapace tcsh@'\n"
        )
    }
}

/// cmd-clink: tab-separated `value\tdisplay\tdesc\tappendchar`.
pub mod cmd_clink {
    use super::*;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\t'], "")
    }

    pub fn action_raw_values(current_word: &str, meta: &Meta, values: &RawValues) -> String {
        let _ = current_word;
        values
            .iter()
            .map(|value| {
                let append_char = if matches_nospace(meta, value) { "" } else { " " };
                format!(
                    "{}\t{}\t{}\t{}",
                    sanitize(&value.value),
                    sanitize(&value.display),
                    sanitize(&trimmed_description(&value.description)),
                    append_char
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn snippet(binary: &str) -> String {
        format!(
            r#"local clink = require('clink')
clink.argmatcher('{binary}')
:addarg({{ function(word) {{
    local compline = os.getenv('CARAPACE_COMPLINE') or ''
    local data = io.popen('{binary} _carapace cmd-clink ' .. compline):read('*a')
    return {{}}
end }} }})
"#
        )
    }
}

/// Ion: JSON `{Value, Display}` suggestions.
pub mod ion {
    use super::*;
    use serde::Serialize;

    fn sanitize(value: &str) -> String {
        value.replace(['\n', '\r'], "")
    }

    #[derive(Serialize)]
    struct Suggestion {
        #[serde(rename = "Value")]
        value: String,
        #[serde(rename = "Display")]
        display: String,
    }

    pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
        let suggestions: Vec<Suggestion> = values
            .iter()
            .map(|value| {
                let mut suggestion = RawValue {
                    ..value.clone()
                };
                if !matches_nospace(meta, &suggestion) {
                    suggestion.value.push(' ');
                }
                Suggestion {
                    value: sanitize(&suggestion.value),
                    display: sanitize(&suggestion.display),
                }
            })
            .collect();
        serde_json::to_string(&suggestions).expect("ion serialization cannot fail")
    }
}

/// bash-ble: tab-delimited with `\x1c`-separated suffix field.
pub mod bash_ble {
    use super::*;

    pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
        values
            .iter()
            .map(|value| {
                let suffix = if matches_nospace(meta, value) { "" } else { " " };
                format!(
                    "{}\t{}\x1c{}\x1c{}\x1c{}",
                    value.value,
                    value.display,
                    "",
                    suffix,
                    trimmed_description(&value.description)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn snippet(binary: &str) -> String {
        format!(
            "complete -F _{binary}_completion {binary}\n\n_{binary}_completion() {{\n  local IFS=$'\\n'\n  local response\n  response=$(echo \"${{COMP_LINE:0:${{COMP_POINT}}}}''\" | xargs {binary} _carapace bash-ble 2>/dev/null)\n  ble/complete/cand/yield -S file ${{response//$'\\x1c'/\\x1c}}\n}}\n"
        )
    }
}
