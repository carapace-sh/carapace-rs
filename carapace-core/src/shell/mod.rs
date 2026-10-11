//! Shared per-shell dispatch (port of Go `internal/shell/shell.go`).

pub mod bash;
pub mod elvish;
pub mod export;
pub mod fish;
pub mod lscolors;
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

    /// Detect the shell from the parent process chain (Go
    /// `ps.DetermineShell`): walk `/proc` upward and match the first
    /// executable belonging to a known shell. Bash is reported as
    /// bash-ble when ble.sh is attached.
    ///
    /// Returns `None` on non-unix platforms (the `/proc` walk is the only
    /// supported source here).
    pub fn detect() -> Option<Self> {
        let mut pid = std::process::id();
        loop {
            pid = process_parent(pid)?;
            let mut name = process_name(pid);
            if matches!(name.as_str(), "linker" | "linker64") {
                // Termux rewrites execve through the system linker; the
                // real binary is the first cmdline entry
                name = std::fs::read_to_string(format!("/proc/{pid}/cmdline").as_str())
                    .ok()
                    .and_then(|data| {
                        data.split('\0')
                            .next()
                            .map(|path| path.rsplit('/').next().unwrap_or(path).to_owned())
                    })
                    .unwrap_or(name);
            }
            let trimmed = name.trim_end_matches(".exe");
            let base = trimmed.split('-').next().unwrap_or(trimmed);
            match base {
                "bash" => {
                    return Some(if is_ble() { Self::BashBle } else { Self::Bash });
                }
                "cmd" => return Some(Self::CmdClink),
                "elvish" => return Some(Self::Elvish),
                "fish" => return Some(Self::Fish),
                "ion" => return Some(Self::Ion),
                "nu" => return Some(Self::Nushell),
                "oil" | "osh" => return Some(Self::Oil),
                "powershell" | "pwsh" => return Some(Self::PowerShell),
                "tcsh" => return Some(Self::Tcsh),
                "xonsh" => return Some(Self::Xonsh),
                "zsh" => return Some(Self::Zsh),
                _ => {
                    if name.contains("xonsh-wrapped") {
                        // nix packaged version
                        return Some(Self::Xonsh);
                    }
                }
            }
        }
    }

    /// The shell's name (Go's shell identifier).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::BashBle => "bash-ble",
            Self::CmdClink => "cmd-clink",
            Self::Elvish => "elvish",
            Self::Fish => "fish",
            Self::Ion => "ion",
            Self::Nushell => "nushell",
            Self::Oil => "oil",
            Self::PowerShell => "powershell",
            Self::Tcsh => "tcsh",
            Self::Xonsh => "xonsh",
            Self::Zsh => "zsh",
            Self::Export => "export",
        }
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

/// The parent pid of the process, from `/proc/{pid}/stat`.
fn process_parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // the comm field is parenthesized and may contain spaces
    let after = stat.rsplit(')').next()?;
    let mut fields = after.split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse().ok()
}

/// The executable name of the process, from `/proc/{pid}/stat` comm.
fn process_name(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let start = stat.find('(')?;
            let end = stat.rfind(')')?;
            Some(stat[start + 1..end].to_owned())
        })
        .unwrap_or_default()
}

