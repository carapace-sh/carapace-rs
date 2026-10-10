//! Shell bridges spawning an interactive shell with a custom rc file.

use carapace_core::Action;

const BASH_SNIPPET: &str = include_str!("bash.sh");

/// Bridge completions registered in bash via bash-completion scripts.
pub fn bash(command: &[&str]) -> Action {
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let Some(config_dir) = super::config_dir(c) else {
                return Action::message("failed to resolve config dir");
            };
            let rc_path = config_dir.join("carapace/bridge/bash/.bashrc");
            if let Err(error) = super::ensure_exists(&rc_path) {
                return Action::message(error.to_string());
            }

            let mut args = command.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            let mut joined = bash_join(&args);
            if c.value.is_empty() {
                joined = joined.trim_end_matches("\"\"").to_owned();
            }

            let snippet_path = std::env::temp_dir().join(format!(
                "carapace-bridge_bash_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            if let Err(error) = std::fs::write(&snippet_path, BASH_SNIPPET) {
                return Action::message(error.to_string());
            }

            let mut context = c.clone();
            context.env.insert("COMP_LINE".to_owned(), joined);
            let result = context.execute(
                "bash",
                &[
                    "--rcfile",
                    &rc_path.to_string_lossy(),
                    "-i",
                    &snippet_path.to_string_lossy(),
                ],
            );
            std::fs::remove_file(&snippet_path).ok();

            match result {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let mut values = Vec::new();
                    for line in text.lines() {
                        let line = line.trim_end();
                        if line.is_empty() && values.is_empty() {
                            continue;
                        }
                        match line.split_once('(') {
                            Some((value, description)) => {
                                values.push(carapace_core::RawValue::described(
                                    value.trim(),
                                    description.trim().trim_end_matches(')'),
                                ));
                            }
                            None => {
                                values.push(carapace_core::RawValue::new(line.trim()));
                            }
                        }
                    }
                    let action = if values.is_empty() {
                        Action::files(Vec::<String>::new())
                    } else {
                        Action::raw(values)
                    };
                    action.nospace(['/', '=', '@', ':', '.', ','])
                }
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bash-quote and join arguments for `COMP_LINE` and bridge inputs.
pub(crate) fn bash_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            let escaped = arg.replace('\'', "'\\''");
            format!("'{escaped}'")
        })
        .collect::<Vec<_>>()
        .join(" ")
}
