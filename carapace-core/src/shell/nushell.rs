//! Nushell value formatting (port of Go `internal/shell/nushell/action.go`).

use serde::Serialize;

use crate::meta::Meta;
use crate::shell::style::Style;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r'], "")
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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

#[derive(Serialize, Clone, Default)]
struct NushellStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attr: Option<String>,
}

fn convert_style(style: &str) -> Option<NushellStyle> {
    if style.is_empty() {
        return None;
    }
    let parsed = Style::parse(style);
    let mut result = NushellStyle::default();
    let mut attributes = String::new();
    if parsed.blink {
        attributes.push('l');
    }
    if parsed.bold {
        attributes.push('b');
    }
    if parsed.dim {
        attributes.push('d');
    }
    if parsed.italic {
        attributes.push('i');
    }
    if parsed.inverse {
        attributes.push('r');
    }
    if parsed.underlined {
        attributes.push('u');
    }
    result.fg = parsed.foreground;
    result.bg = parsed.background;
    if !attributes.is_empty() {
        result.attr = Some(attributes);
    }
    Some(result)
}

#[derive(Serialize)]
struct Record {
    value: String,
    display: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<NushellStyle>,
}

/// Format values for nushell as a JSON array of records.
pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
    let records: Vec<Record> = values
        .iter()
        .map(|value| {
            let nospace = matches_nospace(meta, &value.value);
            let mut quoted = sanitize(&value.value);
            if quoted.contains(|c: char| " {}()[]<>$&\"'|;#\\`".contains(c)) {
                if quoted.starts_with('~') {
                    quoted = format!("~\"{}\"", escape(&quoted[1..]));
                } else {
                    quoted = format!("\"{}\"", escape(&quoted));
                }
            }
            if !nospace {
                quoted.push(' ');
            }
            Record {
                value: quoted,
                display: sanitize(&value.display),
                description: trimmed_description(&value.description),
                style: convert_style(&value.style),
            }
        })
        .collect();
    serde_json::to_string(&records).expect("nushell serialization cannot fail")
}

/// Generate the nushell completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"let {binary}_completer = {{|spans|
  {binary} _carapace nushell ...$spans | from json
}}
$env.config = ($env.config? | default {{}})
$env.config.completions = ($env.config.completions? | default {{ external: {{}} }})
$env.config.completions.external = ($env.config.completions.external? | default {{}})
$env.config.completions.external.completer = ($env.config.completions.external.completer? | default {{}} | merge {{ "{binary}": ${binary}_completer }})
"#
    )
}
