//! Builder-style integration API (port of Go `carapace.Gen(cmd)`).

use crate::completer::CommandCompleter;
use carapace_core::Action;

/// Completion configuration for a clap command.
///
/// Mirrors Go carapace's `Gen`: completers are attached to the underlying
/// clap command and stay with it.
#[derive(Debug)]
pub struct Gen {
    cmd: clap::Command,
}

impl Gen {
    /// Wrap a clap command for completion configuration.
    ///
    /// The command is built so positional indices are resolved.
    pub fn new(mut cmd: clap::Command) -> Self {
        cmd.build();
        Self { cmd }
    }

    /// Complete the positional at the given index (1-based).
    pub fn positional_completion(mut self, index: usize, action: Action) -> Self {
        super::positional_completion(&mut self.cmd, index, action);
        self
    }

    /// Complete any positional beyond the defined ones.
    pub fn positional_any_completion(mut self, action: Action) -> Self {
        self.cmd = self.cmd.add(CommandCompleter::new(action));
        self
    }

    /// Complete the value(s) of the named flag.
    pub fn flag_completion(mut self, name: &str, action: Action) -> Self {
        super::flag_completion(&mut self.cmd, name, action);
        self
    }

    /// Complete any positional after `--`.
    pub fn positional_dash_any_completion(mut self, action: Action) -> Self {
        self.cmd = self.cmd.add(crate::DashAnyCompleter::new(action));
        self
    }

    /// Complete the value(s) of the arg with the given clap id.
    pub fn arg_completion(mut self, id: &str, action: Action) -> Self {
        super::arg_completion(&mut self.cmd, id, action);
        self
    }

    /// Unwrap the configured clap command.
    pub fn into_command(self) -> clap::Command {
        self.cmd
    }
}
