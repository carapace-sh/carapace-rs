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
    /// Environment available to the action.
    pub env: HashMap<String, String>,
    /// Working directory for the action.
    pub dir: PathBuf,
    /// Sandbox mock for tests: mocked replies and cache root (None in production).
    pub mock: Option<crate::mock::Mock>,
}

impl Context {
    /// Create a context from a word list: the last word is the value being completed.
    pub fn new(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        let (value, args) = match args.split_last() {
            Some((value, args)) => (value.clone(), args.to_vec()),
            None => (String::new(), Vec::new()),
        };
        Self {
            value,
            args,
            env: current_env(),
            dir: std::env::current_dir().unwrap_or_default(),
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

    /// Replace `${VAR}` placeholders using the context environment.
    pub fn envsubst(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(start) = rest.find("${") {
            result.push_str(&rest[..start]);
            rest = &rest[start + 2..];
            match rest.find('}') {
                Some(end) => {
                    let var = &rest[..end];
                    result.push_str(&self.get_env(var));
                    rest = &rest[end + 1..];
                }
                None => {
                    result.push_str("${");
                }
            }
        }
        result.push_str(rest);
        result
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
