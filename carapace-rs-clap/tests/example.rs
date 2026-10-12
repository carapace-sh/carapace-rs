//! Sandbox tests for the example command tree (port of Go
//! `carapace/example/cmd/*_test.go`).
//!
//! Each topic module mirrors a Go test file; expectations are compared as
//! normalized export JSON (prefix-filtered, sorted, uid-stripped) like Go's
//! sandbox. Tests touching `CARAPACE_*` environment variables hold the
//! process env lock since the engine reads configuration from the process
//! environment (Go reads it in the `go run` child).

#[path = "../examples/example/cmd/mod.rs"]
mod cmd;

use carapace_rs::Action;
use carapace_rs_testkit::Sandbox;

mod root {
    use super::*;

    #[test]
    fn completes_subcommands_grouped() {
        let sandbox = Sandbox::new("example-root");
        sandbox
            .run(cmd::root_cmd, &[""])
            .expect(carapace_rs::batch(vec![
                Action::values_described([
                    ("action", "action example"),
                    ("alias", "action example"),
                    ("flag", "flag example"),
                ])
                .style("blue")
                .tag("main commands"),
                Action::values_described([("modifier", "modifier example")])
                    .style("yellow")
                    .tag("modifier commands"),
                Action::values_described([("plugin", "dynamic plugin command")])
                    .style("magenta")
                    .tag("plugin commands"),
                Action::values_described([
                    ("chain", "shorthand chain"),
                    ("compat", ""),
                    (
                        "completion",
                        "Generate the autocompletion script for the specified shell",
                    ),
                    ("group", "group example"),
                    ("help", "Help about any command"),
                    ("interspersed", "interspersed example"),
                    ("multiparts", "multiparts example"),
                    ("special", ""),
                    ("subcommand", "subcommand example"),
                ])
                .tag("other commands"),
            ]));
    }

    #[test]
    fn filters_by_prefix() {
        let sandbox = Sandbox::new("example-root-prefix");
        sandbox.run(cmd::root_cmd, &["a"]).expect(
            Action::values_described([("action", "action example"), ("alias", "action example")])
                .style("blue")
                .tag("main commands"),
        );
        sandbox.run(cmd::root_cmd, &["action"]).expect(
            Action::values_described([("action", "action example")])
                .style("blue")
                .tag("main commands"),
        );
    }

    #[test]
    fn completes_flags_split_by_prefix() {
        let sandbox = Sandbox::new("example-root-flags");
        sandbox.run(cmd::root_cmd, &["-"]).expect(
            carapace_rs::batch(vec![
                Action::styled_values_described([
                    ("-C", "change work directory", "blue"),
                    ("-a", "multiflag", "blue"),
                    ("-h", "help for example", ""),
                    ("-p", "Help message for persistentFlag", "yellow"),
                    ("-t", "Help message for toggle", ""),
                    ("-v", "version for example", ""),
                ])
                .tag("shorthand flags"),
                Action::styled_values_described([
                    ("--chdir", "change work directory", "blue"),
                    ("--array", "multiflag", "blue"),
                    ("--help", "help for example", ""),
                    (
                        "--persistentFlag",
                        "Help message for persistentFlag",
                        "yellow",
                    ),
                    (
                        "--persistentFlag2",
                        "Help message for persistentFlag2",
                        "blue",
                    ),
                    ("--toggle", "Help message for toggle", ""),
                    ("--version", "version for example", ""),
                ])
                .tag("longhand flags"),
            ])
            .nospace(['.']),
        );

        sandbox.run(cmd::root_cmd, &["--"]).expect(
            Action::styled_values_described([
                ("--array", "multiflag", "blue"),
                ("--chdir", "change work directory", "blue"),
                ("--help", "help for example", ""),
                (
                    "--persistentFlag",
                    "Help message for persistentFlag",
                    "yellow",
                ),
                (
                    "--persistentFlag2",
                    "Help message for persistentFlag2",
                    "blue",
                ),
                ("--toggle", "Help message for toggle", ""),
                ("--version", "version for example", ""),
            ])
            .nospace(['.'])
            .tag("longhand flags"),
        );

        sandbox.run(cmd::root_cmd, &["--a"]).expect(
            Action::styled_values_described([("--array", "multiflag", "blue")])
                .nospace(['.'])
                .tag("longhand flags"),
        );

        sandbox.run(cmd::root_cmd, &["--array"]).expect(
            Action::styled_values_described([("--array", "multiflag", "blue")])
                .nospace(['.'])
                .tag("longhand flags"),
        );

        sandbox.run(cmd::root_cmd, &["--array", "", "--a"]).expect(
            Action::styled_values_described([("--array", "multiflag", "blue")])
                .nospace(['.'])
                .tag("longhand flags"),
        );

        sandbox.run(cmd::root_cmd, &["-a", "", "--a"]).expect(
            Action::styled_values_described([("--array", "multiflag", "blue")])
                .nospace(['.'])
                .tag("longhand flags"),
        );
    }

    #[test]
    fn completes_optargs() {
        let sandbox = Sandbox::new("example-root-optarg");
        sandbox.run(cmd::root_cmd, &["--persistentFlag="]).expect(
            Action::values(["p1", "p2", "p3"])
                .prefix("--persistentFlag=")
                .usage("Help message for persistentFlag"),
        );
        sandbox.run(cmd::root_cmd, &["--persistentFlag=p"]).expect(
            Action::values(["p1", "p2", "p3"])
                .prefix("--persistentFlag=")
                .usage("Help message for persistentFlag"),
        );
        sandbox.run(cmd::root_cmd, &["--toggle="]).expect(
            Action::styled_values([("true", "green"), ("false", "red")])
                .prefix("--toggle=")
                .usage("Help message for toggle"),
        );
        sandbox.run(cmd::root_cmd, &["--toggle=tru"]).expect(
            Action::styled_values([("true", "green")])
                .prefix("--toggle=")
                .usage("Help message for toggle"),
        );
    }

