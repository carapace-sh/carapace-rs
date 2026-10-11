//! Extension types attaching carapace completers to clap args and commands.

use carapace_core::Action;
use clap::builder::{ArgExt, CommandExt};

/// Completion for the value(s) of an [`clap::Arg`], stored as an arg extension.
#[derive(Clone)]
pub struct ArgCompleter(pub Action);

impl ArgCompleter {
    pub fn new(action: Action) -> Self {
        Self(action)
    }
}

impl std::fmt::Debug for ArgCompleter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ArgCompleter").finish_non_exhaustive()
    }
}

impl ArgExt for ArgCompleter {}

/// Completion for any positional after `--`, stored as a command extension.
#[derive(Clone)]
pub struct DashAnyCompleter(pub Action);

impl DashAnyCompleter {
    pub fn new(action: Action) -> Self {
        Self(action)
    }
}

impl std::fmt::Debug for DashAnyCompleter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DashAnyCompleter").finish_non_exhaustive()
    }
}

impl CommandExt for DashAnyCompleter {}

/// Command-level completion (e.g. any-positional), stored as a command extension.
#[derive(Clone, Default)]
pub struct CommandCompleter(pub Action);

impl CommandCompleter {
    pub fn new(action: Action) -> Self {
        Self(action)
    }
}

impl std::fmt::Debug for CommandCompleter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CommandCompleter").finish_non_exhaustive()
    }
}

impl CommandExt for CommandCompleter {}

/// Per-index completion for positionals after `--`, stored as a command
/// extension. Entries are (1-based dash-argument index, action); setting
/// the same index again replaces the previous action.
#[derive(Clone, Debug, Default)]
pub struct DashPositionalCompleter(pub Vec<(usize, Action)>);

impl CommandExt for DashPositionalCompleter {}
