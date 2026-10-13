use carapace_rs::Action;
use carapace_rs_bridge as bridge;
use carapace_rs_clap::Carapace;
use carapace_rs_testkit::Sandbox;
use clap::builder::ArgAction;
use clap::{Arg, Command};

/// A mock root unique per call, so parallel tests never share a cache dir.
fn mock_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "carapace-mock-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("cache")).ok();
    dir
}

/// Serializes tests that touch process environment variables.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Create a fake executable on the PATH so `look_path` finds it; the
/// command itself is answered by a mock reply.
fn fake_on_path(name: &str) -> (std::path::PathBuf, std::sync::MutexGuard<'static, ()>) {
    let guard = env_lock();
    let dir = std::env::temp_dir().join(format!(
        "carapace-fakebin-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join(name);
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let previous = std::env::var("PATH").unwrap_or_default();
    unsafe {
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(dir.clone()).chain(std::env::split_paths(&previous)),
            )
            .unwrap(),
        );
    }
    (dir, guard)
}

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
        || cli().positional_any_completion(bridge::carapace(["git"])),
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
fn fish_bridge_injects_custom_config() {
    // the fish bridge delegates to the real `fish` binary; a fake on the
    // PATH records the invocation and answers like fish would
    let (path_dir, _guard) = fake_on_path("fish");
    let record = std::env::temp_dir().join(format!("carapace-fish-record-{}", std::process::id()));
    std::fs::write(
        path_dir.join("fish"),
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > {}\nprintf 'one\\ttwo-description\\n'\n",
            record.to_string_lossy()
        ),
    )
    .unwrap();

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
        || cli().positional_any_completion(bridge::fish::action(["bridgefish"])),
        &[""],
    );
    std::fs::remove_dir_all(&dir).ok();
    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    std::fs::remove_file(&record).ok();

    // the fish snippet sources the bridge config and completes the command
    assert!(recorded.contains("source "), "{recorded:?}");
    assert!(recorded.contains("config.fish"), "{recorded:?}");
    assert!(recorded.contains("--do-complete"), "{recorded:?}");

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
                cli().positional_any_completion(bridge::clap([script.as_str()]))
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
        || cli().positional_any_completion(bridge::clap([script2.as_str()])),
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
fn zsh_bridge_injects_custom_config() {
    // the zsh bridge delegates to the real `zsh` binary; a fake on the
    // PATH records the invocation and answers like the capture script
    let (path_dir, _guard) = fake_on_path("zsh");
    let record = std::env::temp_dir().join(format!("carapace-zsh-record-{}", std::process::id()));
    std::fs::write(
        path_dir.join("zsh"),
        format!(
            "#!/bin/sh\nshift 3\nprintf '%s\\n' \"$*\" >> {}\nprintf '%s\\n' \"$CARAPACE_BRIDGE_CONFIG_HOME\" >> {}\nprintf 'one -- first value\\r\\n'\n",
            record.to_string_lossy(),
            record.to_string_lossy()
        ),
    )
    .unwrap();

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
        || cli().positional_any_completion(bridge::zsh::action(["bridgezsh"])),
        &[""],
    );
    std::fs::remove_dir_all(&dir).ok();
    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    std::fs::remove_file(&record).ok();

    // the capture invocation passes the command and the config home
    assert!(recorded.contains("bridgezsh"), "{recorded:?}");
    assert!(
        recorded.contains(dir.to_string_lossy().as_ref()),
        "{recorded:?}"
    );

    let values = run.values();
    assert_eq!(values, vec!["one"]);
    assert!(run.output().contains("first value"));
}

#[test]
fn bridge_dispatch_rejects_unknown() {
    let action = bridge::action("nope", ["x"]);
    let invoked = action.invoke(&carapace_rs::Context::default());
    assert!(!invoked.to_export().meta.messages.is_empty());
}

#[test]
fn cobra_bridge_parses_values_and_descriptions() {
    let mut sandbox = Sandbox::new("bridge-cobra-values");
    sandbox
        .reply("git", &["__complete", ""])
        .with("commit\trecord changes\nbranch\tlist branches\n:0\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
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
                .flag_completion([("branch", bridge::cobra::action(["git"]))])
        },
        &["--branch="],
    );
    assert_eq!(run.values(), vec!["--branch=main".to_string()]);
}

#[test]
fn cobra_bridge_prefixes_bare_flag_values() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("cobra-import"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("git", &["__complete".to_owned(), "--branch=".to_owned()]),
            "main\n:0\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::cobra::action(["git"]).invoke(&carapace_rs::Context::new(["--branch="]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["--branch=main".to_string()]);
}

