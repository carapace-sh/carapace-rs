//! Runtime state passed to action callbacks during invocation.

use std::collections::HashMap;
use std::path::PathBuf;

/// Runtime state during completion.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// Current value being completed (or the current part during multipart completion).
    pub value: String,
    /// Positional arguments of the current command, excluding the one being completed.
    pub args: Vec<String>,
    /// Previous parts of the value during multipart completion.
    pub parts: Vec<String>,
    /// Positional arguments of the current command including those after
    /// `--` (Go `Flags().Args()`); only populated while completing dash
    /// arguments, `None` otherwise.
    pub all_positionals: Option<Vec<String>>,
    /// Flags with values seen during traversal (Go's pflag `Changed`
    /// state plus the consumed values); used for repeatable/conflict
    /// filtering and callback actions.
    pub parsed_flags: Vec<(String, Vec<String>)>,
    /// Environment available to the action.
    pub env: HashMap<String, String>,
    /// Working directory for the action.
    pub dir: PathBuf,
    /// Sandbox mock for tests: mocked replies and cache root (None in production).
    pub mock: Option<crate::mock::Mock>,
    /// `CARAPACE_*` completion flags, parsed from the environment map
    /// (Go reads them from process environment functions).
    pub config: crate::config::Flags,
}

impl Context {
    /// Create a context from a word list: the last word is the value being completed.
    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        let (value, args) = match args.split_last() {
            Some((value, args)) => (value.clone(), args.to_vec()),
            None => (String::new(), Vec::new()),
        };
        let env = current_env();
        let config = crate::config::Flags::from_env_map(&env);
        Self {
            value,
            args,
            env,
            dir: std::env::current_dir().unwrap_or_default(),
            config,
            ..Default::default()
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = dir.into();
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self.config = crate::config::Flags::from_env_map(&self.env);
        self
    }

    /// Attach a sandbox mock: mocked replies and the cache root for tests.
    pub fn mock(mut self, mock: crate::mock::Mock) -> Self {
        self.mock = Some(mock);
        self
    }

