//! Normalized command model consumed by the traversal engine.
//!
//! Framework adapters (e.g. `carapace-rs-clap`) convert their native command
//! trees into this model; the engine itself never sees framework types.

use crate::action::Action;

/// Flag-prefix character. `'-'` for POSIX-like syntax; kept as a field so
/// custom prefixes (e.g. `'&'` for elvish) keep working.
pub const DEFAULT_FLAG_PREFIX: char = '-';

/// How many arguments a flag consumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nargs {
    /// Exactly `n` arguments.
    Exact(usize),
    /// At least `min` arguments, at most `max` (clap's bounded
    /// `num_args(min..=max)` with `min > 0, max > min`); values below the
    /// minimum consume unconditionally, above it the lookahead stops at
    /// flag-lookalikes.
    Range(usize, usize),
    /// At least one argument, arbitrary many.
    AtLeastOne,
    /// Zero or more arguments.
    Any,
}

impl Default for Nargs {
    fn default() -> Self {
        Self::Exact(1)
    }
}

/// Which value-binding forms a flag accepts (port of pflagfork `ArgumentStyle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgumentStyle {
    /// `-f value`
    pub accepts_next: bool,
    /// `-f=value`
    pub accepts_delimited: bool,
    /// `-fvalue` (POSIX shorthand attachment)
    pub accepts_attached: bool,
}

impl ArgumentStyle {
    /// All binding forms accepted (pflag default).
    pub const ALL: Self = Self {
        accepts_next: true,
        accepts_delimited: true,
        accepts_attached: true,
    };

    pub const NONE: Self = Self {
        accepts_next: false,
        accepts_delimited: false,
        accepts_attached: false,
    };
}

impl Default for ArgumentStyle {
    fn default() -> Self {
        Self::ALL
    }
}

/// A flag definition with completion metadata.
#[derive(Clone, Debug, Default)]
pub struct Flag {
    /// Identifier matching the framework's arg id (e.g. clap arg id); for
    /// YAML spec flags the flag key exactly as written (Go spec parity) and
    /// the shorthand string for shorthand-only flags. This is the canonical
    /// identity: `Context::parsed_flags` is keyed on it, while the user-facing
    /// spelling lives in [`Flag::name`].
    pub id: String,
    /// Long name (without prefix), e.g. `verbose`.
    pub name: String,
    /// Shorthand character, if any.
    pub shorthand: Option<char>,
    /// Whether the flag was defined with a shorthand but no long name
    /// (pflagfork `ShorthandOnly`; the command-structure export omits the
    /// longhand for these).
    pub shorthand_only: bool,
    /// Long aliases.
    pub aliases: Vec<String>,
    /// Whether the flag takes a value (`false` for booleans and counters).
    pub takes_value: bool,
    /// How many values the flag consumes.
    pub nargs: Nargs,
    /// Whether the value is optional (optarg, pflag `NoOptDefVal` analog).
    pub optional: bool,
    /// Which value-binding forms are accepted.
    pub argument_style: ArgumentStyle,
    /// Whether the flag may occur multiple times.
    pub repeatable: bool,
    /// Whether the flag is required.
    pub required: bool,
    /// Whether the flag is hidden from completion.
    pub hidden: bool,
    /// Whether the flag is persistent (cobra `PersistentFlags`, spec
    /// `persistentflags`): inherited by subcommands; adapters materialize
    /// copies on each child with this marker so the dump can recover the
    /// original section.
    pub persistent: bool,
    /// Whether values may start with the flag prefix (clap `allow_hyphen_values`).
    pub allow_hyphen_values: bool,
    /// Help text.
    pub description: String,
    /// Style reference for flag-name completion.
    pub style: Option<String>,
    /// Tag for flag-name completion (e.g. `localflags`).
    pub tag: Option<String>,
    /// Completion for the flag's value(s), if overridden.
    pub completion: Option<Action>,
    /// Names of flags this flag conflicts with (cobra's mutually-exclusive
    /// annotation, directional here): a conflicting flag that was already
    /// given removes this flag from the flag completion.
    pub conflicts: Vec<String>,
    /// Character separating multiple values inside one word (clap
    /// `value_delimiter`): completion completes the last segment only.
    pub value_delimiter: Option<char>,
}

impl Flag {
    /// Whether this flag still consumes `arg` as its next value.
    ///
    /// Optargs never consume the next positional; multi-value flags consume
    /// until satisfied; variadic flags stop at anything that looks like a flag.
    pub fn consumes(&self, args_so_far: usize, arg: &str) -> bool {
        if !self.takes_value || self.optional {
            return false;
        }
        match self.nargs {
            Nargs::Exact(n) => args_so_far < n,
            Nargs::Range(min, max) => {
                args_so_far < max && (args_so_far < min || !self.looks_like_flag(arg))
            }
            Nargs::AtLeastOne | Nargs::Any => args_so_far == 0 || !self.looks_like_flag(arg),
        }
    }

    /// Whether `arg` looks like a flag for variadic consumption purposes.
    ///
    /// Flags accepting hyphen values never treat such arguments as flags.
    pub fn looks_like_flag(&self, arg: &str) -> bool {
        !self.allow_hyphen_values && arg.starts_with(DEFAULT_FLAG_PREFIX)
    }

