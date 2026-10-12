use carapace_rs::{Action, Command, Flag, Positional, traverse};

fn app() -> Command {
    Command::new("app")
        .flag(Flag {
            id: "verbose".into(),
            name: "verbose".into(),
            shorthand: Some('v'),
            ..Default::default()
        })
        .flag(Flag {
            id: "output".into(),
            name: "output".into(),
            shorthand: Some('o'),
            takes_value: true,
            completion: Some(Action::values(["file1", "file2"])),
            ..Default::default()
        })
        .flag(Flag {
            id: "include".into(),
            name: "include".into(),
            takes_value: true,
            nargs: carapace_rs::Nargs::AtLeastOne,
            completion: Some(Action::values(["inc1", "inc2"])),
            ..Default::default()
        })
        .flag(Flag {
            id: "define".into(),
            name: "define".into(),
            takes_value: true,
            optional: true,
            completion: Some(Action::values(["key1", "key2"])),
            ..Default::default()
        })
        .positional(Positional {
            name: "target".into(),
            completion: Some(Action::values(["t1", "t2"])),
            ..Default::default()
        })
        .positional(Positional {
            name: "rest".into(),
            repeatable: true,
            completion: Some(Action::values(["r1", "r2"])),
            ..Default::default()
        })
        .subcommand(
            Command::new("build")
                .description("build things")
                .flag(Flag {
                    id: "jobs".into(),
                    name: "jobs".into(),
                    takes_value: true,
                    completion: Some(Action::values(["1", "2"])),
                    ..Default::default()
                })
                .positional(Positional {
                    name: "artifact".into(),
                    completion: Some(Action::values(["bin", "lib"])),
                    ..Default::default()
                }),
        )
        .subcommand(Command::new("test").description("run tests"))
}

fn complete(args: &[&str]) -> Vec<String> {
    let completion = traverse(&app(), args.iter().map(|s| s.to_string()).collect());
    let mut values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    values.sort();
    values
}

#[test]
fn completes_positionals() {
    assert_eq!(complete(&[]), vec!["build", "t1", "t2", "test"]);
    assert_eq!(complete(&["t1", ""]), vec!["r1", "r2"]);
    assert_eq!(complete(&["t1", "r1", ""]), vec!["r1", "r2"]);
}

#[test]
fn completes_flag_names() {
    // `--` excludes shorthand candidates (they do not carry the prefix,
    // Go `actionFlags` wraps the batch in `MultiParts(".")`)
    assert_eq!(
        complete(&["--"]),
        vec!["--define", "--include", "--output", "--verbose"]
    );
    assert_eq!(
        complete(&["-"]),
        vec!["--define", "--include", "--output", "--verbose", "-o", "-v"]
    );
}

#[test]
fn completes_flag_value_after_separate_arg() {
    // `--output ` completes the flag's value
    assert_eq!(complete(&["--output", ""]), vec!["file1", "file2"]);
    // shorthand
    assert_eq!(complete(&["-o", ""]), vec!["file1", "file2"]);
}

#[test]
fn completes_attached_flag_value() {
    assert_eq!(
        complete(&["--output="]),
        vec!["--output=file1", "--output=file2"]
    );
    assert_eq!(complete(&["-o="]), vec!["-o=file1", "-o=file2"]);
    assert_eq!(complete(&["-ofile"]), vec!["-ofile1", "-ofile2"]);
}

#[test]
fn completes_optional_flag_value() {
    // optarg with explicit delimiter
    assert_eq!(
        complete(&["--define="]),
        vec!["--define=key1", "--define=key2"]
    );
    // optarg never consumes the next positional
    assert_eq!(
        complete(&["--define", ""]),
        vec!["build", "t1", "t2", "test"]
    );
}

#[test]
fn completes_bool_flag_value_with_delimiter() {
    assert_eq!(
        complete(&["--verbose="]),
        vec!["--verbose=false", "--verbose=true"]
    );
}

#[test]
fn flag_consumes_next_arg() {
    // after --output file1, no positionals were consumed yet: positional 0 + commands
    assert_eq!(
        complete(&["--output", "file1", ""]),
        vec!["build", "t1", "t2", "test"]
    );
    // variadic flag keeps consuming
    assert_eq!(complete(&["--include", "inc1", ""]), vec!["inc1", "inc2"]);
}

#[test]
fn completes_after_subcommand() {
    assert_eq!(complete(&["build", ""]), vec!["bin", "lib"]);
    assert_eq!(complete(&["build", "--jobs", ""]), vec!["1", "2"]);
    // `-j` is a shorthand series whose member takes a required value: the
    // series aborts (Go `actionFlags` shorthand handling)
    assert_eq!(complete(&["build", "-j"]), Vec::<String>::new());
}

