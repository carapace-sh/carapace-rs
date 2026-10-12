//! YAML spec support for carapace (port of Go `pkg/spec`).
//!
//! Parses user specs into the normalized [`carapace_rs::model::Command`]
//! tree, resolving `$`-prefixed macros into completion actions.

mod flagkey;
mod macro_resolve;
mod marshal;

pub use marshal::to_yaml;

use carapace_rs::model::{Command, Positional};
use carapace_rs::{Action, RawValue};
use serde::Deserialize;

/// Parsing mode of a command (spec `parsing` field).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Parsing {
    #[default]
    Interspersed,
    NonInterspersed,
    Disabled,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    parsing: Option<String>,
    #[serde(default)]
    flags: std::collections::BTreeMap<String, FlagSpec>,
    #[serde(default, rename = "persistentflags")]
    persistent_flags: std::collections::BTreeMap<String, FlagSpec>,
    #[serde(default)]
    commands: Vec<Spec>,
    #[serde(default)]
    completion: CompletionSpec,
}

/// A flag definition: plain description string or an object with extras.
#[derive(Clone, Debug, Default)]
struct FlagSpec {
    description: String,
    nargs: Option<i64>,
    #[allow(dead_code)] // parsed for completeness; defaults don't affect completion
    default: Option<String>,
}

impl<'de> Deserialize<'de> for FlagSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        match value {
            serde_yaml::Value::String(description) => Ok(Self {
                description,
                ..Default::default()
            }),
            serde_yaml::Value::Mapping(_) => {
                #[derive(Deserialize)]
                struct Fields {
                    #[serde(default)]
                    description: String,
                    #[serde(default)]
                    nargs: Option<i64>,
                    #[serde(default)]
                    default: Option<String>,
                }
                let fields = Fields::deserialize(value).map_err(serde::de::Error::custom)?;
                Ok(Self {
                    description: fields.description,
                    nargs: fields.nargs,
                    default: fields.default,
                })
            }
            other => Err(serde::de::Error::custom(format!(
                "invalid flag definition: {other:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CompletionSpec {
    #[serde(default)]
    flag: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    positional: Vec<Vec<String>>,
    #[serde(default)]
    positionalany: Option<Vec<String>>,
    #[serde(default)]
    dash: Option<Vec<Vec<String>>>,
    #[serde(default)]
    dashany: Option<Vec<String>>,
}

/// Parse a YAML spec into a command tree.
pub fn from_yaml(yaml: &str) -> Result<Command, String> {
    let spec: Spec = serde_yaml::from_str(yaml).map_err(|error| error.to_string())?;
    Ok(convert(spec))
}

fn convert(spec: Spec) -> Command {
    // name may embed a usage string after the first space
    let name = spec.name.split(' ').next().unwrap_or_default().to_owned();

    let parsing = match spec.parsing.as_deref() {
        Some("non-interspersed") => Parsing::NonInterspersed,
        Some("disabled") => Parsing::Disabled,
        _ => Parsing::Interspersed,
    };

    let mut model = Command::new(&name)
        .description(spec.description.clone())
        .flag_prefix('-');
    model.aliases = spec.aliases.clone();
    model.group = spec.group;
    model.hidden = spec.hidden;
    model.interspersed = parsing == Parsing::Interspersed;
    model.disable_flag_parsing = parsing == Parsing::Disabled;

    for (key, flag_spec) in spec.flags.iter().chain(&spec.persistent_flags) {
        if let Some(flag) = flagkey::parse(key, flag_spec) {
            model.flags.push(flag);
        }
    }

    for (index, positional) in spec.completion.positional.iter().enumerate() {
        while model.positionals.len() <= index {
            model.positionals.push(Positional::default());
        }
        model.positionals[index].completion = Some(values_action(positional));
    }
    if let Some(any) = &spec.completion.positionalany {
        model.positional_any = Some(values_action(any));
    }
    if let Some(dash) = &spec.completion.dash {
        for (index, positional) in dash.iter().enumerate() {
            while model.dash_positionals.len() <= index {
                model.dash_positionals.push(Positional::default());
            }
            model.dash_positionals[index].completion = Some(values_action(positional));
        }
    }
    if let Some(dash_any) = &spec.completion.dashany {
        model.dash_any = Some(values_action(dash_any));
    }

    for (flag_name, values) in &spec.completion.flag {
        if let Some(flag) = model.flags.iter_mut().find(|flag| {
            &flag.name == flag_name
                || flag.shorthand.map(|c| c.to_string()).as_deref() == Some(flag_name.as_str())
        }) {
            flag.completion = Some(values_action(values));
        }
    }

    for subcommand in spec.commands {
        model.subcommands.push(convert(subcommand));
    }

    model
}

/// Build a completion action from a list of macro/value entries.
///
/// Plain entries accumulate into values; the first macro provides the base
/// action (further macros merge in); `$chdir`-style modifiers apply to all.
fn values_action(entries: &[String]) -> Action {
    let mut base: Option<Action> = None;
    let mut values: Vec<RawValue> = Vec::new();
    let mut modifiers: Vec<Box<dyn Fn(Action) -> Action + Send + Sync>> = Vec::new();

    for entry in entries {
        if entry.starts_with('$') {
            if let Some(modifier) = macro_resolve::modifier(entry) {
                modifiers.push(modifier);
            } else {
                let action = macro_resolve::resolve(entry);
                base = Some(match base {
                    Some(existing) => {
                        let merged = carapace_rs::batch(vec![existing, action]);
                        Action::callback(move |c| merged.invoke(c).to_action())
                    }
                    None => action,
                });
            }
        } else {
            values.push(macro_resolve::parse_value(entry));
        }
    }

    let mut action = match base {
        Some(base) => {
            let values_action = Action::raw(values);
            let merged = carapace_rs::batch(vec![base, values_action]);
            Action::callback(move |c| merged.invoke(c).to_action())
        }
        None if values.is_empty() => Action::default(),
        None => Action::raw(values),
    };
    for modifier in modifiers {
        action = modifier(action);
    }
    action
}
