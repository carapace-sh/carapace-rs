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
            let action = complete_path(&c, |path| path.is_dir());
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
            let action = complete_path(&c, |path| {
                suffixes.is_empty()
                    || suffixes
                        .iter()
                        .any(|suffix| c.value.to_ascii_lowercase().ends_with(&suffix.to_ascii_lowercase()) || path.extension().map(|ext| format!(".{}", ext.to_string_lossy())) == Some(suffix.clone()))
            });
            match action {
                Some(action) => action,
                None => Self::message("no matches"),
            }
        })
    }
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
