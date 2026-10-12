//! `special` command: special characters and mutually exclusive flags (Go
//! `example/cmd/special.go`).

use carapace_rs::Action;
use carapace_rs_clap::ArgConflicts;
use carapace_rs_clap::Carapace;
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_hidden_help;

/// The special-character string the positional completes.
pub const SPECIAL: &str = "p1 & < > ' \" { } $ # | ? ( ) ;  [ ] * \\ $() ${} ` ``";

pub fn cmd() -> Command {
    with_hidden_help(Command::new("special"))
        .arg(
            Arg::new("count")
                .short('c')
                .long("count")
                .action(ArgAction::Count)
                .help("count flag"),
        )
        .arg(
            Arg::new("exclusive")
                .long("exclusive")
                .action(ArgAction::SetTrue)
                .help("mutually exclusive flag")
                .add(ArgConflicts(vec!["exclusiveRepeatable".to_owned()])),
        )
        .arg(
            Arg::new("exclusiveRepeatable")
                .long("exclusiveRepeatable")
                .action(ArgAction::Count)
                .help("mutually exclusive repeatable flag")
                .add(ArgConflicts(vec!["exclusive".to_owned()])),
        )
        .arg(
            Arg::new("optarg")
                .short('o')
                .long("optarg")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .default_missing_value(" ")
                .help("optional argument"),
        )
        .flag_completion([("optarg", Action::values(["optarg1", "optarg2", "optarg3"]))])
        .positional_completion([Action::values([SPECIAL])])
}
