//! Centralized `CARAPACE_*` environment configuration.

/// carapace style names resolved from `styles.json` (defaults when absent).
#[derive(Clone, Debug)]
pub struct CarapaceStyles {
    pub value: String,
    pub description: String,
    pub error: String,
    pub usage: String,
    pub keyword_ambiguous: String,
    pub keyword_negative: String,
    pub keyword_positive: String,
    pub keyword_unknown: String,
    pub flag_arg: String,
    pub flag_multi_arg: String,
    pub flag_no_arg: String,
    pub flag_opt_arg: String,
    /// Highlight styles 1-12 (command-group styling and general
    /// consumer use).
    pub highlight: [String; 12],
    pub log_level_trace: String,
    pub log_level_debug: String,
    pub log_level_info: String,
    pub log_level_warning: String,
    pub log_level_error: String,
    pub log_level_critical: String,
    pub log_level_fatal: String,
}

impl Default for CarapaceStyles {
    fn default() -> Self {
        Self {
            value: String::new(),
            description: "dim".to_owned(),
            error: "bold red".to_owned(),
            usage: "dim".to_owned(),
            keyword_ambiguous: "yellow".to_owned(),
            keyword_negative: "red".to_owned(),
            keyword_positive: "green".to_owned(),
            keyword_unknown: "dim white".to_owned(),
            flag_arg: "blue".to_owned(),
            flag_multi_arg: "magenta".to_owned(),
            flag_no_arg: String::new(),
            flag_opt_arg: "yellow".to_owned(),
            highlight: [
                "blue".to_owned(),
                "yellow".to_owned(),
                "magenta".to_owned(),
                "cyan".to_owned(),
                "green".to_owned(),
                "dim blue".to_owned(),
                "dim yellow".to_owned(),
                "dim magenta".to_owned(),
                "dim cyan".to_owned(),
                "dim green".to_owned(),
                "bold".to_owned(),
                "dim bold".to_owned(),
            ],
            log_level_trace: "blue".to_owned(),
            log_level_debug: "dim white".to_owned(),
            log_level_info: "green".to_owned(),
            log_level_warning: "yellow".to_owned(),
            log_level_error: "magenta".to_owned(),
            log_level_critical: "red".to_owned(),
            log_level_fatal: "cyan".to_owned(),
        }
    }
}

impl CarapaceStyles {
    /// The style for a highlight level 0..11 (Go `carapace.Highlight`).
    pub fn highlight(&self, level: usize) -> String {
        self.highlight.get(level).cloned().unwrap_or_default()
    }
}

/// User style configuration: the carapace styles plus custom named styles.
#[derive(Clone, Debug, Default)]
pub struct Styles {
    pub carapace: CarapaceStyles,
    /// Custom style names (e.g. `keyword` -> `blue bold`).
    pub custom: std::collections::HashMap<String, String>,
}

/// A field of a registered style configuration (Go `config.Field`).
#[derive(Clone, Debug)]
pub struct StyleField {
    pub name: String,
    pub description: String,
    pub style: String,
    pub tag: String,
}

impl Styles {
    /// The registered style configuration names (Go
    /// `config.Styles.Keys()`); user-defined custom styles are not
    /// registered configurations.
    pub fn keys(&self) -> Vec<String> {
        vec!["carapace".to_owned()]
    }

