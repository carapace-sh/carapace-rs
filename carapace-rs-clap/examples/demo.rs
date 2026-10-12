//! Demo CLI: `cargo run -p carapace-rs-clap --example demo -- _carapace bash demo ...`
//!
//! The word list after the shell mirrors the generated snippets: command
//! name first, cursor word last.
//!
//! Register completions with (bash): `source <(cargo run ... -- _carapace bash)`.

use carapace_rs::Action;
use carapace_rs_clap::Carapace;
use clap::builder::{ArgAction, ValueHint};
use clap::{Arg, Command};

fn cli() -> Command {
    let cmd = Command::new("demo")
        .about("carapace-rs-clap demo")
        .arg(
            Arg::new("profile")
                .short('p')
                .long("profile")
                .action(ArgAction::Set)
                .help("deployment profile"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .action(ArgAction::Set)
                .value_hint(ValueHint::AnyPath)
                .help("output path"),
        )
        .arg(Arg::new("target").index(1).help("build target"))
        .arg(
            Arg::new("rest")
                .num_args(0..)
                .index(2)
                .trailing_var_arg(true),
        )
        .subcommand(
            Command::new("build").about("build things").arg(
                Arg::new("jobs")
                    .short('j')
                    .long("jobs")
                    .action(ArgAction::Set),
            ),
        )
        .subcommand(Command::new("test").about("run tests"));

    cmd.flag_completion([("profile", Action::values(["dev", "staging", "prod"]))])
        .positional_completion([Action::values(["x86", "arm", "wasm"])])
        .positional_any_completion(Action::files(Vec::<String>::new()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(output) = carapace_rs_clap::complete(cli, &args) {
        println!("{output}");
        return;
    }
    let matches = cli().get_matches();
    if let Some(profile) = matches.get_one::<String>("profile") {
        println!("profile: {profile}");
    }
    if let Some((subcommand, _)) = matches.subcommand() {
        println!("subcommand: {subcommand}");
    }
}