#[test]
fn dash_disables_flag_completion() {
    assert_eq!(complete(&["--", ""]), vec!["t1", "t2"]);
    assert_eq!(complete(&["t1", "--", ""]), vec!["t1", "t2"]);
    assert_eq!(complete(&["t1", "--", "x", ""]), vec!["r1", "r2"]);
    assert_eq!(complete(&["--", "t1", ""]), vec!["r1", "r2"]);
}

#[test]
fn shorthand_series_prefix() {
    // series of bool flags with partial shorthand: flag-name completion prefixed
    let completion = traverse(&app(), vec!["-v".to_string(), "-".to_string()]);
    let values = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect::<Vec<_>>();
    assert!(values.contains(&"-o".to_string()));
    assert!(values.contains(&"--output".to_string()));
}

#[test]
fn unknown_flags_are_reported_unless_lenient() {
    // pflag errors on unknown flags; `CARAPACE_LENIENT` falls back to
    // ignoring them (Go `env.Lenient`)
    let completion = traverse(&app(), vec!["--unknown".to_string(), "".to_string()]);
    let invoked = completion.action.invoke(&completion.context);
    assert_eq!(invoked.values, Vec::new());
    assert_eq!(
        invoked.meta.messages,
        vec!["unknown flag: --unknown".to_owned()]
    );

    let _guard = env_lock();
    unsafe { std::env::set_var("CARAPACE_LENIENT", "1") };
    assert_eq!(
        complete(&["--unknown", ""]),
        vec!["build", "t1", "t2", "test"]
    );
    unsafe { std::env::remove_var("CARAPACE_LENIENT") };
}

/// Serializes tests that touch `CARAPACE_*` environment variables.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn positional_any_fallback() {
    let cmd = Command::new("any")
        .positional(Positional {
            name: "only".into(),
            repeatable: true,
            ..Default::default()
        })
        .positional_any(Action::values(["any1"]));
    let completion = traverse(
        &cmd,
        vec!["a".to_string(), "b".into(), "c".into(), "".into()],
    );
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["any1"]);
}

#[test]
fn context_args_track_positionals() {
    let completion = traverse(&app(), vec!["t1".to_string(), "r1".into(), "".into()]);
    assert_eq!(completion.context.args, vec!["t1", "r1"]);
}

#[test]
fn subcommand_recursion_ignores_completed_value() {
    // "app build xyz<TAB>" completes the empty word inside `build`,
    // not the value "xyz" itself
    assert_eq!(complete(&["build", "xyz"]), vec!["bin", "lib"]);
    // partial word equal to the subcommand name completes commands/positionals
    assert_eq!(complete(&["build"]), vec!["build", "t1", "t2", "test"]);
}

#[test]
fn fixed_nargs_consume_flag_like_values() {
    // pflag consumes the next argument unconditionally for fixed nargs
    let app = app();
    let parsed = carapace_rs::traverse::parse::parse(
        &app,
        &["--output".to_string(), "--flaglike".to_string()],
    );
    assert_eq!(
        parsed.flags,
        vec![("output".to_string(), vec!["--flaglike".to_string()])]
    );
    assert!(parsed.positionals.is_empty());
}

#[test]
fn hyphen_values_are_consumed_as_flag_arguments() {
    let cmd = Command::new("app").flag(Flag {
        id: "grep".into(),
        name: "grep".into(),
        takes_value: true,
        nargs: carapace_rs::Nargs::Any,
        allow_hyphen_values: true,
        completion: Some(Action::values(["pattern1", "pattern2"])),
        ..Default::default()
    });

    // "-not-a-flag" is consumed as a value of --grep, completing its value
    let completion = traverse(
        &cmd,
        vec![
            "--grep".into(),
            "first".into(),
            "-not-a-flag".into(),
            "".into(),
        ],
    );
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|value| value.value)
        .collect();
    assert_eq!(values, vec!["pattern1", "pattern2"]);

    // without allow_hyphen_values, "-not-a-flag" ends variadic consumption
    // once a value was consumed (the first value is always taken)
    let cmd = Command::new("app").flag(Flag {
        id: "grep".into(),
        name: "grep".into(),
        takes_value: true,
        nargs: carapace_rs::Nargs::Any,
        completion: Some(Action::values(["pattern1", "pattern2"])),
        ..Default::default()
    });
    let completion = traverse(
        &cmd,
        vec![
            "--grep".into(),
            "first".into(),
            "-not-a-flag".into(),
            "".into(),
        ],
    );
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|value| value.value)
        .collect();
    assert!(values.is_empty(), "hyphen arg ends variadic consumption");
}

