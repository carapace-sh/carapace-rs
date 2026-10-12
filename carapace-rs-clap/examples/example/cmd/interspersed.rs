//! `interspersed` command: no flag/positional interspersing (Go
//! `example/cmd/interspersed.go`).

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("interspersed").about("interspersed example"))
        .arg(
            Arg::new("bool")
                .short('b')
                .long("bool")
                .action(ArgAction::SetTrue)
                .help("bool flag"),
        )
        .arg(
            Arg::new("string")
                .short('s')
                .long("string")
                .action(ArgAction::Set)
                .help("string flag"),
        )
        // `SetInterspersed(false)`: the trailing positional disables
        // interspersed flag parsing for the command
        .arg(
            Arg::new("pos2")
                .help("positional2")
                .index(2)
                .num_args(1..)
                .trailing_var_arg(true),
        )
        .arg(Arg::new("pos1").help("positional1").index(1))
        .positional_completion([
            Action::values(["p1", "positional1"]),
            Action::values(["p2", "positional2"]),
        ])
        .dash_completion([
            Action::values_described([("d1", "dash1")]),
            Action::values_described([("d2", "dash2")]),
        ])
}
