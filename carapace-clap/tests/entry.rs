use carapace_core::shell::Shell;

fn factory() -> clap::Command {
    use clap::builder::ArgAction;
    use clap::{Arg, Command};
    Command::new("myapp")
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .arg(Arg::new("target").index(1))
}

#[test]
fn completes_through_bash_format() {
    let output = carapace_clap::complete_output(
        factory,
        Shell::Bash,
        &["--profile".to_string(), "".to_string()],
    );
    // no completers attached: flag names are not completed when not at flag
    // position; --profile is followed by empty value but profile has no
    // completion -> message-free empty output
    assert!(output.ends_with("\u{1}") || output.contains('\u{1}'));
}

#[test]
fn bash_flag_name_completion_output() {
    let output = carapace_clap::complete_output(factory, Shell::Bash, &["--".to_string()]);
    let (_nospace, data) = output.split_once('\u{1}').unwrap();
    let values: Vec<&str> = data.lines().collect();
    assert_eq!(values, vec!["--help", "--profile"]);
}

#[test]
fn export_output_is_json() {
    let output = carapace_clap::complete_output(factory, Shell::Export, &["--".to_string()]);
    let parsed = carapace_core::export::Export::from_json(&output).unwrap();
    assert_eq!(parsed.values[1].value, "--profile");
}

#[test]
fn complete_dispatches_on_carapace_arg() {
    let args = vec![
        "_carapace".to_string(),
        "bash".to_string(),
        "--".to_string(),
    ];
    assert!(carapace_clap::complete(factory, &args).is_some());
    assert!(carapace_clap::complete(factory, &["run".to_string()]).is_none());
}

#[test]
fn snippet_contains_registration() {
    let snippet = carapace_clap::snippet(factory, Shell::Bash);
    assert!(snippet.contains("complete -o noquote -F _myapp_completion myapp"));
    assert!(snippet.contains("xargs myapp _carapace bash"));
}