#[test]
fn completes_attached_flag_value_with_partial_value() {
    // the arg prefix is the flag name including the delimiter, so the
    // partially typed value is replaced by the completed values
    assert_eq!(
        complete(&["--output=fi"]),
        vec!["--output=file1", "--output=file2"]
    );
    assert_eq!(complete(&["-o=fi"]), vec!["-o=file1", "-o=file2"]);
}

#[test]
fn flag_names_carry_kind_styles() {
    // Go Flag.Style(): FlagNoArg, FlagArg, FlagMultiArg, FlagOptArg
    let completion = traverse(&app(), vec!["--".to_string()]);
    let mut values: Vec<(String, String)> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| (v.value, v.style))
        .collect();
    values.sort();

    let styles: std::collections::HashMap<String, String> = values.into_iter().collect();
    assert_eq!(styles["--verbose"], ""); // no argument (style.Default)
    assert_eq!(styles["--output"], "blue"); // single argument
    assert_eq!(styles["--include"], "magenta"); // multiple arguments
    assert_eq!(styles["--define"], "yellow"); // optional argument
}

#[test]
fn bool_flag_values_carry_keyword_styles() {
    // Go StyleF(style.ForKeyword): true is positive, false negative
    let completion = traverse(&app(), vec!["--verbose=".to_string()]);
    let mut values: Vec<(String, String)> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| (v.value, v.style))
        .collect();
    values.sort();

    let styles: std::collections::HashMap<String, String> = values.into_iter().collect();
    assert_eq!(styles["--verbose=false"], "red");
    assert_eq!(styles["--verbose=true"], "green");
}

#[test]
fn command_groups_style_and_tag() {
    use carapace_rs::model::Command as ModelCommand;
    let cmd = ModelCommand::new("app")
        .subcommand(
            ModelCommand::new("alpha")
                .description("first group")
                .alias("al"),
        )
        .subcommand(ModelCommand::new("beta").group("admin"))
        .subcommand(ModelCommand::new("gamma").group("admin"))
        .subcommand(ModelCommand::new("delta").group("ops"))
        .subcommand(ModelCommand::new("plain"));

    let completion = traverse(&cmd, vec!["".to_string()]);
    let invoked = completion.action.invoke(&completion.context);

    let mut entries: Vec<(String, String, String)> = invoked
        .values
        .into_iter()
        .map(|v| (v.value, v.tag, v.style))
        .collect();
    entries.sort();

    let by_value: std::collections::HashMap<String, (String, String)> =
        entries.into_iter().map(|(v, t, s)| (v, (t, s))).collect();

    // ungrouped commands in a grouped tree are "other commands", unstyled
    assert_eq!(
        by_value["plain"],
        ("other commands".to_owned(), String::new())
    );
    // the first group's position maps to Highlight1
    assert_eq!(
        by_value["beta"],
        ("admin commands".to_owned(), "blue".to_owned())
    );
    assert_eq!(
        by_value["gamma"],
        ("admin commands".to_owned(), "blue".to_owned())
    );
    // the second group maps to Highlight2
    assert_eq!(
        by_value["delta"],
        ("ops commands".to_owned(), "yellow".to_owned())
    );
    // aliases carry the same group
    assert_eq!(by_value["al"], ("other commands".to_owned(), String::new()));
}

#[test]
fn ungrouped_tree_tags_plain_commands() {
    let completion = traverse(&app(), vec!["".to_string()]);
    let invoked = completion.action.invoke(&completion.context);
    // command candidates use the plain tag; positional candidates keep theirs
    let commands: Vec<_> = invoked
        .values
        .iter()
        .filter(|v| v.value == "build" || v.value == "test")
        .collect();
    assert!(
        commands
            .iter()
            .all(|v| v.tag == "commands" && v.style.is_empty()),
        "ungrouped tree uses the plain commands tag"
    );
}

