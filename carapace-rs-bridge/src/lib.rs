//! Bridge external completion frameworks into carapace actions
//! (port of Go `carapace-bridge`).
//!
//! Bridges spawn the external completer, parse its output conventions
//! and convert them into [`Action`]s.

pub mod argcomplete;
pub mod aws;
pub mod bpaf;
pub mod carapace;
pub mod carapace_bin;
pub mod clap;
pub mod click;
pub mod cobra;
pub mod complete;
pub mod fish;
pub mod gcloud;
pub mod inshellisense;
pub mod jj;
pub mod kingpin;
pub mod kitten;
pub mod powershell;
pub mod shell;
pub mod symfony;
pub mod typer;
pub mod urfavecli;
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
        "kitten" => kitten::action(command),
        "kingpin" => kingpin::action(command),
        "powershell" => powershell::action(command),
        "symfony" => symfony::action(command),
        "typer" => typer::action(command),
        "urfavecli" => urfavecli::action(command),
        "urfavecli_v1" => urfavecli::urfavecli_v1(command),
        "yargs" => yargs::action(command),
        "argcomplete_v1" => argcomplete::v1(command),
        "fish" => fish::action(command),
        "gcloud" => gcloud::action(command),
        "bash" => shell::bash(command),
        "zsh" => zsh::action(command),
        "carapace" => carapace::action(command),
        "carapace-bin" => carapace_bin::action(command),
        "clap" => clap::action(command),
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

/// Find an executable on the PATH, reading `PATH` from the context
/// environment (Go `exec.LookPath`).
pub(crate) fn look_path(c: &carapace_rs::Context, program: &str) -> Option<std::path::PathBuf> {
    let path = c.lookup_env("PATH")?;
    std::env::split_paths(path).find_map(|dir| {
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
    std::fs::create_dir_all(path.parent().ok_or("path has no parent directory")?)
        .map_err(|error| error.to_string())?;
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