#[test]
fn cobra_bridge_honors_nospace_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-nospace");
    sandbox.reply("git", &["__complete", ""]).with("dir/\n:2\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
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
        || cli().positional_any_completion(bridge::inshellisense::action(["bridgeish"])),
        &[""],
    );
    assert_eq!(run.values(), vec!["one".to_string()]);
}

#[test]
fn inshellisense_bridge_includes_flags_for_flag_words() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("cobra-nospace"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "inshellisense",
                &["complete".to_owned(), "bridgeish -".to_owned()],
            ),
            r#"{"suggestions":[
                {"name":"one","allNames":["one"],"description":"first","icon":""},
                {"name":"--flag","allNames":["--flag","-f"],"description":"a flag","icon":"\ud83d\udd17"}
            ]}"#
            .to_owned(),
        )]),
    };
    let invoked = bridge::inshellisense::action(["bridgeish"])
        .invoke(&carapace_rs::Context::new(["-"]).mock(mock));

    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(values.contains(&"--flag".to_string()), "{values:?}");
    assert!(values.contains(&"-f".to_string()), "{values:?}");
    assert!(!values.contains(&"one".to_string()), "{values:?}");
}

#[test]
fn cobra_bridge_error_directive() {
    let mut sandbox = Sandbox::new("bridge-cobra-error");
    sandbox.reply("git", &["__complete", ""]).with(":1\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
        &[""],
    );
    let output = run.output();
    assert!(output.contains("values\":[]"), "{}", output);
}

#[test]
fn cobra_bridge_truncated_output_yields_no_candidates() {
    // output without a directive line: no candidates, no message
    // (Go's readDirective error path returns ActionValues())
    let mut sandbox = Sandbox::new("bridge-cobra-truncated");
    sandbox.reply("git", &["__complete", ""]).with("value\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
        &[""],
    );
    assert!(run.output().contains("values\":[]"), "{}", run.output());
}

#[test]
fn cobra_bridge_nofilecomp_directive_skips_file_fallback() {
    let mut sandbox = Sandbox::new("bridge-cobra-nofilecomp");
    sandbox.reply("git", &["__complete", ""]).with(":4\n");

    let run = sandbox.run(
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
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
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
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
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
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
        || cli().positional_any_completion(bridge::cobra::action(["git"])),
        &[""],
    );
    let output = run.output();
    assert!(
        output.contains("\"one\"") && output.contains("\"two\""),
        "{}",
        output
    );
}

#[test]
fn inshellisense_bridge_styles_files() {
    // Go `StyleF(style.ForPathExt).Tag("files")`: file suggestions are
    // styled by extension from LS_COLORS
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("ins-values"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "inshellisense",
                &["complete".to_owned(), "bridgeish  ".to_owned()],
            ),
            r#"{"suggestions":[
                {"name":"img.png","allNames":["img.png"],"description":"a file","icon":"\ud83d\udcc0"}
            ]}"#
            .to_owned(),
        )]),
    };
    let invoked = bridge::inshellisense::action(["bridgeish"]).invoke(
        &carapace_rs::Context::new([""])
            .env("LS_COLORS", "*.png=01;35")
            .mock(mock),
    );

    let styled: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.style.clone()))
        .collect();
    assert_eq!(
        styled,
        vec![("img.png".to_owned(), "magenta bold".to_owned())]
    );
}

#[test]
fn carapace_bin_bridge_imports_export_json() {
    // Go bridge.ActionCarapaceBin: `carapace <cmd> export "" <args...>`
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("ins-flags"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "carapace",
                &[
                    "git".to_owned(),
                    "export".to_owned(),
                    String::new(),
                    String::new(),
                ],
            ),
            r#"{"version":"v0.7.0","messages":[],"noprefix":"","nospace":"","usage":"","values":[{"value":"commit","display":"commit","description":"record changes"}]}"#
                .to_owned(),
        )]),
    };
    let invoked = bridge::carapace_bin(["git"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["commit".to_owned()]);
}