    pub fn lookup_env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(|v| v.as_str())
    }

    pub fn get_env(&self, key: &str) -> String {
        self.lookup_env(key).unwrap_or_default().to_owned()
    }

    /// Replace `${VAR}` placeholders using the context environment
    /// (Go `Context.Envsubst`, drone/envsubst subset).
    ///
    /// Supported forms: `${VAR}`, `${VAR:-default}`, `${VAR/old/new}`
    /// (first occurrence), `${VAR//old/new}` (all occurrences), and
    /// `${!VAR...}` which passes the unexpanded form through literally.
    /// `\` escapes `/` and `\` inside replacement patterns.
    pub fn envsubst(&self, s: &str) -> Result<String, String> {
        let mut result = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(start) = rest.find("${") {
            result.push_str(&rest[..start]);
            rest = &rest[start + 2..];

            // `${!...}`: the unexpanded form is passed through verbatim
            if let Some(stripped) = rest.strip_prefix('!') {
                let end = stripped.find('}').ok_or("bad substitution")?;
                result.push_str("${");
                result.push_str(&stripped[..end]);
                result.push('}');
                rest = &stripped[end + 1..];
                continue;
            }

            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.is_empty() {
                return Err("bad substitution".to_owned());
            }
            rest = &rest[name.len()..];

            if let Some(value_rest) = rest.strip_prefix('}') {
                result.push_str(&self.get_env(&name));
                rest = value_rest;
                continue;
            }

            if let Some(default_rest) = rest.strip_prefix(":-") {
                let end = default_rest.find('}').ok_or("bad substitution")?;
                let value = self.get_env(&name);
                result.push_str(if value.is_empty() {
                    &default_rest[..end]
                } else {
                    &value
                });
                rest = &default_rest[end + 1..];
                continue;
            }

            if let Some(replace_rest) = rest.strip_prefix('/') {
                let all = replace_rest.starts_with('/');
                let replace_rest = if all {
                    &replace_rest[1..]
                } else {
                    replace_rest
                };
                let (old, replace_rest) = replace_rest.split_once('/').ok_or("bad substitution")?;
                let end = replace_rest.find('}').ok_or("bad substitution")?;
                let new = unescape_replacement(&replace_rest[..end]);
                let value = self.get_env(&name);
                result.push_str(&if all {
                    value.replace(&unescape_replacement(old), &new)
                } else {
                    value.replacen(&unescape_replacement(old), &new, 1)
                });
                rest = &replace_rest[end + 1..];
                continue;
            }

            return Err("bad substitution".to_owned());
        }
        result.push_str(rest);
        Ok(result)
    }

    /// Absolute representation of a path (Go `Context.Abs`): relative paths
    /// resolve against the context directory, `~` expands to the home
    /// directory, and the result is cleaned with trailing slashes preserved.
    pub fn abs(&self, path: &str) -> Result<String, String> {
        let path = path.replace('\\', "/");
        let path = if !path.starts_with('/') && !path.starts_with('~') && !has_volume_prefix(&path)
        {
            if self.dir.as_os_str().is_empty() {
                format!("./{path}")
            } else {
                format!("{}/{path}", self.dir.to_string_lossy())
            }
        } else {
            path
        };

        let path = expand_home(&path)?;

        // a bare volume prefix must not join with the current directory
        let path = if path.chars().count() == 2 && has_volume_prefix(&path) {
            format!("{path}/")
        } else {
            path
        };

        let result = absolutize(&path);
        if path.ends_with("/.") && !result.ends_with("/.") {
            Ok(format!("{result}/."))
        } else if path.ends_with('/') && !result.ends_with('/') {
            Ok(format!("{result}/"))
        } else {
            Ok(result)
        }
    }

    /// Execute a command with this context's environment and directory.
    ///
    /// When a sandbox mock is attached to the context and a reply is
    /// registered for the command, the mocked output is returned instead.
    pub fn execute(&self, program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
        if let Some(reply) = self
            .mock
            .as_ref()
            .and_then(|mock| mock.reply(program, args))
        {
            return Ok(reply.into_bytes());
        }
        let mut command = std::process::Command::new(program);
        command.args(args).env_clear().envs(&self.env);
        if !self.dir.as_os_str().is_empty() {
            command.current_dir(&self.dir);
        }
        let output = command.output().map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    /// Shift `n` arguments off the front of `args`.
    pub fn shifted(&self, n: usize) -> Self {
        let mut shifted = self.clone();
        let n = n.min(shifted.args.len());
        shifted.args.drain(..n);
        shifted
    }
}

fn current_env() -> HashMap<String, String> {
    std::env::vars().collect()
}

/// Windows-style drive prefix (`C:`), a no-op on other platforms
/// (Go `util.HasVolumePrefix`).
fn has_volume_prefix(s: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Expand a leading `~` to the home directory (Go `expandHome`, without
/// zsh named directories).
fn expand_home(path: &str) -> Result<String, String> {
    if !path.starts_with('~') {
        return Ok(path.to_owned());
    }
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map_err(|_| "cannot expand user home directory".to_owned())?
        .replace('\\', "/");
    match path {
        "~" => Ok(home),
        _ => Ok(path.replacen("~/", &format!("{home}/"), 1)),
    }
}

/// Make a slash path absolute and lexically clean (Go `filepath.Abs`).
fn absolutize(path: &str) -> String {
    let joined = if path.starts_with('/') || has_volume_prefix(path) {
        path.to_owned()
    } else {
        let cwd = std::env::current_dir()
            .map(|dir| dir.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        format!("{cwd}/{path}")
    };
    clean_path(&joined)
}

/// Lexically normalize a slash path (Go `filepath.Clean`).
fn clean_path(path: &str) -> String {
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if !parts.is_empty() && parts.last() != Some(&"..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    if rooted {
        format!("/{}", parts.join("/"))
    } else if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// Unescape `\/` and `\\` in replacement patterns
/// (drone/envsubst `scanEscape`).
fn unescape_replacement(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(escaped @ ('/' | '\\')) => result.push(escaped),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}
