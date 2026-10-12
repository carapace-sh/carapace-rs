use carapace_rs::Action;
use carapace_rs_bridge as bridge;
use carapace_rs_clap::Carapace;
use carapace_rs_testkit::Sandbox;
use clap::builder::ArgAction;
use clap::{Arg, Command};

fn cli() -> Command {
    Command::new("outer").disable_help_subcommand(true).arg(
        Arg::new("rest")
            .num_args(0..)
            .index(1)
            .action(ArgAction::Append),
    )
}

#[test]
fn carapace_rs_bridge_completes_git() {
    let mut sandbox = Sandbox::new("bridge-carapace");
    sandbox
        .reply("git", &["_carapace", "export", "", ""])
        .with(
            // carapace-bin export shape: flattened lowercase meta, matcher
            // strings, lowercase value keys
            r#"{"version":"v0.7.0","messages":[],"noprefix":"","nospace":"","usage":"","values":[{"value":"commit","display":"commit","description":"record changes"}]}"#,
        );
    let run = sandbox.run(
        || cli().positional_any_completion(bridge::carapace(&["git"])),
        &[""],
    );
    let values = run.values();
    assert!(
        values.contains(&"commit".to_string()),
        "git subcommands expected, got: {:?}",
        values.iter().take(10).collect::<Vec<_>>()
    );
}

#[test]
fn fish_bridge_uses_custom_config() {
    let dir = std::env::temp_dir().join(format!("carapace-fish-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("carapace/bridge/fish")).unwrap();
    std::fs::write(
        dir.join("carapace/bridge/fish/config.fish"),
        "complete -c bridgefish -f -a 'one\\ttwo-description'\n",
    )
    .unwrap();

    let mut sandbox = Sandbox::new("bridge-fish");
    sandbox.env("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());
    let run = sandbox.run(
        || cli().positional_any_completion(bridge::fish::action(&["bridgefish"])),
        &[""],
    );
    std::fs::remove_dir_all(&dir).ok();

    let values = run.values();
    assert_eq!(values, vec!["one"]);
    assert!(run.output().contains("two-description"));
}

#[test]
fn clap_bridge_parses_lines_and_nospace() {
    let script = std::env::temp_dir().join(format!("carapace-clap-bridge-{}", std::process::id()));
    std::fs::write(
        &script,
        "#!/bin/sh\ncase \"$*\" in\n  *'--'*'dir/'*) echo \"dir/\" ;;\n  *) echo \"fast\"; echo \"slow\" ;;\nesac\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let script = script.to_string_lossy().into_owned();

    let run = {
        let sandbox = Sandbox::new("bridge-clap");
        sandbox.run(
            || {
                let script = script.clone();
                cli().positional_any_completion(bridge::clap(&[script.as_str()]))
            },
            &[""],
        )
    };
    std::fs::remove_file(&script).ok();

    let values = run.values();
    assert!(values.contains(&"fast".to_string()), "{values:?}");
    assert!(values.contains(&"slow".to_string()), "{values:?}");
    assert!(
        !run.output().contains("\"nospace\":\"*\""),
        "no /-suffix: no nospace"
    );

    let sandbox = Sandbox::new("bridge-clap-nospace");
    let script_clone = script.clone();
    let _ = script_clone;
    let script2 = {
        let path =
            std::env::temp_dir().join(format!("carapace-clap-bridge2-{}", std::process::id()));
        std::fs::write(&path, "#!/bin/sh\necho \"dir/x\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_string_lossy().into_owned()
    };
    let run = sandbox.run(
        || cli().positional_any_completion(bridge::clap(&[script2.as_str()])),
        &[""],
    );
    std::fs::remove_file(&script2).ok();
    assert!(
        run.output().contains("\"nospace\":\"*\""),
        "dir/x triggers nospace: {}",
        run.output()
    );
    assert!(
        run.values().contains(&"dir/x".to_string()),
        "{}",
        run.output()
    );
}

#[test]
fn zsh_bridge_uses_custom_config() {
    let dir = std::env::temp_dir().join(format!("carapace-zsh-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("carapace/bridge/zsh")).unwrap();
    std::fs::write(
        dir.join("carapace/bridge/zsh/.zshrc"),
        "compdef _bridgezsh bridgezsh\n_bridgezsh() { local -a d; d=('first value'); compadd -d d one }\n",
    )
    .unwrap();

    let mut sandbox = Sandbox::new("bridge-zsh");
    sandbox.env("XDG_CONFIG_HOME", dir.to_string_lossy().as_ref());
    let run = sandbox.run(
        || cli().positional_any_completion(bridge::zsh::action(&["bridgezsh"])),
        &[""],
    );
    std::fs::remove_dir_all(&dir).ok();

    let values = run.values();
    assert_eq!(values, vec!["one"]);
    assert!(run.output().contains("first value"));
}

#[test]
fn bridge_dispatch_rejects_unknown() {
    let action = bridge::action("nope", &["x"]);
    let invoked = action.invoke(&carapace_rs::Context::default());
    assert!(!invoked.meta.messages.is_empty());
}

#[test]
fn cobra_bridge_parses_values_and_descriptions() {
    let mut sandbox = Sandbox::new("bridge-cobra-values");
    sandbox
        .reply("git", &["__complete", ""])
        .with("commit\trecord changes\nbranch\tlist branches\n:0\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    run.expect(Action::values_described([
        ("branch", "list branches"),
        ("commit", "record changes"),
    ]));
}

#[test]
fn cobra_bridge_reattaches_flag_prefix() {
    let mut sandbox = Sandbox::new("bridge-cobra-prefix");
    sandbox.reply("git", &["__complete", ""]).with("main\n:0\n");

    let run = sandbox.run(
        || {
            Command::new("outer")
                .arg(Arg::new("branch").long("branch").action(ArgAction::Set))
                .flag_completion([("branch", bridge::cobra::action(&["git"]))])
        },
        &["--branch="],
    );
    assert_eq!(run.values(), vec!["--branch=main".to_string()]);
}

#[test]
fn cobra_bridge_prefixes_bare_flag_values() {
    let mock = carapace_rs::mock::Mock {
        dir: std::env::temp_dir(),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("git", &["__complete", "--branch="]),
            "main\n:0\n".to_owned(),
        )]),
    };
    let invoked = bridge::cobra::action(&["git"])
        .invoke(&carapace_rs::Context::new(["--branch="]).mock(mock));
    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert_eq!(values, vec!["--branch=main".to_string()]);
}

#[test]
fn cobra_bridge_honors_nospace_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-nospace");
    sandbox.reply("git", &["__complete", ""]).with("dir/\n:2\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    assert!(
        run.output().contains("\"nospace\":\"*\""),
        "{}",
        run.output()
    );
}

#[test]
fn inshellisense_bridge_skips_flags_for_plain_words() {
    let mut sandbox = Sandbox::new("bridge-inshell-values");
    sandbox
        .reply("inshellisense", &["complete", "bridgeish  "])
        .with(
            r#"{"suggestions":[
                {"name":"one","allNames":["one"],"description":"first","icon":""},
                {"name":"--flag","allNames":["--flag","-f"],"description":"a flag","icon":"\ud83d\udd17"}
            ]}"#,
        );

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::inshellisense::action(&["bridgeish"])),
        &[""],
    );
    assert_eq!(run.values(), vec!["one".to_string()]);
}

