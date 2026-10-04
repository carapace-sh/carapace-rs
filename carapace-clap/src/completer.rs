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

/// Command-level completion (e.g. any-positional), stored as a command extension.
#[derive(Clone)]
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
