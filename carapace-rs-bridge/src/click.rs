//! Click bridge: click's zsh-completion protocol via
//! `_<PROGRAM>_COMPLETE=zsh_complete` and `COMP_WORDS`/`COMP_CWORD`
//! (port of Go `bridge.ActionClick`).

use carapace_rs::Action;

/// Bridge completions registered in click (pallets/click).
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

            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            let mut words = args.clone();
            words.push(c.value.clone());
            let comp_line = format!(
                "{} {}",
                command[0],
                carapace_rs_shlex::join(&words, carapace_rs_shlex::Format::Zsh)
            );

            let mut context = c.clone();
            context.env.insert(
                format!("_{}_COMPLETE", command[0].to_uppercase()),
                "zsh_complete".to_owned(),
            );
            context.env.insert("COMP_WORDS".to_owned(), comp_line);
            context
                .env
                .insert("COMP_CWORD".to_owned(), (args.len() + 1).to_string());

            match context.execute(&command[0], Vec::<String>::new()) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    // Go splits on "\n" and keeps the trailing empty
                    // element, which the triple stride relies on
                    let lines: Vec<&str> = text.split('\n').collect();
                    let mut values = Vec::new();
                    let mut index = 0;
                    while index + 3 < lines.len() {
                        match lines[index] {
                            // type
                            "dir" => return Action::directories(),
                            "file" => return Action::files(Vec::<String>::new()),
                            "plain" => {
                                let description = match lines[index + 2] {
                                    "_" => String::new(),
                                    other => other.to_owned(),
                                };
                                values.push(carapace_rs::RawValue::described(
                                    lines[index + 1],
                                    description,
                                ));
                            }
                            _ => {}
                        }
                        index += 3;
                    }
                    Action::raw(values)
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
