//! Typer bridge: the fish-style `_TYPER_COMPLETE_*` protocol
//! (port of Go `bridge.ActionTyper`).

use carapace_rs::Action;

/// Bridge completions registered in typer (fastapi/typer).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            if super::look_path(&command[0]).is_none() {
                return Action::message(format!("command not found: {}", command[0]));
            }

            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            let current = c.value.clone();

            // typer reads the full command line from _TYPER_COMPLETE_ARGS
            // (shlex-style split) and treats the last word as incomplete
            // unless it ends with a space
            let mut words = args.clone();
            if current.is_empty() {
                words.push(String::new());
            } else {
                words.push(current);
            }
            let comp_line = format!(
                "{} {}",
                command[0],
                carapace_rs_shlex::join(&words, carapace_rs_shlex::Format::Bash)
            );

            let sanitized = command[0].replace(['-', '.'], "_");
            let mut context = c.clone();
            context.env.insert(
                format!("_{}_COMPLETE", sanitized.to_uppercase()),
                "complete_fish".to_owned(),
            );
            context.env.insert(
                "_TYPER_COMPLETE_FISH_ACTION".to_owned(),
                "get-args".to_owned(),
            );
            context
                .env
                .insert("_TYPER_COMPLETE_ARGS".to_owned(), comp_line);

            match context.execute(&command[0], Vec::<String>::new()) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut values = Vec::new();
                    for line in text.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        let (value, description) = match line.split_once('\t') {
                            Some((value, description)) => (value, description),
                            None => (line, ""),
                        };
                        values.push(carapace_rs::RawValue::described(value, description));
                    }
                    if values.is_empty() {
                        return Action::files(Vec::<String>::new());
                    }
                    Action::raw(values)
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