#[test]
fn inshellisense_bridge_includes_flags_for_flag_words() {
    let mock = carapace_rs::mock::Mock {
        dir: std::env::temp_dir(),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("inshellisense", &["complete", "bridgeish -"]),
            r#"{"suggestions":[
                {"name":"one","allNames":["one"],"description":"first","icon":""},
                {"name":"--flag","allNames":["--flag","-f"],"description":"a flag","icon":"\ud83d\udd17"}
            ]}"#
            .to_owned(),
        )]),
    };
    let invoked = bridge::inshellisense::action(&["bridgeish"])
        .invoke(&carapace_rs::Context::new(["-"]).mock(mock));

    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert!(values.contains(&"--flag".to_string()), "{values:?}");
    assert!(values.contains(&"-f".to_string()), "{values:?}");
    assert!(!values.contains(&"one".to_string()), "{values:?}");
}

#[test]
fn cobra_bridge_error_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-error");
    sandbox.reply("git", &["__complete", ""]).with(":1\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    let output = run.output();
    assert!(output.contains("values\":[]"), "{}", output);
}

#[test]
fn cobra_bridge_nofilecomp_directive_skips_file_fallback() {
    let mut sandbox = Sandbox::new("bridge-cobra-nofilecomp");
    sandbox.reply("git", &["__complete", ""]).with(":4\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    let output = run.output();
    // no values and no file fallback
    assert!(output.contains("values\":[]"), "{}", output);
}

#[test]
fn cobra_bridge_filterfileext_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-filterext");
    sandbox.files([("main.go", ""), ("README.md", ""), ("subdir/x", "")]);
    sandbox
        .reply("git", &["__complete", ""])
        .with("go\nmod\n:8\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    let output = run.output();
    assert!(output.contains("main.go"), "{}", output);
    assert!(output.contains("subdir/"), "{}", output);
    // `README.md` does not match the `.mod` extension filter
    assert!(!output.contains("README.md"), "{}", output);
}

#[test]
fn cobra_bridge_filterdirs_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-filterdirs");
    sandbox.files([("subdir/file1.txt", "")]);
    sandbox.reply("git", &["__complete", ""]).with(":16\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    let output = run.output();
    assert!(
        output.contains("subdir/") && !output.contains("file1.txt"),
        "{}",
        output
    );
}

#[test]
fn cobra_bridge_with_values_and_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-values-directive");
    sandbox
        .reply("git", &["__complete", ""])
        .with("one\ntwo\n:4\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(&["git"])),
        &[""],
    );
    let output = run.output();
    assert!(
        output.contains("\"one\"") && output.contains("\"two\""),
        "{}",
        output
    );
}