    /// The fields of a registered style configuration (Go
    /// `config.Styles.Fields(name)`): the field name, its description and
    /// tag, and the currently configured style. `None` for unknown names.
    pub fn fields(&self, name: &str) -> Option<Vec<StyleField>> {
        if name != "carapace" {
            return None;
        }
        let carapace = &self.carapace;
        let field = |name: &str, description: &str, tag: &str, style: &str| StyleField {
            name: name.to_owned(),
            description: description.to_owned(),
            style: style.to_owned(),
            tag: tag.to_owned(),
        };
        let mut fields = vec![
            field(
                "Value",
                "default style for values",
                "core styles",
                &carapace.value,
            ),
            field(
                "Description",
                "default style for descriptions",
                "core styles",
                &carapace.description,
            ),
            field(
                "Error",
                "default style for errors",
                "core styles",
                &carapace.error,
            ),
            field(
                "Usage",
                "default style for usage",
                "core styles",
                &carapace.usage,
            ),
            field(
                "KeywordAmbiguous",
                "keyword describing a ambiguous state",
                "keyword styles",
                &carapace.keyword_ambiguous,
            ),
            field(
                "KeywordNegative",
                "keyword describing a negative state",
                "keyword styles",
                &carapace.keyword_negative,
            ),
            field(
                "KeywordPositive",
                "keyword describing a positive state",
                "keyword styles",
                &carapace.keyword_positive,
            ),
            field(
                "KeywordUnknown",
                "keyword describing an unknown state",
                "keyword styles",
                &carapace.keyword_unknown,
            ),
        ];
        for (name, style) in [
            ("Trace", &carapace.log_level_trace),
            ("Debug", &carapace.log_level_debug),
            ("Info", &carapace.log_level_info),
            ("Warning", &carapace.log_level_warning),
            ("Error", &carapace.log_level_error),
            ("Critical", &carapace.log_level_critical),
            ("Fatal", &carapace.log_level_fatal),
        ] {
            fields.push(field(
                &format!("LogLevel{name}"),
                &format!("LogLevel {name}"),
                "loglevel styles",
                style,
            ));
        }
        for (level, style) in carapace.highlight.iter().enumerate() {
            fields.push(field(
                &format!("Highlight{}", level + 1),
                &format!("Highlight {}", level + 1),
                "highlight styles",
                style,
            ));
        }
        fields.push(field(
            "FlagArg",
            "flag with argument",
            "flag styles",
            &carapace.flag_arg,
        ));
        fields.push(field(
            "FlagMultiArg",
            "flag with multiple arguments",
            "flag styles",
            &carapace.flag_multi_arg,
        ));
        fields.push(field(
            "FlagNoArg",
            "flag without argument",
            "flag styles",
            &carapace.flag_no_arg,
        ));
        fields.push(field(
            "FlagOptArg",
            "flag with optional argument",
            "flag styles",
            &carapace.flag_opt_arg,
        ));
        Some(fields)
    }
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
            "carapace.FlagArg" => styles.carapace.flag_arg = style,
            "carapace.FlagMultiArg" => styles.carapace.flag_multi_arg = style,
            "carapace.FlagNoArg" => styles.carapace.flag_no_arg = style,
            "carapace.FlagOptArg" => styles.carapace.flag_opt_arg = style,
            "carapace.KeywordAmbiguous" => styles.carapace.keyword_ambiguous = style,
            "carapace.KeywordNegative" => styles.carapace.keyword_negative = style,
            "carapace.KeywordPositive" => styles.carapace.keyword_positive = style,
            "carapace.KeywordUnknown" => styles.carapace.keyword_unknown = style,
            "carapace.LogLevelTrace" => styles.carapace.log_level_trace = style,
            "carapace.LogLevelDebug" => styles.carapace.log_level_debug = style,
            "carapace.LogLevelInfo" => styles.carapace.log_level_info = style,
            "carapace.LogLevelWarning" => styles.carapace.log_level_warning = style,
            "carapace.LogLevelError" => styles.carapace.log_level_error = style,
            "carapace.LogLevelCritical" => styles.carapace.log_level_critical = style,
            "carapace.LogLevelFatal" => styles.carapace.log_level_fatal = style,
            other if other.starts_with("carapace.Highlight") => {
                if let Some(level) = other.strip_prefix("carapace.Highlight") {
                    if let Ok(index) = level.parse::<usize>() {
                        if (1..=12).contains(&index) {
                            styles.carapace.highlight[index - 1] = style;
                        }
                    }
                }
            }
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

/// Completion flags carried by the [`Context`](crate::Context)
/// (Go reads the same `CARAPACE_*` variables from process environment
/// functions; here they travel with the completion so embedded and
/// sandboxed completions can scope them).
#[derive(Clone, Debug, PartialEq)]
pub struct Flags {
    /// `CARAPACE_HIDDEN`: hidden command/flag visibility.
    pub hidden: Hidden,
    /// `CARAPACE_LENIENT`: unknown flags are ignored instead of erroring.
    pub lenient: bool,
    /// `CARAPACE_MERGEFLAGS`: `None` when unset (zsh merges implicitly).
    pub merge_flags: Option<bool>,
    /// `CARAPACE_UNFILTERED`: candidates are not filtered by prefix.
    pub unfiltered: bool,
    /// `CARAPACE_NOSPACE`: additional nospace characters.
    pub nospace: Option<String>,
    /// `CARAPACE_COLOR`/`NO_COLOR`/`CLICOLOR`: styling disabled.
    pub color_disabled: bool,
    /// `CARAPACE_TOOLTIP` (powershell tooltips, default enabled).
    pub tooltip: bool,
    /// `CARAPACE_DESCRIPTION_LENGTH` (default 80).
    pub description_length: usize,
    /// `CARAPACE_MATCH`: prefix matching is case-insensitive.
    pub case_insensitive: bool,
    /// `CARAPACE_EXPERIMENTAL`: experimental features (tabdance uids).
    pub experimental: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            hidden: Hidden::None,
            lenient: false,
            merge_flags: None,
            unfiltered: false,
            nospace: None,
            color_disabled: false,
            tooltip: true,
            description_length: 80,
            case_insensitive: false,
            experimental: false,
        }
    }
}

