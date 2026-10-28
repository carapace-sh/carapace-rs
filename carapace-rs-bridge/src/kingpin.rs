//! Kingpin bridge: `--completion-bash` output
//! (port of Go `bridge.ActionKingpin`).

use carapace_rs::Action;

/// Bridge completions registered in kingpin (alecthomas/kingpin).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut args = vec!["--completion-bash".to_owned()];
            args.extend(command[1..].iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());

            match c.execute(&command[0], &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.split('\n').collect();

                    // fewer than two lines without a flag word means the
                    // completer had nothing: fall back to files
                    if lines.len() < 2 && !c.value.starts_with('-') {
                        return Action::files([]);
                    }

                    let mut action = Action::values(lines.iter().map(|l| l.to_string()));
                    let any_nospace = lines
                        .iter()
                        .any(|line| line.chars().rev().skip(1).any(|c| "/=@:.,".contains(c)));
                    if any_nospace {
                        action = action.nospace([]);
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
