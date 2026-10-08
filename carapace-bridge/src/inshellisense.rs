//! Inshellisense bridge: `inshellisense complete <input>` with JSON output.

use carapace_core::Action;

/// Bridge completions from microsoft/inshellisense.
pub fn action(command: &[&str]) -> Action {
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            let mut input = super::shell::bash_join(&args);
            if input.ends_with("''") {
                // inshellisense can't handle quotes yet; the empty word
                // degrades to a trailing space
                input = format!("{} ", &input[..input.len() - 2]);
            }

            match c.execute("inshellisense", &["complete", &input]) {
                Ok(output) => parse_output(&String::from_utf8_lossy(&output), &c.value),
                Err(error) => Action::message(error),
            }
        })
    })
}

fn parse_output(output: &str, value: &str) -> Action {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) else {
        return Action::message("failed to parse inshellisense output");
    };
    let suggestions = parsed
        .get("suggestions")
        .and_then(|suggestions| suggestions.as_array())
        .cloned()
        .unwrap_or_default();

    let flag_value = value.starts_with('-') && value.contains('=');
    let mut vals: Vec<(String, String)> = Vec::new();
    let mut flags: Vec<(String, String)> = Vec::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for suggestion in suggestions {
        let description = suggestion
            .get("description")
            .and_then(|description| description.as_str())
            .unwrap_or_default()
            .to_owned();
        let icon = suggestion
            .get("icon")
            .and_then(|icon| icon.as_str())
            .unwrap_or_default();
        let names: Vec<String> = match suggestion
            .get("allNames")
            .and_then(|names| names.as_array())
        {
            Some(names) => names
                .iter()
                .filter_map(|name| name.as_str())
                .map(String::from)
                .collect(),
            None => match suggestion.get("name").and_then(|name| name.as_str()) {
                Some(name) => vec![name.to_owned()],
                None => continue,
            },
        };

        for name in names {
            if !value.starts_with('-') && name.starts_with('-') {
                continue;
            }
            if !(name.starts_with(value) || flag_value) {
                continue;
            }
            match icon {
                "\u{1f4c0}" => files.push((name, description.clone())), // 📀
                "\u{1f517}" => flags.push((name, description.clone())), // 🔗
                _ => vals.push((name, description.clone())),
            }
        }
    }

    let mut action = carapace_core::batch(vec![
        Action::values_described(vals),
        Action::values_described(flags).tag("flags"),
        Action::values_described(files).tag("files"),
    ]);
    if flag_value {
        let prefix = value
            .split_once('=')
            .map(|(prefix, _)| format!("{prefix}="))
            .unwrap_or_default();
        action = action.prefix(prefix);
    }
    action
}
