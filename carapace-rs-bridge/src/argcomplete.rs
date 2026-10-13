use carapace_rs::Action;

/// Bridge completions registered in argcomplete (kislyuk/argcomplete).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            if super::look_path(&command[0]).is_none() {
                return Action::message(format!("command not found: {}", command[0]));
            }

            let mut args: Vec<String> = command[1..].to_vec();
            args.extend(c.args.iter().cloned());
            let mut current = c.value.clone();

            // argcomplete does not complete partial flag names or
            // positionals; an optarg flag (`--flag=`) is handled as a
            // normal arg by the completer and its prefix re-attached after
            let mut prefix = String::new();
            if current.starts_with("--") {
                if let Some((flag, _)) = current.split_once('=') {
                    prefix = format!("{flag}=");
                    args.push(flag.to_owned());
                    current = String::new();
                } else {
                    current = "--".to_owned();
                }
            } else {
                current = String::new();
            }

            let Some((temp_path, _temp_file)) = create_temp_file() else {
                return Action::message("could not create temp file");
            };

            let mut words = args.clone();
            words.push(current);
            let comp_line = format!("{} {}", command[0], words.join(" "));

            let mut context = c.clone();
            for (key, value) in [
                ("_ARGCOMPLETE", "1"),
                ("_ARGCOMPLETE_DFS", "\t"),
                ("_ARGCOMPLETE_IFS", "\n"),
                ("_ARGCOMPLETE_SHELL", "fish"),
                ("_ARGCOMPLETE_SUPPRESS_SPACE", "1"),
                ("COMP_LINE", comp_line.as_str()),
                ("COMP_POINT", &comp_line.len().to_string()),
            ] {
                context.env.insert(key.to_owned(), value.to_owned());
            }
            context.env.insert(
                "_ARGCOMPLETE_STDOUT_FILENAME".to_owned(),
                temp_path.to_string_lossy().into_owned(),
            );

            let result = context.execute(&command[0], Vec::<String>::new());
            let output = std::fs::read_to_string(temp_path.as_path());
            std::fs::remove_file(&temp_path).ok();

            match result {
                Err(error) => Action::message(error),
                Ok(_) => match output {
                    Err(error) => Action::message(error.to_string()),
                    Ok(output) => {
                        let (values, nospace) =
                            parse_argcomplete_output(&output, c.value.starts_with('-'));

                        // fallback to file completions when nothing came
                        // back; an optarg value completes files for the
                        // part after `=`
                        let optarg_value =
                            c.value.split_once('=').map(|(_, value)| value.to_owned());
                        let inner = if values.is_empty() {
                            Action::callback(move |context: &carapace_rs::Context| {
                                match &optarg_value {
                                    Some(value) => Action::files(Vec::<String>::new())
                                        .invoke(&context.clone().value(value.clone()))
                                        .to_action(),
                                    None => Action::files(Vec::<String>::new()),
                                }
                            })
                        } else {
                            Action::raw(values)
                        };
                        let mut action = inner.prefix(prefix);
                        if nospace {
                            action = action.nospace(['=', '/', ',']);
                        }
                        action
                    }
                },
            }
        })
    })
}

/// Parse the `_ARGCOMPLETE` output lines: tab-separated value/description
/// pairs; flag candidates are filtered unless the current word is a flag;
/// the nospace flag comes from candidates ending in `=`, `/` or `,`
/// (Go's parsing loop in `actionArgcompleteV3`).
fn parse_argcomplete_output(output: &str, is_flag: bool) -> (Vec<carapace_rs::RawValue>, bool) {
    let mut values = Vec::new();
    let mut nospace = false;
    for line in output.lines() {
        if !is_flag && line.starts_with('-') {
            continue;
        }
        if line.ends_with('=') || line.ends_with('/') || line.ends_with(',') {
            nospace = true;
        }
        let mut fields = line.splitn(2, '\t');
        let value = fields.next().unwrap_or_default();
        if value.is_empty() {
            continue;
        }
        values.push(carapace_rs::RawValue::described(
            value,
            fields.next().unwrap_or_default(),
        ));
    }
    (values, nospace)
}

/// Create a uniquely named temp file under `<tmp>/carapace-bridge`
/// (Go `os.CreateTemp`); the caller removes it after reading.
fn create_temp_file() -> Option<(std::path::PathBuf, std::fs::File)> {
    let dir = std::env::temp_dir().join("carapace-bridge");
    std::fs::create_dir_all(&dir).ok()?;
    for attempt in 0..64u32 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = dir.join(format!("argcomplete_{nanos}_{attempt}"));
        if let Ok(file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            return Some((path, file));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_argcomplete_output_reads_descriptions() {
        let (values, nospace) = parse_argcomplete_output("one\tfirst\ntwo\n", false);
        let described: Vec<(String, String)> = values
            .iter()
            .map(|v| (v.value.clone(), v.description.clone()))
            .collect();
        assert_eq!(
            described,
            vec![
                ("one".to_owned(), "first".to_owned()),
                ("two".to_owned(), String::new()),
            ]
        );
        assert!(!nospace);
    }

    #[test]
    fn parse_argcomplete_output_filters_flags_for_plain_words() {
        let (values, _) = parse_argcomplete_output("--flag\tflag desc\nvalue\n", false);
        let values: Vec<String> = values.iter().map(|v| v.value.clone()).collect();
        assert_eq!(values, vec!["value".to_owned()]);

        let (values, _) = parse_argcomplete_output("--flag\tflag desc\nvalue\n", true);
        let values: Vec<String> = values.iter().map(|v| v.value.clone()).collect();
        assert_eq!(values, vec!["--flag".to_owned(), "value".to_owned()]);
    }

    #[test]
    fn parse_argcomplete_output_detects_nospace_suffixes() {
        for (line, expected) in [
            ("dir/", true),
            ("flag=", true),
            ("list,", true),
            ("plain", false),
        ] {
            let (_, nospace) = parse_argcomplete_output(line, false);
            assert_eq!(nospace, expected, "{line}");
        }
    }
}
