//! `action` command: every action constructor (Go `example/cmd/action.go`).

use std::sync::Arc;

use carapace_rs::Action;
use carapace_rs_clap::{Carapace, execute_embedded};
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_help;

pub fn cmd(commands: Option<Action>) -> Command {
    let mut cmd = with_help(
        Command::new("action")
            .about("action example")
            .alias("alias")
            .completion_group("main")
            .arg(flag_arg("callback", "ActionCallback()"))
            .arg(flag_arg("cobra", "ActionCobra()"))
            .arg(flag_arg("commands", "ActionCommands()"))
            .arg(flag_arg("directories", "ActionDirectories()"))
            .arg(flag_arg("execcommand", "ActionExecCommand()"))
            .arg(flag_arg("execcommandE", "ActionExecCommand()"))
            .arg(flag_arg("executables", "ActionExecutables()"))
            .arg(flag_arg("files", "ActionFiles()"))
            .arg(flag_arg(
                "files-filtered",
                "ActionFiles(\".md\", \"go.mod\", \"go.sum\")",
            ))
            .arg(flag_arg("import", "ActionImport()"))
            .arg(flag_arg("message", "ActionMessage()"))
            .arg(flag_arg("message-multiple", "ActionMessage()"))
            .arg(flag_arg("multiparts", "ActionMultiParts()"))
            .arg(flag_arg(
                "multiparts-nested",
                "ActionMultiParts(...ActionMultiParts...)",
            ))
            .arg(flag_arg("multipartsn", "ActionMultiPartsN()"))
            .arg(flag_arg("multipartsn-empty", "ActionMultiPartsN()"))
            .arg(flag_arg("styles", "ActionStyles()"))
            .arg(flag_arg("styleconfig", "ActionStyleConfig()"))
            .arg(flag_arg("styled-values", "ActionStyledValues()"))
            .arg(flag_arg(
                "styled-values-described",
                "ActionStyledValuesDescribed()",
            ))
            .arg(flag_arg("values", "ActionValues()"))
            .arg(flag_arg("values-described", "ActionValuesDescribed()")),
    );

    let mut completions = vec![
        ("callback", callback()),
        // Go's `ActionCobra()` completes via a cobra binary; here the
        // directive semantics are bridge tests (`carapace-rs-bridge`), so the
        // flag keeps its name and usage label with a stand-in action
        ("cobra", Action::values(["one", "two"]).nospace([])),
        ("directories", Action::directories()),
        (
            "execcommand",
            Action::exec_command("git", ["remote"], |output| {
                let text = String::from_utf8_lossy(output);
                let lines: Vec<String> = text
                    .split('\n')
                    .filter(|line| !line.is_empty())
                    .map(String::from)
                    .collect();
                Action::values(lines)
            }),
        ),
        (
            "execcommandE",
            Action::exec_command_e("false", [""; 0], |result| match result {
                Err(_) => Action::message("failed with exit status 1"),
                Ok(_) => Action::default(),
            }),
        ),
        ("executables", Action::executables()),
        ("files", Action::files([])),
        ("files-filtered", Action::files([".md", "go.mod", "go.sum"])),
        (
            "import",
            Action::import(
                r#"{
  "version": "unknown",
  "nospace": "",
  "values": [
    {
      "value": "first",
      "display": "first"
    },
    {
      "value": "second",
      "display": "second"
    },
    {
      "value": "third",
      "display": "third"
    }
  ]
}"#,
            ),
        ),
        ("message", Action::message("example message")),
        (
            "message-multiple",
            carapace_rs::batch(vec![
                Action::message("first message"),
                Action::message("second message"),
                Action::message("third message"),
                Action::values(["one", "two", "three"]),
            ]),
        ),
        (
            "multiparts",
            Action::multiparts(":", |c| match c.parts.len() {
                0 => Action::values(["userA", "userB"])
                    .invoke(c)
                    .suffix(":")
                    .to_action(),
                1 => Action::values(["groupA", "groupB"]),
                _ => Action::default(),
            }),
        ),
        ("multiparts-nested", multiparts_nested()),
        ("multipartsn", multipartsn()),
        ("multipartsn-empty", multipartsn_empty()),
        ("styles", Action::styles([])),
        ("styleconfig", Action::style_config()),
        (
            "styled-values",
            Action::styled_values([
                ("first", ""),
                ("second", "blue"),
                ("third", "bg-bright-black magenta bold"),
            ]),
        ),
        (
            "styled-values-described",
            Action::styled_values_described([
                ("first", "description of first", "blink"),
                ("second", "description of second", "color210 underlined"),
                ("third", "description of third", "#112233 italic"),
                ("thirdalias", "description of third", "bg-bright-magenta"),
            ]),
        ),
        ("values", Action::values(["first", "second", "third"])),
        (
            "values-described",
            Action::values_described([
                ("first", "description of first"),
                ("second", "description of second"),
                ("third", "description of third"),
            ]),
        ),
    ];
    if let Some(commands) = commands {
        completions.push(("commands", commands));
    }

    cmd = cmd
        .flag_completion(completions)
        .positional_any_completion(execute_embedded(embedded_cmd()));

    cmd
}

