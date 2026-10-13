//! Aws bridge: `aws_completer` with a patched `COMP_LINE`
//! (port of Go `bridge.ActionAws`).

use carapace_rs::Action;

/// Bridge completions registered in aws (aws/aws-cli).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |_command| {
        Action::callback(move |c| {
            let mut context = c.clone();
            let mut value = c.value.clone();
            if value == "-" {
                // no shorthand flags: expand to the longhand first, which
                // is what the completer needs
                value.push('-');
                context.value = value.clone();
            }
            let mut words = c.args.clone();
            words.push(value);
            context
                .env
                .insert("COMP_LINE".to_owned(), format!("aws {}", words.join(" ")));

            match context.execute("aws_completer", Vec::<String>::new()) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.split('\n').collect();
                    if lines.first() == Some(&"") {
                        return Action::default();
                    }
                    // v1 appends a space to every candidate
                    let lines: Vec<String> = lines
                        .into_iter()
                        .map(|line| line.strip_suffix(' ').unwrap_or(line).to_owned())
                        .collect();
                    // the final empty split is dropped (Go `len-1`)
                    let candidates = &lines[..lines.len().saturating_sub(1)];
                    let mut action = Action::values(candidates.iter().cloned());
                    let file_scheme = ["file://", "fileb://"]
                        .iter()
                        .find(|scheme| c.value.starts_with(**scheme))
                        .copied();
                    if let Some(scheme) = file_scheme {
                        action = action.nospace(['/']).style_f(move |context, raw| {
                            carapace_rs::shell::for_path(
                                context,
                                raw.value.strip_prefix(scheme).unwrap_or(&raw.value),
                            )
                        });
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
