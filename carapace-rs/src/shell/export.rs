//! Command-structure export (port of Go `internal/shell/export`).
//!
//! `binary _carapace export` (snippet mode) serializes the command tree as
//! JSON for external consumers. The shape follows Go exactly: capitalized
//! keys, `omitempty` semantics, subcommands filtered by visibility.
//!
//! Model-mapping limits (the normalized model carries less than cobra):
//! the single help text maps to `Short` (`Long` stays empty), all flags are
//! reported as `LocalFlags` (no persistent/global distinction), `Type` is
//! `bool`/`string` only, and `NoOptDefVal` is never emitted.

use serde::Serialize;

use crate::model::Command;

#[derive(Serialize)]
struct Structure {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Short")]
    short: String,
    #[serde(rename = "Long", skip_serializing_if = "String::is_empty")]
    long: String,
    #[serde(rename = "Aliases", skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(rename = "Commands", skip_serializing_if = "Vec::is_empty")]
    commands: Vec<Structure>,
    #[serde(rename = "LocalFlags", skip_serializing_if = "Vec::is_empty")]
    local_flags: Vec<FlagStructure>,
    #[serde(rename = "PersistentFlags", skip_serializing_if = "Vec::is_empty")]
    persistent_flags: Vec<FlagStructure>,
}

#[derive(Serialize)]
struct FlagStructure {
    #[serde(rename = "Longhand", skip_serializing_if = "String::is_empty")]
    longhand: String,
    #[serde(rename = "Shorthand", skip_serializing_if = "String::is_empty")]
    shorthand: String,
    #[serde(rename = "Usage")]
    usage: String,
    #[serde(rename = "Type")]
    value_type: String,
    #[serde(rename = "NoOptDefVal", skip_serializing_if = "String::is_empty")]
    no_opt_def_val: String,
}

/// Serialize the command tree as JSON (Go `export.Snippet`).
///
/// The `help` subcommand is excluded: the framework auto-generates it and
/// Go replaces cobra's help with a hidden fake, so Go exports never contain
/// it either.
pub fn snippet(root: &Command) -> String {
    serde_json::to_string(&convert(root)).unwrap_or_default()
}

fn convert(cmd: &Command) -> Structure {
    Structure {
        name: cmd.name.clone(),
        short: cmd.description.clone(),
        long: String::new(),
        aliases: cmd.aliases.clone(),
        commands: cmd
            .subcommands
            .iter()
            .filter(|sub| !sub.hidden && sub.name != "help")
            .map(convert)
            .collect(),
        local_flags: cmd.flags.iter().map(flag_structure).collect(),
        persistent_flags: Vec::new(),
    }
}

fn flag_structure(flag: &crate::model::Flag) -> FlagStructure {
    FlagStructure {
        longhand: if flag.shorthand_only {
            String::new()
        } else {
            flag.name.clone()
        },
        shorthand: flag
            .shorthand
            .map(|short| short.to_string())
            .unwrap_or_default(),
        usage: flag.description.clone(),
        value_type: if flag.takes_value {
            "string".to_owned()
        } else {
            "bool".to_owned()
        },
        no_opt_def_val: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Flag;

    #[test]
    fn serializes_tree_with_go_keys() {
        let model = Command::new("myapp")
            .description("the app")
            .alias("app")
            .flag(Flag {
                name: "verbose".to_owned(),
                shorthand: Some('v'),
                ..Default::default()
            })
            .flag(Flag {
                name: "profile".to_owned(),
                takes_value: true,
                description: "deployment profile".to_owned(),
                ..Default::default()
            })
            .subcommand(Command {
                hidden: true,
                ..Command::new("hidden")
            })
            .subcommand(Command::new("build"));

        let json = snippet(&model);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["Name"], "myapp");
        assert_eq!(value["Short"], "the app");
        assert_eq!(value["Aliases"], serde_json::json!(["app"]));
        assert_eq!(value["LocalFlags"][0]["Longhand"], "verbose");
        assert_eq!(value["LocalFlags"][0]["Shorthand"], "v");
        assert_eq!(value["LocalFlags"][0]["Type"], "bool");
        assert_eq!(value["LocalFlags"][1]["Type"], "string");
        assert_eq!(value["LocalFlags"][1]["Usage"], "deployment profile");
        // omitempty keys are absent; Usage/Type are always emitted (Go tags)
        assert!(value.get("Long").is_none());
        assert!(value.get("PersistentFlags").is_none());
        assert!(value["LocalFlags"][0].get("Usage").is_some());
        assert!(value["LocalFlags"][0].get("NoOptDefVal").is_none());
        // hidden subcommands are filtered
        assert_eq!(value["Commands"][0]["Name"], "build");
    }

    #[test]
    fn shorthand_only_flag_omits_longhand() {
        let model = Command::new("myapp").flag(Flag {
            name: "p".to_owned(),
            shorthand: Some('p'),
            shorthand_only: true,
            ..Default::default()
        });
        let value: serde_json::Value = serde_json::from_str(&snippet(&model)).unwrap();
        assert!(value["LocalFlags"][0].get("Longhand").is_none());
        assert_eq!(value["LocalFlags"][0]["Shorthand"], "p");
    }
}
