//! `group` command: subcommand groups (Go `example/cmd/group.go`).

use carapace_rs_clap::Carapace;
use clap::Command;

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("group").about("group example"))
        .subcommand(Command::new("sub1").completion_group("main"))
        .subcommand(Command::new("sub2").completion_group("main"))
        .subcommand(Command::new("sub3").completion_group("setup"))
        .subcommand(Command::new("sub4").completion_group("setup"))
        .subcommand(Command::new("sub5"))
}
