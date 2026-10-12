//! Fish bridge: `fish --no-config --command <snippet>` with a custom
//! config directory under `~/.config/carapace/bridge/fish`.

use carapace_rs::Action;

/// Bridge completions registered in fish.
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
            let fish_config_dir = config_dir.join("carapace/bridge/fish");
            let config_path = fish_config_dir.join("config.fish");
            if let Err(error) = super::ensure_exists(&config_path) {
                return Action::message(error);
            }

            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            // fish-quote the config dir (Go `%[1]q`): a raw path breaks
            // the snippet on apostrophes
            let quoted_dir = carapace_rs_shlex::quote_word(
                &fish_config_dir.to_string_lossy(),
                carapace_rs_shlex::Format::Fish,
            );
            let snippet = format!(
                "source {quoted_dir}/config.fish;complete --do-complete=\"{}\"",
                carapace_rs_shlex::join(&args, carapace_rs_shlex::Format::Fish)
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "\\$"),
            );

            match c.execute("fish", ["--no-config", "--command", &snippet]) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut values = Vec::new();
                    for line in text.lines() {
                        let mut fields = line.splitn(2, '\t');
                        let value = fields.next().unwrap_or_default().to_owned();
                        let description = fields.next().unwrap_or_default().to_owned();
                        values.push(carapace_rs::RawValue::described(value, description));
                    }
                    Action::raw(values).nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
