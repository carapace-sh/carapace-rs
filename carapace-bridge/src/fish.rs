//! Fish bridge: `fish --no-config --command <snippet>` with a custom
//! config directory under `~/.config/carapace/bridge/fish`.

use carapace_core::Action;

/// Fish-quote and join arguments for `complete --do-complete`.
fn fish_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            let escaped = arg.replace('\\', "\\\\").replace('\'', "\\'");
            format!("'{escaped}'")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Bridge completions registered in fish.
pub fn action(command: &[&str]) -> Action {
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let Some(config_dir) = super::config_dir(c) else {
                return Action::message("failed to resolve config dir");
            };
            let fish_config_dir = config_dir.join("carapace/bridge/fish");
            let config_path = fish_config_dir.join("config.fish");
            if let Err(error) = super::ensure_exists(&config_path) {
                return Action::message(error);
            }

            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            let snippet = format!(
                "source '{}/config.fish';complete --do-complete=\"{}\"",
                fish_config_dir.display(),
                fish_join(&args)
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "\\$"),
            );

            match c.execute("fish", &["--no-config", "--command", &snippet]) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut values = Vec::new();
                    for line in text.lines() {
                        let mut fields = line.splitn(2, '\t');
                        let value = fields.next().unwrap_or_default().to_owned();
                        let description = fields.next().unwrap_or_default().to_owned();
                        values.push(carapace_core::RawValue::described(value, description));
                    }
                    Action::raw(values).nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
