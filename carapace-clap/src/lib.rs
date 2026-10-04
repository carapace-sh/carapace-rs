//! carapace completion for [clap](https://github.com/clap-rs/clap)-based CLIs.
//!
//! Completions are attached to clap args via extension slots and converted
//! into the framework-independent [`carapace_core::model::Command`] tree
//! consumed by the traversal engine.

mod completer;
mod entry;
mod convert;
mod integration;

pub use completer::{ArgCompleter, CommandCompleter, DashAnyCompleter};
pub use convert::to_model;
pub use entry::{complete, complete_output, execute_embedded, snippet};
pub use integration::Gen;

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

/// Attach a completion action to the positional at the given index (1-based).
pub fn positional_completion(
    cmd: &mut clap::Command,
    index: usize,
    action: carapace_core::Action,
) {
    let id = cmd
        .get_positionals()
        .find(|arg| arg.get_index() == Some(index))
        .map(|arg| arg.get_id().as_str().to_owned());
    if let Some(id) = id {
        arg_completion(cmd, &id, action);
    }
}
