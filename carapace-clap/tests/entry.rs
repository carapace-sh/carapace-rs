use carapace_core::shell::Shell;
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
