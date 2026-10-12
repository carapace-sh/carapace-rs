//! carapace completion for [clap](https://github.com/clap-rs/clap)-based CLIs.
//!
//! Completions are attached to clap args via extension slots and converted
//! into the framework-independent [`carapace_rs::model::Command`] tree
//! consumed by the traversal engine.

mod completer;
mod convert;
mod entry;
mod integration;

pub use completer::{
    ArgCompleter, CommandCompleter, DashAnyCompleter, DashCompleter, FlagCompleter,
    PositionalCompleter,
};
pub use convert::to_model;
pub use entry::{complete, complete_output, execute_embedded, snippet};
pub use integration::Carapace;

/// Wrap an action expression in an [`ArgCompleter`] for use in clap derive's
/// `add = <expr>` escape hatch.
///
/// ```
/// # use clap::Parser;
/// #[derive(Parser)]
/// struct Cli {
///     #[arg(long, add = carapace_rs_clap::completer!(carapace_rs::Action::values(["dev", "prod"])))]
///     profile: Option<String>,
/// }
/// ```
#[macro_export]
macro_rules! completer {
    ($action:expr) => {
        $crate::ArgCompleter::new($action)
    };
}

/// Wrap an action expression in a [`CommandCompleter`] for use in clap
/// derive's `add = <expr>` escape hatch.
#[macro_export]
macro_rules! command_completer {
    ($action:expr) => {
        $crate::CommandCompleter::new($action)
    };
}

/// Define completion for flags by name (port of Go `FlagCompletion` with
/// its `ActionMap`): keys are flag long names, aliases or shorthands.
/// Later calls merge per key (a repeated key replaces that flag's action).
pub fn flag_completion<K: Into<String>>(
    cmd: &mut clap::Command,
    actions: impl IntoIterator<Item = (K, carapace_rs::Action)>,
) {
    let mut entries = cmd
        .get::<FlagCompleter>()
        .map(|completer| completer.0.clone())
        .unwrap_or_default();
    entries.extend(
        actions
            .into_iter()
            .map(|(key, action)| (key.into(), action)),
    );
    *cmd = std::mem::take(cmd).add(FlagCompleter(entries));
}

/// Define completion for the defined positionals, in order (port of Go
/// `PositionalCompletion`): the Nth action completes the Nth positional,
/// as long as that positional has no own completion slot. A later call
/// replaces the whole list (Go assignment semantics).
pub fn positional_completion(
    cmd: &mut clap::Command,
    actions: impl IntoIterator<Item = carapace_rs::Action>,
) {
    let actions: Vec<_> = actions.into_iter().collect();
    *cmd = std::mem::take(cmd).add(PositionalCompleter(actions));
}

/// Define completion for the positional arguments after `--`, in order
/// (port of Go `DashCompletion`): the Nth action completes the Nth dash
/// argument, entries beyond the defined ones fall back to
/// [`dash_any_completion`]. A later call replaces the whole list.
pub fn dash_completion(
    cmd: &mut clap::Command,
    actions: impl IntoIterator<Item = carapace_rs::Action>,
) {
    let actions: Vec<_> = actions.into_iter().collect();
    *cmd = std::mem::take(cmd).add(DashCompleter(actions));
}

pub use crate::completer::{ArgConflicts, ArgPersistent};
