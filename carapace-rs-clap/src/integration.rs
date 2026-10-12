//! Fluent completion wiring for clap commands (port of Go `carapace.Gen`).

use crate::completer::{CommandCompleter, CommandMeta, DashAnyCompleter};
use carapace_rs::Action;

/// Completion wiring for [`clap::Command`].
///
/// Go carapace wraps the command in a `Carapace` struct because Go cannot
/// add methods to foreign types; here the extension trait provides the same
/// chaining directly on `clap::Command`:
///
/// ```rust
/// use carapace_rs_clap::Carapace;
/// use carapace_rs::Action;
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
    /// Assign the command to a group (cobra `GroupID`): the group id
    /// becomes the completion tag and selects the highlight style.
    /// Named `completion_group` because clap's inherent `Command::group`
    /// takes precedence over trait methods.
    fn completion_group(self, id: impl Into<String>) -> Self;
    /// Disable flag parsing entirely (cobra `DisableFlagParsing`): every
    /// argument completes as a positional.
    fn disable_flag_parsing(self) -> Self;
    /// Set the full use line (cobra `cmd.Use`), used as the fallback usage
    /// for positional completions when it has more than one field.
    fn use_line(self, line: impl Into<String>) -> Self;
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

    fn completion_group(self, id: impl Into<String>) -> Self {
        let mut meta = self.get::<CommandMeta>().cloned().unwrap_or_default();
        meta.group = Some(id.into());
        self.add(meta)
    }

    fn disable_flag_parsing(self) -> Self {
        let mut meta = self.get::<CommandMeta>().cloned().unwrap_or_default();
        meta.disable_flag_parsing = true;
        self.add(meta)
    }

    fn use_line(self, line: impl Into<String>) -> Self {
        let mut meta = self.get::<CommandMeta>().cloned().unwrap_or_default();
        meta.use_line = Some(line.into());
        self.add(meta)
    }
}

/// Merge [`CommandMeta`] extensions into the normalized model.
pub(crate) fn apply_meta(cmd: &clap::Command, model: &mut carapace_rs::model::Command) {
    if let Some(meta) = cmd.get::<CommandMeta>() {
        model.group = meta.group.clone();
        model.disable_flag_parsing |= meta.disable_flag_parsing;
        if meta.use_line.is_some() {
            model.use_line = meta.use_line.clone();
        }
    }
}
