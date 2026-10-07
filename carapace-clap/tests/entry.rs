use carapace_core::shell::Shell;
use clap::builder::ArgAction;
use clap::{Arg, Command};
use carapace_clap::Gen;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes tests that touch COMP_* environment variables.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clear_comp_env() {
    for name in ["COMP_LINE", "COMP_POINT", "COMP_TYPE", "COMP_WORDBREAKS"] {
        unsafe { std::env::remove_var(name) };
    }
}

fn factory() -> clap::Command {
    Command::new("myapp")
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .arg(Arg::new("target").index(1))
}

#[test]
fn completes_through_bash_format() {
    let _guard = env_lock();
    clear_comp_env();
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
    let _guard = env_lock();
    clear_comp_env();
    let output = carapace_clap::complete_output(factory, Shell::Bash, &["--".to_string()]);
    let (_nospace, data) = output.split_once('\u{1}').unwrap();
    let values: Vec<&str> = data.lines().collect();
    assert_eq!(values, vec!["--help", "--profile"]);
}

#[test]
fn export_output_is_json() {
    let _guard = env_lock();
    clear_comp_env();
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

#[test]
fn bash_patch_relexes_compline() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };
    unsafe { std::env::set_var("COMP_LINE", "myapp --profile dev --") };
    unsafe { std::env::set_var("COMP_POINT", "24") };
    unsafe { std::env::set_var("COMP_TYPE", "9") };

    let output = carapace_clap::complete_output(
        factory,
        Shell::Bash,
        &["--profile".to_string(), "dev".to_string(), "--".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };
    unsafe { std::env::remove_var("COMP_TYPE") };

    let (_nospace, data) = output.split_once('\u{1}').unwrap();
    let values: Vec<&str> = data.lines().collect();
    assert!(values.contains(&"--profile"), "re-lexed args drive flag completion: {values:?}");
}

#[test]
fn bash_patch_redirect_completes_files() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };
    unsafe { std::env::set_var("COMP_LINE", "myapp > ") };
    unsafe { std::env::set_var("COMP_POINT", "9") };

    let output = carapace_clap::complete_output(
        factory,
        Shell::Bash,
        &[">".to_string(), "".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    // redirect target: file completion kicks in (no flag names)
    assert!(!output.contains("--profile"), "redirect target must complete files: {output}");
}

#[test]
fn bash_patch_quoted_compline() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };
    unsafe { std::env::set_var("COMP_LINE", "myapp 'a b' --") };
    unsafe { std::env::set_var("COMP_POINT", "15") };

    let output = carapace_clap::complete_output(
        factory,
        Shell::Bash,
        &["'a".to_string(), "b'".to_string(), "--".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    let (_nospace, data) = output.split_once('\u{1}').unwrap();
    assert!(data.lines().any(|line| line.contains("--profile")), "{data}");
}

#[test]
fn carapace_without_shell_prints_detected_snippet() {
    let _guard = env_lock();
    unsafe { std::env::set_var("SHELL", "/usr/bin/zsh") };
    let output = carapace_clap::complete(factory, &["_carapace".to_string()]);
    unsafe { std::env::remove_var("SHELL") };
    let output = output.unwrap();
    assert!(output.contains("compdef _myapp_completion myapp"), "{output}");
}

#[test]
fn carapace_with_shell_prints_snippet() {
    let output = carapace_clap::complete(factory, &["_carapace".to_string(), "fish".to_string()]);
    let output = output.unwrap();
    assert!(output.contains("complete -c 'myapp' -f -a '(_myapp_completion)' -r"));
}

#[test]
fn carapace_unknown_shell_yields_nothing() {
    assert!(carapace_clap::complete(factory, &["_carapace".to_string(), "csh".to_string()]).is_none());
}

#[test]
fn style_set_persists_to_styles_json() {
    let dir = std::env::temp_dir().join(format!("carapace-styleset-{}", std::process::id()));
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };

    let output = carapace_clap::complete(
        factory,
        &[
            "_carapace".to_string(),
            "style".to_string(),
            "set".to_string(),
            "carapace.Error".to_string(),
            "magenta".to_string(),
        ],
    );
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };

    assert_eq!(output.as_deref(), Some(""));
    let content = std::fs::read_to_string(dir.join("carapace/styles.json")).unwrap();
    assert!(content.contains("magenta"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn execute_embedded_completes_nested_tree() {
    use carapace_core::Action;

    let embedded = Command::new("inner")
        .arg(Arg::new("mode").index(1))
        .arg(Arg::new("rest").num_args(0..).index(2));

    // outer command delegates its positionals to the embedded tree
    let outer = Command::new("myapp").arg(Arg::new("rest").num_args(0..).index(1));
    let outer = Gen::new(outer)
        .positional_any_completion(carapace_clap::execute_embedded(embedded))
        .into_command();

    let model = carapace_clap::to_model(&mut outer.clone());
    let completion = carapace_core::traverse(&model, vec!["".to_string()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert!(values.is_empty(), "embedded tree has no completers: {values:?}");

    // with completers attached on the embedded command
    let embedded = Gen::new(
        Command::new("inner").arg(Arg::new("mode").index(1)),
    )
    .positional_completion(1, Action::values(["fast", "slow"]))
    .into_command();

    let outer = Command::new("myapp").arg(Arg::new("rest").num_args(0..).index(1));
    let outer = Gen::new(outer)
        .positional_any_completion(carapace_clap::execute_embedded(embedded))
        .into_command();

    let model = carapace_clap::to_model(&mut outer.clone());
    let completion = carapace_core::traverse(&model, vec!["".to_string()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["fast", "slow"]);
}

#[test]
fn spec_dumps_yaml() {
    let output = carapace_clap::complete(factory, &["_carapace".to_string(), "spec".to_string()])
        .expect("spec is handled");
    assert!(output.starts_with("# yaml-language-server:"), "{output}");
    assert!(output.contains("name: myapp"), "{output}");
    assert!(output.contains("--profile="), "{output}");

    let model = carapace_spec::from_yaml(&output).unwrap();
    assert_eq!(model.name, "myapp");
    assert!(model.flags.iter().any(|flag| flag.name == "profile"));
}
