//! Xonsh value formatting (port of Go `internal/shell/xonsh/action.go`).

use serde::Serialize;

use crate::meta::Meta;
use crate::value::RawValues;
use crate::shell::style::Style;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\t'], "").replace('\'', "\\'")
}

fn trimmed_description(description: &str) -> String {
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

fn matches_nospace(meta: &Meta, value: &str) -> bool {
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.ends_with(*suffix))
}

fn convert_style(style: &str) -> String {
    let parsed = Style::parse(&format!("bg-default fg-default {style}"));
    let mut parts = Vec::new();
    if let Some(bg) = &parsed.background {
        parts.push(format!("bg:{bg}"));
    }
    if let Some(fg) = &parsed.foreground {
        parts.push(format!("fg:{fg}"));
    }
    if parsed.bold { parts.push("bold".to_owned()); }
    if parsed.italic { parts.push("italic".to_owned()); }
    if parsed.underlined { parts.push("underline".to_owned()); }
    parts.join(" ")
}

#[derive(Serialize)]
struct RichCompletion {
    #[serde(rename = "Value")]
    value: String,
    #[serde(rename = "Display")]
    display: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "Style")]
    style: String,
}

/// Format values for xonsh as a JSON array of RichCompletions.
pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
    let records: Vec<RichCompletion> = values
        .iter()
        .map(|value| {
            let sanitized = sanitize(&value.value);
            let append_space = !matches_nospace(meta, &sanitized);
            let mut quoted = sanitized;
            if quoted.contains(|c: char| " ()[]{}*$?\"|<>&;#`".contains(c)) {
                if quoted.contains('\\') {
                    quoted = format!("r'{quoted}'");
                } else {
                    quoted = format!("'{quoted}'");
                }
            }
            if append_space {
                quoted.push(' ');
            }
            RichCompletion {
                value: quoted,
                display: sanitize(&value.display),
                description: trimmed_description(&value.description),
                style: convert_style(&value.style),
            }
        })
        .collect();
    serde_json::to_string(&records).expect("xonsh serialization cannot fail")
}

/// Generate the xonsh completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"from xonsh.completers._aliases import _add_one_completer
__xsh_data = $(echo "" | {binary} _carapace xonsh)
_add_one_completer({{}}["{binary}"], __xsh_data, "readline", "")
del __xsh_data
"#
    )
}
