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
                    parse_bash_output(&text)
                }
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Parse bash-completion output lines into the file fallback or the styled
/// value action (Go's `ActionExecCommand` callback in `bash.go`).
fn parse_bash_output(text: &str) -> Action {
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
                    // Go: TrimSpace(TrimSuffix(desc, ")")) — one trailing
                    // paren only, nested parens survive
                    description.strip_suffix(')').unwrap_or(description).trim(),
                ));
            }
            None => {
                values.push(carapace_rs::RawValue::new(line.trim()));
            }
        }
    }
    let action = if values.is_empty() {
        Action::files([])
    } else {
        Action::raw(values)
            .style_f(|context, value| carapace_rs::style::for_path(context, &value.value))
    };
    action.nospace(['/', '=', '@', ':', '.', ','])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bash_output_reads_descriptions() {
        // spf13/cobra and carapace style: `value (description)`
        let action = parse_bash_output("commit (record changes)\npush (upload)\n");
        let invoked = action.invoke(&carapace_rs::Context::default());
        let described: Vec<(String, String)> = invoked
            .to_export()
            .values
            .iter()
            .map(|v| (v.value.clone(), v.description.clone()))
            .collect();
        assert_eq!(
            described,
            vec![
                ("commit".to_owned(), "record changes".to_owned()),
                ("push".to_owned(), "upload".to_owned()),
            ]
        );
    }

    #[test]
    fn parse_bash_output_trims_trailing_parens_and_spaces() {
        // the closing paren of a nested expression must survive, only the
        // description terminator is trimmed (Go: TrimSpace(TrimSuffix(...)))
        let action = parse_bash_output("value (desc (nested)) \n");
        let invoked = action.invoke(&carapace_rs::Context::default());
        assert_eq!(invoked.to_export().values[0].description, "desc (nested)");
    }

    #[test]
    fn parse_bash_output_plain_lines_have_no_description() {
        let action = parse_bash_output("one\n");
        let invoked = action.invoke(&carapace_rs::Context::default());
        assert_eq!(invoked.to_export().values[0].value, "one");
        assert_eq!(invoked.to_export().values[0].description, "");
        // plain words keep the nospace set from the bridge
        assert!(
            invoked.to_export().meta.nospace.contains('/'),
            "{:?}",
            invoked.to_export().meta
        );
    }

    #[test]
    fn parse_bash_output_empty_falls_back_to_files() {
        let action = parse_bash_output("");
        let invoked = action.invoke(&carapace_rs::Context::default().dir(std::env::temp_dir()));
        assert!(
            !invoked.to_export().values.is_empty()
                && invoked.to_export().values.iter().all(|v| v.tag == "files"),
            "expected file candidates, got {:?}",
            invoked.to_export().values
        );
    }
}