#[test]
fn implicit_uids_on_flags_and_subcommands() {
    use carapace_rs::model::Command as ModelCommand;
    let cmd = ModelCommand::new("app")
        .flag(Flag {
            id: "verbose".into(),
            name: "verbose".into(),
            shorthand: Some('v'),
            ..Default::default()
        })
        .subcommand(ModelCommand::new("sub").alias("s").subcommand(
            ModelCommand::new("deep").flag(Flag {
                id: "force".into(),
                name: "force".into(),
                ..Default::default()
            }),
        ));

    // flag candidates carry cmd://app?flag=<name>
    let completion = traverse(&cmd, vec!["-".to_string()]);
    let invoked = completion.action.invoke(&completion.context);
    let uid_of = |value: &str| {
        invoked
            .values
            .iter()
            .find(|v| v.value == value)
            .unwrap()
            .uid
            .clone()
            .unwrap()
    };
    assert_eq!(uid_of("--verbose"), "cmd://app?flag=verbose");
    assert_eq!(uid_of("-v"), "cmd://app?flag=verbose");

    // subcommand candidates carry cmd://app/<name>; aliases resolve to the
    // actual name (Go's cmd.Find in the UidF closure)
    let completion = traverse(&cmd, vec!["".to_string()]);
    let invoked = completion.action.invoke(&completion.context);
    let uid_of = |value: &str| {
        invoked
            .values
            .iter()
            .find(|v| v.value == value)
            .unwrap()
            .uid
            .clone()
            .unwrap()
    };
    assert_eq!(uid_of("sub"), "cmd://app/sub");
    assert_eq!(uid_of("s"), "cmd://app/sub");

    // deeper nesting accumulates the path
    let completion = traverse(&cmd, vec!["sub".to_string(), "".to_string()]);
    let invoked = completion.action.invoke(&completion.context);
    let deep = invoked
        .values
        .iter()
        .find(|v| v.value == "deep")
        .unwrap()
        .uid
        .clone()
        .unwrap();
    assert_eq!(deep, "cmd://app/sub/deep");

    // flag uids inside a subcommand use the nested path
    let completion = traverse(
        &cmd,
        vec!["sub".to_string(), "deep".to_string(), "-".to_string()],
    );
    let invoked = completion.action.invoke(&completion.context);
    let force = invoked
        .values
        .iter()
        .find(|v| v.value == "--force")
        .unwrap()
        .uid
        .clone()
        .unwrap();
    assert_eq!(force, "cmd://app/sub/deep?flag=force");
}

#[test]
fn probe_interspersed_parse() {
    let cmd = Command::new("app")
        .flag(Flag {
            id: "bool".into(),
            name: "bool".into(),
            shorthand: Some('b'),
            ..Default::default()
        })
        .positional(Positional {
            name: "pos1".into(),
            completion: Some(Action::values(["p1"])),
            ..Default::default()
        })
        .positional(Positional {
            name: "pos2".into(),
            completion: Some(Action::values(["p2"])),
            ..Default::default()
        })
        .interspersed(false);
    let parsed = carapace_rs::traverse::parse::parse(
        &cmd,
        &["--bool".to_string(), "p1".to_string(), "--".to_string()],
    );
    println!(
        "PROBE positionals={:?} dash={}",
        parsed.positionals, parsed.dash
    );
}

#[test]
fn probe_prefix_modifier() {
    let a = Action::values(["p1"]).prefix("--persistentFlag=");
    let invoked = a.invoke(&carapace_rs::Context::default().value("--persistentFlag="));
    println!(
        "PROBE prefix out={:?}",
        invoked
            .values
            .iter()
            .map(|v| v.value.clone())
            .collect::<Vec<_>>()
    );
    let b = Action::values(["p1"]);
    let wrapped = Action::callback(move |c| {
        let invoked = b.invoke(c);
        let mut action = invoked.to_action();
        if action.meta.usage.as_deref().is_none_or(str::is_empty) {
            action.meta.usage = Some("usage".to_owned());
        }
        action
    })
    .prefix("--persistentFlag=");
    let invoked2 = wrapped.invoke(&carapace_rs::Context::default().value("--persistentFlag="));
    println!(
        "PROBE wrapped out={:?}",
        invoked2
            .values
            .iter()
            .map(|v| v.value.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn probe_shlex_redirect() {
    for input in ["pos1>", "pos1>subdir/", "pos1>subdir/file1.txt --b"] {
        let c = carapace_rs_shlex::complete(input, carapace_rs::Format::Bash);
        println!(
            "PROBE input={input:?} words={:?} current={:?} redirect={}",
            c.words, c.current_word, c.is_redirect
        );
    }
}

#[test]
fn probe_for_path_colors() {
    let context = carapace_rs::Context::default();
    for p in ["dirA/", "dirC/", "file5.go", "missing.txt"] {
        println!(
            "PROBE for_path {p:?} = {:?}",
            carapace_rs::shell::for_path(&context, p)
        );
    }
}
