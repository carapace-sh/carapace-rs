//! Shell bridges spawning an interactive shell with a custom rc file.

use carapace_rs::Action;

const BASH_SNIPPET: &str = include_str!("bash.sh");

/// Bridge completions registered in bash via bash-completion scripts.
pub fn bash<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
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
            let mut joined = carapace_rs_shlex::join(&args, carapace_rs_shlex::Format::Bash);
            if c.value.is_empty() {
                // the empty value arg renders as `""`
                joined = joined.strip_suffix("\"\"").unwrap_or(&joined).to_owned();
            }

            // unique file created atomically (Go `os.CreateTemp`):
            // `create_new` fails when the name is taken, following
            // symlinks is not a concern since the file is fresh
            let mut snippet_path = None;
            for attempt in 0..64u32 {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                let candidate =
                    std::env::temp_dir().join(format!("carapace-bridge_bash_{nanos}_{attempt}"));
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&candidate)
                {
                    Ok(mut file) => {
                        use std::io::Write;
                        if let Err(error) = file.write_all(BASH_SNIPPET.as_bytes()) {
                            return Action::message(error.to_string());
                        }
                        snippet_path = Some(candidate);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Action::message(error.to_string()),
                }
            }
            let Some(snippet_path) = snippet_path else {
                return Action::message("could not create snippet file".to_owned());
            };

            let mut context = c.clone();
            context.env.insert("COMP_LINE".to_owned(), joined);
            let result = context.execute(
                "bash",
                [
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
                                values.push(carapace_rs::RawValue::described(
                                    value.trim(),
                                    // Go: TrimSpace(TrimSuffix(desc, ")))
                                    description.trim_end_matches(')').trim(),
                                ));
                            }
                            None => {
                                values.push(carapace_rs::RawValue::new(line.trim()));
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
