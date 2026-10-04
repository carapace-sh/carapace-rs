//! PowerShell value formatting (port of Go `internal/shell/powershell/action.go`).

use serde::Serialize;

use crate::meta::Meta;
use crate::value::RawValues;
use crate::shell::sgr;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\t'], "")
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

/// CompletionResult doesn't like empty parameters; replace with a space.
fn ensure_not_empty(value: &str) -> String {
    if value.is_empty() {
        " ".to_owned()
    } else {
        value.to_owned()
    }
}

#[derive(Serialize)]
struct CompletionResult {
    #[serde(rename = "CompletionText")]
    completion_text: String,
    #[serde(rename = "ListItemText")]
    list_item_text: String,
    #[serde(rename = "ToolTip")]
    tool_tip: String,
}

fn color_disabled() -> bool {
    std::env::var("CARAPACE_COLOR").is_ok_and(|value| value == "disabled")
}

fn tooltip_enabled() -> bool {
    std::env::var("CARAPACE_TOOLTIP")
        .map(|value| value != "0" && !value.is_empty())
        .unwrap_or(true)
}

/// Format values for PowerShell as a JSON array of CompletionResults.
pub fn action_raw_values(_current_word: &str, meta: &Meta, values: &RawValues) -> String {
    let nocolor = color_disabled();
    let tooltip_enabled = tooltip_enabled();

    let results: Vec<CompletionResult> = values
        .iter()
        .filter(|value| !value.value.is_empty())
        .map(|value| {
            let sanitized = sanitize(&value.value);
            let nospace = matches_nospace(meta, &sanitized);
            let mut completion_text = sanitized;
            if completion_text.contains(|c: char| " {}()[]*$?\"|<>&(),;#`".contains(c)) {
                completion_text = format!("'{completion_text}'");
            }
            if !nospace {
                completion_text.push(' ');
            }

            let style = if value.style.is_empty() {
                "default".to_owned()
            } else {
                value.style.clone()
            };

            let mut tooltip = " ".to_owned();
            let mut list_item_text;
            let mut description = sanitize(&value.description);
            if tooltip_enabled && !description.is_empty() {
                tooltip = if nocolor {
                    trimmed_description(&description)
                } else {
                    format!(
                        "`e[{}m`e[{}m{}`e[21;22;23;24;25;29;39;49m",
                        sgr("default"),
                        sgr("underlined"),
                        trimmed_description(&description)
                    )
                };
                description = String::new();
            }

            if nocolor {
                list_item_text = sanitize(&value.display);
                if !description.is_empty() {
                    list_item_text =
                        format!("{list_item_text} ({})", trimmed_description(&description));
                }
            } else {
                list_item_text = format!(
                    "`e[21;22;23;24;25;29m`e[{}m{}`e[21;22;23;24;25;29;39;49m",
                    sgr(&style),
                    sanitize(&value.display)
                );
                if !description.is_empty() {
                    list_item_text.push_str(&format!(
                        "`e[{}m `e[{}m({})`e[21;22;23;24;25;29;39;49m",
                        sgr("default"),
                        sgr("underlined"),
                        trimmed_description(&description)
                    ));
                }
                list_item_text.push_str("`e[0m");
            }

            CompletionResult {
                completion_text,
                list_item_text: ensure_not_empty(&list_item_text),
                tool_tip: ensure_not_empty(&tooltip),
            }
        })
        .collect();
    serde_json::to_string(&results).expect("powershell serialization cannot fail")
}

/// Generate the PowerShell completion snippet for a binary.
pub fn snippet(binary: &str) -> String {
    format!(
        r#"Register-ArgumentCompleter -Native -CommandName '{binary}' -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = "$($commandElements[0])"
    $commandElements = $($commandElements | Select-Object -Skip 1)
    $arguments = $commandElements -join ' '
    $compline = $command + ' ' + $arguments

    $data = echo "$($compline)''" | xargs {binary} _carapace powershell
    if ($LASTEXITCODE -eq 1) {{
        $data = echo "$($compline)'" | xargs {binary} _carapace powershell
        if ($LASTEXITCODE -eq 1) {{
            $data = echo "$($compline)`"" | xargs {binary} _carapace powershell
        }}
    }}

    ConvertFrom-Json $data | ForEach-Object -Process {{
        [System.Management.Automation.CompletionResult]::new($_.CompletionText, $_.ListItemText, 'ParameterValue', $_.ToolTip)
    }}
}}
"#
    )
}
