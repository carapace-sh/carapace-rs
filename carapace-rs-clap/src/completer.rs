//! Extension types attaching carapace completers to clap args and commands.

use carapace_rs::Action;
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

/// Flag conflict names (cobra `MarkFlagsMutuallyExclusive`, directional):
/// a conflicting flag that was already given removes this flag from the
/// flag-name completion. Stored as an arg extension since clap does not
/// expose its conflict graph.
#[derive(Clone, Debug, Default)]
pub struct ArgConflicts(pub Vec<String>);

impl ArgExt for ArgConflicts {}

/// Marks an arg as persistent (cobra `PersistentFlags`): the flag and its
/// completion are inherited by all subcommands.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArgPersistent;

impl ArgExt for ArgPersistent {}

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

/// Completion for the defined positionals, in order (port of Go's
/// command-level `positional` storage): the Nth action completes the Nth
/// positional. Stored as a command extension; a positional's own
/// [`ArgCompleter`] slot (e.g. from the macros) takes precedence.
#[derive(Clone, Debug, Default)]
pub struct PositionalCompleter(pub Vec<Action>);

impl CommandExt for PositionalCompleter {}

/// Completion for flags by name (port of Go's command-level `flag`
/// ActionMap storage): keys are flag long names, aliases or shorthands.
/// Stored as a command extension and resolved at conversion time, so flags
/// may be defined after the call; a flag's own [`ArgCompleter`] slot (e.g.
/// from the macros) takes precedence, and a later call merges per key.
#[derive(Clone, Debug, Default)]
pub struct FlagCompleter(pub Vec<(String, Action)>);

impl CommandExt for FlagCompleter {}

/// Completion for the positional arguments after `--`, in order (port of
/// Go's command-level `dash` storage): the Nth action completes the Nth
/// dash argument. Stored as a command extension; entries beyond the
/// defined ones fall back to [`DashAnyCompleter`].
#[derive(Clone, Debug, Default)]
pub struct DashCompleter(pub Vec<Action>);

impl CommandExt for DashCompleter {}

/// Model properties cobra commands have but clap cannot express, carried
/// as a command extension and merged into the normalized model at
/// conversion time (Go `GroupID`, `DisableFlagParsing`, `cmd.Use`).
#[derive(Clone, Debug, Default)]
pub struct CommandMeta {
    /// Group id this command belongs to (Go `GroupID`).
    pub group: Option<String>,
    /// Whether flag parsing is disabled entirely (Go `DisableFlagParsing`).
    pub disable_flag_parsing: bool,
    /// Full use line (Go `cmd.Use`); the positional completion fallback
    /// usage when it has more than one field.
    pub use_line: Option<String>,
}

impl CommandExt for CommandMeta {}
