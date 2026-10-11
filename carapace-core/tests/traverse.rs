use carapace_core::{Action, Command, Flag, Positional, traverse};

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
            nargs: carapace_core::Nargs::AtLeastOne,
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
    assert_eq!(
        complete(&["--"]),
        vec!["--define", "--include", "--output", "--verbose", "-o", "-v"]
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
    assert_eq!(complete(&["build", "-j"]), vec!["--jobs"]);
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
fn unknown_flags_are_ignored() {
    assert_eq!(
        complete(&["--unknown", ""]),
        vec!["build", "t1", "t2", "test"]
    );
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
    let parsed = carapace_core::traverse::parse::parse(
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
        nargs: carapace_core::Nargs::Any,
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
        nargs: carapace_core::Nargs::Any,
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
    assert_eq!(styles["-v"], "");
    assert_eq!(styles["-o"], "blue");
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