    pub fn is_optarg(&self) -> bool {
        // Go's `NoOptDefVal != ""`: boolean and counter flags carry a
        // default value, so they are optargs as well (while still styling
        // as `FlagNoArg`)
        self.optional || !self.takes_value
    }
}

/// A positional argument definition.
#[derive(Clone, Debug, Default)]
pub struct Positional {
    /// Display name.
    pub name: String,
    /// Whether the positional may be omitted.
    pub optional: bool,
    /// Whether the positional may occur multiple times.
    pub repeatable: bool,
    /// Help text.
    pub description: String,
    /// Completion for this positional.
    pub completion: Option<Action>,
    /// Tag for completion values.
    pub tag: Option<String>,
    /// Character separating multiple values inside one word (clap
    /// `value_delimiter`): completion completes the last segment only.
    pub value_delimiter: Option<char>,
}

/// A command node with flags, positionals and subcommands.
#[derive(Clone, Debug, Default)]
pub struct Command {
    /// Command name.
    pub name: String,
    /// Command aliases.
    pub aliases: Vec<String>,
    /// Shorthand character selecting this subcommand (clap `short_flag`).
    pub short_flag: Option<char>,
    /// Flags local to this command.
    pub flags: Vec<Flag>,
    /// Positional arguments.
    pub positionals: Vec<Positional>,
    /// Subcommands.
    pub subcommands: Vec<Command>,
    /// Completion for any positional beyond defined ones.
    pub positional_any: Option<Action>,
    /// Positional completions after `--` (per index).
    pub dash_positionals: Vec<Positional>,
    /// Completion for any positional after `--`.
    pub dash_any: Option<Action>,
    /// Whether flag parsing is disabled entirely (spec `parsing: disabled`).
    pub disable_flag_parsing: bool,
    /// Whether the command is hidden from completion.
    pub hidden: bool,
    /// Group id this command belongs to (spec `group:`; completion tags
    /// and the Highlight styles derive from it, Go `common.Group`).
    pub group: Option<String>,
    /// Whether flags may be interspersed with positionals.
    pub interspersed: bool,
    /// Flag-prefix character.
    pub flag_prefix: char,
    /// Help text.
    pub description: String,
    /// Full use line (Go `cmd.Use`, e.g. `action [pos1] [pos2]`); the
    /// positional completion fallback usage when it has more than one
    /// field. `None` when not expressible (clap trees default to None).
    pub use_line: Option<String>,
}

impl Command {
    /// Create a command with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            interspersed: true,
            flag_prefix: DEFAULT_FLAG_PREFIX,
            ..Default::default()
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn use_line(mut self, use_line: impl Into<String>) -> Self {
        self.use_line = Some(use_line.into());
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn flag(mut self, flag: Flag) -> Self {
        self.flags.push(flag);
        self
    }

    pub fn positional(mut self, positional: Positional) -> Self {
        self.positionals.push(positional);
        self
    }

    pub fn subcommand(mut self, command: Command) -> Self {
        self.subcommands.push(command);
        self
    }

    pub fn positional_any(mut self, action: Action) -> Self {
        self.positional_any = Some(action);
        self
    }

    pub fn interspersed(mut self, interspersed: bool) -> Self {
        self.interspersed = interspersed;
        self
    }

    pub fn flag_prefix(mut self, prefix: char) -> Self {
        self.flag_prefix = prefix;
        self
    }

    /// Look up a flag by long name or alias.
    pub fn lookup_long(&self, name: &str) -> Option<&Flag> {
        self.flags
            .iter()
            .find(|f| f.name == name || f.aliases.iter().any(|a| a == name))
    }

    /// Look up a flag by shorthand character.
    pub fn lookup_short(&self, c: char) -> Option<&Flag> {
        self.flags.iter().find(|f| f.shorthand == Some(c))
    }

    /// Look up a flag by id.
    pub fn lookup_id(&self, id: &str) -> Option<&Flag> {
        self.flags.iter().find(|f| f.id == id)
    }

    /// Find a subcommand by name or alias.
    pub fn lookup_subcommand(&self, name: &str) -> Option<&Command> {
        self.subcommands
            .iter()
            .find(|c| c.name == name || c.aliases.iter().any(|a| a == name))
            .or_else(|| {
                // a shorthand token (`-x`) selects a subcommand with a
                // matching `short_flag` (clap subcommand short flags)
                let token = name.strip_prefix(self.flag_prefix)?;
                let mut chars = token.chars();
                let (flag, rest) = (chars.next()?, chars.next());
                if rest.is_some() {
                    return None;
                }
                self.subcommands.iter().find(|c| c.short_flag == Some(flag))
            })
    }

    /// The positional at `index`, resolving repeatable positionals.
    pub fn positional_at(&self, index: usize) -> Option<&Positional> {
        match self.positionals.get(index) {
            Some(p) => Some(p),
            None => {
                let last = self.positionals.last()?;
                last.repeatable.then_some(last)
            }
        }
    }
}