    #[test]
    fn merges_flags_when_configured() {
        let mut sandbox = Sandbox::new("example-root-mergeflags");
        sandbox.env("CARAPACE_MERGEFLAGS", "1");
        sandbox.run(cmd::root_cmd, &["-"]).expect(
            Action::styled_values_described([
                ("-C", "change work directory", "blue"),
                ("-a", "multiflag", "blue"),
                ("-h", "help for example", ""),
                ("-p", "Help message for persistentFlag", "yellow"),
                ("-t", "Help message for toggle", ""),
                ("-v", "version for example", ""),
                ("--chdir", "change work directory", "blue"),
                ("--array", "multiflag", "blue"),
                ("--help", "help for example", ""),
                (
                    "--persistentFlag",
                    "Help message for persistentFlag",
                    "yellow",
                ),
                (
                    "--persistentFlag2",
                    "Help message for persistentFlag2",
                    "blue",
                ),
                ("--toggle", "Help message for toggle", ""),
                ("--version", "version for example", ""),
            ])
            .tag("flags")
            .nospace(['.']),
        );

        sandbox.env("CARAPACE_MERGEFLAGS", "0");
        sandbox.run(cmd::root_cmd, &["-"]).expect(
            carapace_rs::batch(vec![
                Action::styled_values_described([
                    ("-C", "change work directory", "blue"),
                    ("-a", "multiflag", "blue"),
                    ("-h", "help for example", ""),
                    ("-p", "Help message for persistentFlag", "yellow"),
                    ("-t", "Help message for toggle", ""),
                    ("-v", "version for example", ""),
                ])
                .tag("shorthand flags"),
                Action::styled_values_described([
                    ("--chdir", "change work directory", "blue"),
                    ("--array", "multiflag", "blue"),
                    ("--help", "help for example", ""),
                    (
                        "--persistentFlag",
                        "Help message for persistentFlag",
                        "yellow",
                    ),
                    (
                        "--persistentFlag2",
                        "Help message for persistentFlag2",
                        "blue",
                    ),
                    ("--toggle", "Help message for toggle", ""),
                    ("--version", "version for example", ""),
                ])
                .tag("longhand flags"),
            ])
            .nospace(['.']),
        );
    }
}

mod help {
    use super::*;

    #[test]
    fn completes_commands() {
        let sandbox = Sandbox::new("example-help");
        sandbox.run(cmd::root_cmd, &["help", "a"]).expect(
            Action::values_described([("action", "action example"), ("alias", "action example")])
                .style("blue")
                .tag("main commands")
                .usage("help [command]"),
        );
    }
}

mod flag {
    use super::*;

