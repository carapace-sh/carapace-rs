//! Complete bridge: posener/complete's `COMP_LINE` protocol
//! (port of Go `bridge.ActionComplete`).

use carapace_rs::Action;

/// Bridge completions registered in complete (posener/complete).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            let comp_line = format!("{} {} {}", command[0], args.join(" "), c.value);

            let mut context = c.clone();
            context.env.insert("COMP_LINE".to_owned(), comp_line);

            match context.execute(&command[0], Vec::<String>::new()) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.lines().collect();
                    // the final empty split is dropped (Go `len-1`)
                    let candidates = if lines.last() == Some(&"") {
                        &lines[..lines.len() - 1]
                    } else {
                        &lines[..]
                    };
                    let mut action = Action::values_with(candidates.iter().map(|l| l.to_string()));
                    // a nospace character within any candidate (excluding
                    // its last character) suppresses the trailing space
                    if candidates
                        .iter()
                        .any(|line| line.chars().rev().skip(1).any(|c| "/=@:.,".contains(c)))
                    {
                        action = action.nospace(['/', '=', '@', ':', '.', ',']);
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
