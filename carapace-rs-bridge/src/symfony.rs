//! Symfony bridge: the console `_complete` command with zsh input
//! tokens (port of Go `bridge.ActionSymfony`).

use carapace_rs::Action;

/// Bridge completions registered in symfony (symfony/console).
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

            let current = c.value.clone();
            let mut tokens: Vec<String> = command.clone();
            tokens.extend(c.args.iter().cloned());

            // the cursor floats at the end unless a current word is typed
            let mut current_index = tokens.len();
            if !current.is_empty() {
                tokens.push(current.clone());
                current_index = tokens.len() - 1;
            }

            let mut args = vec![
                "_complete".to_owned(),
                "--no-interaction".to_owned(),
                "-s".to_owned(),
                "zsh".to_owned(),
                "-c".to_owned(),
                current_index.to_string(),
            ];
            for token in &tokens {
                if token.is_empty() {
                    continue; // empty tokens are skipped like in the official script
                }
                args.push(format!("-i{token}"));
            }

            let mut context = c.clone();
            context
                .env
                .insert("SHELL_VERBOSITY".to_owned(), "0".to_owned());

            match context.execute(&command[0], &args) {
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

                    let mut action = Action::raw(values);
                    // complete `--flag=<TAB>` by prefixing suggested values
                    // with the flag, unless the suggestions are options
                    // themselves
                    if current.starts_with('-') {
                        if let Some((flag, _)) = current.split_once('=') {
                            let options = values_start_with_option(&action);
                            if !options {
                                action = action.prefix(format!("{flag}="));
                            }
                        }
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Whether any of the completed candidates is itself an option: peek at
/// the invoked values (Go checks the parsed value list).
fn values_start_with_option(action: &Action) -> bool {
    let invoked = action.invoke(&carapace_rs::Context::default());
    invoked
        .values
        .iter()
        .any(|value| value.value.starts_with('-'))
}
