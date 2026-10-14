//! Powershell bridge: `CommandCompletion::CompleteInput` through
//! pwsh with a custom profile (port of Go `bridge.ActionPowershell`).

use carapace_rs::Action;

/// Bridge completions registered in powershell.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let Some(config_dir) = super::config_dir(c) else {
                return Action::message("failed to resolve config dir");
            };
            let profile_path =
                config_dir.join("carapace/bridge/powershell/Microsoft.PowerShell_profile.ps1");
            if let Err(error) = super::ensure_exists(&profile_path) {
                return Action::message(error);
            }

            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            let line = carapace_rs_shlex::join(&args, carapace_rs_shlex::Format::Powershell);
            let snippet = format!(
                "Get-Content \"{}\" | Out-String | Invoke-Expression; [System.Management.Automation.CommandCompletion]::CompleteInput(\"{}\", {}, $null).CompletionMatches | ConvertTo-Json ",
                profile_path.to_string_lossy(),
                line,
                line.chars().count(),
            );

            match c.execute("pwsh", ["-Command", &snippet]) {
                Ok(output) if output.is_empty() => Action::default(),
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&text);
                    let result = match parsed {
                        Ok(serde_json::Value::Array(entries)) => entries,
                        Ok(single) => vec![single],
                        Err(error) => return Action::message(error.to_string()),
                    };

                    let mut suffixes: Vec<char> = Vec::new();
                    let mut values = Vec::new();
                    for entry in result {
                        let completion_text = entry
                            .get("CompletionText")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned();
                        let tool_tip = entry
                            .get("ToolTip")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned();
                        let chars: Vec<char> = completion_text.chars().collect();
                        if chars.len() > 2 && completion_text.ends_with(' ') {
                            // the trailing space suppresses only its own char
                            suffixes.push(chars[chars.len() - 1]);
                        }
                        let completion_text = completion_text
                            .strip_suffix(' ')
                            .unwrap_or(&completion_text);
                        let tool_tip = if tool_tip == completion_text {
                            String::new()
                        } else {
                            tool_tip
                        };
                        values.push(carapace_rs::RawValue::described(completion_text, tool_tip));
                    }
                    let action = Action::raw(values).style_f(|context, value| {
                        carapace_rs::style::for_path(context, &value.value)
                    });
                    if suffixes.is_empty() {
                        action
                    } else {
                        action.nospace(suffixes)
                    }
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
