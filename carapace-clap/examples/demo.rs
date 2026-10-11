//! Demo CLI: `cargo run -p carapace-clap --example demo -- _carapace bash ...`
//!
//! Register completions with (bash): `source <(cargo run ... -- _carapace bash)`.

use carapace_clap::Carapace;
use carapace_core::Action;
use clap::builder::{ArgAction, ValueHint};
use clap::{Arg, Command};

fn cli() -> Command {
    let cmd = Command::new("demo")
        .about("carapace-clap demo")
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
    if let Some(output) = carapace_clap::complete(cli, &args) {
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
