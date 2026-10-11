//! Fluent completion wiring for clap commands (port of Go `carapace.Gen`).

use crate::completer::{CommandCompleter, DashAnyCompleter};
use carapace_core::Action;

/// Completion wiring for [`clap::Command`].
///
/// Go carapace wraps the command in a `Carapace` struct because Go cannot
/// add methods to foreign types; here the extension trait provides the same
/// chaining directly on `clap::Command`:
///
/// ```rust
/// use carapace_clap::Carapace;
/// use carapace_core::Action;
/// use clap::Command;
///
/// let cmd = Command::new("myapp")
///     .flag_completion([("profile", Action::values(["dev", "prod"]))]);
/// ```
pub trait Carapace {
    /// Complete the value(s) of the named flags: keys are flag long names,
    /// aliases or shorthands (Go's `FlagCompletion` ActionMap).
    fn flag_completion<K: Into<String>>(
        self,
        actions: impl IntoIterator<Item = (K, Action)>,
    ) -> Self;
    /// Complete the defined positionals, in order: the Nth action completes
    /// the Nth positional (unless it has its own completion slot).
    fn positional_completion(self, actions: impl IntoIterator<Item = Action>) -> Self;
    /// Complete any positional beyond the defined ones.
    fn positional_any_completion(self, action: Action) -> Self;
    /// Complete the positional arguments after `--`, in order; entries
    /// beyond the defined ones fall back to [`Carapace::dash_any_completion`].
    fn dash_completion(self, actions: impl IntoIterator<Item = Action>) -> Self;
    /// Complete any positional after `--`.
    fn dash_any_completion(self, action: Action) -> Self;
}

impl Carapace for clap::Command {
    fn flag_completion<K: Into<String>>(
        mut self,
        actions: impl IntoIterator<Item = (K, Action)>,
    ) -> Self {
        self.build();
        crate::flag_completion(&mut self, actions);
        self
    }

    fn positional_completion(mut self, actions: impl IntoIterator<Item = Action>) -> Self {
        self.build();
        crate::positional_completion(&mut self, actions);
        self
    }

    fn positional_any_completion(mut self, action: Action) -> Self {
        self.build();
        self = self.add(CommandCompleter::new(action));
        self
    }

    fn dash_completion(mut self, actions: impl IntoIterator<Item = Action>) -> Self {
        self.build();
        crate::dash_completion(&mut self, actions);
        self
    }

    fn dash_any_completion(mut self, action: Action) -> Self {
        self.build();
        self = self.add(DashAnyCompleter::new(action));
        self
    }
}