impl Flags {
    /// Parse the flags from an environment map.
    pub fn from_env_map(env: &std::collections::HashMap<String, String>) -> Self {
        let var = |name: &str| env.get(name).map(String::as_str);
        let flag = |name: &str| var(name).is_some_and(|value| !value.is_empty() && value != "0");
        Self {
            hidden: match var("CARAPACE_HIDDEN").and_then(|v| v.parse::<u8>().ok()) {
                Some(1) => Hidden::ExcludeCarapace,
                Some(2) => Hidden::IncludeCarapace,
                _ => Hidden::None,
            },
            lenient: flag("CARAPACE_LENIENT"),
            merge_flags: var("CARAPACE_MERGEFLAGS").map(|v| !v.is_empty() && v != "0"),
            unfiltered: flag("CARAPACE_UNFILTERED"),
            nospace: var("CARAPACE_NOSPACE")
                .filter(|value| !value.is_empty())
                .map(String::from),
            color_disabled: match var("CARAPACE_COLOR") {
                Some(value) => value == "0",
                None => flag("NO_COLOR") || var("CLICOLOR").is_some_and(|v| v == "0"),
            },
            tooltip: !var("CARAPACE_TOOLTIP").is_some_and(|v| v.is_empty() || v == "0"),
            description_length: var("CARAPACE_DESCRIPTION_LENGTH")
                .and_then(|value| value.parse().ok())
                .unwrap_or(80),
            case_insensitive: matches!(var("CARAPACE_MATCH"), Some("CASE_INSENSITIVE") | Some("1")),
            experimental: flag("CARAPACE_EXPERIMENTAL"),
        }
    }

    /// Parse the flags from the process environment.
    pub fn from_env() -> Self {
        let env: std::collections::HashMap<String, String> = std::env::vars().collect();
        Self::from_env_map(&env)
    }
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

/// Read a `CARAPACE_*` env var as a flag (set and not `0`).
pub fn flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.is_empty() && value != "0")
}

pub fn log_enabled() -> bool {
    flag("CARAPACE_LOG")
}

pub fn experimental() -> bool {
    flag("CARAPACE_EXPERIMENTAL")
}

/// Case-aware prefix test (port of `pkg/match.HasPrefix`).
pub fn has_prefix(value: &str, prefix: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        value.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        value.starts_with(prefix)
    }
}

/// Case-aware prefix strip (port of `pkg/match.TrimPrefix`).
pub fn trim_prefix<'a>(value: &'a str, prefix: &str, case_insensitive: bool) -> &'a str {
    if has_prefix(value, prefix, case_insensitive) {
        &value[prefix.len()..]
    } else {
        value
    }
}

/// Look up a custom style by name from the style configuration.
pub fn custom_style(name: &str) -> Option<String> {
    styles().custom.get(name).cloned()
}
