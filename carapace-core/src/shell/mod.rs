//! Shared per-shell dispatch (port of Go `internal/shell/shell.go`).

pub mod bash;
pub mod elvish;
pub mod export;
pub mod fish;
pub mod nushell;
pub mod powershell;
pub mod simple;
pub mod style;
pub mod xonsh;
pub mod zsh;

use crate::meta::Meta;
use crate::value::{RawValue, RawValues, by_display, unique};

/// A target shell for completion output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    BashBle,
    CmdClink,
    Elvish,
    Fish,
    Ion,
    Nushell,
    Oil,
    PowerShell,
    Tcsh,
    Xonsh,
    Zsh,
    Export,
}

impl Shell {
    /// Parse a shell name as used in `_carapace <shell>` invocations.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Self::Bash),
            "bash-ble" => Some(Self::BashBle),
            "cmd-clink" => Some(Self::CmdClink),
            "ion" => Some(Self::Ion),
            "oil" => Some(Self::Oil),
            "tcsh" => Some(Self::Tcsh),
            "elvish" => Some(Self::Elvish),
            "fish" => Some(Self::Fish),
            "nushell" => Some(Self::Nushell),
            "powershell" => Some(Self::PowerShell),
            "xonsh" => Some(Self::Xonsh),
            "zsh" => Some(Self::Zsh),
            "export" => Some(Self::Export),
            _ => None,
        }
    }

    /// Detect the shell from the `$SHELL` environment variable.
    pub fn detect() -> Option<Self> {
        let shell = std::env::var("SHELL").ok()?;
        let name = shell.rsplit('/').next()?;
        Self::parse(match name {
            "nu" => "nushell",
            "osh" | "ysh" => "oil",
            "pwsh" | "powershell" => "powershell",
            other => other,
        })
    }

    /// Shells with native message support (messages are not integrated as values).
    fn native_messages(&self) -> bool {
        matches!(self, Self::Elvish | Self::Zsh | Self::Export)
    }

    /// Format invoked values for the shell, applying the shared dispatch steps.
    pub fn value(
        &self,
        current_word: &str,
        meta: &Meta,
        values: &RawValues,
        bash_context: &bash::Context,
    ) -> String {
        let mut meta = meta.clone();
        let mut values = values.clone();

        if env_color_disabled() {
            for value in &mut values {
                value.style = String::new();
            }
        }

        if !env_unfiltered() {
            values.retain(|value| crate::config::has_prefix(&value.value, current_word));
        }

        if env_merge_flags() || *self == Self::Zsh {
            for value in &mut values {
                if matches!(value.tag.as_str(), "shorthand flags" | "longhand flags") {
                    value.tag = "flags".to_owned();
                }
            }
        }

        if !self.native_messages() {
            values = integrate_messages(values, current_word, &meta.messages);
        }

        if *self != Self::Export {
            if !meta.messages.is_empty() {
                meta.nospace.push('*');
            } else if let Some(nospace) = env_nospace() {
                meta.nospace.extend(nospace.chars());
            }
        }

        values.sort_by(by_display);
        for value in &mut values {
            value.uid = None;
        }
        unique(&mut values);

        match self {
            Self::Bash => bash::action_raw_values(current_word, &meta, &values, bash_context),
            Self::BashBle => simple::bash_ble::action_raw_values(current_word, &meta, &values),
            Self::CmdClink => simple::cmd_clink::action_raw_values(current_word, &meta, &values),
            Self::Ion => simple::ion::action_raw_values(current_word, &meta, &values),
            Self::Oil => simple::oil::action_raw_values(current_word, &meta, &values),
            Self::Tcsh => simple::tcsh::action_raw_values(current_word, &meta, &values),
            Self::Elvish => elvish::action_raw_values(current_word, &meta, &values),
            Self::Fish => fish::action_raw_values(&values),
            Self::Nushell => nushell::action_raw_values(current_word, &meta, &values),
            Self::PowerShell => powershell::action_raw_values(current_word, &meta, &values),
            Self::Xonsh => xonsh::action_raw_values(current_word, &meta, &values),
            Self::Zsh => zsh::action_raw_values(current_word, &meta, &values),
            Self::Export => crate::export::Export::from_parts(&meta, &values).to_json(),
        }
    }

    /// Generate the shell integration snippet for a binary.
    pub fn snippet(&self, binary: &str) -> String {
        match self {
            Self::Bash => bash::snippet(binary),
            Self::BashBle => simple::bash_ble::snippet(binary),
            Self::CmdClink => simple::cmd_clink::snippet(binary),
            Self::Ion => String::new(),
            Self::Oil => simple::oil::snippet(binary),
            Self::Tcsh => simple::tcsh::snippet(binary),
            Self::Elvish => elvish::snippet(binary),
            Self::Fish => fish::snippet(binary),
            Self::Nushell => nushell::snippet(binary),
            Self::PowerShell => powershell::snippet(binary),
            Self::Xonsh => xonsh::snippet(binary),
            Self::Zsh => zsh::snippet(binary),
            Self::Export => String::new(),
        }
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| !value.is_empty() && value != "0")
}

