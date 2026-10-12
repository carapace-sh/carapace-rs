//! Elvish value formatting (port of Go `internal/shell/elvish/action.go`).

use serde::Serialize;

use super::{Flags, trimmed_description};
use crate::meta::Meta;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], "")
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
pub fn action_raw_values(
    _current_word: &str,
    meta: &Meta,
    values: &RawValues,
    flags: &Flags,
) -> String {
    // Go falls back to the configured styles; unparseable ones keep "default"
    let carapace_styles = crate::config::styles().carapace;
    let value_style = fallback_style(&carapace_styles.value);
    let description_style = fallback_style(&carapace_styles.description);

    let candidates: Vec<ComplexCandidate> = values
        .iter()
        .map(|value| {
            let suffix = if matches_nospace(meta, value) {
                ""
            } else {
                " "
            };
            let style = if value.style.is_empty() {
                value_style.clone()
            } else {
                value.style.clone()
            };
            ComplexCandidate {
                value: sanitize(&value.value),
                display: sanitize(&value.display),
                description: trimmed_description(
                    &sanitize(&value.description),
                    flags.description_length,
                ),
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
        description_style,
        candidates,
    };
    serde_json::to_string(&completion).expect("elvish serialization cannot fail")
}

/// A style string for display, defaulting to `"default"` when unset
/// (Go's elvish `valueStyle`/`descriptionStyle` fallback).
fn fallback_style(style: &str) -> String {
    if style.is_empty() {
        "default".to_owned()
    } else {
        style.to_owned()
    }
}

/// Generate the elvish completion snippet (Go `elvish.SnippetSingle`).
pub fn snippet(binary: &str) -> String {
    format!(
        r##"set edit:completion:arg-completer[{binary}] = {{|@arg|
    {binary} _carapace elvish (all $arg) | from-json | each {{|completion|
		put $completion[Messages] | all (one) | each {{|m|
			edit:notify (styled "error: " red)$m
		}}
		if (not-eq $completion[Usage] "") {{
			edit:notify (styled "usage: " $completion[DescriptionStyle])$completion[Usage]
		}}
		put $completion[Candidates] | all (one) | peach {{|c|
			if (eq $c[Description] "") {{
		    	edit:complex-candidate $c[Value] &display=(styled $c[Display] $c[Style]) &code-suffix=$c[CodeSuffix]
			}} else {{
		    	edit:complex-candidate $c[Value] &display=(styled $c[Display] $c[Style])(styled " " $completion[DescriptionStyle]" bg-default")(styled "("$c[Description]")" $completion[DescriptionStyle]) &code-suffix=$c[CodeSuffix]
			}}
		}}
    }}
}}"##
    )
}