#[test]
fn carapace_bin_bridge_falls_back_to_files_on_empty_output() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("carapace-bin-import"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "carapace",
                &[
                    "git".to_owned(),
                    "export".to_owned(),
                    String::new(),
                    String::new(),
                ],
            ),
            String::new(),
        )]),
    };
    let invoked = bridge::carapace_bin(["git"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    // the fallback completes real files
    assert!(
        !invoked.to_export().values.is_empty()
            && invoked.to_export().values.iter().all(|v| v.tag == "files"),
        "{:?}",
        invoked.to_export().values
    );
}

#[test]
fn click_bridge_parses_zsh_protocol() {
    let (_path_dir, _guard) = fake_on_path("fakeclick");

    // click emits type/value/description triples
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("click"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("fakeclick", &[]),
            "plain\none\nfirst\nplain\ntwo\n_\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::click::action(["fakeclick"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    // `_` descriptions are empty
    assert_eq!(
        described,
        vec![
            ("one".to_owned(), "first".to_owned()),
            ("two".to_owned(), String::new()),
        ]
    );
}

#[test]
fn complete_bridge_reads_comp_line_output() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("complete"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("fakecomplete", &[]),
            "one\nhttp://x\n\n".to_owned(),
        )]),
    };
    let invoked = bridge::complete::action(["fakecomplete"])
        .invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["http://x".to_owned(), "one".to_owned()]);
    // a nospace character inside a candidate suppresses the trailing space
    assert!(
        !invoked.to_export().meta.nospace.is_empty(),
        "{:?}",
        invoked.to_export().meta
    );
}

#[test]
fn yargs_bridge_parses_colon_descriptions() {
    let (_path_dir, _guard) = fake_on_path("fakeyargs");

    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("yargs"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakeyargs",
                &["--get-yargs-completions".to_owned(), String::new()],
            ),
            "one:first\ntwo\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::yargs::action(["fakeyargs"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![
            ("one".to_owned(), "first".to_owned()),
            ("two".to_owned(), String::new()),
        ]
    );
}

#[test]
fn jj_bridge_filters_by_flag_kind() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("jj"),
        replies: std::collections::HashMap::from([
            (
                carapace_rs::mock::Mock::key(
                    "fakejj",
                    &["--".to_owned(), "fakejj".to_owned(), "-".to_owned()],
                ),
                "--edit\tuse an editor\nrevision\tA revision\n".to_owned(),
            ),
            (
                carapace_rs::mock::Mock::key(
                    "fakejj",
                    &["--".to_owned(), "fakejj".to_owned(), String::new()],
                ),
                "--edit\tuse an editor\nrevision\tA revision\n".to_owned(),
            ),
        ]),
    };
    // completing a flag word keeps only flags
    let invoked =
        bridge::jj::action(["fakejj"]).invoke(&carapace_rs::Context::new(["-"]).mock(mock.clone()));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["--edit".to_owned()]);

    // completing a plain word keeps only non-flags
    let invoked =
        bridge::jj::action(["fakejj"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["revision".to_owned()]);
}

#[test]
fn kingpin_bridge_falls_back_to_files() {
    // a single (empty) output line without a flag word: files
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("kingpin-files"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakekingpin",
                &["--completion-bash".to_owned(), String::new()],
            ),
            "".to_owned(),
        )]),
    };
    let invoked = bridge::kingpin::action(["fakekingpin"])
        .invoke(&carapace_rs::Context::new([""]).mock(mock));
    assert!(
        !invoked.to_export().values.is_empty()
            && invoked.to_export().values.iter().all(|v| v.tag == "files"),
        "{:?}",
        invoked.to_export().values
    );

    // candidates are completed as plain values (Go keeps the empty
    // elements of the line split)
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("kingpin-values"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakekingpin",
                &["--completion-bash".to_owned(), String::new()],
            ),
            "one\ntwo\n\n".to_owned(),
        )]),
    };
    let invoked = bridge::kingpin::action(["fakekingpin"])
        .invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(
        values,
        vec![
            String::new(),
            String::new(),
            "one".to_owned(),
            "two".to_owned()
        ]
    );
}

#[test]
fn bpaf_bridge_reverses_fish_output() {
    let (_path_dir, _guard) = fake_on_path("fakebpaf");

    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("bpaf"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakebpaf",
                &["--bpaf-complete-rev=9".to_owned(), String::new()],
            ),
            "second\tlast\nfirst\tfirst-description\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::bpaf::action(["fakebpaf"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![
            ("first".to_owned(), "first-description".to_owned()),
            ("second".to_owned(), "last".to_owned()),
        ]
    );
}

#[test]
fn typer_bridge_reads_typer_complete_args() {
    let (path_dir, _guard) = fake_on_path("faketyper");
    let _ = path_dir;
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("typer"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("faketyper", &[]),
            "one\tfirst-description\n\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::typer::action(["faketyper"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![("one".to_owned(), "first-description".to_owned())]
    );
}

