//! Display coloring (port of Go `pkg/style`).
//!
//! Styles are space-separated strings of kebab-case tokens, e.g.
//! `bold blue`, `bg-red`, `color123`, `#ff8800`. [`Style`] is the parsed
//! representation and can convert back to an SGR sequence.

use crate::Context;

pub const DEFAULT: &str = "";

pub const BLACK: &str = "black";
pub const RED: &str = "red";
pub const GREEN: &str = "green";
pub const YELLOW: &str = "yellow";
pub const BLUE: &str = "blue";
pub const MAGENTA: &str = "magenta";
pub const CYAN: &str = "cyan";
pub const WHITE: &str = "white";

pub const BRIGHT_BLACK: &str = "bright-black";
pub const BRIGHT_RED: &str = "bright-red";
pub const BRIGHT_GREEN: &str = "bright-green";
pub const BRIGHT_YELLOW: &str = "bright-yellow";
pub const BRIGHT_BLUE: &str = "bright-blue";
pub const BRIGHT_MAGENTA: &str = "bright-magenta";
pub const BRIGHT_CYAN: &str = "bright-cyan";
pub const BRIGHT_WHITE: &str = "bright-white";

pub const BG_BLACK: &str = "bg-black";
pub const BG_RED: &str = "bg-red";
pub const BG_GREEN: &str = "bg-green";
pub const BG_YELLOW: &str = "bg-yellow";
pub const BG_BLUE: &str = "bg-blue";
pub const BG_MAGENTA: &str = "bg-magenta";
pub const BG_CYAN: &str = "bg-cyan";
pub const BG_WHITE: &str = "bg-white";

pub const BG_BRIGHT_BLACK: &str = "bg-bright-black";
pub const BG_BRIGHT_RED: &str = "bg-bright-red";
pub const BG_BRIGHT_GREEN: &str = "bg-bright-green";
pub const BG_BRIGHT_YELLOW: &str = "bg-bright-yellow";
pub const BG_BRIGHT_BLUE: &str = "bg-bright-blue";
pub const BG_BRIGHT_MAGENTA: &str = "bg-bright-magenta";
pub const BG_BRIGHT_CYAN: &str = "bg-bright-cyan";
pub const BG_BRIGHT_WHITE: &str = "bg-bright-white";

pub const BOLD: &str = "bold";
pub const DIM: &str = "dim";
pub const ITALIC: &str = "italic";
pub const UNDERLINED: &str = "underlined";
pub const BLINK: &str = "blink";
pub const INVERSE: &str = "inverse";

/// Combine different styles (Go `style.Of(styles ...Style)`).
///
/// The item type is concrete, so `of([])` needs no annotation.
pub fn of<'a>(styles: impl IntoIterator<Item = &'a str>) -> String {
    styles
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

/// Return a color from the xterm 256-color palette (Go `style.XTerm256Color`).
pub fn xterm_256_color(index: u8) -> String {
    format!("color{index}")
}

/// Return a 24-bit true color (Go `style.TrueColor`).
pub fn true_color(red: u8, green: u8, blue: u8) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// Return the SGR sequence for the given style (Go `style.SGR`).
pub fn sgr(style: &str) -> String {
    parse(style).sgr()
}

/// Parse a style string, skipping unparsable words (Go `style.Parse`).
pub fn parse(input: &str) -> Style {
    let mut style = Style::default();
    for word in input.split(' ') {
        style.apply_word(word);
    }
    style
}

/// Whether every word of the style string parses (Go `ui.ParseStyling`
/// returning nil when any word is invalid, including the empty string).
pub fn is_parsable(input: &str) -> bool {
    !input.is_empty()
        && input
            .split(' ')
            .all(|word| Style::default().apply_word(word))
}

/// Parsed representation of a style string (Go `ui.Style`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underlined: bool,
    pub blink: bool,
    pub inverse: bool,
}

impl Style {
    /// Parse a space-separated style string (e.g. `blue underlined`),
    /// skipping unparsable words (Go `style.Parse`).
    pub fn parse(input: &str) -> Self {
        parse(input)
    }

