//! Jj bridge: jj-vcs' fish-style `COMPLETE` protocol
//! (port of Go `bridge.ActionJJ`).

use carapace_rs::Action;

/// Bridge completions registered in jj (jj-vcs).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut context = c.clone();
            context.env.insert("COMPLETE".to_owned(), "fish".to_owned());

            let mut args = vec!["--".to_owned()];
            args.extend(command.iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            match context.execute(&command[0], args.iter().map(String::as_str)) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut lines: Vec<&str> = text.split('\n').collect();
                    if lines.last() == Some(&"") {
                        lines.pop();
                    }
                    // jj mixes flags and values in one stream; only the
                    // kind matching the current word is offered
                    let flags = c.value.starts_with('-');
                    let values: Vec<carapace_rs::RawValue> = lines
                        .iter()
                        .filter_map(|line| {
                            let (value, description) = match line.split_once('\t') {
                                Some((value, description)) => (value, description),
                                None => (*line, ""),
                            };
                            (flags == value.starts_with('-'))
                                .then(|| carapace_rs::RawValue::described(value, description))
                        })
                        .collect();
                    Action::raw(values)
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
