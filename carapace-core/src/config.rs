//! Centralized `CARAPACE_*` environment configuration.

/// carapace style names resolved from `styles.json` (defaults when absent).
#[derive(Clone, Debug)]
pub struct CarapaceStyles {
    pub value: String,
    pub description: String,
    pub error: String,
    pub usage: String,
}

impl Default for CarapaceStyles {
    fn default() -> Self {
        Self {
            value: String::new(),
            description: String::new(),
            error: "underlined".to_owned(),
            usage: "italic".to_owned(),
        }
    }
}

/// User style configuration: the carapace styles plus custom named styles.
#[derive(Clone, Debug, Default)]
pub struct Styles {
    pub carapace: CarapaceStyles,
    /// Custom style names (e.g. `keyword` -> `blue bold`).
    pub custom: std::collections::HashMap<String, String>,
}

static STYLES: std::sync::RwLock<Option<Styles>> = std::sync::RwLock::new(None);

/// Resolve the style configuration, loading `styles.json` on first use.
///
/// `$XDG_CONFIG_HOME/carapace/styles.json` (fallback `~/.config`), mapping
/// style names to style strings. `carapace.Value`, `carapace.Description`,
/// `carapace.Error` and `carapace.Usage` configure the built-in styles;
/// other names register custom styles.
pub fn styles() -> Styles {
    let mut guard = STYLES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        *guard = Some(load_styles());
    }
    guard.as_ref().unwrap().clone()
}

/// Override the style configuration (mainly for tests).
pub fn set_styles(styles: Styles) {
    let mut guard = STYLES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(styles);
}

/// Clear the cached style configuration so the next [`styles`] call reloads.
pub fn reset_styles() {
    let mut guard = STYLES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

fn load_styles() -> Styles {
    let Some(path) = config_file("styles.json") else {
        return Styles::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Styles::default();
    };
    let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, String>>(&content)
    else {
        return Styles::default();
    };
    let mut styles = Styles::default();
    let mut carapace_fields = std::collections::HashMap::new();
    for (name, style) in parsed {
        match name.as_str() {
            "carapace.Value" => styles.carapace.value = style,
            "carapace.Description" => styles.carapace.description = style,
            "carapace.Error" => styles.carapace.error = style,
            "carapace.Usage" => styles.carapace.usage = style,
            other => {
                carapace_fields.insert(other.to_owned(), style);
            }
        }
    }
    styles.custom = carapace_fields;
    styles
}

fn config_dir() -> Option<std::path::PathBuf> {
    // honors XDG_CONFIG_HOME on unix; platform directory on Windows/macOS
    dirs::config_dir().map(|dir| dir.join("carapace"))
}

fn config_file(name: &str) -> Option<std::path::PathBuf> {
    let path = config_dir()?.join(name);
    path.exists().then_some(path)
}

/// Hidden-flag mode from `CARAPACE_HIDDEN`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Hidden {
    /// Show normally hidden commands/flags as configured.
    #[default]
    None,
    /// Exclude carapace-internal commands.
    ExcludeCarapace,
    /// Include everything, even hidden.
    IncludeCarapace,
}

pub fn hidden() -> Hidden {
    match std::env::var("CARAPACE_HIDDEN")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
    {
        Some(1) => Hidden::ExcludeCarapace,
        Some(2) => Hidden::IncludeCarapace,
        _ => Hidden::None,
    }
}

/// Read a `CARAPACE_*` env var as a flag (set and not `0`).
pub fn flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.is_empty() && value != "0")
}

pub fn log_enabled() -> bool {
    flag("CARAPACE_LOG")
}

pub fn lenient() -> bool {
    flag("CARAPACE_LENIENT")
}

pub fn unfiltered() -> bool {
    flag("CARAPACE_UNFILTERED")
}

pub fn merge_flags() -> bool {
    flag("CARAPACE_MERGEFLAGS")
}

pub fn experimental() -> bool {
    flag("CARAPACE_EXPERIMENTAL")
}

pub fn nospace() -> Option<String> {
    std::env::var("CARAPACE_NOSPACE")
        .ok()
        .filter(|value| !value.is_empty())
}

/// Color disable logic: explicit `CARAPACE_COLOR` wins, then `NO_COLOR`,
/// then `CLICOLOR`.
pub fn color_disabled() -> bool {
    match std::env::var("CARAPACE_COLOR") {
        Ok(value) => value == "0",
        Err(_) => flag("NO_COLOR") || std::env::var("CLICOLOR").is_ok_and(|v| v == "0"),
    }
}

/// Whether prefix matching is case-insensitive (`CARAPACE_MATCH`).
pub fn case_insensitive() -> bool {
    matches!(
        std::env::var("CARAPACE_MATCH").as_deref(),
        Ok("CASE_INSENSITIVE") | Ok("1")
    )
}

/// Case-aware prefix test (port of `pkg/match.HasPrefix`).
pub fn has_prefix(value: &str, prefix: &str) -> bool {
    if case_insensitive() {
        value.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        value.starts_with(prefix)
    }
}

/// Case-aware prefix strip (port of `pkg/match.TrimPrefix`).
pub fn trim_prefix<'a>(value: &'a str, prefix: &str) -> &'a str {
    if has_prefix(value, prefix) {
        &value[prefix.len()..]
    } else {
        value
    }
}

/// Look up a custom style by name from the style configuration.
pub fn custom_style(name: &str) -> Option<String> {
    styles().custom.get(name).cloned()
}

/// Description display length (default 80).
pub fn description_length() -> usize {
    std::env::var("CARAPACE_DESCRIPTION_LENGTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80)
}
