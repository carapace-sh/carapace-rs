//! `multiparts` command: multipart completion with various dividers (Go
//! `example/cmd/multiparts.go`).

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("multiparts").about("multiparts example"))
        .arg(
            Arg::new("at")
                .long("at")
                .action(ArgAction::Set)
                .help("multiparts with @ as divider"),
        )
        .arg(
            Arg::new("colon")
                .long("colon")
                .action(ArgAction::Set)
                .help("multiparts with : as divider"),
        )
        .arg(
            Arg::new("comma")
                .long("comma")
                .action(ArgAction::Set)
                .help("multiparts with , as divider"),
        )
        .arg(
            Arg::new("dot")
                .long("dot")
                .action(ArgAction::Set)
                .help("multiparts with . as divider"),
        )
        .arg(
            Arg::new("dotdotdot")
                .long("dotdotdot")
                .action(ArgAction::Set)
                .help("multiparts with ... as divider"),
        )
        .arg(
            Arg::new("equals")
                .long("equals")
                .action(ArgAction::Set)
                .help("multiparts with = as divider"),
        )
        .arg(
            Arg::new("none")
                .long("none")
                .action(ArgAction::Set)
                .help("multiparts without divider"),
        )
        .arg(
            Arg::new("none-zero")
                .long("none-zero")
                .action(ArgAction::Set)
                .help("multiparts without divider limited to 0"),
        )
        .arg(
            Arg::new("none-one")
                .long("none-one")
                .action(ArgAction::Set)
                .help("multiparts without divider limited to 1"),
        )
        .arg(
            Arg::new("none-two")
                .long("none-two")
                .action(ArgAction::Set)
                .help("multiparts without divider limited to 2"),
        )
        .arg(
            Arg::new("none-three")
                .long("none-three")
                .action(ArgAction::Set)
                .help("multiparts without divider limited to 3"),
        )
        .arg(
            Arg::new("slash")
                .long("slash")
                .action(ArgAction::Set)
                .help("multiparts with / as divider"),
        )
        .arg(
            Arg::new("space")
                .long("space")
                .action(ArgAction::Set)
                .help("multiparts with space as divider"),
        )
        .flag_completion([
            ("at", multiparts_test("@")),
            ("colon", multiparts_test(":")),
            ("comma", multiparts_test(",")),
            ("dot", multiparts_test(".")),
            ("dotdotdot", multiparts_test("...")),
            ("equals", multiparts_test("=")),
            (
                "none",
                Action::values_described([
                    ("a", "first"),
                    ("b", "second"),
                    ("c", "third"),
                    ("d", "fourth"),
                ])
                .unique_list(""),
            ),
            (
                "none-zero",
                Action::multiparts_n("", 0, |_| Action::message("unreachable")),
            ),
            (
                "none-one",
                Action::multiparts_n("", 1, |_| Action::values(["a", "b"])),
            ),
            (
                "none-two",
                Action::multiparts_n("", 2, |c| match c.parts.len() {
                    0 => Action::values_described([("a", "zero"), ("b", "zero")]).style("blue"),
                    _ => Action::values_described([
                        ("a", "default"),
                        ("b", "default"),
                        ("c", "default"),
                    ])
                    .style("red")
                    .unique_list(""),
                }),
            ),
            (
                "none-three",
                Action::multiparts_n("", 3, |c| match c.parts.len() {
                    0 => Action::values_described([("a", "zero"), ("b", "zero")]).style("blue"),
                    1 => Action::values_described([("a", "one"), ("b", "one"), ("c", "one")])
                        .style("red"),
                    _ => Action::values_described([
                        ("a", "default"),
                        ("b", "default"),
                        ("c", "default"),
                        ("d", "default"),
                    ])
                    .style("green")
                    .unique_list(""),
                }),
            ),
            ("slash", multiparts_test("/")),
            (
                "space",
                Action::values(["one", "two", "three", "four"]).unique_list(" "),
            ),
        ])
        .positional_completion([multiparts_positional()])
}

fn multiparts_test(divider: &'static str) -> Action {
    Action::multiparts(divider, move |c| match c.parts.len() {
        0 => test_values().suffix(divider),
        1 => test_values().filter_parts().suffix(divider),
        2 => test_values().filter_parts(),
        _ => Action::values(Vec::<String>::new()),
    })
}

fn test_values() -> Action {
    Action::values_described([
        ("first", "first value"),
        ("second", "second value"),
        ("third with space", "third value"),
        ("fourth", "fourth value"),
    ])
}

fn multiparts_positional() -> Action {
    Action::multiparts(",", |c_entries| {
        let outer_parts = c_entries.parts.clone();
        Action::multiparts("=", move |c| match c.parts.len() {
            0 => {
                let keys: Vec<String> = outer_parts
                    .iter()
                    .map(|entry| entry.split('=').next().unwrap_or_default().to_owned())
                    .collect();
                Action::values(["FILE", "DIRECTORY", "VALUE"])
                    .filter(keys)
                    .suffix("=")
            }
            1 => match c.parts[0].as_str() {
                "FILE" => Action::files(Vec::<String>::new()),
                "DIRECTORY" => Action::directories(),
                "VALUE" => Action::values(["one", "two", "three"]),
                _ => Action::values(Vec::<String>::new()),
            },
            _ => Action::values(Vec::<String>::new()),
        })
    })
}
