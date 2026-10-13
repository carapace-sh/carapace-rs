//! Bridge external completion frameworks into carapace actions
//! (port of Go `carapace-bridge`).
//!
//! Bridges spawn the external completer, parse its output conventions
//! and convert them into [`Action`]s.

pub mod argcomplete;
pub mod aws;
pub mod bpaf;
pub mod click;
pub mod cobra;
pub mod complete;
pub mod fish;
pub mod inshellisense;
pub mod jj;
pub mod kingpin;
pub mod shell;
pub mod yargs;
pub mod zsh;

use carapace_rs::Action;
use carapace_rs::model::Command;
use std::sync::Arc;

/// Dispatch a bridge by name.
pub fn action<I, S>(name: &str, command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match name {
        "argcomplete" => argcomplete::action(command),
        "aws" => aws::action(command),
        "bpaf" => bpaf::action(command),
        "click" => click::action(command),
        "complete" => complete::action(command),
        "jj" => jj::action(command),
        "kingpin" => kingpin::action(command),
        "yargs" => yargs::action(command),
        "fish" => fish::action(command),
        "bash" => shell::bash(command),
        "zsh" => zsh::action(command),
        "carapace" => carapace(command),
        "carapace-bin" => carapace_bin(command),
        "clap" => clap(command),
        "cobra" => cobra::action(command),
        "inshellisense" => inshellisense::action(command),
        other => Action::message(format!("unknown bridge: {other}")),
    }
}

/// User config directory (`XDG_CONFIG_HOME` or `$HOME/.config`),
/// resolved from the context environment (the context's environment is
/// what spawned commands see, Go `xdg.UserConfigDir` reads the process
/// environment — identical in production, consistent in the sandbox).
pub(crate) fn config_dir(context: &carapace_rs::Context) -> Option<std::path::PathBuf> {
    if let Some(dir) = context.lookup_env("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(std::path::PathBuf::from(dir));
        }
    }
    #[cfg(unix)]
    if let Some(home) = context.lookup_env("HOME") {
        if !home.is_empty() {
            return Some(std::path::PathBuf::from(home).join(".config"));
        }
    }
    // platform directory elsewhere (not env-driven)
    dirs::config_dir()
}

/// Find an executable on the PATH (Go `exec.LookPath`).
pub(crate) fn look_path(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var("PATH").ok()?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(program);
        let with_ext = if cfg!(windows) {
            dir.join(format!("{program}.exe"))
        } else {
            candidate.clone()
        };
        [candidate, with_ext]
            .into_iter()
            .find(|path| path.is_file())
    })
}

/// Create an empty file (and its parent directories) if it does not exist.
pub(crate) fn ensure_exists(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
    std::fs::write(path, "").map_err(|error| error.to_string())
}

/// Wrap a bridge function with the "no command provided" behavior: the
/// first positional selects the executable (executables + files), later
/// positionals delegate to the bridge shifted by one.
pub fn action_command<I, S, F>(command: I, f: F) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    F: Fn(&[String]) -> Action + Send + Sync + 'static,
{
    let command: Vec<String> = command.into_iter().map(|c| c.as_ref().to_owned()).collect();
    if !command.is_empty() {
        return f(&command);
    }

    let mut mini = Command::new("bridge");
    mini.positionals.push({
        let mut positional = carapace_rs::Positional {
            name: "command".to_owned(),
            ..Default::default()
        };
        positional.completion = Some(carapace_rs::batch(vec![
            Action::executables(),
            Action::files(Vec::<String>::new()),
        ]));
        positional
    });
    mini.positional_any = Some(Action::callback(move |c| {
        let Some(first) = c.args.first() else {
            return Action::default();
        };
        let mut command = vec![first.clone()];
        command.extend(c.args.iter().skip(1).cloned());
        f(&command).shift(1)
    }));
    Action::execute(Arc::new(mini))
}

/// Bridge completions from the Go carapace binary: parses the export JSON.
pub fn carapace<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        let mut args = vec!["_carapace".to_owned(), "export".to_owned(), String::new()];
        args.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = args.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&command_name, &args) {
                Ok(output) if output.is_empty() => Action::default(),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bridge completions registered in carapace-bin.
pub fn carapace_bin<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let bin = "carapace".to_owned();
        let mut prefix = vec![command[0].clone(), "export".to_owned(), String::new()];
        prefix.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = prefix.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&bin, &args) {
                Ok(output) if output.is_empty() => Action::files(Vec::<String>::new()),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bridge completions from an external clap binary (unstable-dynamic engine).
pub fn clap<I, S>(command: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        Action::callback(move |c| {
            let index = command.len() + c.args.len();
            let mut args = vec![
                "complete".to_owned(),
                "--index".to_owned(),
                index.to_string(),
                "--type".to_owned(),
                "9".to_owned(),
                "--no-space".to_owned(),
                "--ifs=\n".to_owned(),
                "--".to_owned(),
            ];
            args.extend(command.iter().cloned());
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&command_name, &args) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.lines().collect();
                    let mut action = Action::values(lines.iter().copied());
                    for line in &lines {
                        if !line.is_empty() {
                            // everything but the last character (Go slices
                            // bytes; here the boundary must stay a char
                            // boundary or multi-byte output panics)
                            let last = line.chars().last().map_or(0, char::len_utf8);
                            if line[..line.len() - last].contains(['/', '=', '@', ':', '.', ',']) {
                                action = action.nospace([]);
                                break;
                            }
                        }
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
