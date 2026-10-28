//! Kitten bridge: kitty's `kitten __complete__ json` protocol with the
//! match requests passed as JSON on stdin (port of Go
//! `bridge.ActionKitten`).

use carapace_rs::Action;

/// Bridge completions registered in kitten (kovidgoyal/kitty).
pub fn action<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    super::action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        Action::callback(move |c| {
            let mut request = command.clone();
            request.extend(c.args.iter().cloned());
            request.push(c.value.clone());

            let mut match_requests = vec![request.clone()];
            if c.value == "-" {
                // some longhand flags are missing when called with `-`
                let mut longhand_request = command.clone();
                longhand_request.extend(c.args.iter().cloned());
                longhand_request.push("--".to_owned());
                match_requests.push(longhand_request);
            }

            // partial directories aren't always completed (e.g. a tmp
            // directory with `kitty /tm`)
            let mut filepath_request = command.clone();
            filepath_request.extend(c.args.iter().cloned());
            filepath_request.push(directory_of(&c.value));
            match_requests.push(filepath_request);

            let payload = serde_json::to_string(&match_requests)
                .map_err(|error| error.to_string())
                .and_then(|payload| {
                    c.execute_stdin("kitten", ["__complete__", "json"], payload.as_bytes())
                });
            let output = match payload {
                Ok(output) => output,
                Err(error) => {
                    // a failed completer surfaces only its first error line
                    let first_line = error.split('\n').next().unwrap_or_default().trim();
                    return Action::message(if first_line.is_empty() {
                        error
                    } else {
                        first_line.to_owned()
                    });
                }
            };

            let match_results: Vec<KittenResult> = match serde_json::from_slice(&output) {
                Ok(results) => results,
                Err(error) => return Action::message(error.to_string()),
            };

            if match_results.is_empty() {
                return Action::default();
            }

            let mut nospace = false;
            let mut batch: Vec<Action> = Vec::new();
            for match_result in &match_results {
                for group in &match_result.groups {
                    match group.title.as_str() {
                        "Directories" => batch.push(Action::directories()),
                        "Executables" => {
                            let dir = directory_of(&c.value);
                            batch.push(Action::directories());
                            batch.push(
                                Action::executables_in([dir.as_str()]).prefix(format!("{dir}/")),
                            );
                        }
                        "Executables in PATH" => batch.push(Action::executables()),
                        "Options" => {
                            let (longhand, shorthand): (Vec<Match>, Vec<Match>) = group
                                .matches
                                .iter()
                                .cloned()
                                .partition(|m| m.word.starts_with("--"));
                            batch.push(matches_action(longhand).tag("longhand flags"));
                            batch.push(matches_action(shorthand).tag("shorthand flags"));
                        }
                        "Keywords" => batch.push(
                            matches_action(group.matches.clone())
                                .tag("keywords")
                                .style_f(|_, value| crate_keyword_style(&value.value)),
                        ),
                        "Sub-commands" => {
                            batch.push(matches_action(group.matches.clone()).tag("commands"))
                        }
                        _ => {
                            if group.is_files {
                                // carapace file completion restricted to the
                                // matches the completer provided
                                let retain: Vec<String> = group
                                    .matches
                                    .iter()
                                    .map(|m| m.word.replace('\\', "/"))
                                    .collect();
                                batch.push(Action::files([]).retain(retain.iter().cloned()));
                            } else {
                                batch.push(
                                    matches_action(group.matches.clone())
                                        .tag(group.title.to_lowercase()),
                                );
                            }
                        }
                    }
                    nospace = nospace || group.no_trailing_space;
                }
            }

            let action = carapace_rs::batch(batch);
            if nospace { action.nospace([]) } else { action }
        })
    })
}

#[derive(serde::Deserialize)]
struct KittenResult {
    #[serde(default)]
    groups: Vec<Group>,
}

#[derive(serde::Deserialize)]
struct Group {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "no_trailing_space")]
    no_trailing_space: bool,
    #[serde(default, rename = "is_files")]
    is_files: bool,
    #[serde(default)]
    matches: Vec<Match>,
}

#[derive(Clone, serde::Deserialize)]
struct Match {
    #[serde(default)]
    word: String,
    #[serde(default)]
    description: String,
}

/// The matches of a group as a described-values action.
fn matches_action(matches: Vec<Match>) -> Action {
    Action::values_described(
        matches
            .into_iter()
            .map(|m| (m.word, m.description))
            .collect::<Vec<(String, String)>>(),
    )
}

/// The directory part of a word (Go `filepath.Dir` with slash separators).
fn directory_of(word: &str) -> String {
    let slashed = word.replace('\\', "/");
    match slashed.rfind('/') {
        Some(0) => "/".to_owned(),
        Some(index) => slashed[..index].to_owned(),
        None => ".".to_owned(),
    }
}

/// The keyword style from the style configuration (Go `style.ForKeyword`).
fn crate_keyword_style(word: &str) -> String {
    carapace_rs::style::keyword_style(word)
}