    /// Parse a numeric SGR sequence (e.g. `01;34` from LS_COLORS) into a
    /// [`Style`] (Go `ui.StyleFromSGR`).
    pub fn from_sgr(s: &str) -> Self {
        let codes: Vec<Option<u8>> = s.split(';').map(|code| code.parse::<u8>().ok()).collect();
        let mut style = Style::default();
        let mut index = 0;
        while index < codes.len() {
            let Some(code) = codes[index] else {
                index += 1;
                continue;
            };
            match code {
                1 => style.bold = true,
                2 => style.dim = true,
                3 => style.italic = true,
                4 => style.underlined = true,
                5 => style.blink = true,
                7 => style.inverse = true,
                30..=37 => style.foreground = Some(ansi_color(code - 30)),
                39 => style.foreground = None,
                40..=47 => style.background = Some(ansi_color(code - 40)),
                49 => style.background = None,
                90..=97 => style.foreground = Some(format!("bright-{}", ansi_color(code - 90))),
                100..=107 => style.background = Some(format!("bright-{}", ansi_color(code - 100))),
                38 if index + 3 < codes.len() + 1 => {
                    // extended foreground: 38;5;n (256-color) or 38;2;r;g;b
                    let params: Vec<u8> = codes[index + 1..].iter().filter_map(|c| *c).collect();
                    match params.first() {
                        Some(5) if params.len() >= 2 => {
                            style.foreground = Some(format!("color{}", params[1]));
                            index += 2;
                        }
                        Some(2) if params.len() >= 4 => {
                            style.foreground = Some(format!(
                                "#{:02x}{:02x}{:02x}",
                                params[1], params[2], params[3]
                            ));
                            index += 4;
                        }
                        _ => {}
                    }
                }
                48 if index + 3 < codes.len() + 1 => {
                    let params: Vec<u8> = codes[index + 1..].iter().filter_map(|c| *c).collect();
                    match params.first() {
                        Some(5) if params.len() >= 2 => {
                            style.background = Some(format!("color{}", params[1]));
                            index += 2;
                        }
                        Some(2) if params.len() >= 4 => {
                            style.background = Some(format!(
                                "#{:02x}{:02x}{:02x}",
                                params[1], params[2], params[3]
                            ));
                            index += 4;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            index += 1;
        }
        style
    }

    /// The SGR sequence for this style (Go `ui.Style.SGR`).
    pub fn sgr(&self) -> String {
        let mut codes: Vec<String> = Vec::new();
        for (set, code) in [
            (self.bold, "1"),
            (self.dim, "2"),
            (self.italic, "3"),
            (self.underlined, "4"),
            (self.blink, "5"),
            (self.inverse, "7"),
        ] {
            if set {
                codes.push(code.to_owned());
            }
        }
        if let Some(foreground) = &self.foreground {
            codes.push(color_sgr(foreground, false));
        }
        if let Some(background) = &self.background {
            codes.push(color_sgr(background, true));
        }
        codes.join(";")
    }

    /// Apply a single styling word, returning whether it was recognized
    /// (Go `ui.ParseStyling` for one word).
    fn apply_word(&mut self, word: &str) -> bool {
        if word == "default" || word == "fg-default" {
            self.foreground = None;
            return true;
        }
        if word == "bg-default" {
            self.background = None;
            return true;
        }
        if let Some(color) = word.strip_prefix("fg-") {
            if let Some(color) = parse_color(color) {
                self.foreground = Some(color);
                return true;
            }
            return false;
        }
        if let Some(color) = word.strip_prefix("bg-") {
            if let Some(color) = parse_color(color) {
                self.background = Some(color);
                return true;
            }
            return false;
        }
        if let Some(field) = word.strip_prefix("no-") {
            return self.apply_bool(field, false);
        }
        if let Some(field) = word.strip_prefix("toggle-") {
            return match field {
                "bold" => {
                    self.bold = !self.bold;
                    true
                }
                "dim" => {
                    self.dim = !self.dim;
                    true
                }
                "italic" => {
                    self.italic = !self.italic;
                    true
                }
                "underlined" => {
                    self.underlined = !self.underlined;
                    true
                }
                "blink" => {
                    self.blink = !self.blink;
                    true
                }
                "inverse" => {
                    self.inverse = !self.inverse;
                    true
                }
                _ => false,
            };
        }
        if self.apply_bool(word, true) {
            return true;
        }
        if let Some(color) = parse_color(word) {
            self.foreground = Some(color);
            return true;
        }
        false
    }

    fn apply_bool(&mut self, field: &str, value: bool) -> bool {
        match field {
            "bold" => self.bold = value,
            "dim" => self.dim = value,
            "italic" => self.italic = value,
            "underlined" => self.underlined = value,
            "blink" => self.blink = value,
            "inverse" => self.inverse = value,
            _ => return false,
        }
        true
    }
}

const COLOR_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// Parse a color name (Go `ui.parseColor`): builtin names, `colorN`
/// (0..255) and `#rrggbb`.
fn parse_color(name: &str) -> Option<String> {
    if COLOR_NAMES.contains(&name) {
        return Some(name.to_owned());
    }
    if let Some(bright) = name.strip_prefix("bright-") {
        if COLOR_NAMES.contains(&bright) {
            return Some(name.to_owned());
        }
        return None;
    }
    if let Some(index) = name.strip_prefix("color") {
        if let Ok(index) = index.parse::<u16>() {
            if index < 256 {
                return Some(format!("color{index}"));
            }
        }
        return None;
    }
    if name.len() == 7 && name.starts_with('#') {
        let (red, green, blue) = (
            u8::from_str_radix(&name[1..3], 16).ok()?,
            u8::from_str_radix(&name[3..5], 16).ok()?,
            u8::from_str_radix(&name[5..7], 16).ok()?,
        );
        return Some(format!("#{red:02x}{green:02x}{blue:02x}"));
    }
    None
}

/// The SGR codes for a color name (Go `ui.Color` SGR methods). Unknown
/// colors fall back to the default code (formatters embed the result in
/// SGR sequences, where an empty segment would corrupt them).
fn color_sgr(color: &str, background: bool) -> String {
    let (default, base, bright_base, extended): (&str, u16, u16, &str) = if background {
        ("49", 40, 100, "48")
    } else {
        ("39", 30, 90, "38")
    };
    if color == "default" {
        return default.to_owned();
    }
    if let Some(index) = COLOR_NAMES.iter().position(|name| *name == color) {
        return (base + index as u16).to_string();
    }
    if let Some(bright) = color.strip_prefix("bright-") {
        if let Some(index) = COLOR_NAMES.iter().position(|name| *name == bright) {
            return (bright_base + index as u16).to_string();
        }
        return default.to_owned();
    }
    if let Some(index) = color
        .strip_prefix("color")
        .and_then(|index| index.parse::<u16>().ok())
        .filter(|index| *index <= 255)
    {
        return format!("{extended};5;{index}");
    }
    if let Some(rgb) = color.strip_prefix('#').filter(|rgb| rgb.len() == 6) {
        return format!(
            "{extended};2;{};{};{}",
            u8::from_str_radix(&rgb[0..2], 16).unwrap_or(0),
            u8::from_str_radix(&rgb[2..4], 16).unwrap_or(0),
            u8::from_str_radix(&rgb[4..6], 16).unwrap_or(0),
        );
    }
    default.to_owned()
}

fn ansi_color(code: u8) -> String {
    COLOR_NAMES[code as usize].to_owned()
}

/// Convert an SGR sequence into a style string (Go `style.fromSGR`).
fn from_sgr(sgr: &str) -> String {
    let style = Style::from_sgr(sgr);
    let mut result: Vec<String> = Vec::new();
    if let Some(foreground) = &style.foreground {
        result.push(foreground.clone());
    }
    if let Some(background) = &style.background {
        result.push(format!("bg-{background}"));
    }
    if style.bold {
        result.push(BOLD.to_owned());
    }
    if style.dim {
        result.push(DIM.to_owned());
    }
    if style.italic {
        result.push(ITALIC.to_owned());
    }
    if style.underlined {
        result.push(UNDERLINED.to_owned());
    }
    if style.blink {
        result.push(BLINK.to_owned());
    }
    if style.inverse {
        result.push(INVERSE.to_owned());
    }
    of(result.iter().map(String::as_str))
}

/// The style for a known keyword (Go `style.ForKeyword`): dash and
/// underscore are ignored and matching is case-insensitive.
pub fn for_keyword(value: &str) -> String {
    let normalized: String = value
        .to_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect();
    let styles = crate::config::styles().carapace;
    let (positive, negative, ambiguous, unknown) = (
        &styles.keyword_positive,
        &styles.keyword_negative,
        &styles.keyword_ambiguous,
        &styles.keyword_unknown,
    );
    match normalized.as_str() {
        "1" | "y" | "yes" | "true" | "on" | "enable" | "enabled" | "all" | "full" | "loose"
        | "public" | "internal" | "asc" | "ascending" | "up" | "open" | "opened" | "always"
        | "start" | "started" | "starting" | "run" | "running" | "create" | "created"
        | "onsuccess" | "success" | "active" | "healthy" | "resolved" | "connected" | "present"
        | "exists" | "completed" | "merged" | "keep" | "continue" | "allow" | "visible"
        | "tracked" => positive.clone(),
        "0" | "n" | "no" | "false" | "off" | "disable" | "disabled" | "none" | "empty"
        | "strict" | "private" | "external" | "desc" | "descending" | "down" | "close"
        | "closed" | "never" | "remove" | "removed" | "removing" | "stop" | "stopped"
        | "stopping" | "exit" | "exited" | "exiting" | "dead" | "delete" | "deleted"
        | "onfailure" | "onerror" | "error" | "failed" | "failure" | "fatal" | "block" | "deny"
        | "hidden" | "dirty" | "inactive" | "unhealthy" | "unresolved" | "disconnected"
        | "absent" | "notexists" | "missing" | "canceled" | "cancelled" => negative.clone(),
        "some" | "auto" | "inprogress" | "pause" | "paused" | "pausing" | "restart"
        | "restarted" | "restarting" | "warn" | "nonblock" | "ondemand" | "pending" | "idle"
        | "ask" | "untracked" | "default" | "quiet" | "manual" | "only" | "silent"
        | "scheduled" | "skipped" => ambiguous.clone(),
        "unknown" | "backoff" => unknown.clone(),
        _ => String::new(),
    }
}

/// The style for a log level name (Go `style.ForLogLevel`).
pub fn for_log_level(level: &str) -> String {
    let styles = crate::config::styles().carapace;
    match level.to_lowercase().as_str() {
        "trace" => styles.log_level_trace,
        "debug" | "vdebug" => styles.log_level_debug,
        "info" => styles.log_level_info,
        "warn" | "warning" => styles.log_level_warning,
        "err" | "error" => styles.log_level_error,
        "crit" | "critical" => styles.log_level_critical,
        "fatal" | "panic" => styles.log_level_fatal,
        _ => String::new(),
    }
}

/// The style for a path from `LS_COLORS` (Go `style.ForPath`).
pub fn for_path(context: &Context, path: &str) -> String {
    let path = context.abs(path).unwrap_or_else(|_| path.to_owned());
    let ls_colors = context.get_env("LS_COLORS");
    let colorist = crate::shell::lscolors::colorist(&ls_colors);
    from_sgr(&colorist.get_style(&path))
}

/// The style for a path by extension only (Go `style.ForPathExt`).
pub fn for_path_ext(context: &Context, path: &str) -> String {
    let ls_colors = context.get_env("LS_COLORS");
    let colorist = crate::shell::lscolors::colorist(&ls_colors);
    from_sgr(&colorist.get_style_ext(path))
}

/// The style for an extension name (Go `style.ForExtension`).
pub fn for_extension(context: &Context, extension: &str) -> String {
    for_path_ext(context, &format!(".{extension}"))
}
