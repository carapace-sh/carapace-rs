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
        Self::callback(
            |c| match complete_path(c, "directories", &[], |path| path.is_dir()) {
                Some(action) => action,
                None => Self::message("no matches"),
            },
        )
    }

    /// Complete files, optionally filtered by suffix.
    pub fn files(suffixes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let suffixes: Vec<String> = suffixes.into_iter().map(Into::into).collect();
        Self::callback(move |c| {
            let filter = |path: &std::path::Path| {
                suffixes.is_empty()
                    || suffixes.iter().any(|suffix| {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        name.to_ascii_lowercase()
                            .ends_with(&suffix.to_ascii_lowercase())
                    })
            };
            match complete_path(c, "files", &suffixes, filter) {
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
            let mut completion = crate::traverse(&cmd, args);
            // the embedded tree runs with the outer context's environment,
            // directory and sandbox mock
            completion.context.env = c.env.clone();
            completion.context.dir = c.dir.clone();
            completion.context.mock = c.mock.clone();
            completion.action.invoke(&completion.context).to_action()
        })
    }

    /// Execute a command and complete from its stdout.
    ///
    /// Errors surface as message actions; the callback receives the raw
    /// stdout bytes.
    pub fn exec_command<I, S, F>(command: &str, args: I, f: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(&[u8]) -> Self + Send + Sync + 'static,
    {
        let command = command.to_owned();
        let args: Vec<String> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_owned())
            .collect();
        Self::callback(move |c| match c.execute(&command, &args) {
            Ok(stdout) => f(&stdout),
            Err(error) => Self::message(error),
        })
    }

    /// Like [`exec_command`](Self::exec_command) but the callback receives
    /// the error explicitly.
    pub fn exec_command_e<I, S, F>(command: &str, args: I, f: F) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: Fn(Result<&[u8], String>) -> Self + Send + Sync + 'static,
    {
        let command = command.to_owned();
        let args: Vec<String> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_owned())
            .collect();
        Self::callback(move |c| match c.execute(&command, &args) {
            Ok(stdout) => f(Ok(&stdout)),
            Err(error) => f(Err(error)),
        })
    }

    /// Complete style values (Go `ActionStyles`).
    ///
    /// The already-present styles are combined into each candidate and
    /// suppress the foreground or background half of the catalog once
    /// taken; the 256-color ranges expand contextually.
    pub fn styles(existing: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let existing: Vec<String> = existing.into_iter().map(Into::into).collect();
        Self::callback(move |c| {
            let fg = existing.iter().any(|s| is_foreground_color(s));
            let bg = existing.iter().any(|s| s.starts_with("bg-"));
            let combined = |value: &str| {
                let mut styles = existing.clone();
                styles.push(value.to_owned());
                styles.join(" ")
            };

            let mut values: Vec<RawValue> = Vec::new();
            if !fg {
                for color in FOREGROUND_COLORS {
                    values.push(RawValue::styled(color.to_owned(), combined(color)));
                }
                if c.value.starts_with("color") {
                    for index in 0..=255u16 {
                        values.push(RawValue::styled(
                            format!("color{index}"),
                            combined(&format!("color{index}")),
                        ));
                    }
                } else {
                    // Go uses `style.Of(styles...)`: the existing styles only
                    values.push(RawValue::styled("color", existing.join(" ")));
                }
            }
            if !bg {
                for color in FOREGROUND_COLORS {
                    let name = format!("bg-{color}");
                    values.push(RawValue::styled(&name, combined(&name)));
                }
                if c.value.starts_with("bg-color") {
                    for index in 0..=255u16 {
                        values.push(RawValue::styled(
                            format!("bg-color{index}"),
                            combined(&format!("bg-color{index}")),
                        ));
                    }
                } else {
                    values.push(RawValue::styled("bg-color", existing.join(" ")));
                }
            }
            for attribute in ["bold", "dim", "italic", "underlined", "blink", "inverse"] {
                values.push(RawValue::styled(attribute, combined(attribute)));
            }
            Action::raw(values).nospace(['r']).tag("styles")
        })
    }

    /// Complete style configuration values (e.g. `carapace.Error=bold red`).
    pub fn style_config() -> Self {
        Self::multiparts("=", |c| match c.parts.len() {
            0 => Self::multiparts(".", |c| match c.parts.len() {
                0 => {
                    let keys = crate::config::styles().keys();
                    Self::values(keys).suffix(".")
                }
                1 => {
                    let name = c.parts[0].clone();
                    match crate::config::styles().fields(&name) {
                        Some(fields) => crate::batch(
                            fields
                                .iter()
                                .map(|field| {
                                    Self::styled_values_described([(
                                        field.name.clone(),
                                        field.description.clone(),
                                        field.style.clone(),
                                    )])
                                    .tag(field.tag.clone())
                                })
                                .collect(),
                        )
                        .suffix("="),
                        None => Self::message(format!("unknown config: '{name}'")),
                    }
                }
                _ => Self::values(Vec::<String>::new()),
            }),
            1 => Self::multiparts(",", |c| {
                Self::styles(c.parts.iter().cloned())
                    .filter_parts()
                    .nospace([])
            }),
            _ => Self::values(Vec::<String>::new()),
        })
    }
}

