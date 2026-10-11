use carapace_clap::Carapace;
use carapace_core::Action;
use carapace_core::shell::Shell;
use clap::builder::ArgAction;
use clap::{Arg, Command};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes tests that touch COMP_* environment variables.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    let _guard = env_lock();
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
    assert!(snippet.starts_with("#!/bin/bash"), "{snippet}");
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
    assert!(
        values.contains(&"--profile"),
        "re-lexed args drive flag completion: {values:?}"
    );
}

#[test]
fn bash_patch_redirect_completes_files() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };
    unsafe { std::env::set_var("COMP_LINE", "myapp > ") };
    unsafe { std::env::set_var("COMP_POINT", "9") };

    let output =
        carapace_clap::complete_output(factory, Shell::Bash, &[">".to_string(), "".to_string()]);
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    // redirect target: file completion kicks in (no flag names)
    assert!(
        !output.contains("--profile"),
        "redirect target must complete files: {output}"
    );
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
    assert!(
        data.lines().any(|line| line.contains("--profile")),
        "{data}"
    );
}

#[test]
fn carapace_without_shell_prints_detected_snippet() {
    let _guard = env_lock();
    unsafe { std::env::set_var("SHELL", "/usr/bin/zsh") };
    let output = carapace_clap::complete(factory, &["_carapace".to_string()]);
    unsafe { std::env::remove_var("SHELL") };
    let output = output.unwrap();
    assert!(
        output.contains("compdef _myapp_completion myapp"),
        "{output}"
    );
}

#[test]
fn carapace_with_shell_prints_snippet() {
    let output = carapace_clap::complete(factory, &["_carapace".to_string(), "fish".to_string()]);
    let output = output.unwrap();
    assert!(output.contains("complete -c 'myapp' -f -a '(_myapp_completion)' -r"));
}

#[test]
fn carapace_unknown_shell_yields_nothing() {
    assert!(
        carapace_clap::complete(factory, &["_carapace".to_string(), "csh".to_string()]).is_none()
    );
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
    let outer = Command::new("myapp")
        .arg(Arg::new("rest").num_args(0..).index(1))
        .positional_any_completion(carapace_clap::execute_embedded(embedded));

    let model = carapace_clap::to_model(&mut outer.clone());
    let completion = carapace_core::traverse(&model, vec!["".to_string()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert!(
        values.is_empty(),
        "embedded tree has no completers: {values:?}"
    );

    // with completers attached on the embedded command
    let embedded = Command::new("inner")
        .arg(Arg::new("mode").index(1))
        .positional_completion([Action::values(["fast", "slow"])]);

    let outer = Command::new("myapp")
        .arg(Arg::new("rest").num_args(0..).index(1))
        .positional_any_completion(carapace_clap::execute_embedded(embedded));

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

#[test]
fn bash_list_mode_renders_display_with_description() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp ") };
    unsafe { std::env::set_var("COMP_POINT", "6") };
    unsafe { std::env::set_var("COMP_TYPE", "63") };
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };

    let output = carapace_clap::complete_output(
        || {
            Command::new("myapp")
                .arg(Arg::new("target").index(1))
                .positional_completion([Action::values_described([
                    ("alpha", "first"),
                    ("beta", "second"),
                ])])
        },
        Shell::Bash,
        &["".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };
    unsafe { std::env::remove_var("COMP_TYPE") };

    let (nospace, data) = output.split_once('\u{1}').unwrap();
    assert_eq!(nospace, "true", "list mode forces nospace: {output:?}");
    assert_eq!(data, "alpha (first)\nbeta (second)", "{output:?}");
}

#[test]
fn bash_quotes_values_containing_wordbreak_chars() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp ") };
    unsafe { std::env::set_var("COMP_POINT", "6") };
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };

    let output = carapace_clap::complete_output(
        || {
            let cmd = Command::new("myapp").arg(Arg::new("target").index(1));
            cmd.positional_completion([Action::values(["a:b"])])
        },
        Shell::Bash,
        &["".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    assert!(output.contains("\"a:b\""), "{output:?}");
}

#[test]
fn bash_trims_wordbreak_prefix_from_values() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp --profile=de") };
    unsafe { std::env::set_var("COMP_POINT", "18") };
    unsafe { std::env::set_var("COMP_WORDBREAKS", " \"'@><=;|&(:") };

    let output = carapace_clap::complete_output(
        || {
            let cmd = Command::new("myapp")
                .arg(Arg::new("profile").long("profile").action(ArgAction::Set));
            cmd.flag_completion("profile", Action::values(["dev"]))
        },
        Shell::Bash,
        &["--profile=de".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    // the wordbreak prefix is trimmed so bash re-attaches it itself
    assert!(output.contains("dev"), "{output:?}");
    assert!(!output.contains("--profile=dev"), "{output:?}");
}

#[test]
fn bash_completes_inside_open_quote() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp \"ab") };
    unsafe { std::env::set_var("COMP_POINT", "10") };

    // shlex drops the unterminated token; the completion must still see it
    let output = carapace_clap::complete_output(
        || {
            let cmd = Command::new("myapp").arg(Arg::new("target").index(1));
            cmd.positional_completion([Action::values(["abc", "abd"])])
        },
        Shell::Bash,
        &["\"ab".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    assert!(output.contains("abc"), "{output:?}");
}

#[test]
fn bash_completes_after_trailing_escape() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp a\\") };
    unsafe { std::env::set_var("COMP_POINT", "9") };

    let output = carapace_clap::complete_output(
        || {
            let cmd = Command::new("myapp").arg(Arg::new("target").index(1));
            cmd.positional_completion([Action::values(["a1", "a2"])])
        },
        Shell::Bash,
        &["a\\".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    assert!(output.contains("a1"), "{output:?}");
}

#[test]
fn bash_completes_inside_quoted_whitespace() {
    let _guard = env_lock();
    clear_comp_env();
    unsafe { std::env::set_var("COMP_LINE", "myapp 'ab c") };
    unsafe { std::env::set_var("COMP_POINT", "12") };

    // quoted whitespace stays within the word: value is "ab c"
    let output = carapace_clap::complete_output(
        || {
            let cmd = Command::new("myapp").arg(Arg::new("target").index(1));
            cmd.positional_completion([Action::values(["ab c1", "other"])])
        },
        Shell::Bash,
        &["'ab c".to_string()],
    );
    unsafe { std::env::remove_var("COMP_LINE") };
    unsafe { std::env::remove_var("COMP_POINT") };

    assert!(output.contains("ab c1"), "{output:?}");
}
