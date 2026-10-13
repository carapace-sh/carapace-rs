//! YAML spec support for carapace (port of Go `pkg/spec`).
//!
//! Parses user specs into the normalized [`carapace_rs::model::Command`]
//! tree, resolving `$`-prefixed macros into completion actions.

mod flagkey;
mod macro_registry;
mod macro_resolve;
mod marshal;

pub use macro_registry::{
    Macro, MacroFn, MacroMap, add_macro, invoke_macro, macro_description, macro_names,
};
pub use marshal::to_yaml;

use carapace_rs::model::{Command, Flag, Positional};
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
    convert(spec, &[])
}

fn convert(spec: Spec, inherited: &[Flag]) -> Result<Command, String> {
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

    for (key, flag_spec) in &spec.flags {
        model.flags.push(flagkey::parse(key, flag_spec)?);
    }
    for (key, flag_spec) in &spec.persistent_flags {
        let mut flag = flagkey::parse(key, flag_spec)?;
        flag.persistent = true;
        model.flags.push(flag);
    }
    // inherited persistent flags materialize on every subcommand (cobra's
    // shared flag object lookup); the `persistent` marker keeps the dump
    // able to recover the original section
    for flag in inherited {
        model.flags.push(flag.clone());
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

    // collect after the completion mapping so the copies carry it down
    let mut persistent: Vec<Flag> = inherited.to_vec();
    for flag in &model.flags {
        if flag.persistent
            && !inherited
                .iter()
                .any(|inherited| inherited.name == flag.name)
        {
            persistent.push(flag.clone());
        }
    }

    for subcommand in spec.commands {
        model.subcommands.push(convert(subcommand, &persistent)?);
    }

    Ok(model)
}

/// Build a completion action from a list of macro/value entries.
///
/// The entries are envsubst'ed at invocation time with `C_ARG<n>`,
/// `C_VALUE` and `C_FLAG_<NAME>` exported onto the context (Go
/// `action.Parse`); a substitution error degrades the entry to a
/// message action. Plain entries accumulate into values; macros merge
/// in via batch. An entry may chain ` ||| ` modifiers; a modifier as
/// the *first* part of an entry applies to the whole batch (Go
/// `action.Parse`).
pub(crate) fn values_action(entries: &[String]) -> Action {
    let entries: Vec<String> = entries.to_vec();
    Action::callback(move |c| {
        // Go's `action.Parse` sets the exports on the invocation context
        // so macros and nested actions see them too
        let mut context = c.clone();
        let args = context.args.clone();
        for (index, arg) in args.iter().enumerate() {
            context.env.insert(format!("C_ARG{index}"), arg.clone());
        }
        let value = context.value.clone();
        context.env.insert("C_VALUE".to_owned(), value);
        let parsed_flags = context.parsed_flags.clone();
        for (flag, values) in parsed_flags {
            context
                .env
                .insert(format!("C_FLAG_{}", flag.to_uppercase()), values.join(","));
        }

        let substituted: Vec<Result<String, String>> = entries
            .iter()
            .map(|entry| {
                context
                    .envsubst(entry)
                    .map_err(|error| format!("{error}: {entry:?}"))
            })
            .collect();
        build_values_action(&substituted)
            .invoke(&context)
            .to_action()
    })
}

/// Assemble the substituted entries into the batch action (Go
/// `action.Parse`'s ` ||| ` split and batch composition).
fn build_values_action(entries: &[Result<String, String>]) -> Action {
    let mut base: Option<Action> = None;
    let mut values: Vec<RawValue> = Vec::new();
    let mut batch_modifiers: Vec<Box<dyn Fn(Action) -> Action + Send + Sync>> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(message) => {
                base = Some(match base {
                    Some(existing) => {
                        let merged = carapace_rs::batch(vec![existing, Action::message(message)]);
                        Action::callback(move |c| merged.invoke(c).to_action())
                    }
                    None => Action::message(message),
                });
                continue;
            }
        };
        let mut parts = entry.split(" ||| ");
        let first = parts.next().unwrap_or_default();
        if first.starts_with('$') && macro_resolve::is_batch_modifier(first) {
            if let Some(modifier) = macro_resolve::modifier(first) {
                batch_modifiers.push(modifier);
            }
            for part in parts {
                if let Some(modifier) = macro_resolve::modifier(part) {
                    batch_modifiers.push(modifier);
                }
            }
            continue;
        }
        let parts: Vec<&str> = parts.collect();
        if !first.starts_with('$') && parts.is_empty() {
            // plain entries without modifiers accumulate into one values action
            values.push(macro_resolve::parse_value(first));
            continue;
        }
        let mut action = if first.starts_with('$') {
            macro_resolve::resolve(first)
        } else {
            Action::raw(vec![macro_resolve::parse_value(first)])
        };
        for part in parts {
            action = match macro_resolve::modifier(part) {
                Some(modifier) => modifier(action),
                None => Action::message(format!("unknown macro: {part}")),
            };
        }
        base = Some(match base {
            Some(existing) => {
                let merged = carapace_rs::batch(vec![existing, action]);
                Action::callback(move |c| merged.invoke(c).to_action())
            }
            None => action,
        });
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
    for modifier in batch_modifiers {
        action = modifier(action);
    }
    action
}
