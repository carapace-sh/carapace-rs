//! Default action constructors, ported from Go carapace's `defaultActions.go`.

use std::path::{Path, PathBuf};

use crate::action::Action;
use crate::context::Context;
use crate::value::RawValue;

impl Action {
    /// Complete static values.
    pub fn values<I, V>(values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<String>,
    {
        Self::raw(values.into_iter().map(RawValue::new).collect())
    }

    /// Complete static values with descriptions (pairs: value, description).
    pub fn values_described<I, V, D>(values: I) -> Self
    where
        I: IntoIterator<Item = (V, D)>,
        V: Into<String>,
        D: Into<String>,
    {
        Self::raw(
            values
                .into_iter()
                .map(|(value, description)| RawValue::described(value, description))
                .collect(),
        )
    }

    /// Complete static values with styles (pairs: value, style).
    pub fn styled_values<I, V, S>(values: I) -> Self
    where
        I: IntoIterator<Item = (V, S)>,
        V: Into<String>,
        S: Into<String>,
    {
        Self::raw(
            values
                .into_iter()
                .map(|(value, style)| RawValue::styled(value, style))
                .collect(),
        )
    }

    /// Complete static values with descriptions and styles (triples).
    pub fn styled_values_described<I, V, D, S>(values: I) -> Self
    where
        I: IntoIterator<Item = (V, D, S)>,
        V: Into<String>,
        D: Into<String>,
        S: Into<String>,
    {
        Self::raw(
            values
                .into_iter()
                .map(|(value, description, style)| {
                    RawValue::styled_described(value, description, style)
                })
                .collect(),
        )
    }

    /// Display a help message instead of completions.
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            meta: crate::meta::Meta::default().message(message.into()),
            ..Default::default()
        }
    }

    /// Complete directories in the current (or context) directory.
    pub fn directories() -> Self {
        Self::callback(|c| {
            let action = complete_path(c, |path| path.is_dir());
            match action {
                Some(action) => action.style("carapace.Value=blue"),
                None => Self::message("no matches"),
            }
        })
    }

    /// Complete files, optionally filtered by suffix.
    pub fn files(suffixes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let suffixes: Vec<String> = suffixes.into_iter().map(Into::into).collect();
        Self::callback(move |c| {
            let action = complete_path(c, |path| {
                suffixes.is_empty()
                    || suffixes.iter().any(|suffix| {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        name.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase())
                    })
            });
            match action {
                Some(action) => action,
                None => Self::message("no matches"),
            }
        })
    }
}

impl Action {
    /// Execute completion against an embedded command tree (port of
    /// `ActionExecute`).
    ///
    /// Go round-trips through a subprocess (`_carapace export`); in-process
    /// we traverse the embedded tree directly, forwarding the context args
    /// and current value.
    pub fn execute(cmd: std::sync::Arc<crate::model::Command>) -> Self {
        Self::callback(move |c| {
            let mut args = c.args.clone();
            args.push(c.value.clone());
            let completion = crate::traverse(&cmd, args);
            completion
                .action
                .invoke(&completion.context)
                .to_action()
        })
    }

    /// Execute a command and complete from its stdout.
    ///
    /// Errors surface as message actions; the callback receives the raw
    /// stdout bytes.
    pub fn exec_command<F>(command: &str, args: &[&str], f: F) -> Self
    where
        F: Fn(&[u8]) -> Self + Send + Sync + 'static,
    {
        let command = command.to_owned();
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        Self::callback(move |c| match c.execute(&command, &args.iter().map(String::as_str).collect::<Vec<_>>()) {
            Ok(stdout) => f(&stdout),
            Err(error) => Self::message(error),
        })
    }

    /// Like [`exec_command`](Self::exec_command) but the callback receives
    /// the error explicitly.
    pub fn exec_command_e<F>(command: &str, args: &[&str], f: F) -> Self
    where
        F: Fn(Result<&[u8], String>) -> Self + Send + Sync + 'static,
    {
        let command = command.to_owned();
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        Self::callback(move |c| {
            match c.execute(&command, &args.iter().map(String::as_str).collect::<Vec<_>>()) {
                Ok(stdout) => f(Ok(&stdout)),
                Err(error) => f(Err(error)),
            }
        })
    }

    /// Complete known style names (for `style` configuration).
    pub fn styles() -> Self {
        Self::values([
            "blue", "cyan", "magenta", "white", "red", "green", "yellow", "black", "default",
            "bold", "italic", "underlined", "dimmed", "inverse",
        ])
    }

    /// Complete style configuration values (e.g. `carapace.Error=underlined`).
    pub fn style_config() -> Self {
        let config = crate::config::styles();
        let mut values = vec![
            format!("carapace.Value={}", config.carapace.value),
            format!("carapace.Description={}", config.carapace.description),
            format!("carapace.Error={}", config.carapace.error),
            format!("carapace.Usage={}", config.carapace.usage),
        ];
        for (name, style) in &config.custom {
            values.push(format!("{name}={style}"));
        }
        Self::raw(values.into_iter().map(RawValue::new).collect())
    }
}

impl Action {
    /// Complete executables available on `PATH`.
    pub fn executables() -> Self {
        Self::callback(|c| {
            let path = c.get_env("PATH");
            let mut values = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for dir in path.split(':').filter(|dir| !dir.is_empty()) {
                let Ok(entries) = std::fs::read_dir(dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if seen.insert(name.clone()) && is_executable(&entry.path()) {
                        values.push(RawValue::new(name));
                    }
                }
            }
            sort_executables(&mut values, &c.value);
            Action::raw(values)
        })
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.extension().is_some_and(|ext| ext == "exe")
    }
}

fn sort_executables(values: &mut [RawValue], _prefix: &str) {
    values.sort_by(|a, b| a.value.cmp(&b.value));
}

fn complete_path(context: &Context, filter: impl Fn(&Path) -> bool) -> Option<Action> {
    let value = &context.value;
    let (dir, prefix) = match value.rfind('/') {
        Some(index) => (value[..index].to_owned(), value[index + 1..].to_owned()),
        None => (String::new(), value.clone()),
    };
    let dir = if dir.is_empty() {
        context.dir.clone()
    } else {
        let expanded = expand_tilde(&dir);
        let path = PathBuf::from(&expanded);
        if path.is_absolute() {
            path
        } else {
            context.dir.join(path)
        }
    };
    let entries = std::fs::read_dir(&dir).ok()?;
    let mut values = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) {
            continue;
        }
        let path = entry.path();
        if !filter(&path) && !path.is_dir() {
            continue;
        }
        let mut candidate = name;
        if path.is_dir() {
            candidate.push('/');
        }
        let mut value = RawValue::new(format!("{}/{}", dir_prefix(&dir), candidate));
        if path.is_dir() {
            value = value.nospace(true);
        }
        values.push(value);
    }
    Some(Action::raw(values))
}

fn dir_prefix(dir: &Path) -> String {
    let rendered = dir.to_string_lossy().into_owned();
    if rendered.is_empty() {
        String::new()
    } else if rendered.ends_with('/') {
        rendered
    } else {
        format!("{rendered}/")
    }
}

fn expand_tilde(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => match std::env::var("HOME") {
            Ok(home) => format!("{home}/{rest}"),
            Err(_) => path.to_owned(),
        },
        None if path == "~" => std::env::var("HOME").unwrap_or_else(|_| path.to_owned()),
        None => path.to_owned(),
    }
}