fn flag_arg(name: &'static str, description: &str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Set)
        .help(description.to_owned())
}

fn callback() -> Action {
    Action::callback(|c| {
        if let Some((_, values)) = c.parsed_flags.iter().find(|(name, _)| name == "values") {
            return Action::message(format!(
                "values flag is set to: '{}'",
                values.first().map(String::as_str).unwrap_or_default()
            ));
        }
        Action::message("values flag is not set")
    })
}

fn multiparts_nested() -> Action {
    Action::multiparts(",", |c_entries| {
        let outer_parts = c_entries.parts.clone();
        Action::multiparts("=", move |c| match c.parts.len() {
            0 => {
                let keys: Vec<String> = outer_parts
                    .iter()
                    .map(|entry| entry.split('=').next().unwrap_or_default().to_owned())
                    .collect();
                Action::values(["FILE", "DIRECTORY", "VALUE"])
                    .invoke(c)
                    .filter(keys)
                    .suffix("=")
                    .to_action()
            }
            1 => match c.parts[0].as_str() {
                "FILE" => Action::files([]).nospace([]),
                "DIRECTORY" => Action::directories().nospace([]),
                "VALUE" => Action::values(["one", "two", "three"]).nospace([]),
                _ => Action::default(),
            },
            _ => Action::default(),
        })
    })
}

fn multipartsn() -> Action {
    Action::multiparts_n("=", 2, |c| match c.parts.len() {
        0 => Action::values(["one", "two"]).suffix("="),
        1 => Action::multiparts("=", |c| match c.parts.len() {
            0 => Action::values(["three", "four"]).suffix("="),
            1 => Action::values(["five", "six"]),
            _ => Action::default(),
        }),
        _ => Action::message("should never happen"),
    })
}

fn multipartsn_empty() -> Action {
    Action::multiparts_n("", 2, |c| match c.parts.len() {
        0 => Action::values(["a", "b"]),
        1 => Action::values(["c", "d", "e"]).unique_list(""),
        _ => Action::message("should never happen"),
    })
}

fn embedded_cmd() -> Command {
    with_help(Command::new("embedded"))
        .arg(
            Arg::new("embedded-bool")
                .long("embedded-bool")
                .action(ArgAction::SetTrue)
                .help("embedded bool flag"),
        )
        .arg(
            Arg::new("embedded-string")
                .long("embedded-string")
                .action(ArgAction::Set)
                .help("embedded string flag"),
        )
        .arg(
            Arg::new("embedded-optarg")
                .long("embedded-optarg")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_missing_value(" ")
                .help("embedded optarg flag"),
        )
        .flag_completion([
            ("embedded-string", Action::values(["es1", "es2", "es3"])),
            ("embedded-optarg", Action::values(["eo1", "eo2", "eo3"])),
        ])
        .positional_completion([
            Action::values(["embeddedPositional1", "embeddedP1"]),
            Action::values(["embeddedPositional2 with space", "embeddedP2 with space"]),
        ])
}

/// The embedded tree as a model snapshot (used by `Action::execute`
/// consumers outside the tree; kept for symmetry with the Go source).
#[allow(dead_code)]
pub fn embedded_model() -> Arc<carapace_rs::model::Command> {
    let mut cmd = embedded_cmd();
    Arc::new(carapace_rs_clap::to_model(&mut cmd))
}
