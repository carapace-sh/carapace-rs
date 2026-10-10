//! Simple line-based formatters: oil, tcsh, cmd-clink, ion, bash-ble.

use super::trimmed_description;
use super::{common_display_prefix, common_value_prefix, matches_nospace};
use crate::meta::Meta;
use crate::value::{RawValue, RawValues};

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
                    format!(
                        "{} ({})",
                        value.value,
                        sanitize(&trimmed_description(&value.description))
                    )
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
                '\\' | '&'
                    | '<'
                    | '>'
                    | '`'
                    | '\''
                    | '"'
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
                    let description = quoter(
                        &sanitize(&trimmed_description(&value.description)).replace(' ', "_"),
                    );
                    format!("{}_({})", quoter(&sanitize(&value.value)), description)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn snippet(binary: &str) -> String {
        format!("complete {binary} 'C@*,p@{binary} `echo $COMMAND_LINE[:]` _carapace tcsh@'\n")
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
                let append_char = if matches_nospace(meta, value) {
                    ""
                } else {
                    " "
                };
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
                let mut suggestion = RawValue { ..value.clone() };
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
                let suffix = if matches_nospace(meta, value) {
                    ""
                } else {
                    " "
                };
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
