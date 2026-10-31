use carapace_rs::Action;
use carapace_rs_clap::completer;
use carapace_rs_testkit::Sandbox;
use clap::{CommandFactory, Parser};

#[derive(Parser)]
#[command(name = "deriveapp", disable_help_subcommand = true)]
struct Cli {
    /// the deployment profile
    #[arg(
        long,
        add = completer!(Action::values(["dev", "prod"]))
    )]
    profile: Option<String>,
}

#[test]
fn derive_completer_attaches_to_arg() {
    let sandbox = Sandbox::new("clap-derive");
    let run = sandbox.run(Cli::command, &["--profile", ""]);
    run.expect(Action::values(["dev", "prod"]).usage("the deployment profile"));
}
