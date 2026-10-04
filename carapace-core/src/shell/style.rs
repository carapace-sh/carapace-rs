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
