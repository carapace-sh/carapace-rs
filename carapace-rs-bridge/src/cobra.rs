//! Cobra bridge: the `__complete` protocol with `ShellCompDirective` bits.

use carapace_rs::Action;

const DIRECTIVE_ERROR: i32 = 1;
const DIRECTIVE_NO_SPACE: i32 = 2;
const DIRECTIVE_NO_FILE_COMP: i32 = 4;
const DIRECTIVE_FILTER_FILE_EXT: i32 = 8;
const DIRECTIVE_FILTER_DIRS: i32 = 16;

/// Bridge completions from a cobra binary via its hidden `__complete` command.
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        Action::callback(move |c| {
            let mut args = vec!["__complete".to_owned()];
            args.extend(command[1..].iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(
                &command_name,
                args.iter().map(String::as_str).collect::<Vec<_>>(),
            ) {
                Ok(output) => parse_output(&String::from_utf8_lossy(&output), &c.value),
                Err(error) => Action::message(error),
            }
        })
    })
}

fn parse_output(output: &str, value: &str) -> Action {
    let lines: Vec<&str> = output.split('\n').collect();
    // directive unparseable or missing: no candidates (Go `readDirective`
    // error path returns ActionValues())
    let Some(directive) = lines
        .len()
        .checked_sub(2)
        .and_then(|index| lines.get(index))
        .and_then(|line| line.strip_prefix(':'))
        .and_then(|directive| directive.parse::<i32>().ok())
    else {
        return Action::values(Vec::<String>::new());
    };

    let mut action = if directive & DIRECTIVE_FILTER_DIRS != 0 {
        action_directories(&lines)
    } else if directive & DIRECTIVE_FILTER_FILE_EXT != 0 {
        action_files(&lines)
    } else {
        action_values(&lines)
    };

    if lines.len() < 3 && directive & DIRECTIVE_NO_FILE_COMP == 0 {
        action = Action::files(Vec::<String>::new());
    }
    if directive & DIRECTIVE_ERROR != 0 {
        action = Action::values(Vec::<String>::new());
    }
    if directive & DIRECTIVE_NO_SPACE != 0 {
        action = action.nospace([]);
    }
    if let Some(prefix) = attached_flag_prefix(value) {
        // cobra returns bare values for flags with an attached value
        // (`--flag=value`); re-attach the flag prefix so the values match
        // the current word
        action = action.prefix(prefix);
    }
    action
}

fn action_values(lines: &[&str]) -> Action {
    let mut values: Vec<(String, String)> = Vec::new();
    for line in &lines[..lines.len().saturating_sub(2)] {
        match line.split_once('\t') {
            Some((value, description)) => {
                values.push((value.to_owned(), description.to_owned()));
            }
            None => values.push(((*line).to_owned(), String::new())),
        }
    }
    Action::values_described(values)
}

fn action_files(lines: &[&str]) -> Action {
    let extensions: Vec<String> = lines[..lines.len().saturating_sub(2)]
        .iter()
        .map(|line| format!(".{line}"))
        .collect();
    Action::files(extensions)
}

fn action_directories(lines: &[&str]) -> Action {
    if lines.len() > 2 {
        Action::directories().chdir(lines[0])
    } else {
        Action::directories()
    }
}

/// `--flag=value` prefix of the current word, if any.
fn attached_flag_prefix(value: &str) -> Option<String> {
    if !value.starts_with('-') {
        return None;
    }
    value.find('=').map(|index| value[..=index].to_owned())
}
