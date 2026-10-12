use carapace_rs::{Action, Context};

const EXAMPLE: &str = r#"
name: mycmd
aliases: [myc]
description: My command description
parsing: interspersed
flags:
  -v, --verbose: be verbose
  -f, --file=: input file
  --two=: {description: two args, nargs: 2}
  -o, --output?: optional output
  --hidden&: hidden flag
  --required!: required flag
  --tag*: repeatable
commands:
  - name: sub
    description: subcommand
    flags:
      -x: exclusive flag
    completion:
      positional:
        - ["$directories"]
      positionalany: ["$files"]
completion:
  flag:
    file: ["$files"]
    verbose: ["true", "false"]
  positional:
    - ["first\tdescribed", "second"]
    - ["$files([.go, .mod])"]
  positionalany: ["$files"]
"#;

#[test]
fn parses_spec_structure() {
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    assert_eq!(model.name, "mycmd");
    assert!(model.aliases.contains(&"myc".to_string()));
    assert_eq!(model.description, "My command description");
    assert!(model.interspersed);
    assert!(!model.disable_flag_parsing);

    let verbose = model.lookup_long("verbose").unwrap();
    assert_eq!(verbose.shorthand, Some('v'));
    assert!(!verbose.takes_value);

    let file = model.lookup_long("file").unwrap();
    assert!(file.takes_value);
    assert_eq!(
        file.completion
            .as_ref()
            .unwrap()
            .invoke(&Context::default())
            .values
            .len(),
        0
    );

    let two = model.lookup_long("two").unwrap();
    assert_eq!(two.nargs, carapace_rs::Nargs::Exact(2));

    let output = model.lookup_long("output").unwrap();
    assert!(output.optional);
    assert_eq!(output.shorthand, Some('o'));

    assert!(model.lookup_long("hidden").unwrap().hidden);
    assert!(model.lookup_long("required").unwrap().required);
    assert!(model.lookup_long("tag").unwrap().repeatable);

    let sub = model.lookup_subcommand("sub").unwrap();
    assert!(sub.lookup_short('x').is_some());
}

#[test]
fn resolves_macros() {
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    let first = model.positionals[0].completion.as_ref().unwrap();
    let invoked = first.invoke(&Context::default());
    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert_eq!(values, vec!["first", "second"]);
    assert_eq!(invoked.values[0].description, "described");

    let second = model.positionals[1].completion.as_ref().unwrap();
    assert!(second.is_callback(), "$files is a callback action");

    let any = model.positional_any.as_ref().unwrap();
    let invoked = any.invoke(&Context::default().dir("/nonexistent"));
    // $files on a nonexistent dir resolves to an error message
    assert!(!invoked.meta.messages.is_empty() || !invoked.values.is_empty());
}

#[test]
fn parsing_modes() {
    let model = carapace_rs_spec::from_yaml(
        r#"
name: cmd
parsing: disabled
flags:
  --flag=: value
"#,
    )
    .unwrap();
    assert!(model.disable_flag_parsing);
    assert!(!model.interspersed);

    let model = carapace_rs_spec::from_yaml("name: cmd\nparsing: non-interspersed\n").unwrap();
    assert!(!model.disable_flag_parsing);
    assert!(!model.interspersed);
}

