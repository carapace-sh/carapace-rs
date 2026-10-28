//! Urfavecli bridge: urfave/cli's shell-completion flags
//! (port of Go `bridge.ActionUrfavecli` / `ActionUrfavecliV1`).

use carapace_rs::Action;

/// Bridge completions registered in urfave/cli (v3).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    urfavecli_v3(command)
}

/// Bridge completions registered in urfave/cli (v3).
pub fn urfavecli_v3<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            args.push("--generate-shell-completion".to_owned());

            let mut context = c.clone();
            context.env.insert("SHELL".to_owned(), "zsh".to_owned());
            match context.execute(&command[0], &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.split('\n').collect();
                    if lines.len() <= 1 {
                        return Action::files([]);
                    }
                    let mut values = Vec::new();
                    for line in lines {
                        if line.is_empty() {
                            continue;
                        }
                        let (name, description) = match line.split_once(':') {
                            Some((name, description)) => (name, description),
                            None => (line, ""),
                        };
                        values.push(carapace_rs::RawValue::described(name, description));
                    }
                    Action::raw(values).nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bridge completions registered in urfave/cli (v1–v2).
pub fn urfavecli_v1<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            args.push("--generate-bash-completion".to_owned());

            match c.execute(&command[0], &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.split('\n').collect();
                    if lines.len() <= 1 {
                        return Action::files([]);
                    }
                    Action::values_with(lines[..lines.len() - 1].iter().map(|l| l.to_string()))
                        .nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
