use carapace_core::Action;
use carapace_rs::flag_completion;
use carapace_testkit::Sandbox;
use clap::{CommandFactory, Parser};

#[derive(Parser)]
#[command(name = "facadeapp", disable_help_subcommand = true)]
struct Cli {
    /// the deployment profile
    #[arg(
        long,
        add = carapace_rs::completer!(carapace_rs::Action::values(["dev", "prod"]))
    )]
    profile: Option<String>,
}

#[test]
fn facade_reexports_wire_completions() {
    let sandbox = Sandbox::new("facade-reexports");
    let run = sandbox.run(Cli::command, &["--profile", ""]);
    run.expect(Action::values(["dev", "prod"]));
}

#[test]
fn facade_reexports_free_functions() {
    let mut cmd = clap::Command::new("facadeapp").arg(clap::Arg::new("profile").long("profile"));
    flag_completion(&mut cmd, [("profile", Action::values(["dev", "prod"]))]);
    assert!(cmd.get_arguments().any(|arg| arg.get_id() == "profile"));
}