    #[test]
    fn completes_dash_arguments() {
        let sandbox = Sandbox::new("example-flag-dash");
        sandbox
            .run(cmd::root_cmd, &["flag", "--", ""])
            .expect(Action::values(["d1", "dash1", "-d1", "--dash1"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "-"])
            .expect(Action::values(["-d1", "--dash1"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "--"])
            .expect(Action::values(["--dash1"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", ""])
            .expect(Action::values(["d2", "dash2", "-d2", "--dash2"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", "-"])
            .expect(Action::values(["-d2", "--dash2"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", "--"])
            .expect(Action::values(["--dash2"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", "d2", ""])
            .expect(Action::values(["dAny", "dashAny", "-dAny", "--dashAny"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", "d2", "-"])
            .expect(Action::values(["-dAny", "--dashAny"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "--", "d1", "d2", "--"])
            .expect(Action::values(["--dashAny"]));
    }
}

mod flag_disabled {
    use super::*;

    #[test]
    fn completes_positionals_without_flag_parsing() {
        let sandbox = Sandbox::new("example-flag-disabled");
        sandbox
            .run(cmd::root_cmd, &["flag", "disabled", ""])
            .expect(Action::values(["-p1", "positional1"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "disabled", "-p1", ""])
            .expect(Action::values(["p2", "--positional2"]));
        sandbox
            .run(cmd::root_cmd, &["flag", "disabled", "-p1", "p2", ""])
            .expect(Action::values(Vec::<String>::new()));
    }
}

mod interspersed {
    use super::*;

    #[test]
    fn stops_flag_parsing_at_positionals() {
        let sandbox = Sandbox::new("example-interspersed");
        sandbox.run(cmd::root_cmd, &["interspersed", "--s"]).expect(
            Action::values_described([("--string", "string flag")])
                .style("blue")
                .nospace(['.'])
                .tag("longhand flags"),
        );
        sandbox
            .run(cmd::root_cmd, &["interspersed", "--bool", "--s"])
            .expect(
                Action::values_described([("--string", "string flag")])
                    .style("blue")
                    .nospace(['.'])
                    .tag("longhand flags"),
            );
        sandbox
            .run(cmd::root_cmd, &["interspersed", "--bool", ""])
            .expect(Action::values(["p1", "positional1"]));
        sandbox
            .run(cmd::root_cmd, &["interspersed", "--bool", "p1", "-"])
            .expect(Action::values(Vec::<String>::new()));
        sandbox
            .run(cmd::root_cmd, &["interspersed", "--bool", "p1", "--", ""])
            .expect(Action::values(Vec::<String>::new()));
        sandbox
            .run(
                cmd::root_cmd,
                &["interspersed", "--bool", "p1", "--", "", ""],
            )
            .expect(Action::values(Vec::<String>::new()));
    }
}

mod special {
    use super::*;

    #[test]
    fn respects_mutually_exclusive_flags() {
        let sandbox = Sandbox::new("example-special");
        sandbox.run(cmd::root_cmd, &["special", "--e"]).expect(
            Action::values_described([
                ("--exclusive", "mutually exclusive flag"),
                (
                    "--exclusiveRepeatable",
                    "mutually exclusive repeatable flag",
                ),
            ])
            .nospace(['.'])
            .tag("longhand flags"),
        );
        sandbox
            .run(cmd::root_cmd, &["special", "--exclusive", "--e"])
            .expect(Action::values(Vec::<String>::new()).nospace(['.']));
        sandbox
            .run(cmd::root_cmd, &["special", "--exclusiveRepeatable", "--e"])
            .expect(
                Action::values_described([(
                    "--exclusiveRepeatable",
                    "mutually exclusive repeatable flag",
                )])
                .nospace(['.'])
                .tag("longhand flags"),
            );
    }
}

mod subcommand {
    use super::*;

    #[test]
    fn completes_aliases_groups_and_hidden() {
        let sandbox = Sandbox::new("example-subcommand");
        sandbox
            .run(cmd::root_cmd, &["subcommand", ""])
            .expect(carapace_rs::batch(vec![
                Action::values_described([
                    ("a1", "subcommand with alias"),
                    ("a2", "subcommand with alias"),
                    ("alias", "subcommand with alias"),
                ])
                .tag("other commands"),
                Action::values_described([("group", "subcommand with group")])
                    .tag("group commands")
                    .style("blue"),
            ]));
    }

    #[test]
    fn includes_hidden_when_configured() {
        let mut sandbox = Sandbox::new("example-subcommand-hidden");
        sandbox.env("CARAPACE_HIDDEN", "1");
        sandbox
            .run(cmd::root_cmd, &["subcommand", ""])
            .expect(carapace_rs::batch(vec![
                Action::values_described([
                    ("a1", "subcommand with alias"),
                    ("a2", "subcommand with alias"),
                    ("alias", "subcommand with alias"),
                    ("hidden", "hidden subcommand"),
                ])
                .tag("other commands"),
                Action::values_described([("group", "subcommand with group")])
                    .tag("group commands")
                    .style("blue"),
            ]));

        sandbox.env("CARAPACE_HIDDEN", "2");
        sandbox
            .run(cmd::root_cmd, &["subcommand", ""])
            .expect(carapace_rs::batch(vec![
                Action::values_described([
                    ("a1", "subcommand with alias"),
                    ("a2", "subcommand with alias"),
                    ("alias", "subcommand with alias"),
                    ("hidden", "hidden subcommand"),
                    ("_carapace", ""),
                ])
                .tag("other commands"),
                Action::values_described([("group", "subcommand with group")])
                    .tag("group commands")
                    .style("blue"),
            ]));
    }
}

mod chain {
    use super::*;

    #[test]
    fn completes_shorthand_chains() {
        let sandbox = Sandbox::new("example-chain");
        sandbox.run(cmd::root_cmd, &["chain", "-b"]).expect(
            Action::styled_values([("c", ""), ("o", "yellow"), ("v", "blue")])
                .prefix("-b")
                .nospace(['c', 'o'])
                .tag("shorthand flags"),
        );
        sandbox.run(cmd::root_cmd, &["chain", "-bc"]).expect(
            Action::styled_values([("c", ""), ("o", "yellow"), ("v", "blue")])
                .prefix("-bc")
                .nospace(['c', 'o'])
                .tag("shorthand flags"),
        );
        sandbox.run(cmd::root_cmd, &["chain", "-bcc"]).expect(
            Action::styled_values([("c", ""), ("o", "yellow"), ("v", "blue")])
                .prefix("-bcc")
                .nospace(['c', 'o'])
                .tag("shorthand flags"),
        );
        sandbox.run(cmd::root_cmd, &["chain", "-bcco"]).expect(
            Action::styled_values([("c", ""), ("v", "blue")])
                .prefix("-bcco")
                .nospace(['c'])
                .tag("shorthand flags"),
        );
        sandbox
            .run(cmd::root_cmd, &["chain", "-bcco", ""])
            .expect(Action::values(["p1", "positional1"]));
        sandbox
            .run(cmd::root_cmd, &["chain", "-bcco="])
            .expect(Action::values(["opt1", "opt2"]).prefix("-bcco="));
        sandbox
            .run(cmd::root_cmd, &["chain", "-bccv", ""])
            .expect(Action::values(["val1", "val2"]));
        sandbox
            .run(cmd::root_cmd, &["chain", "-bccv="])
            .expect(Action::values(["val1", "val2"]).prefix("-bccv="));
        sandbox
            .run(cmd::root_cmd, &["chain", "-bccv", "val1", "-c"])
            .expect(
                Action::styled_values([("c", ""), ("o", "yellow")])
                    .prefix("-c")
                    .nospace(['c', 'o'])
                    .tag("shorthand flags"),
            );
    }
}

mod action {
    use super::*;
    use carapace_rs::shell;

    const FILES: [(&str, &str); 5] = [
        ("dirA/file1.txt", ""),
        ("dirA/file2.png", ""),
        ("dirB/dirC/file3.go", ""),
        ("dirB/file4.md", ""),
        ("file5.go", ""),
    ];

    fn for_path() -> impl Fn(&carapace_rs::Context, &carapace_rs::RawValue) -> String + Copy {
        |context, value| shell::for_path(context, &value.value)
    }

    #[test]
    fn completes_flag_actions() {
        let mut sandbox = Sandbox::new("example-action");
        sandbox.files(FILES);
        sandbox.reply("git", &["remote"]).with("origin\nfork");

        sandbox
            .run(cmd::root_cmd, &["action", "--callback", ""])
            .expect(Action::message("values flag is not set").usage("ActionCallback()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--cobra", ""])
            .expect(
                Action::values(["one", "two"])
                    .nospace([])
                    .usage("ActionCobra()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--commands", "s"])
            .expect(
                Action::values_described([("special", ""), ("subcommand", "subcommand example")])
                    .suffix(" ")
                    .nospace([])
                    .tag("other commands")
                    .usage("ActionCommands()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--commands", "subcommand "])
            .expect(
                carapace_rs::batch(vec![
                    Action::values_described([
                        ("a1", "subcommand with alias"),
                        ("a2", "subcommand with alias"),
                        ("alias", "subcommand with alias"),
                    ])
                    .tag("other commands"),
                    Action::values_described([("group", "subcommand with group")])
                        .style("blue")
                        .tag("group commands"),
                ])
                .prefix("subcommand ")
                .suffix(" ")
                .nospace([])
                .usage("ActionCommands()"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--commands", "subcommand unknown "],
            )
            .expect(
                Action::message("unknown subcommand \"unknown\" for \"subcommand\"")
                    .nospace([])
                    .usage("ActionCommands()"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--commands", "subcommand hidden "],
            )
            .expect(
                Action::values_described([("visible", "visible subcommand of a hidden command")])
                    .prefix("subcommand hidden ")
                    .suffix(" ")
                    .nospace([])
                    .tag("commands")
                    .usage("ActionCommands()"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--values", "first", "--callback", ""],
            )
            .expect(Action::message("values flag is set to: 'first'").usage("ActionCallback()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--directories", ""])
            .expect(
                Action::values(["dirA/", "dirB/"])
                    .tag("directories")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionDirectories()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--directories", "dirB/"])
            .expect(
                Action::values(["dirC/"])
                    .prefix("dirB/")
                    .tag("directories")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionDirectories()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--execcommand", ""])
            .expect(Action::values(["origin", "fork"]).usage("ActionExecCommand()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--files", ""])
            .expect(
                Action::values(["dirA/", "dirB/", "file5.go"])
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionFiles()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--files-filtered", ""])
            .expect(
                Action::values(["dirA/", "dirB/"])
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionFiles(\".md\", \"go.mod\", \"go.sum\")"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--files-filtered", "dirB/"])
            .expect(
                Action::values(["dirC/", "file4.md"])
                    .prefix("dirB/")
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionFiles(\".md\", \"go.mod\", \"go.sum\")"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--import", ""])
            .expect(Action::values(["first", "second", "third"]).usage("ActionImport()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--import", "s"])
            .expect(Action::values(["second"]).usage("ActionImport()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--message", ""])
            .expect(Action::message("example message").usage("ActionMessage()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--message-multiple", "t"])
            .expect(
                carapace_rs::batch(vec![
                    Action::message("first message"),
                    Action::message("second message"),
                    Action::message("third message"),
                    Action::values(["one", "two", "three"]),
                ])
                .usage("ActionMessage()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multiparts", ""])
            .expect(
                Action::values(["userA", "userB"])
                    .suffix(":")
                    .nospace([':'])
                    .usage("ActionMultiParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multiparts", "userA:"])
            .expect(
                Action::values(["groupA", "groupB"])
                    .prefix("userA:")
                    .nospace([':'])
                    .usage("ActionMultiParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multiparts-nested", ""])
            .expect(
                Action::values(["DIRECTORY", "FILE", "VALUE"])
                    .suffix("=")
                    .nospace([',', '='])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multiparts-nested", "VALUE="])
            .expect(
                Action::values(["one", "two", "three"])
                    .prefix("VALUE=")
                    .nospace([])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--multiparts-nested", "VALUE=two,"],
            )
            .expect(
                Action::values(["DIRECTORY", "FILE"])
                    .prefix("VALUE=two,")
                    .suffix("=")
                    .nospace([',', '='])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--multiparts-nested", "VALUE=two,DIRECTORY="],
            )
            .expect(
                Action::values(["dirA/", "dirB/"])
                    .tag("directories")
                    .style_f(for_path())
                    .prefix("VALUE=two,DIRECTORY=")
                    .nospace([])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--styled-values", "s"])
            .expect(Action::styled_values([("second", "blue")]).usage("ActionStyledValues()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--styled-values-described", "t"])
            .expect(
                Action::styled_values_described([
                    ("third", "description of third", "#112233 italic"),
                    ("thirdalias", "description of third", "bg-bright-magenta"),
                ])
                .usage("ActionStyledValuesDescribed()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--values", "sec"])
            .expect(Action::values(["second"]).usage("ActionValues()"));
        sandbox
            .run(cmd::root_cmd, &["action", "--values-described", "third"])
            .expect(
                Action::values_described([("third", "description of third")])
                    .usage("ActionValuesDescribed()"),
            );
    }

    #[test]
    fn completes_embedded_positionals() {
        let sandbox = Sandbox::new("example-action-embedded");
        sandbox.run(cmd::root_cmd, &["action", "embe"]).expect(
            Action::values(["embeddedP1", "embeddedPositional1"])
                .usage("action [pos1] [pos2] [--] [dashAny]..."),
        );
        sandbox
            .run(cmd::root_cmd, &["action", "embeddedP1", "embeddedP2 "])
            .expect(
                Action::values(["embeddedP2 with space"])
                    .usage("action [pos1] [pos2] [--] [dashAny]..."),
            );
    }

    #[test]
    fn completes_dash_via_embedded() {
        let sandbox = Sandbox::new("example-action-dash");
        sandbox.run(cmd::root_cmd, &["action", "--", ""]).expect(
            Action::values(["embeddedP1", "embeddedPositional1"])
                .usage("action [pos1] [pos2] [--] [dashAny]..."),
        );
        sandbox.run(cmd::root_cmd, &["action", "--", "-"]).expect(
            carapace_rs::batch(vec![
                Action::styled_values_described([("-h", "help for embedded", "")])
                    .tag("shorthand flags"),
                Action::styled_values_described([
                    ("--embedded-bool", "embedded bool flag", ""),
                    ("--embedded-optarg", "embedded optarg flag", "yellow"),
                    ("--embedded-string", "embedded string flag", "blue"),
                    ("--help", "help for embedded", ""),
                ])
                .tag("longhand flags"),
            ])
            .nospace(['.'])
            .usage("action [pos1] [pos2] [--] [dashAny]..."),
        );
        sandbox.run(cmd::root_cmd, &["action", "--", "--"]).expect(
            Action::styled_values_described([
                ("--embedded-bool", "embedded bool flag", ""),
                ("--embedded-optarg", "embedded optarg flag", "yellow"),
                ("--embedded-string", "embedded string flag", "blue"),
                ("--help", "help for embedded", ""),
            ])
            .nospace(['.'])
            .usage("action [pos1] [pos2] [--] [dashAny]...")
            .tag("longhand flags"),
        );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--", "embeddedP1", "--embedded-optarg="],
            )
            .expect(
                Action::values(["eo1", "eo2", "eo3"])
                    .prefix("--embedded-optarg=")
                    .usage("embedded optarg flag"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--", "embeddedP1", "--embedded-string", ""],
            )
            .expect(Action::values(["es1", "es2", "es3"]).usage("embedded string flag"));
        sandbox
            .run(
                cmd::root_cmd,
                &[
                    "action",
                    "embeddedP1",
                    "--styled-values",
                    "second",
                    "--",
                    "--embedded-string",
                    "es1",
                    "",
                ],
            )
            .expect(
                Action::values(["embeddedP2 with space", "embeddedPositional2 with space"])
                    .usage("action [pos1] [pos2] [--] [dashAny]..."),
            );
    }

    #[test]
    fn unknown_flag_and_lenient() {
        let mut sandbox = Sandbox::new("example-action-unknown");
        sandbox
            .run(cmd::root_cmd, &["action", "--unknown", ""])
            .expect(Action::message("unknown flag: --unknown"));

        sandbox.env("CARAPACE_LENIENT", "1");
        sandbox
            .run(cmd::root_cmd, &["action", "--unknown", ""])
            .expect(
                Action::values(["embeddedP1", "embeddedPositional1"])
                    .usage("action [pos1] [pos2] [--] [dashAny]..."),
            );
    }

    #[test]
    fn completes_persistent_flags() {
        let sandbox = Sandbox::new("example-action-persistent");
        sandbox
            .run(cmd::root_cmd, &["action", "--persistentFlag="])
            .expect(
                Action::values(["p1", "p2", "p3"])
                    .prefix("--persistentFlag=")
                    .usage("Help message for persistentFlag"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--persistentFlag2", ""])
            .expect(Action::values(["p4", "p5", "p6"]).usage("Help message for persistentFlag2"));
    }

    #[test]
    fn completes_attached_values() {
        let sandbox = Sandbox::new("example-action-attached");
        sandbox.files(FILES);
        sandbox.run(cmd::root_cmd, &["action", "--values="]).expect(
            Action::values(["first", "second", "third"])
                .prefix("--values=")
                .usage("ActionValues()"),
        );
        sandbox
            .run(cmd::root_cmd, &["action", "--values=f"])
            .expect(
                Action::values(["first"])
                    .prefix("--values=")
                    .usage("ActionValues()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--values=first", ""])
            .expect(
                Action::values(["embeddedP1", "embeddedPositional1"])
                    .usage("action [pos1] [pos2] [--] [dashAny]..."),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multiparts-nested=VALUE="])
            .expect(
                Action::values(["one", "two", "three"])
                    .prefix("--multiparts-nested=VALUE=")
                    .nospace([])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["action", "--multiparts-nested=VALUE=two,DIRECTORY="],
            )
            .expect(
                Action::values(["dirA/", "dirB/"])
                    .tag("directories")
                    .style_f(for_path())
                    .prefix("--multiparts-nested=VALUE=two,DIRECTORY=")
                    .nospace([])
                    .usage("ActionMultiParts(...ActionMultiParts...)"),
            );
    }

    #[test]
    fn completes_multipartsn() {
        let sandbox = Sandbox::new("example-action-multipartsn");
        sandbox
            .run(cmd::root_cmd, &["action", "--multipartsn", ""])
            .expect(
                Action::values(["one", "two"])
                    .suffix("=")
                    .nospace(['='])
                    .usage("ActionMultiPartsN()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multipartsn", "o"])
            .expect(
                Action::values(["one"])
                    .suffix("=")
                    .nospace(['='])
                    .usage("ActionMultiPartsN()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multipartsn", "one="])
            .expect(
                Action::values(["three", "four"])
                    .prefix("one=")
                    .suffix("=")
                    .nospace(['='])
                    .usage("ActionMultiPartsN()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--multipartsn", "one=three="])
            .expect(
                Action::values(["five", "six"])
                    .prefix("one=three=")
                    .nospace(['='])
                    .usage("ActionMultiPartsN()"),
            );
    }

    #[cfg(unix)]
    #[test]
    fn completes_symlinks() {
        let sandbox = Sandbox::new("example-action-symlink");
        sandbox.files(FILES);
        let dir = sandbox.work_dir();
        std::os::unix::fs::symlink(dir.join("dirA"), dir.join("symA")).unwrap();
        std::os::unix::fs::symlink(dir.join("missing"), dir.join("symB")).unwrap();

        sandbox
            .run(cmd::root_cmd, &["action", "--directories", ""])
            .expect(
                Action::values(["dirA/", "dirB/", "symA/"])
                    .tag("directories")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionDirectories()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--files", "symA/"])
            .expect(
                Action::values(["file1.txt", "file2.png"])
                    .prefix("symA/")
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionFiles()"),
            );
        sandbox
            .run(cmd::root_cmd, &["action", "--files", "s"])
            .expect(
                Action::values(["symA/", "symB"])
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ActionFiles()"),
            );
    }
}

mod modifier {
    use super::*;
    use carapace_rs::shell;

    fn for_path() -> impl Fn(&carapace_rs::Context, &carapace_rs::RawValue) -> String + Copy {
        |context, value| shell::for_path(context, &value.value)
    }

    fn for_path_ext() -> impl Fn(&carapace_rs::Context, &carapace_rs::RawValue) -> String + Copy {
        |context, value| shell::for_path_ext(context, &value.value)
    }

    #[test]
    fn batch_merges_descriptions() {
        let sandbox = Sandbox::new("example-modifier-batch");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--batch", ""])
            .expect(
                Action::values_described([
                    ("A", "description of A"),
                    ("B", "description of second B"),
                    ("C", "description of second C"),
                    ("D", "description of D"),
                ])
                .usage("Batch()"),
            );
    }

    #[test]
    fn cache_roundtrips_until_cleared() {
        let sandbox = Sandbox::new("example-modifier-cache");
        let cached = sandbox
            .run(cmd::root_cmd, &["modifier", "--cache", ""])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--cache", ""])
            .expect_output(&cached);

        sandbox.clear_cache();
        assert_ne!(
            sandbox
                .run(cmd::root_cmd, &["modifier", "--cache", ""])
                .output(),
            cached,
            "cache must revalidate after clear_cache"
        );
    }

    #[test]
    fn filter_and_retain() {
        let sandbox = Sandbox::new("example-modifier-filter");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--filter", ""])
            .expect(Action::values_described([("1", "one"), ("3", "three")]).usage("Filter()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--retain", ""])
            .expect(Action::values_described([("2", "two"), ("4", "four")]).usage("Retain()"));
    }

    #[test]
    fn shift_reports_remaining_args() {
        let sandbox = Sandbox::new("example-modifier-shift");
        sandbox
            .run(cmd::root_cmd, &["modifier", "one", "--shift", ""])
            .expect(Action::message("[]").usage("Shift()"));
        sandbox
            .run(
                cmd::root_cmd,
                &["modifier", "one", "two", "three", "--shift", ""],
            )
            .expect(Action::message(r#"["two", "three"]"#).usage("Shift()"));
    }

    #[test]
    fn timeout_falls_back() {
        let sandbox = Sandbox::new("example-modifier-timeout");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--timeout", ""])
            .expect(Action::message("timeout exceeded").usage("Timeout()"));
    }

    #[test]
    fn usage_is_surfaced() {
        let sandbox = Sandbox::new("example-modifier-usage");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--usage", ""])
            .expect(Action::values(Vec::<String>::new()).usage("explicit usage"));
    }

    #[test]
    fn chdir_completes_relative() {
        // Go TestChdir: a standalone action completing relative to the
        // chdir'ed directory
        let sandbox = Sandbox::new("example-modifier-chdir");
        sandbox.files([("subdir/file1.txt", "")]);
        sandbox
            .run(
                || {
                    use carapace_rs_clap::Carapace;
                    clap::Command::new("chdirapp").positional_any_completion(
                        Action::files(Vec::<String>::new()).chdir("subdir"),
                    )
                },
                &[""],
            )
            .expect(
                Action::values(["file1.txt"])
                    .tag("files")
                    .style_f(|context, value| {
                        shell::for_path(context, &format!("subdir/{}", value.value))
                    })
                    .nospace(['/']),
            );
    }

    #[test]
    fn chdirf_resolves_git_work_tree() {
        let mut sandbox = Sandbox::new("example-modifier-chdirf");
        sandbox.files([
            (".git/config", ""),
            ("file1.txt", ""),
            ("subdirA/file2.txt", ""),
            ("subdirB/file3.txt", ""),
        ]);
        sandbox.env("LS_COLORS", "");

        sandbox
            .run(cmd::root_cmd, &["modifier", "--chdirf", ""])
            .expect(
                Action::values(["subdirA/", "subdirB/", "file1.txt"])
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ChdirF()"),
            );

        sandbox.env("GIT_WORK_TREE", "subdirB/");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--chdirf", ""])
            .expect(
                Action::values(["file3.txt"])
                    .tag("files")
                    .style_f(for_path())
                    .nospace(['/'])
                    .usage("ChdirF()"),
            );
    }

    #[test]
    fn multiparts_segments() {
        let sandbox = Sandbox::new("example-modifier-multiparts");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multiparts", ""])
            .expect(
                Action::values(["dir/"])
                    .nospace(['/'])
                    .usage("MultiParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multiparts", "dir/"])
            .expect(
                Action::values(["subdir1/", "subdir2/"])
                    .prefix("dir/")
                    .nospace(['/'])
                    .usage("MultiParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multiparts", "dir/subdir1/"])
            .expect(
                Action::values(["fileA.txt", "fileB.txt"])
                    .prefix("dir/subdir1/")
                    .nospace(['/'])
                    .usage("MultiParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multiparts", "dir/subdir2/"])
            .expect(
                Action::values(["fileC.txt"])
                    .prefix("dir/subdir2/")
                    .nospace(['/'])
                    .usage("MultiParts()"),
            );
    }

    #[test]
    fn noprefix_and_prefix() {
        let mut sandbox = Sandbox::new("example-modifier-prefix");
        sandbox.files([("subdir/file1.txt", "")]);
        sandbox.env("LS_COLORS", "");

        sandbox
            .run(cmd::root_cmd, &["modifier", "--noprefix", ""])
            .expect(
                Action::values(["--", "---", "one--", "one---"])
                    .no_prefix(['-'])
                    .usage("NoPrefix()"),
            );

        sandbox
            .run(cmd::root_cmd, &["modifier", "--prefix", ""])
            .expect(
                Action::values(["subdir/"])
                    .style_f(for_path())
                    .prefix("file://")
                    .nospace(['/'])
                    .usage("Prefix()")
                    .tag("files"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--prefix", "file"])
            .expect(
                Action::values(["subdir/"])
                    .style_f(for_path())
                    .prefix("file://")
                    .nospace(['/'])
                    .usage("Prefix()")
                    .tag("files"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--prefix", "file://subdir/f"])
            .expect(
                Action::values(["file1.txt"])
                    .style_f(for_path())
                    .prefix("file://subdir/")
                    .nospace(['/'])
                    .usage("Prefix()")
                    .tag("files"),
            );
    }

    #[test]
    fn filterargs_and_filterparts() {
        let sandbox = Sandbox::new("example-modifier-filterargs");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--filterargs", ""])
            .expect(Action::values(["one", "two", "three"]).usage("FilterArgs()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "one", "--filterargs", ""])
            .expect(Action::values(["two", "three"]).usage("FilterArgs()"));
        sandbox
            .run(
                cmd::root_cmd,
                &["modifier", "one", "three", "--filterargs", ""],
            )
            .expect(Action::values(["two"]).usage("FilterArgs()"));

        sandbox
            .run(cmd::root_cmd, &["modifier", "--filterparts", ""])
            .expect(
                Action::values(["one", "two", "three"])
                    .suffix(",")
                    .nospace([','])
                    .usage("FilterParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--filterparts", "one,"])
            .expect(
                Action::values(["two", "three"])
                    .prefix("one,")
                    .suffix(",")
                    .nospace([','])
                    .usage("FilterParts()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--filterparts", "one,three,"])
            .expect(
                Action::values(["two"])
                    .prefix("one,three,")
                    .suffix(",")
                    .nospace([','])
                    .usage("FilterParts()"),
            );
    }

    #[test]
    fn multipartsp_placeholders() {
        let sandbox = Sandbox::new("example-modifier-multipartsp");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multipartsp", ""])
            .expect(
                Action::styled_values_described([
                    ("keys/", "", ""),
                    ("styles", "list", "yellow"),
                    ("styles/", "", ""),
                ])
                .nospace(['/'])
                .usage("MultiPartsP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multipartsp", "keys/"])
            .expect(
                Action::values(["key1", "key1/", "key2", "key2/"])
                    .prefix("keys/")
                    .nospace(['/'])
                    .usage("MultiPartsP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multipartsp", "keys/key1/"])
            .expect(
                Action::values(["val1", "val2"])
                    .prefix("keys/key1/")
                    .nospace(['/'])
                    .usage("MultiPartsP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multipartsp", "keys/key2/"])
            .expect(
                Action::values(["val3", "val4"])
                    .prefix("keys/key2/")
                    .nospace(['/'])
                    .usage("MultiPartsP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--multipartsp", "styles/c"])
            .expect(
                carapace_rs::batch(vec![
                    Action::styled_values([("color", ""), ("cyan", "cyan")]).tag("styles"),
                    Action::styled_values_described([("custom", "custom style", "blue blink")]),
                ])
                .prefix("styles/")
                .nospace(['/', 'r'])
                .usage("MultiPartsP()"),
            );
    }

    #[test]
    fn split_relexes_the_value() {
        let mut sandbox = Sandbox::new("example-modifier-split");
        sandbox.files([("subdir/file1.txt", "")]);
        sandbox.env("LS_COLORS", "");

        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", ""])
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .suffix(" ")
                    .usage("Split()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", "pos1 "])
            .expect(
                Action::values(["subdir/"])
                    .style_f(for_path_ext())
                    .prefix("pos1 ")
                    .nospace(['*'])
                    .usage("Split()")
                    .tag("files"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", "pos1 --"])
            .expect(
                Action::styled_values_described([
                    ("--bool", "bool flag", ""),
                    ("--string", "string flag", "blue"),
                ])
                .prefix("pos1 ")
                .suffix(" ")
                .nospace(['*'])
                .usage("Split()")
                .tag("longhand flags"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", "pos1 --bool="])
            .expect(
                Action::styled_values([("true", "green"), ("false", "red")])
                    .prefix("pos1 --bool=")
                    .suffix(" ")
                    .nospace(['*'])
                    .usage("bool flag"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", "pos1 \"--bool="])
            .expect(
                Action::styled_values([("true", "green"), ("false", "red")])
                    .prefix("pos1 \"--bool=")
                    .suffix("\" ")
                    .nospace(['*'])
                    .usage("bool flag"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--split", "pos1 '--bool="])
            .expect(
                Action::styled_values([("true", "green"), ("false", "red")])
                    .prefix("pos1 '--bool=")
                    .suffix("' ")
                    .nospace(['*'])
                    .usage("bool flag"),
            );
    }

    #[test]
    fn splitp_supports_pipelines() {
        let mut sandbox = Sandbox::new("example-modifier-splitp");
        sandbox.files([("subdir/file1.txt", "")]);
        sandbox.env("LS_COLORS", "");

        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1>"])
            .expect(
                Action::values(["subdir/"])
                    .nospace(['*'])
                    .style_f(for_path())
                    .prefix("pos1>")
                    .usage("SplitP()")
                    .tag("files"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1>subdir/"])
            .expect(
                Action::values(["file1.txt"])
                    .nospace(['*'])
                    .style_f(for_path())
                    .prefix("pos1>subdir/")
                    .suffix(" ")
                    .usage("SplitP()")
                    .tag("files"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["modifier", "--splitp", "pos1>subdir/file1.txt --b"],
            )
            .expect(
                Action::values_described([("--bool", "bool flag")])
                    .nospace(['*'])
                    .prefix("pos1>subdir/file1.txt ")
                    .suffix(" ")
                    .usage("SplitP()")
                    .tag("longhand flags"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1 1>"])
            .expect(
                Action::values(["subdir/"])
                    .nospace(['*'])
                    .style_f(for_path())
                    .prefix("pos1 1>")
                    .usage("SplitP()")
                    .tag("files"),
            );
        sandbox
            .run(
                cmd::root_cmd,
                &["modifier", "--splitp", "<> subdir/file1.txt "],
            )
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .prefix("<> subdir/file1.txt ")
                    .suffix(" ")
                    .usage("SplitP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1|"])
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .prefix("pos1|")
                    .suffix(" ")
                    .usage("SplitP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1|&"])
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .prefix("pos1|&")
                    .suffix(" ")
                    .usage("SplitP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1 ;"])
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .prefix("pos1 ;")
                    .suffix(" ")
                    .usage("SplitP()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--splitp", "pos1 | "])
            .expect(
                Action::values(["pos1", "positional1"])
                    .nospace(['*'])
                    .prefix("pos1 | ")
                    .suffix(" ")
                    .usage("SplitP()"),
            );
    }

    #[test]
    fn unless_conditions() {
        let sandbox = Sandbox::new("example-modifier-unless");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unless", "true:"])
            .expect(
                Action::values(["yes", "positive"])
                    .prefix("true:")
                    .nospace([':'])
                    .usage("Unless()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unless", "false:"])
            .expect(
                Action::values(["no", "negative"])
                    .prefix("false:")
                    .nospace([':'])
                    .usage("Unless()"),
            );
    }

    #[test]
    fn unlessf_completing_path() {
        let sandbox = Sandbox::new("example-modifier-unlessf");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unlessf", ""])
            .expect(
                Action::values(["./local", "~/home", "/abs", "one", "two", "three"])
                    .usage("UnlessF()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unlessf", "t"])
            .expect(Action::values(["two", "three"]).usage("UnlessF()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unlessf", "."])
            .expect(Action::values(Vec::<String>::new()).usage("UnlessF()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unlessf", "~"])
            .expect(Action::values(Vec::<String>::new()).usage("UnlessF()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unlessf", "/"])
            .expect(Action::values(Vec::<String>::new()).usage("UnlessF()"));
    }

    #[test]
    fn unique_variants() {
        let sandbox = Sandbox::new("example-modifier-unique");
        sandbox
            .run(cmd::root_cmd, &["modifier", "--unique", ""])
            .expect(Action::values(["one", "two", "three"]).usage("Unique()"));
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelist", ""])
            .expect(
                Action::values(["one", "two", "three"])
                    .nospace([])
                    .usage("UniqueList()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelist", "two,"])
            .expect(
                Action::values(["one", "three"])
                    .prefix("two,")
                    .nospace([])
                    .usage("UniqueList()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelistf", ""])
            .expect(
                Action::values(["one", "two", "three"])
                    .nospace([])
                    .usage("UniqueListF()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelistf", "two,"])
            .expect(
                Action::values(["one", "three"])
                    .prefix("two,")
                    .nospace([])
                    .usage("UniqueListF()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelistf", "two:"])
            .expect(
                Action::values(["1", "2", "3"])
                    .prefix("two:")
                    .nospace([])
                    .usage("UniqueListF()"),
            );
        sandbox
            .run(cmd::root_cmd, &["modifier", "--uniquelistf", "two:1,"])
            .expect(
                Action::values(["one", "three"])
                    .prefix("two:1,")
                    .nospace([])
                    .usage("UniqueListF()"),
            );
    }
}

mod multiparts {
    use super::*;
    use carapace_rs::shell;

    fn for_path() -> impl Fn(&carapace_rs::Context, &carapace_rs::RawValue) -> String + Copy {
        |context, value| shell::for_path(context, &value.value)
    }

    const FILES: [(&str, &str); 5] = [
        ("dirA/file1.txt", ""),
        ("dirA/file2.png", ""),
        ("dirB/dirC/file3.go", ""),
        ("dirB/file4.md", ""),
        ("file5.go", ""),
    ];

    #[test]
    fn completes_positional_entries() {
        let sandbox = Sandbox::new("example-multiparts");
        sandbox.files(FILES);

        sandbox.run(cmd::root_cmd, &["multiparts", ""]).expect(
            Action::values(["DIRECTORY", "FILE", "VALUE"])
                .suffix("=")
                .nospace([',', '=']),
        );
        sandbox.run(cmd::root_cmd, &["multiparts", "D"]).expect(
            Action::values(["DIRECTORY"])
                .suffix("=")
                .nospace([',', '=']),
        );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "DIRECTORY"])
            .expect(
                Action::values(["DIRECTORY"])
                    .suffix("=")
                    .nospace([',', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "DIRECTORY="])
            .expect(
                Action::values(["dirA/", "dirB/"])
                    .tag("directories")
                    .style_f(for_path())
                    .prefix("DIRECTORY=")
                    .nospace([',', '/', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE="])
            .expect(
                Action::values(["one", "two", "three"])
                    .prefix("VALUE=")
                    .nospace([',', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE=o"])
            .expect(Action::values(["one"]).prefix("VALUE=").nospace([',', '=']));
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE=one,"])
            .expect(
                Action::values(["DIRECTORY", "FILE"])
                    .prefix("VALUE=one,")
                    .suffix("=")
                    .nospace([',', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE=one,F"])
            .expect(
                Action::values(["FILE"])
                    .prefix("VALUE=one,")
                    .suffix("=")
                    .nospace([',', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE=one,FILE="])
            .expect(
                Action::values(["dirA/", "dirB/", "file5.go"])
                    .tag("files")
                    .style_f(for_path())
                    .prefix("VALUE=one,FILE=")
                    .nospace([',', '/', '=']),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "VALUE=one,FILE=dirB/"])
            .expect(
                Action::values(["dirC/", "file4.md"])
                    .tag("files")
                    .prefix("dirB/")
                    .style_f(for_path())
                    .prefix("VALUE=one,FILE=")
                    .nospace([',', '/', '=']),
            );
    }

    #[test]
    fn completes_none_divider_variants() {
        let sandbox = Sandbox::new("example-multiparts-none");
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-zero", ""])
            .expect(
                Action::message("invalid value for n [ActionMultiParts]: 0")
                    .usage("multiparts without divider limited to 0"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-one", ""])
            .expect(Action::values(["a", "b"]).usage("multiparts without divider limited to 1"));
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-one", "a"])
            .expect(Action::values(["a"]).usage("multiparts without divider limited to 1"));
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-two", ""])
            .expect(
                Action::values_described([("a", "zero"), ("b", "zero")])
                    .nospace([])
                    .style("blue")
                    .usage("multiparts without divider limited to 2"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-two", "a"])
            .expect(
                Action::values_described([("a", "default"), ("b", "default"), ("c", "default")])
                    .prefix("a")
                    .nospace([])
                    .style("red")
                    .usage("multiparts without divider limited to 2"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-two", "ab"])
            .expect(
                Action::values_described([("a", "default"), ("c", "default")])
                    .prefix("ab")
                    .nospace([])
                    .style("red")
                    .usage("multiparts without divider limited to 2"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-two", "abc"])
            .expect(
                Action::values_described([("a", "default")])
                    .prefix("abc")
                    .nospace([])
                    .style("red")
                    .usage("multiparts without divider limited to 2"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-three", "a"])
            .expect(
                Action::values_described([("a", "one"), ("b", "one"), ("c", "one")])
                    .prefix("a")
                    .nospace([])
                    .style("red")
                    .usage("multiparts without divider limited to 3"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-three", "ab"])
            .expect(
                Action::values_described([
                    ("a", "default"),
                    ("b", "default"),
                    ("c", "default"),
                    ("d", "default"),
                ])
                .prefix("ab")
                .nospace([])
                .style("green")
                .usage("multiparts without divider limited to 3"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-three", "abc"])
            .expect(
                Action::values_described([("a", "default"), ("b", "default"), ("d", "default")])
                    .prefix("abc")
                    .nospace([])
                    .style("green")
                    .usage("multiparts without divider limited to 3"),
            );
        sandbox
            .run(cmd::root_cmd, &["multiparts", "--none-three", "abcd"])
            .expect(
                Action::values_described([("a", "default"), ("b", "default")])
                    .prefix("abcd")
                    .nospace([])
                    .style("green")
                    .usage("multiparts without divider limited to 3"),
            );
    }
}

#[test]
fn pre_invoke_chdirs_all_actions() {
    let sandbox = Sandbox::new("example-preinvoke");
    sandbox.files([("dirB/file4.md", ""), ("dirB/dirC/file3.go", "")]);

    // the root's `--chdir` value redirects every action of the tree
    sandbox
        .run(cmd::root_cmd, &["action", "--chdir", "dirB", "--files", ""])
        .expect(
            Action::values(["dirC/", "file4.md"])
                .tag("files")
                .style_f(|context, value| {
                    carapace_rs::shell::for_path(context, &format!("dirB/{}", value.value))
                })
                .nospace(['/'])
                .usage("ActionFiles()"),
        );
    sandbox
        .run(
            cmd::root_cmd,
            &["action", "--chdir", "dirB", "--directories", ""],
        )
        .expect(
            Action::values(["dirC/"])
                .tag("directories")
                .style_f(|context, value| {
                    carapace_rs::shell::for_path(context, &format!("dirB/{}", value.value))
                })
                .nospace(['/'])
                .usage("ActionDirectories()"),
        );
}
