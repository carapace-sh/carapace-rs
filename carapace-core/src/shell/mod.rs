//! Shared per-shell dispatch (port of Go `internal/shell/shell.go`).

pub mod bash;
pub mod fish;
pub mod zsh;

use crate::meta::Meta;
use crate::value::{RawValue, RawValues, by_display, unique};

/// A target shell for completion output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
    Export,
}

impl Shell {
    /// Parse a shell name as used in `_carapace <shell>` invocations.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Self::Bash),
            "fish" => Some(Self::Fish),
            "zsh" => Some(Self::Zsh),
            "export" => Some(Self::Export),
            _ => None,
        }
    }

    /// Shells with native message support (messages are not integrated as values).
    fn native_messages(&self) -> bool {
        matches!(self, Self::Zsh | Self::Export)
    }

    /// Format invoked values for the shell, applying the shared dispatch steps.
    pub fn value(&self, current_word: &str, meta: &Meta, values: &RawValues) -> String {
        let mut meta = meta.clone();
        let mut values = values.clone();

        if env_color_disabled() {
            for value in &mut values {
                value.style = String::new();
            }
        }

        if !env_unfiltered() {
            values.retain(|value| value.value.starts_with(current_word));
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
            Self::Bash => bash::action_raw_values(current_word, &meta, &values, &bash::Context::default()),
            Self::Fish => fish::action_raw_values(&values),
            Self::Zsh => zsh::action_raw_values(current_word, &meta, &values),
            Self::Export => crate::export::Export::from_parts(&meta, &values).to_json(),
        }
    }

    /// Generate the shell integration snippet for a binary.
    pub fn snippet(&self, binary: &str) -> String {
        match self {
            Self::Bash => bash::snippet(binary),
            Self::Fish => fish::snippet(binary),
            Self::Zsh => zsh::snippet(binary),
            Self::Export => String::new(),
        }
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| !value.is_empty() && value != "0")
}

fn env_color_disabled() -> bool {
    std::env::var("CARAPACE_COLOR").is_ok_and(|value| value == "disabled")
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
