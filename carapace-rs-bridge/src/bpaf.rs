//! Bpaf bridge: `--bpaf-complete-rev=9` fish-style output, in reverse
//! order (port of Go `bridge.ActionBpaf`).
//!
//! Note: bpaf's protocol only exposes preferred (long) flag names and
//! evaluates help/version after completion output, so shorthands and
//! `--help`/`--version` are not completed (parity with bpaf's own
//! completions).

use carapace_rs::Action;

/// Bridge completions registered in bpaf (pacak/bpaf).
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

            let mut args = vec!["--bpaf-complete-rev=9".to_owned()];
            args.extend(command[1..].iter().cloned());
            args.extend(c.args.iter().cloned());
            // an empty value is the sentinel for "completing after a space"
            args.push(c.value.clone());

            match c.execute(&command[0], &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut lines: Vec<&str> = text.split('\n').collect();
                    // fish output is in reverse order
                    lines.reverse();

                    let mut values = Vec::new();
                    for line in lines {
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
                        return Action::files([]);
                    }

                    let mut action = Action::raw(values);
                    // a nospace character within any line (excluding its
                    // last character) suppresses the trailing space
                    let any_nospace = text
                        .lines()
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
