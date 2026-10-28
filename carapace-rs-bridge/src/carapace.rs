//! Bridge completions from a Go carapace binary via the export JSON protocol.

use carapace_rs::Action;

/// Bridge completions from the Go carapace binary: parses the export JSON.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        let mut args = vec!["_carapace".to_owned(), "export".to_owned(), String::new()];
        args.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = args.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&command_name, &args) {
                Ok(output) if output.is_empty() => Action::default(),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}
