//! `chain` command: shorthand chains through a disabled-parse command
//! (Go `example/cmd/chain.go`).

use carapace_rs::Action;
use carapace_rs_clap::{Carapace, execute_embedded};
use clap::builder::ArgAction;
use clap::{Arg, Command};

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("chain").about("shorthand chain"))
        .disable_flag_parsing()
        .positional_any_completion(Action::callback(|_| {
            execute_embedded(
                with_hidden_help(Command::new("chain-embedded"))
                    .arg(
                        Arg::new("count")
                            .short('c')
                            .long("count")
                            .action(ArgAction::Count),
                    )
                    .arg(
                        Arg::new("bool")
                            .short('b')
                            .long("bool")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("value")
                            .short('v')
                            .long("value")
                            .action(ArgAction::Set),
                    )
                    .arg(
                        Arg::new("optarg")
                            .short('o')
                            .long("optarg")
                            .action(ArgAction::Set)
                            .num_args(0..=1)
                            .default_missing_value(" "),
                    )
                    .flag_completion([
                        ("value", Action::values(["val1", "val2"])),
                        ("optarg", Action::values(["opt1", "opt2"])),
                    ])
                    .positional_completion([Action::values(["p1", "positional1"])]),
            )
        }))
}