#[test]
fn dash_completion() {
    let model = carapace_rs_spec::from_yaml(
        r#"
name: cmd
completion:
  dash:
    - ["-a", "--alpha"]
  dashany: ["-x", "--xxx"]
"#,
    )
    .unwrap();
    assert_eq!(model.dash_positionals.len(), 1);
    let completion = carapace_rs::traverse(&model, vec!["--".into(), "".into()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["-a", "--alpha"]);
}

#[test]
fn invalid_yaml_is_an_error() {
    assert!(carapace_rs_spec::from_yaml("name: [unclosed").is_err());
}

#[test]
fn unknown_macro_is_a_message() {
    let action = Action::values(["a"]);
    let _ = action;
    let model = carapace_rs_spec::from_yaml(
        r#"
name: cmd
completion:
  positionalany: ["$carapace.tools.git.Refs({tags: true})"]
"#,
    )
    .unwrap();
    let invoked = model.positional_any.unwrap().invoke(&Context::default());
    assert!(!invoked.meta.messages.is_empty());
}

#[test]
fn yaml_round_trip() {
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    let yaml = carapace_rs_spec::to_yaml(&model);
    assert!(yaml.starts_with("# yaml-language-server:"));

    let reparsed = carapace_rs_spec::from_yaml(&yaml).unwrap();
    assert_eq!(reparsed.name, "mycmd");
    assert_eq!(reparsed.aliases, vec!["myc".to_string()]);
    assert_eq!(reparsed.subcommands.len(), 1);
    assert_eq!(reparsed.subcommands[0].name, "sub");

    let verbose = reparsed
        .flags
        .iter()
        .find(|flag| flag.name == "verbose")
        .unwrap();
    assert_eq!(verbose.shorthand, Some('v'));
    assert!(!verbose.takes_value);

    let file = reparsed
        .flags
        .iter()
        .find(|flag| flag.name == "file")
        .unwrap();
    assert!(file.takes_value);
    assert!(!file.optional);

    let output = reparsed
        .flags
        .iter()
        .find(|flag| flag.name == "output")
        .unwrap();
    assert!(output.optional);
    // `?` marks an optarg: the flag takes a value (Go `parseFlag`:
    // Value = Optarg || "=" in modifier)
    assert!(output.takes_value);

    let two = reparsed
        .flags
        .iter()
        .find(|flag| flag.name == "two")
        .unwrap();
    assert!(matches!(two.nargs, carapace_rs::model::Nargs::Exact(2)));
}

#[test]
fn executables_macro() {
    let dir = std::env::temp_dir().join(format!("carapace-spec-exe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("runnable"), "content").unwrap();
    std::fs::write(dir.join("plain"), "content").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.join("runnable"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        std::fs::set_permissions(dir.join("plain"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
    }

    let model = carapace_rs_spec::from_yaml(
        r#"
name: cmd
completion:
  positional:
    - ["$executables"]
"#,
    )
    .unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".into()]);
    let context = completion
        .context
        .env("PATH", dir.to_string_lossy().into_owned());
    let values: Vec<String> = completion
        .action
        .invoke(&context)
        .values
        .into_iter()
        .map(|value| value.value)
        .collect();

    assert_eq!(values, vec!["runnable"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn group_round_trip() {
    let yaml = r#"
name: mycmd
commands:
  - name: grouped
    group: admin
    description: grouped command
  - name: plain
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    assert_eq!(model.subcommands[0].group.as_deref(), Some("admin"));
    assert_eq!(model.subcommands[1].group, None);

    // the group drives tag and highlight style in completion
    let completion = carapace_rs::traverse(&model, vec![String::new()]);
    let invoked = completion.action.invoke(&completion.context);
    let grouped = invoked
        .values
        .iter()
        .find(|v| v.value == "grouped")
        .expect("grouped candidate");
    assert_eq!(grouped.tag, "admin commands");
    assert_eq!(grouped.style, "blue");
    let plain = invoked
        .values
        .iter()
        .find(|v| v.value == "plain")
        .expect("plain candidate");
    assert_eq!(plain.tag, "other commands");

    // and it survives the YAML dump
    let dumped = carapace_rs_spec::to_yaml(&model);
    assert!(dumped.contains("group: admin"), "{dumped}");
}

#[test]
fn optarg_flag_binds_delimited_value() {
    // `--output?` takes a value through `=` but never consumes the next
    // positional (Go's NoOptDefVal semantics)
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    let completion = carapace_rs::traverse(
        &model,
        vec![
            "--output=dev".to_owned(),
            "--file=".to_owned(),
            "".to_owned(),
        ],
    );
    assert_eq!(
        completion.context.parsed_flags[0],
        ("output".to_owned(), vec!["dev".to_owned()])
    );
}

#[test]
fn persistent_flags_round_trip_and_inherit() {
    let yaml = r#"
name: mycmd
flags:
  --local=: local flag
persistentflags:
  -g, --global=: global flag
commands:
  - name: sub
    flags:
      --sub=: sub flag
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();

    // persistent flags materialize on subcommands (cobra inheritance)
    let sub = model.lookup_subcommand("sub").unwrap();
    let global = sub.lookup_long("global").unwrap();
    assert!(global.persistent);
    assert!(global.takes_value);
    assert_eq!(global.shorthand, Some('g'));

    // the dump recovers the section split
    let dumped = carapace_rs_spec::to_yaml(&model);
    let reparsed = carapace_rs_spec::from_yaml(&dumped).unwrap();
    assert!(reparsed.lookup_long("global").unwrap().persistent);
    assert!(!reparsed.lookup_long("local").unwrap().persistent);
    let sub_dumped = reparsed.lookup_subcommand("sub").unwrap();
    assert!(sub_dumped.lookup_long("global").unwrap().persistent);
    assert!(sub_dumped.lookup_long("sub").is_some());
    let yaml_out = carapace_rs_spec::to_yaml(sub_dumped);
    assert!(yaml_out.contains("persistentflags:"), "{yaml_out}");
    assert!(yaml_out.contains("-g, --global"), "{yaml_out}");
}

#[test]
fn hidden_flags_survive_round_trip() {
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    let dumped = carapace_rs_spec::to_yaml(&model);
    assert!(dumped.contains("--hidden&"), "{dumped}");
    let reparsed = carapace_rs_spec::from_yaml(&dumped).unwrap();
    let hidden = reparsed.lookup_long("hidden").unwrap();
    assert!(hidden.hidden);
}

#[test]
fn flag_aliases_round_trip() {
    let yaml = r#"
name: mycmd
flags:
  -v, --verbose, --loud: be verbose
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    assert!(model.lookup_long("loud").is_some());

    let reparsed = carapace_rs_spec::from_yaml(&carapace_rs_spec::to_yaml(&model)).unwrap();
    let verbose = reparsed.lookup_long("verbose").unwrap();
    assert!(verbose.aliases.contains(&"loud".to_owned()));
    assert!(reparsed.lookup_long("loud").is_some());
}

#[test]
fn unsupported_flag_key_is_an_error() {
    // Go's parseFlag errors instead of silently dropping the flag
    let error = carapace_rs_spec::from_yaml("name: mycmd\nflags:\n  -abc: series\n").unwrap_err();
    assert!(error.contains("long shorthand"), "{error}");
}
