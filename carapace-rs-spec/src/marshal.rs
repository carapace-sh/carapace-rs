//! Serialization of a command tree into the YAML spec format
//! (port of Go `internal/spec`).

use carapace_rs::model::{Command, Flag, Nargs};
use serde::Serialize;

/// Dump a command tree as YAML spec (with the schema header comment).
pub fn to_yaml(command: &Command) -> String {
    let yaml =
        serde_yaml::to_string(&SpecOut::from(command)).expect("spec serialization cannot fail");
    format!("# yaml-language-server: $schema=https://carapace.sh/schemas/command.json\n{yaml}")
}

#[derive(Serialize)]
struct SpecOut {
    name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parsing: Option<&'static str>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    flags: std::collections::BTreeMap<String, FlagOut>,
    #[serde(
        skip_serializing_if = "std::collections::BTreeMap::is_empty",
        rename = "persistentflags"
    )]
    persistent_flags: std::collections::BTreeMap<String, FlagOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    commands: Vec<SpecOut>,
}

impl From<&Command> for SpecOut {
    fn from(command: &Command) -> Self {
        // hidden flags are dumped with the `&` suffix (Go's VisitAll
        // includes them); persistent flags go into their own section
        let mut flags = std::collections::BTreeMap::new();
        let mut persistent_flags = std::collections::BTreeMap::new();
        for flag in &command.flags {
            let target = if flag.persistent {
                &mut persistent_flags
            } else {
                &mut flags
            };
            target.insert(definition(flag), FlagOut::from(flag));
        }
        Self {
            name: command.name.clone(),
            aliases: command.aliases.clone(),
            description: command.description.clone(),
            group: command.group.clone(),
            hidden: command.hidden,
            parsing: match (!command.interspersed, command.disable_flag_parsing) {
                (true, true) => Some("disabled"),
                (true, false) => Some("non-interspersed"),
                (false, _) => None,
            },
            flags,
            persistent_flags,
            commands: command
                .subcommands
                .iter()
                .map(SpecOut::from)
                .filter(|sub| sub.name != "_carapace")
                .collect(),
        }
    }
}

/// Flag key with modifier suffixes (port of pflagfork `Flag.Definition`):
/// long aliases append as additional `--long` parts (the parser accepts
/// them), suffix order matches Go: optarg/value, repeatable, required,
/// hidden.
fn definition(flag: &Flag) -> String {
    let mut key = match flag.shorthand {
        Some(c) if !flag.name.is_empty() => format!("-{c}, --{}", flag.name),
        Some(c) => format!("-{c}"),
        None => format!("--{}", flag.name),
    };
    for alias in &flag.aliases {
        if alias.chars().count() == 1 {
            key.push_str(&format!(", -{alias}"));
        } else {
            key.push_str(&format!(", --{alias}"));
        }
    }
    if flag.optional {
        key.push('?');
    } else if flag.takes_value {
        key.push('=');
    }
    if flag.repeatable {
        key.push('*');
    }
    if flag.required {
        key.push('!');
    }
    if flag.hidden {
        key.push('&');
    }
    key
}

#[derive(Serialize)]
struct FlagOut {
    #[serde(skip_serializing_if = "String::is_empty")]
    longhand: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    shorthand: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    repeatable: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    optarg: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    value: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    nargs: Option<i64>,
}

impl From<&Flag> for FlagOut {
    fn from(flag: &Flag) -> Self {
        let nargs = match flag.nargs {
            Nargs::Exact(1) => None,
            Nargs::Exact(n) => Some(n as i64),
            Nargs::Any => Some(-1),
            Nargs::AtLeastOne => Some(-2),
        };
        Self {
            longhand: flag.name.clone(),
            shorthand: flag.shorthand.map(|c| c.to_string()).unwrap_or_default(),
            description: flag.description.clone(),
            repeatable: flag.repeatable,
            optarg: flag.optional,
            value: flag.takes_value,
            required: flag.required,
            nargs,
        }
    }
}
