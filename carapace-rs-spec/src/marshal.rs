//! Serialization of a command tree into the YAML spec format
//! (port of Go `internal/spec`).

use carapace_rs::model::{Command, Flag, Nargs};
use serde::Serialize;
use std::collections::BTreeMap;

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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    flags: BTreeMap<String, FlagValue>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", rename = "persistentflags")]
    persistent_flags: BTreeMap<String, FlagValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    commands: Vec<SpecOut>,
}

impl From<&Command> for SpecOut {
    fn from(command: &Command) -> Self {
        // hidden flags are dumped with the `&` suffix (Go's VisitAll
        // includes them); persistent flags go into their own section
        let mut flags = BTreeMap::new();
        let mut persistent_flags = BTreeMap::new();
        for flag in &command.flags {
            let target = if flag.persistent {
                &mut persistent_flags
            } else {
                &mut flags
            };
            target.insert(definition(flag), FlagValue::from(flag));
        }
        Self {
            // Go dumps `cmd.Use`, which may embed a usage line after the
            // name (`help [command]`); the parser splits it back off
            name: command
                .use_line
                .clone()
                .unwrap_or_else(|| command.name.clone()),
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
/// them; pflag itself has no aliases). Suffix order matches Go: hidden,
/// required, repeatable, then optarg/value. An optarg without a value
/// type (boolean/count) takes no suffix at all (Go's `IsOptarg` switch).
fn definition(flag: &Flag) -> String {
    let mut key = if flag.shorthand_only {
        // Go pflagfork `ShorthandOnly`: only the shorthand is dumped; the
        // model's `name` is the framework arg id, not a real long flag
        match flag.shorthand {
            Some(c) => format!("-{c}"),
            None => format!("--{}", flag.name),
        }
    } else {
        match flag.shorthand {
            Some(c) if !flag.name.is_empty() => format!("-{c}, --{}", flag.name),
            Some(c) => format!("-{c}"),
            None => format!("--{}", flag.name),
        }
    };
    for alias in &flag.aliases {
        if alias.chars().count() == 1 {
            key.push_str(&format!(", -{alias}"));
        } else {
            key.push_str(&format!(", --{alias}"));
        }
    }
    if flag.hidden {
        key.push('&');
    }
    if flag.required {
        key.push('!');
    }
    if flag.repeatable {
        key.push('*');
    }
    if flag.optional && flag.takes_value {
        key.push('?');
    } else if flag.takes_value {
        key.push('=');
    }
    key
}

/// Flag value: plain description string, or the extended object when the
/// flag carries a non-default `nargs` (Go `FlagSet.MarshalYAML`; the
/// shorthand/longhand/flag-shape fields live in the key, Go's extended
/// form only adds description, nargs and default — the model carries no
/// default values).
#[derive(Serialize)]
#[serde(untagged)]
enum FlagValue {
    Description(String),
    Extended {
        #[serde(skip_serializing_if = "String::is_empty")]
        description: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        nargs: Option<i64>,
    },
}

impl From<&Flag> for FlagValue {
    fn from(flag: &Flag) -> Self {
        let nargs = match flag.nargs {
            Nargs::Exact(0) | Nargs::Exact(1) => None,
            Nargs::Exact(n) => Some(n as i64),
            // optargs carry no nargs (Go's `NoOptDefVal` flags keep the
            // zero Nargs); the adapter shapes optargs as `Any`, so don't
            // invent a `-1` the key's `?` suffix already covers. A spec
            // declaring `nargs` on an optarg (Any + optional) loses it.
            Nargs::Any if flag.optional => None,
            Nargs::Any => Some(-1),
            Nargs::AtLeastOne => Some(-2),
            // bounded nargs has no spec encoding; the dump loses the
            // bounds and the flag re-reads as variadic
            Nargs::Range(_, _) => Some(-1),
        };
        match nargs {
            None => Self::Description(flag.description.clone()),
            Some(nargs) => Self::Extended {
                description: flag.description.clone(),
                nargs: Some(nargs),
            },
        }
    }
}