impl Action {
    /// Complete executables available on `PATH`.
    pub fn executables() -> Self {
        Self::callback(|c| {
            let path = c.get_env("PATH");
            let dirs: Vec<String> = std::env::split_paths(&path)
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect();
            executables_in_dirs(dirs)
        })
    }

    /// Completes executables in the given directories (Go
    /// `ActionExecutables(dirs...)`).
    pub fn executables_in(dirs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let dirs: Vec<String> = dirs.into_iter().map(Into::into).collect();
        Self::callback(move |_| executables_in_dirs(dirs.clone()))
    }
}

/// The shared executable listing for [`Action::executables`] and
/// [`Action::executables_in`].
fn executables_in_dirs(dirs: Vec<String>) -> Action {
    let mut values = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_executable(&entry.path()) && seen.insert(name.clone()) {
                values.push(RawValue::new(name));
            }
        }
    }
    sort_executables(&mut values);
    Action::raw(values)
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

const FOREGROUND_COLORS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
];

fn is_foreground_color(style: &str) -> bool {
    FOREGROUND_COLORS.contains(&style)
        || style.starts_with('#')
        || style.starts_with("color")
        || style.starts_with("fg-")
}

fn sort_executables(values: &mut [RawValue]) {
    values.sort_by(|a, b| a.value.cmp(&b.value));
}

fn complete_path(
    context: &Context,
    host: &str,
    suffixes: &[String],
    filter: impl Fn(&std::path::Path) -> bool,
) -> Option<Action> {
    let value = &context.value;
    let (dir, prefix) = match value.rfind('/') {
        Some(index) => (value[..index].to_owned(), value[index + 1..].to_owned()),
        None => (String::new(), value.clone()),
    };
    // a leading slash with no directory part resolves against the
    // filesystem root (value `/tmp` lists `/`), not the context directory
    let at_root = value.starts_with('/') && dir.is_empty();
    let resolved = if at_root {
        PathBuf::from("/")
    } else if dir.is_empty() {
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
    crate::log::log(&format!(
        "complete_path value={:?} dir={:?} resolved={:?}\n{:?}",
        value,
        context.dir,
        resolved,
        std::backtrace::Backtrace::capture()
    ));
    let entries = std::fs::read_dir(&resolved).ok()?;
    // hidden entries are only offered when completing a hidden path
    // itself (Go `actionPath`'s showHidden)
    let abs = context.abs(value).unwrap_or_else(|_| value.clone());
    let show_hidden = !abs.ends_with('/')
        && std::path::Path::new(&abs)
            .file_name()
            .map(|name| name.to_string_lossy().starts_with('.'))
            .unwrap_or(false);
    let mut values = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) {
            continue;
        }
        if !show_hidden && name.starts_with('.') {
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
        // the value keeps the typed directory part while the display shows
        // only the completed segment (Go's MultiParts prefix re-attachment)
        let typed_dir = if at_root {
            "/".to_owned()
        } else {
            dir_prefix(Path::new(&dir))
        };
        let mut value = RawValue::new(format!("{typed_dir}{candidate}"));
        value.display = candidate;
        if path.is_dir() {
            value = value.nospace(true);
        }
        // LS_COLORS styling (Go ActionFiles' StyleF(style.ForPath))
        value.style = crate::style::for_path(context, &value.value);
        // implicit file uid (Go ActionFiles' UidF: file:// + PathEscape of
        // the absolute path, slashes preserved as separators)
        if let Ok(abs) = context.abs(&value.value) {
            value.uid = Some(format!("file://{}", crate::uid::path_escape(&abs)));
        }
        values.push(value);
    }
    // Go's ActionFiles/ActionDirectories wrap in MultiParts("/"), which adds
    // the divider to the nospace matcher; shells that only consult the meta
    // (nushell, powershell, xonsh) rely on it.
    // The file query (Go ActionFiles'/ActionDirectories' Query) annotates the
    // completion for cache lookups: C_DIR, C_VALUE and the suffix filter.
    let mut query = crate::meta::Query {
        scheme: "file".to_owned(),
        host: host.to_owned(),
        path: String::new(),
        opts: Vec::new(),
    };
    if let Ok(c_dir) = context.abs(&context.dir.to_string_lossy()) {
        query.opts = vec!["C_DIR".to_owned(), c_dir];
    }
    if !context.value.is_empty() {
        query.opts.push("C_VALUE".to_owned());
        query.opts.push(context.value.clone());
    }
    if host == "files" {
        query.opts.push("suffix".to_owned());
        query.opts.push(suffixes.join(","));
    }
    Some(
        Action::raw(values)
            .nospace(['/'])
            .tag(host)
            .with_meta(crate::meta::Meta::default().query(query)),
    )
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
