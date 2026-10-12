//! `subcommand` command tree: aliases, groups, hidden commands (Go
//! `example/cmd/subcommand*.go`).

use carapace_rs_clap::Carapace;
use clap::Command;

use super::with_hidden_help;

pub fn cmd() -> Command {
    with_hidden_help(Command::new("subcommand").about("subcommand example"))
        .subcommand(with_hidden_help(
            Command::new("alias")
                .about("subcommand with alias")
                .alias("a1")
                .alias("a2"),
        ))
        .subcommand(with_hidden_help(
            Command::new("group")
                .about("subcommand with group")
                .completion_group("group"),
        ))
        .subcommand(
            with_hidden_help(Command::new("hidden").about("hidden subcommand").hide(true))
                .subcommand(with_hidden_help(
                    Command::new("visible").about("visible subcommand of a hidden command"),
                )),
        )
}
