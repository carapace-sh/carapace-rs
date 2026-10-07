//! Bridge external completion frameworks into carapace actions
//! (port of Go `carapace-bridge`).
//!
//! Bridges spawn the external completer, parse its output conventions
//! and convert them into [`Action`]s.

pub mod fish;
pub mod shell;
pub mod zsh;

use carapace_core::Action;
use carapace_core::model::Command;
use std::sync::Arc;

/// Dispatch a bridge by name.
pub fn action(name: &str, command: &[&str]) -> Action {
    match name {
        "fish" => fish::action(command),
        "bash" => shell::bash(command),
        "zsh" => zsh::action(command),
        "carapace" => carapace(command),
        "carapace-bin" => carapace_bin(command),
        "clap" => clap(command),
        other => Action::message(format!("unknown bridge: {other}")),
    }
}

/// User config directory (`XDG_CONFIG_HOME` or `$HOME/.config`).
pub(crate) fn config_dir() -> Option<std::path::PathBuf> {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(dir) if !dir.is_empty() => Some(std::path::PathBuf::from(dir)),
        _ => std::env::var("HOME")
            .ok()
            .map(|home| std::path::PathBuf::from(home).join(".config")),
    }
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
pub fn action_command<F>(command: &[&str], f: F) -> Action
where
    F: Fn(&[String]) -> Action + Send + Sync + 'static,
{
    if !command.is_empty() {
        let command: Vec<String> = command.iter().map(|c| c.to_string()).collect();
        return f(&command);
    }

    let mut mini = Command::new("bridge");
    mini.positionals.push({
        let mut positional = carapace_core::Positional {
            name: "command".to_owned(),
            ..Default::default()
        };
        positional.completion = Some(carapace_core::batch(vec![
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
pub fn carapace(command: &[&str]) -> Action {
    action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let command_name = command[0].clone();
        let mut args = vec!["_carapace".to_owned(), "export".to_owned(), String::new()];
        args.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = args.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(
                &command_name,
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
            ) {
                Ok(output) if output.is_empty() => Action::default(),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bridge completions registered in carapace-bin.
pub fn carapace_bin(command: &[&str]) -> Action {
    action_command(command, move |command| {
        let command: Vec<String> = command.to_vec();
        let bin = "carapace".to_owned();
        let mut prefix = vec![command[0].clone(), "export".to_owned(), String::new()];
        prefix.extend(command[1..].iter().cloned());
        Action::callback(move |c| {
            let mut args = prefix.clone();
            args.extend(c.args.iter().cloned());
            args.push(c.value.clone());
            match c.execute(&bin, &args.iter().map(String::as_str).collect::<Vec<_>>()) {
                Ok(output) if output.is_empty() => Action::files(Vec::<String>::new()),
                Ok(output) => Action::import(&String::from_utf8_lossy(&output)),
                Err(error) => Action::message(error),
            }
        })
    })
}

/// Bridge completions from an external clap binary (unstable-dynamic engine).
pub fn clap(command: &[&str]) -> Action {
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
            match c.execute(
                &command_name,
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
            ) {
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output);
                    let lines: Vec<&str> = text.lines().collect();
                    let mut action = Action::values(lines.iter().copied());
                    for line in &lines {
                        if !line.is_empty()
                            && line[..line.len() - 1].contains(['/', '=', '@', ':', '.', ','])
                        {
                            action = action.nospace([]);
                            break;
                        }
                    }
                    action
                }
                Err(error) => Action::message(error),
            }
        })
    })
}
