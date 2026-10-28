//! Bridge completions registered in carapace-bin (the `carapace` binary).

use carapace_rs::Action;

/// Bridge completions registered in carapace-bin.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let bin = "carapace".to_owned();
        let mut prefix = vec![command[0].clone(), "export".to_owned(), String::new()];
        prefix.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = prefix.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&bin, args.iter().map(String::as_str)) {
                Ok(output) if output.is_empty() => Action::files([]),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}
