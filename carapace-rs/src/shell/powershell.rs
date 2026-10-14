//! PowerShell value formatting (port of Go `internal/shell/powershell/action.go`).

use serde::Serialize;

use super::{Flags, trimmed_description};
use crate::meta::Meta;
use crate::style::sgr;
use crate::value::RawValues;

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\t'], "")
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

/// Format values for PowerShell as a JSON array of CompletionResults.
pub fn action_raw_values(
    _current_word: &str,
    meta: &Meta,
    values: &RawValues,
    flags: &Flags,
) -> String {
    let nocolor = flags.color_disabled;
    let tooltip_enabled = flags.tooltip;

    // default styles for values and descriptions consult the carapace
    // style configuration (Go `powershell.ActionRawValues`)
    let carapace_styles = crate::config::styles().carapace;
    let value_style =
        if !carapace_styles.value.is_empty() && crate::style::is_parsable(&carapace_styles.value) {
            carapace_styles.value.clone()
        } else {
            "default".to_owned()
        };
    let description_style = if !carapace_styles.description.is_empty()
        && crate::style::is_parsable(&carapace_styles.description)
    {
        carapace_styles.description.clone()
    } else {
        "default".to_owned()
    };

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

            let style = if value.style.is_empty() || !crate::style::is_parsable(&value.style) {
                value_style.clone()
            } else {
                value.style.clone()
            };

            let mut tooltip = " ".to_owned();
            let mut list_item_text;
            let mut description = sanitize(&value.description);
            if tooltip_enabled && !description.is_empty() {
                tooltip = if nocolor {
                    trimmed_description(&description, flags.description_length)
                } else {
                    format!(
                        "`e[{}m`e[{}m{}`e[21;22;23;24;25;29;39;49m",
                        sgr(&format!("{description_style} bg-default")),
                        sgr(&description_style),
                        trimmed_description(&description, flags.description_length)
                    )
                };
                description = String::new();
            }

            if nocolor {
                list_item_text = sanitize(&value.display);
                if !description.is_empty() {
                    list_item_text = format!(
                        "{list_item_text} ({})",
                        trimmed_description(&description, flags.description_length)
                    );
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
                        sgr(&format!("{description_style} bg-default")),
                        sgr(&description_style),
                        trimmed_description(&description, flags.description_length)
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

/// Generate the powershell completion snippet (Go `powershell.SnippetSingle`).
pub fn snippet(binary: &str) -> String {
    let sanitized = binary.replace('-', "__");
    format!(
        r##"using namespace System.Management.Automation
using namespace System.Management.Automation.Language
Function _{sanitized}_completer {{
    [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("PSAvoidUsingInvokeExpression", "", Scope="Function", Target="*")]
    param($wordToComplete, $commandAst, $cursorPosition)
    $commandElements = $commandAst.CommandElements

    # double quoted value works but seems single quoted needs some fixing (e.g. "example 'acti" -> "example acti")
    $elems = @()
    foreach ($_ in $commandElements) {{
      if ($_.Extent.StartOffset -gt $cursorPosition) {{
          break
      }}
      $t = $_.Extent.Text
      if ($_.Extent.EndOffset -gt $cursorPosition) {{
          $t = $t.Substring(0, $_.Extent.Text.get_Length() - ($_.Extent.EndOffset - $cursorPosition))
      }}

      if ($t.Substring(0,1) -eq "'"){{
        $t = $t.Substring(1)
      }}
      if ($t.get_Length() -gt 0 -and $t.Substring($t.get_Length()-1) -eq "'"){{
        $t = $t.Substring(0,$t.get_Length()-1)
      }}
      if ($t.get_Length() -eq 0){{
        $t = '""'
      }}
      $elems += $t.replace('`,', ',') # quick fix
    }}

    $completions = @(
      if (!$wordToComplete) {{
        {binary} _carapace powershell $($elems| ForEach-Object {{$_}}) '' | ConvertFrom-Json | ForEach-Object {{ [CompletionResult]::new($_.CompletionText, $_.ListItemText.replace('`e[', "`e["), [CompletionResultType]::ParameterValue, $_.ToolTip.replace('`e[', "`e[")) }}
      }} else {{
        {binary} _carapace powershell $($elems| ForEach-Object {{$_}}) | ConvertFrom-Json | ForEach-Object {{ [CompletionResult]::new($_.CompletionText, $_.ListItemText.replace('`e[', "`e["), [CompletionResultType]::ParameterValue, $_.ToolTip.replace('`e[', "`e[")) }}
      }}
    )

    if ($completions.count -eq 0) {{
      return "" # prevent default file completion
    }}

    $completions
}}
Register-ArgumentCompleter -Native -ScriptBlock (Get-Item "Function:_{sanitized}_completer").ScriptBlock -CommandName '{binary}' # ,'{binary}.exe'
"##
    )
}