/// Whether ble.sh is attached, by its exported helper variables
/// (Go `isBLE`).
fn is_ble() -> bool {
    [
        "_bleopt_connect_tty",
        "_ble_util_fdlist_cloexec",
        "_ble_util_fd_null",
        "_ble_util_fd_stderr",
        "_ble_util_fd_stdin",
        "_ble_util_fd_stdout",
        "_ble_util_fdvars_export",
        "_ble_util_fd_zero",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
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

/// The style for a path from `LS_COLORS` (Go `style.ForPath`).
pub fn for_path(context: &crate::Context, path: &str) -> String {
    let path = context.abs(path).unwrap_or_else(|_| path.to_owned());
    let ls_colors = context.get_env("LS_COLORS");
    let colorist = lscolors::colorist(&ls_colors);
    from_sgr(&colorist.get_style(&path))
}

/// The style for a path by extension only (Go `style.ForPathExt`).
pub fn for_path_ext(context: &crate::Context, path: &str) -> String {
    let ls_colors = context.get_env("LS_COLORS");
    let colorist = lscolors::colorist(&ls_colors);
    from_sgr(&colorist.get_style_ext(path))
}

/// The style for an extension name (Go `style.ForExtension`).
pub fn for_extension(context: &crate::Context, extension: &str) -> String {
    for_path_ext(context, &format!(".{extension}"))
}

/// Convert an SGR sequence into a carapace style string (Go `style.fromSGR`).
fn from_sgr(sgr: &str) -> String {
    let style = style::style_from_sgr(sgr);
    let mut result: Vec<String> = Vec::new();
    if let Some(foreground) = &style.foreground {
        result.push(foreground.clone());
    }
    if let Some(background) = &style.background {
        result.push(format!("bg-{background}"));
    }
    if style.bold {
        result.push("bold".to_owned());
    }
    if style.dim {
        result.push("dim".to_owned());
    }
    if style.italic {
        result.push("italic".to_owned());
    }
    if style.underlined {
        result.push("underlined".to_owned());
    }
    if style.blink {
        result.push("blink".to_owned());
    }
    if style.inverse {
        result.push("inverse".to_owned());
    }
    result.join(" ")
}

/// Map a carapace style string to an SGR sequence (Go `style.SGR`).
pub(crate) fn sgr(style_string: &str) -> String {
    let style = style::Style::parse(style_string);
    let mut codes: Vec<String> = Vec::new();
    if style.bold {
        codes.push("1".to_owned());
    }
    if style.dim {
        codes.push("2".to_owned());
    }
    if style.italic {
        codes.push("3".to_owned());
    }
    if style.underlined {
        codes.push("4".to_owned());
    }
    if style.blink {
        codes.push("5".to_owned());
    }
    if style.inverse {
        codes.push("7".to_owned());
    }
    if let Some(foreground) = &style.foreground {
        codes.push(color_sgr(foreground, false));
    }
    if let Some(background) = &style.background {
        codes.push(color_sgr(background, true));
    }
    codes.join(";")
}

fn color_sgr(color: &str, background: bool) -> String {
    let (default, base, bright_base, extended) = if background {
        ("49", 40, 100, "48")
    } else {
        ("39", 30, 90, "38")
    };
    match color {
        "default" => default.to_owned(),
        _ => {
            if let Some(index) = [
                "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
            ]
            .iter()
            .position(|name| *name == color)
            {
                (base + index).to_string()
            } else if let Some(bright) = color.strip_prefix("bright-") {
                match [
                    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
                ]
                .iter()
                .position(|name| *name == bright)
                {
                    Some(index) => (bright_base + index).to_string(),
                    None => default.to_owned(),
                }
            } else if let Some(index) = color
                .strip_prefix("color")
                .and_then(|number| number.parse::<u16>().ok())
                .filter(|number| *number <= 255)
            {
                format!("{extended};5;{index}")
            } else if let Some(rgb) = color.strip_prefix('#').filter(|rgb| rgb.len() == 6) {
                let channels: Vec<String> = rgb
                    .as_bytes()
                    .chunks(2)
                    .map(|pair| String::from_utf8_lossy(pair).into_owned())
                    .collect();
                format!(
                    "{extended};2;{};{};{}",
                    channels[0], channels[1], channels[2]
                )
            } else {
                default.to_owned()
            }
        }
    }
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
pub fn trimmed_description(description: &str) -> String {
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

#[cfg(test)]
mod detect_tests {
    use super::*;

    #[test]
    fn process_chain_parsing() {
        // the helpers read /proc for the current process
        let pid = std::process::id();
        assert!(!process_name(pid).is_empty(), "comm should be readable");
        let parent = process_parent(pid).expect("test binary has a parent");
        assert!(!process_name(parent).is_empty(), "parent comm readable");
    }

    #[test]
    fn detect_matches_first_shell_ancestor() {
        // whatever the chain holds, detection returns a known shell
        if let Some(shell) = Shell::detect() {
            assert!(!shell.name().is_empty());
        }
    }
}
