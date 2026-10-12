//! Minimal style string parsing shared by JSON-based shell formatters.

/// Parsed representation of a carapace style string.
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
    /// Parse a space-separated style string (e.g. `blue underlined`).
    pub fn parse(input: &str) -> Self {
        let mut style = Self::default();
        for token in input.split_whitespace() {
            let token = token.trim();
            if let Some(color) = token.strip_prefix("fg-") {
                style.foreground = Some(color.to_owned());
            } else if let Some(color) = token.strip_prefix("bg-") {
                style.background = Some(color.to_owned());
            } else {
                match token {
                    "bold" => style.bold = true,
                    "dim" => style.dim = true,
                    "italic" => style.italic = true,
                    "underlined" => style.underlined = true,
                    "blink" => style.blink = true,
                    "inverse" => style.inverse = true,
                    "default" => {}
                    other => {
                        style.foreground = Some(other.to_owned());
                    }
                }
            }
        }
        style
    }
}

/// Parse a numeric SGR sequence (e.g. `01;34` from LS_COLORS) into a
/// [`Style`] (Go `ui.StyleFromSGR`).
pub fn style_from_sgr(s: &str) -> Style {
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

fn ansi_color(code: u8) -> String {
    [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ][code as usize]
        .to_owned()
}
