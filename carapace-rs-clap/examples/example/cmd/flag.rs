//! `flag` command: every clap flag shape as a flag (Go `example/cmd/flag.go`).
//!
//! The Go version lists every pflag type, but completion only observes
//! flag *shapes* and pflag has no shape pflag doesn't share, so the flags
//! cover clap's own flag surface instead: valueless booleans (`SetTrue`)
//! and counting (`Count`), optional values (`num_args(0..=1)`), single
//! values (`Set`), repeatable single values (`Append` with
//! `num_args(1)`), delimited multi-value (`value_delimiter`) and
//! variadic multi-value without delimiter. Value parsers are invisible
//! to completion and typed flags are covered by `tests/convert.rs`
//! (possible values as fallback completion), so none are used here.

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_help;

pub fn cmd() -> Command {
    let mut cmd = with_help(
        Command::new("flag")
            .about("flag example")
            .completion_group("main"),
    );

    cmd = cmd.args([
        Arg::new("bool")
            .long("bool")
            .action(ArgAction::SetTrue)
            .help("valueless boolean"),
        Arg::new("count")
            .long("count")
            .action(ArgAction::Count)
            .help("repeatable counter"),
        Arg::new("optarg")
            .long("optarg")
            .action(ArgAction::Set)
            .num_args(0..=1)
            .help("optional value"),
        Arg::new("string")
            .long("string")
            .action(ArgAction::Set)
            .help("single value"),
        Arg::new("repeat")
            .long("repeat")
            .action(ArgAction::Append)
            .num_args(1)
            .help("repeatable single value"),
        Arg::new("slice")
            .long("slice")
            .action(ArgAction::Append)
            .num_args(1..)
            .value_delimiter(',')
            .help("delimited multi-value"),
        Arg::new("array")
            .long("array")
            .action(ArgAction::Append)
            .num_args(1..)
            .help("variadic multi-value"),
    ]);

    cmd = cmd
        .flag_completion([
            ("bool", Action::values(["true", "false"])),
            ("optarg", Action::values(["o1", "o2", "o3"])),
            ("string", Action::values(["s1", "s2", "s3"])),
            ("repeat", Action::values(["r1", "r2", "r3"])),
            ("slice", Action::values(["v1", "v2", "v3"])),
            ("array", Action::values(["a1", "a2", "a3"])),
        ])
        .dash_completion([
            Action::values(["d1", "dash1", "-d1", "--dash1"]),
            Action::values(["d2", "dash2", "-d2", "--dash2"]),
        ])
        .dash_any_completion(Action::values(["dAny", "dashAny", "-dAny", "--dashAny"]));

    cmd
}
