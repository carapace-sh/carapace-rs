//! `disabled` command: flag parsing disabled (Go `example/cmd/flag_disabled.go`).

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::Command;

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("disabled").about("flag parsing disabled"))
        .disable_flag_parsing()
        .positional_completion([
            Action::values(["-p1", "positional1"]),
            Action::values(["p2", "--positional2"]),
        ])
}
