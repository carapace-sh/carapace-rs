//! Xonsh value formatting (port of Go `internal/shell/xonsh/action.go`).

use serde::Serialize;

use super::{Flags, trimmed_description};
use crate::meta::Meta;
use crate::style::Style;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\t'], "").replace('\'', "\\'")
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
    if parsed.bold {
        parts.push("bold".to_owned());
    }
    if parsed.italic {
        parts.push("italic".to_owned());
    }
    if parsed.underlined {
        parts.push("underline".to_owned());
    }
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
pub fn action_raw_values(
    _current_word: &str,
    meta: &Meta,
    values: &RawValues,
    flags: &Flags,
) -> String {
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
                description: trimmed_description(&value.description, flags.description_length),
                style: convert_style(&value.style),
            }
        })
        .collect();
    serde_json::to_string(&records).expect("xonsh serialization cannot fail")
}

/// Generate the xonsh completion snippet for a binary.
/// Generate the xonsh completion snippet (Go `xonsh.SnippetSingle`).
pub fn snippet(binary: &str) -> String {
    let function_name = binary.replace('-', "__");
    format!(
        r#"from xonsh.completers.completer import add_one_completer
from xonsh.completers.tools import contextual_command_completer

@contextual_command_completer
def _{function_name}_completer(context):
    """carapace completer for {binary}"""
    if context.completing_command('{binary}'):
        from json import loads
        from xonsh.completers.tools import sub_proc_get_output, RichCompletion

        def fix_prefix(s):
            """quick fix for partially quoted prefix completion ('prefix',<TAB>)"""
            return s.translate(str.maketrans('', '', '\'"'))

        output, _ = sub_proc_get_output(
            '{binary}', '_carapace', 'xonsh', *[a.value for a in context.args], fix_prefix(context.prefix)
        )

        try:
            result = {{RichCompletion(c["Value"], display=c["Display"], description=c["Description"], prefix_len=len(context.raw_prefix), append_closing_quote=False, style=c["Style"]) for c in loads(output)}}
        except:
            result = {{}}
        if len(result) == 0:
            result = {{RichCompletion(context.prefix, display=context.prefix, description='', prefix_len=len(context.raw_prefix), append_closing_quote=False)}}
        return result

add_one_completer('{binary}', _{function_name}_completer, 'start')
"#
    )
}
