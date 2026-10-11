//! carapace completion for [clap](https://github.com/clap-rs/clap)-based CLIs.
//!
//! Completions are attached to clap args via extension slots and converted
//! into the framework-independent [`carapace_core::model::Command`] tree
//! consumed by the traversal engine.

mod completer;
mod convert;
mod entry;
mod integration;

pub use completer::{
    ArgCompleter, CommandCompleter, DashAnyCompleter, DashCompleter, PositionalCompleter,
};
pub use convert::to_model;
pub use entry::{complete, complete_output, execute_embedded, snippet};
pub use integration::Carapace;

/// Attach a completion action to the value(s) of the flag or positional
/// identified by its clap arg id.
pub fn arg_completion(cmd: &mut clap::Command, id: &str, action: carapace_core::Action) {
    *cmd = std::mem::take(cmd).mut_arg(id, |arg| arg.add(ArgCompleter::new(action)));
}

/// Attach a completion action to the arg with the given long name or shorthand.
pub fn flag_completion(cmd: &mut clap::Command, name: &str, action: carapace_core::Action) {
    let id = cmd
        .get_arguments()
        .find(|arg| {
            arg.get_long() == Some(name)
                || arg.get_short().map(|c| c.to_string()).as_deref() == Some(name)
        })
        .map(|arg| arg.get_id().as_str().to_owned());
    if let Some(id) = id {
        arg_completion(cmd, &id, action);
    }
}

/// Define completion for the defined positionals, in order (port of Go
/// `PositionalCompletion`): the Nth action completes the Nth positional,
/// as long as that positional has no own completion slot. A later call
/// replaces the whole list (Go assignment semantics).
pub fn positional_completion(
    cmd: &mut clap::Command,
    actions: impl IntoIterator<Item = carapace_core::Action>,
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
    actions: impl IntoIterator<Item = carapace_core::Action>,
) {
    let actions: Vec<_> = actions.into_iter().collect();
    *cmd = std::mem::take(cmd).add(DashCompleter(actions));
}
