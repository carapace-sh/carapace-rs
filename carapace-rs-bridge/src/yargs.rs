//! Yargs bridge: `--get-yargs-completions` with `SHELL=zsh`
//! (port of Go `bridge.ActionYargs`).

use carapace_rs::Action;

/// Bridge completions registered in yargs (yargs/yargs).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            if super::look_path(c, &command[0]).is_none() {
                return Action::message(format!("command not found: {}", command[0]));
            }

            let mut context = c.clone();
            context.env.insert("SHELL".to_owned(), "zsh".to_owned());

            let mut args = vec!["--get-yargs-completions".to_owned()];
            args.extend(command[1..].iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            match context.execute(&command[0], &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut values = Vec::new();
                    for line in text.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        let (name, description) = match line.split_once(':') {
                            Some((name, description)) => (name, description),
                            None => (line, ""),
                        };
                        values.push(carapace_rs::RawValue::described(name, description));
                    }
                    if values.is_empty() {
                        return Action::files([]);
                    }
                    Action::raw(values)
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