#[test]
fn urfavecli_bridge_parses_shell_completions() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("urfavecli"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakecli",
                &[String::new(), "--generate-shell-completion".to_owned()],
            ),
            "one:first\n\n".to_owned(),
        )]),
    };
    let invoked =
        bridge::urfavecli::action(["fakecli"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(described, vec![("one".to_owned(), "first".to_owned())]);
    assert!(
        !invoked.to_export().meta.nospace.is_empty(),
        "{:?}",
        invoked.to_export().meta
    );
}

#[test]
fn urfavecli_v1_bridge_reads_bash_completions() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("urfavecli-v1"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakecli",
                &[String::new(), "--generate-bash-completion".to_owned()],
            ),
            "one\ntwo\n\n".to_owned(),
        )]),
    };
    let invoked = bridge::urfavecli::urfavecli_v1(["fakecli"])
        .invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    // the split keeps one empty element (Go `lines[:len(lines)-1]`)
    assert_eq!(
        values,
        vec![String::new(), "one".to_owned(), "two".to_owned()]
    );
}

#[test]
fn symfony_bridge_parses_tab_descriptions() {
    let (path_dir, _guard) = fake_on_path("fakesymfony");
    let _ = path_dir;
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("symfony"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fakesymfony",
                &[
                    "_complete".to_owned(),
                    "--no-interaction".to_owned(),
                    "-s".to_owned(),
                    "zsh".to_owned(),
                    "-c".to_owned(),
                    "1".to_owned(),
                    "-ifakesymfony".to_owned(),
                ],
            ),
            "one\tfirst-description\n".to_owned(),
        )]),
    };
    let invoked = bridge::symfony::action(["fakesymfony"])
        .invoke(&carapace_rs::Context::new([""]).mock(mock));
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![("one".to_owned(), "first-description".to_owned())]
    );
}

#[test]
fn aws_bridge_reads_completer_output() {
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("aws"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("aws_completer", &[]),
            "ec2 \ns3 \n\n".to_owned(),
        )]),
    };
    let invoked = bridge::aws::action(["aws"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    // the v1 space suffix is stripped; the split keeps one empty element
    assert_eq!(
        values,
        vec![String::new(), "ec2".to_owned(), "s3".to_owned()]
    );
}

#[test]
fn kitten_bridge_maps_match_groups() {
    // the request JSON goes to stdin; the mock answers by command
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("kitten"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("kitten", &["__complete__".to_owned(), "json".to_owned()]),
            r#"[{"groups":[
                {"title":"Options","no_trailing_space":true,"is_files":false,"matches":[
                    {"word":"--flag","description":"a longhand"},
                    {"word":"-f","description":"a shorthand"}]},
                {"title":"Sub-commands","no_trailing_space":false,"is_files":false,"matches":[
                    {"word":"run","description":"run it"}]}
            ]}]"#
                .to_owned(),
        )]),
    };
    let invoked =
        bridge::kitten::action(["kitty"]).invoke(&carapace_rs::Context::new([""]).mock(mock));
    let tagged: Vec<(String, String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone(), v.tag.clone()))
        .collect();
    assert_eq!(
        tagged,
        vec![
            (
                "--flag".to_owned(),
                "a longhand".to_owned(),
                "longhand flags".to_owned()
            ),
            (
                "-f".to_owned(),
                "a shorthand".to_owned(),
                "shorthand flags".to_owned()
            ),
            ("run".to_owned(), "run it".to_owned(), "commands".to_owned()),
        ]
    );
    // the options group carries the nospace flag
    assert!(
        !invoked.to_export().meta.nospace.is_empty(),
        "{:?}",
        invoked.to_export().meta
    );
}

#[test]
fn gcloud_bridge_expands_dash_and_sets_update_check_env() {
    let (_dir, _guard) = fake_on_path("gcloud");
    // the v1 protocol runs through `sh -c` with fd redirections
    let script = "gcloud 8>&1 9>&2 1>/dev/null 2>/dev/null";
    let mock = carapace_rs::mock::Mock {
        dir: mock_dir("gcloud"),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key("sh", &["-c".to_owned(), script.to_owned()]),
            "components \n\n".to_owned(),
        )]),
    };
    let context = carapace_rs::Context::new([""]).mock(mock);
    let invoked = bridge::gcloud::action(["gcloud"]).invoke(&context);
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    // the v1 protocol keeps the output verbatim (no space stripping)
    assert_eq!(values, vec!["components ".to_owned()]);
}