fn env_color_disabled() -> bool {
    crate::config::color_disabled()
}

fn env_unfiltered() -> bool {
    env_flag("CARAPACE_UNFILTERED")
}

fn env_merge_flags() -> bool {
    env_flag("CARAPACE_MERGEFLAGS")
}

fn env_nospace() -> Option<String> {
    std::env::var("CARAPACE_NOSPACE")
        .ok()
        .filter(|value| !value.is_empty())
}

/// Integrate messages as synthetic completion values (Go `Messages.Integrate`).
fn integrate_messages(mut values: RawValues, prefix: &str, messages: &[String]) -> RawValues {
    if messages.is_empty() {
        return values;
    }
    let mut sorted = messages.to_vec();
    sorted.sort();

    let mut prefix = prefix.to_owned();
    for suffix in ["ERR", "ER", "E"] {
        if let Some(stripped) = prefix.strip_suffix(suffix) {
            prefix = stripped.to_owned();
            break;
        }
    }

    let mut index = 0usize;
    for message in sorted {
        let (mut value, mut display) = (format!("{prefix}ERR"), "ERR".to_owned());
        loop {
            if index > 0 {
                value = format!("{prefix}ERR{index}");
                display = format!("ERR{index}");
            }
            index += 1;
            if !values.iter().any(|v| v.value == value) {
                break;
            }
        }
        let mut raw = RawValue::styled(value, "underlined");
        raw.display = display;
        raw.description = message;
        values.push(raw);
    }

    if values.len() == 1 {
        values.push(RawValue::new(format!("{prefix}_")));
    }
    values.sort_by(by_display);
    values
}

/// Map a carapace style name to an SGR sequence (subset used by formatters).
pub(crate) fn sgr(style_name: &str) -> String {
    let code = match style_name {
        "bold" => "1",
        "italic" => "3",
        "underlined" => "4",
        "default" | "fg-default" => "39",
        "black" | "fg-black" => "30",
        "red" | "fg-red" => "31",
        "green" | "fg-green" => "32",
        "yellow" | "fg-yellow" => "33",
        "blue" | "fg-blue" => "34",
        "magenta" | "fg-magenta" => "35",
        "cyan" | "fg-cyan" => "36",
        "white" | "fg-white" => "37",
        _ => "39",
    };
    code.to_owned()
}

/// Longest shared prefix of `a` and `b` (char-aligned).
pub(crate) fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let len = a
        .char_indices()
        .zip(b.chars())
        .take_while(|((_, a_char), b_char)| a_char == b_char)
        .map(|((index, a_char), _)| index + a_char.len_utf8())
        .last()
        .unwrap_or(0);
    &a[..len]
}

/// Longest shared prefix across a slice of strings.
pub(crate) fn common_prefix_of(values: &[&str]) -> String {
    let mut iter = values.iter().copied();
    match iter.next() {
        Some(first) => iter.fold(first.to_owned(), |acc, value| {
            common_prefix(&acc, value).to_owned()
        }),
        None => String::new(),
    }
}

/// Longest common prefix of the values' `display` fields.
pub(crate) fn common_display_prefix(values: &RawValues) -> String {
    common_prefix_of(
        &values
            .iter()
            .map(|v| v.display.as_str())
            .collect::<Vec<_>>(),
    )
}

/// Longest common prefix of the values' inserted `value` fields.
pub(crate) fn common_value_prefix(values: &RawValues) -> String {
    common_prefix_of(&values.iter().map(|v| v.value.as_str()).collect::<Vec<_>>())
}

/// Whether completion of `value` suppresses the trailing space.
pub(crate) fn matches_nospace(meta: &Meta, value: &RawValue) -> bool {
    if value.nospace == Some(true) {
        return true;
    }
    meta.nospace
        .iter()
        .any(|suffix| *suffix == '*' || value.value.ends_with(*suffix))
}

/// Description trimmed per `CARAPACE_DESCRIPTION_LENGTH` (default 80).
pub(crate) fn trimmed_description(description: &str) -> String {
    let max_length = crate::config::description_length();
    let first_line = description.split('\n').next().unwrap_or_default();
    let trimmed = first_line.trim();
    let runes: Vec<char> = trimmed.chars().collect();
    if runes.len() > max_length {
        if max_length <= 3 {
            return runes[..max_length].iter().collect();
        }
        let mut result: String = runes[..max_length - 3].iter().collect();
        result.push_str("...");
        return result;
    }
    trimmed.to_owned()
}
