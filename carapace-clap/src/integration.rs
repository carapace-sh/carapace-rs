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
///     .flag_completion("profile", Action::values(["dev", "prod"]));
/// ```
pub trait Carapace {
    /// Complete the value(s) of the arg with the given clap id.
    fn arg_completion(self, id: &str, action: Action) -> Self;
    /// Complete the value(s) of the named flag (long name or shorthand).
    fn flag_completion(self, name: &str, action: Action) -> Self;
    /// Complete the positional at the given index (1-based).
    fn positional_completion(self, index: usize, action: Action) -> Self;
    /// Complete any positional beyond the defined ones.
    fn positional_any_completion(self, action: Action) -> Self;
    /// Complete the dash argument (`--`) at the given 1-based index.
    fn positional_dash_completion(self, index: usize, action: Action) -> Self;
    /// Complete any positional after `--`.
    fn positional_dash_any_completion(self, action: Action) -> Self;
}

impl Carapace for clap::Command {
    fn arg_completion(mut self, id: &str, action: Action) -> Self {
        self.build();
        crate::arg_completion(&mut self, id, action);
        self
    }

    fn flag_completion(mut self, name: &str, action: Action) -> Self {
        self.build();
        crate::flag_completion(&mut self, name, action);
        self
    }

    fn positional_completion(mut self, index: usize, action: Action) -> Self {
        self.build();
        crate::positional_completion(&mut self, index, action);
        self
    }

    fn positional_any_completion(mut self, action: Action) -> Self {
        self.build();
        self = self.add(CommandCompleter::new(action));
        self
    }

    fn positional_dash_completion(mut self, index: usize, action: Action) -> Self {
        self.build();
        crate::positional_dash_completion(&mut self, index, action);
        self
    }

    fn positional_dash_any_completion(mut self, action: Action) -> Self {
        self.build();
        self = self.add(DashAnyCompleter::new(action));
        self
    }
}
