//! Bridge completions from an external clap binary (unstable-dynamic engine).

use carapace_rs::Action;

/// Bridge completions from an external clap binary (unstable-dynamic engine).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        Action::callback(move |c| {
            let index = command.len() + c.args.len();
            let mut args = vec![
                "complete".to_owned(),
                "--index".to_owned(),
                index.to_string(),
                "--type".to_owned(),
                "9".to_owned(),
                "--no-space".to_owned(),
                "--ifs=\n".to_owned(),
                "--".to_owned(),
            ];
            args.extend(command.iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&command_name, args.iter().map(String::as_str)) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.lines().collect();
                    let mut action = Action::values(lines.iter().copied());
                    for line in &lines {
                        if !line.is_empty() {
                            // everything but the last character (Go slices
                            // bytes; here the boundary must stay a char
                            // boundary or multi-byte output panics)
                            let last = line.chars().last().map_or(0, char::len_utf8);
                            if line[..line.len() - last].contains(['/', '=', '@', ':', '.', ',']) {
                                action = action.nospace([]);
                                break;
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
