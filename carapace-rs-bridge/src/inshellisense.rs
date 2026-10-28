//! Inshellisense bridge: `inshellisense complete <input>` with JSON output.

use carapace_rs::Action;

/// Bridge completions from microsoft/inshellisense.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            let mut input = carapace_rs_shlex::join(&args, carapace_rs_shlex::Format::Bash);
            if input.ends_with("\"\"") {
                // inshellisense can't handle quotes yet; the empty word
                // degrades to a trailing space
                input = format!("{} ", &input[..input.len() - 2]);
            }

            match c.execute("inshellisense", ["complete", &input]) {
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

    let mut action = carapace_rs::batch(vec![
        Action::values_described(
            vals.iter()
                .map(|(value, description)| (value.as_str(), description.as_str())),
        ),
        Action::values_described(
            flags
                .iter()
                .map(|(value, description)| (value.as_str(), description.as_str())),
        )
        .tag("flags"),
        Action::values_described(
            files
                .iter()
                .map(|(value, description)| (value.as_str(), description.as_str())),
        )
        .tag("files")
        .style_f(|context, value| carapace_rs::style::for_path_ext(context, &value.value)),
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
