use carapace_bridge as bridge;
use carapace_testkit::Sandbox;
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
fn carapace_bridge_completes_git() {
    let mut sandbox = Sandbox::new("bridge-carapace");
    sandbox
        .reply("git", &["_carapace", "export", "", ""])
        .with(
            r#"{"version":"0.1.0","Meta":{"Messages":[],"Nospace":[]},"Values":[{"Value":"commit","Display":"commit","Description":"record changes"}]}"#,
        );
    let run = sandbox.run(
        || {
            let cmd = cli();
            carapace_clap::Gen::new(cmd)
                .positional_any_completion(bridge::carapace(&["git"]))
                .into_command()
        },
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

    let sandbox = Sandbox::new("bridge-fish");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
    let run = sandbox.run(
        || {
            let cmd = cli();
            carapace_clap::Gen::new(cmd)
                .positional_any_completion(bridge::fish::action(&["bridgefish"]))
                .into_command()
        },
        &[""],
    );
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    std::fs::remove_dir_all(&dir).ok();

    let values = run.values();
    assert_eq!(values, vec!["one"]);
    assert_eq!(run.output().contains("two-description"), true);
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
                let cmd = cli();
                carapace_clap::Gen::new(cmd)
                    .positional_any_completion(bridge::clap(&[script.as_str()]))
                    .into_command()
            },
            &[""],
        )
    };
    std::fs::remove_file(&script).ok();

    let values = run.values();
    assert!(values.contains(&"fast".to_string()), "{values:?}");
    assert!(values.contains(&"slow".to_string()), "{values:?}");
    assert!(
        !run.output().contains("\"Nospace\":[\"*\"]"),
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
        || {
            let cmd = cli();
            carapace_clap::Gen::new(cmd)
                .positional_any_completion(bridge::clap(&[script2.as_str()]))
                .into_command()
        },
        &[""],
    );
    std::fs::remove_file(&script2).ok();
    assert!(
        run.output().contains("\"Nospace\":[\"*\"]"),
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

    let sandbox = Sandbox::new("bridge-zsh");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
    let run = sandbox.run(
        || {
            let cmd = cli();
            carapace_clap::Gen::new(cmd)
                .positional_any_completion(bridge::zsh::action(&["bridgezsh"]))
                .into_command()
        },
        &[""],
    );
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    std::fs::remove_dir_all(&dir).ok();

    let values = run.values();
    assert_eq!(values, vec!["one"]);
    assert_eq!(run.output().contains("first value"), true);
}

#[test]
fn bridge_dispatch_rejects_unknown() {
    let action = bridge::action("nope", &["x"]);
    let invoked = action.invoke(&carapace_core::Context::default());
    assert!(!invoked.meta.messages.is_empty());
}
