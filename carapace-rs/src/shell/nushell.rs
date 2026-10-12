//! Nushell value formatting (port of Go `internal/shell/nushell/action.go`).

use serde::Serialize;

use super::{Flags, trimmed_description};
use crate::meta::Meta;
use crate::shell::style::Style;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r'], "")
}

/// Patch arguments passed unprocessed by nushell: dequote quoted strings
/// and trim backtick strings (port of Go `nushell.Patch`).
///
/// see https://www.nushell.sh/book/working_with_strings.html
pub fn patch(args: Vec<String>) -> Vec<String> {
    let patched: Vec<String> = args
        .into_iter()
        .map(|arg| match arg.chars().next() {
            Some(quote @ ('"' | '\'' | '`')) if arg.ends_with(quote) && arg.len() > 1 => {
                carapace_rs_shlex::complete(&arg, carapace_rs_shlex::Format::Nushell).current_word
            }
            _ => arg,
        })
        .collect();
    crate::log::log(&format!("patching args to {patched:?}"));
    patched
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
pub fn action_raw_values(
    _current_word: &str,
    meta: &Meta,
    values: &RawValues,
    flags: &Flags,
) -> String {
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
                description: trimmed_description(&value.description, flags.description_length),
                style: convert_style(&value.style),
            }
        })
        .collect();
    serde_json::to_string(&records).expect("nushell serialization cannot fail")
}

/// Generate the nushell completion snippet (Go `nushell.SnippetSingle`).
///
/// Standalone mode generates only the completer function; the config
/// wiring is the caller's responsibility (Go keeps it for the multi
/// completer variant only).
pub fn snippet(binary: &str) -> String {
    let sanitized = binary.replace('-', "__");
    format!(
        r#"let {sanitized}_completer = {{|place|
    # backwards compatible workaround for positional completer input, see nushell #18791
    let spans = (if ($place | describe) =~ "record" {{ $place.command }} else {{ $place }})
    {binary} _carapace nushell ...$spans | from json
}}"#
    )
}
