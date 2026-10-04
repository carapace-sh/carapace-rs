//! Elvish value formatting (port of Go `internal/shell/elvish/action.go`).

use serde::Serialize;

use crate::meta::Meta;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
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

fn matches_nospace(meta: &Meta, value: &crate::value::RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}

#[derive(Serialize)]
struct Completion {
    #[serde(rename = "Usage")]
    usage: String,
    #[serde(rename = "Messages")]
    messages: Vec<String>,
    #[serde(rename = "DescriptionStyle")]
    description_style: String,
    #[serde(rename = "Candidates")]
    candidates: Vec<ComplexCandidate>,
}

#[derive(Serialize)]
struct ComplexCandidate {
    #[serde(rename = "Value")]
    value: String,
    #[serde(rename = "Display")]
    display: String,
    #[serde(rename = "Description")]
    description: String,
    #[serde(rename = "CodeSuffix")]
    code_suffix: String,
    #[serde(rename = "Style")]
    style: String,
}

/// Format values for elvish as a JSON completion object.
pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
    let candidates: Vec<ComplexCandidate> = values
        .iter()
        .map(|value| {
            let suffix = if matches_nospace(meta, value) { "" } else { " " };
            let style = if value.style.is_empty() {
                "default".to_owned()
            } else {
                value.style.clone()
            };
            ComplexCandidate {
                value: sanitize(&value.value),
                display: sanitize(&value.display),
                description: trimmed_description(&sanitize(&value.description)),
                code_suffix: suffix.to_owned(),
                style,
            }
        })
        .collect();

    let usage = if candidates.is_empty() {
        meta.usage.clone().unwrap_or_default()
    } else {
        // edit:notify is persistent; avoid spamming the user
        String::new()
    };

    let completion = Completion {
        usage,
        messages: meta.messages.clone(),
        description_style: "default".to_owned(),
        candidates,
    };
    serde_json::to_string(&completion).expect("elvish serialization cannot fail")
}

/// Generate the elvish completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    let mut snippet = String::new();
    snippet.push_str("set edit:completion:arg-completer[");
    snippet.push_str(binary);
    snippet.push_str("] = [@words]{\n");
    snippet.push_str("  var arg = [(all $@words[1:-1] $words[-1])]\n");
    snippet.push_str("  var result = (");
    snippet.push_str(binary);
    snippet.push_str(" _carapace elvish (all $arg) | from-json)\n");
    snippet.push_str("  put $result[Candidates]\n");
    snippet.push_str("}\n");
    snippet
}

