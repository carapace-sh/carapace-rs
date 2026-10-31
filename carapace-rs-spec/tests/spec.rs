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
            .to_export()
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
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert_eq!(values, vec!["first", "second"]);
    assert_eq!(invoked.to_export().values[0].description, "described");

    let second = model.positionals[1].completion.as_ref().unwrap();
    assert!(second.is_callback(), "$files is a callback action");

    let any = model.positional_any.as_ref().unwrap();
    let invoked = any.invoke(&Context::default().dir("/nonexistent"));
    // $files on a nonexistent dir resolves to an error message
    assert!(
        !invoked.to_export().meta.messages.is_empty() || !invoked.to_export().values.is_empty()
    );
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
        .to_export()
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["--alpha", "-a"]);
}

#[test]
fn invalid_yaml_is_an_error() {
    assert!(carapace_rs_spec::from_yaml("name: [unclosed").is_err());
}

#[test]
fn unknown_fields_are_ignored() {
    // carapace-bin spec dialects carry fields (conditions, directives,
    // optionalargs) the core schema doesn't know; Go's yaml.Unmarshal
    // ignores them and so must the Rust parser
    let model = carapace_rs_spec::from_yaml(
        r#"
name: cmd
conditions:
  - {name: cond, values: [a, b]}
directives: ["option"]
optionalargs: [1]
completion:
  positionalany: ["$message(ignored)"]
"#,
    )
    .unwrap();
    assert_eq!(model.name, "cmd");
}

#[test]
fn dummy_macros_complete_nothing() {
    // Go core.go registers the modifier names as dummies: bare usage
    // completes nothing instead of erroring
    for macro_name in [
        "chdir",
        "list",
        "multiparts",
        "nospace",
        "noprefix",
        "uniquelist",
    ] {
        let model = carapace_rs_spec::from_yaml(&format!(
            r#"
name: cmd
completion:
  positionalany: ["${macro_name}"]
"#
        ))
        .unwrap();
        let invoked = model.positional_any.unwrap().invoke(&Context::default());
        assert!(invoked.to_export().values.is_empty(), "{macro_name}");
        assert!(invoked.to_export().meta.messages.is_empty(), "{macro_name}");
    }
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
    assert!(!invoked.to_export().meta.messages.is_empty());
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
        .to_export()
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
        .to_export()
        .values
        .iter()
        .find(|v| v.value == "grouped")
        .cloned()
        .expect("grouped candidate");
    assert_eq!(grouped.tag, "admin commands");
    assert_eq!(grouped.style, "blue");
    let plain = invoked
        .to_export()
        .values
        .iter()
        .find(|v| v.value == "plain")
        .cloned()
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

#[test]
fn modifiers_apply_to_entries_and_batch() {
    let yaml = r#"
name: mycmd
completion:
  positional:
    - ["first ||| $suffix(x)", "second", "dropped"]
  positionalany: ["$files([.rs]) ||| $nospace(/)"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();

    // entry-level filter via ` ||| `
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    assert!(values.contains(&"firstx".to_owned()), "{values:?}");
    assert!(values.contains(&"second".to_owned()));
    assert!(values.contains(&"dropped".to_owned()));
    // the files macro keeps its nospace flag through the modifier chain
    let any = model.positional_any.as_ref().unwrap();
    let invoked = any.invoke(&Context::default().value("src/"));
    assert!(
        invoked.to_export().meta.nospace.contains('/'),
        "{:?}",
        invoked.to_export().meta
    );
}

#[test]
fn entries_envsubst_with_c_exports() {
    // Go `action.Parse` exports C_ARG<n>, C_VALUE and C_FLAG_<NAME> on
    // the context before substituting the entries
    let yaml = r#"
name: mycmd
completion:
  positionalany: ["arg=${C_ARG0:-unset} value=${C_VALUE}", "${C_ARG0:-default}${C_VALUE:-none}"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();

    // completing the second positional: arg0 and value are exported
    let completion = carapace_rs::traverse(&model, vec!["first".to_owned(), "".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    assert!(
        values.contains(&"arg=first value=".to_owned()),
        "{values:?}"
    );
    // C_ARG0 substitutes, the empty C_VALUE takes its default
    assert!(values.contains(&"firstnone".to_owned()), "{values:?}");

    // flag exports: set flags appear uppercase, repeatables joined with ,
    let yaml = r#"
name: mycmd
flags:
  -f, --file=: input file
  --tag*=: repeatable
completion:
  flag:
    file: ["file=${C_FLAG_FILE:-unset}"]
    tag: ["tag=${C_FLAG_FILE:-unset}${C_FLAG_TAG:-none}"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let completion = carapace_rs::traverse(
        &model,
        vec![
            "--file=dev".to_owned(),
            "--tag=a".to_owned(),
            "--tag=b".to_owned(),
            "--file=".to_owned(),
        ],
    );
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    // flag-value candidates are completed with the flag prefix reattached
    assert!(values.contains(&"--file=file=dev".to_owned()), "{values:?}");

    let completion = carapace_rs::traverse(
        &model,
        vec![
            "--file=dev".to_owned(),
            "--tag=a".to_owned(),
            "--tag=b".to_owned(),
            "--tag=".to_owned(),
        ],
    );
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    assert!(
        values.contains(&"--tag=tag=deva,b".to_owned()),
        "{values:?}"
    );

    // unset flag falls back to the default
    let completion = carapace_rs::traverse(&model, vec!["--file=".to_owned()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    assert!(
        values.contains(&"--file=file=unset".to_owned()),
        "{values:?}"
    );
}

#[test]
fn envsubst_escape_and_error_degrade_to_message() {
    let yaml = r#"
name: mycmd
completion:
  positionalany: ["pre${!C_VALUE}", "${}"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|value| value.value.clone())
        .collect();
    // the escaped form passes through verbatim
    assert!(values.contains(&"pre${C_VALUE}".to_owned()), "{values:?}");
    // a bad substitution degrades the entry to a message (Go semantics)
    assert!(
        invoked
            .to_export()
            .meta
            .messages
            .iter()
            .any(|message| message.contains("bad substitution")),
        "{:?}",
        invoked.to_export().meta.messages
    );
}

#[test]
fn message_macro_surfaces_a_message() {
    let yaml = r#"
name: mycmd
completion:
  positionalany: ["$message(oops: something went wrong)"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    assert!(
        invoked.to_export().values.is_empty(),
        "{:?}",
        invoked.to_export().values
    );
    assert!(
        invoked
            .to_export()
            .meta
            .messages
            .iter()
            .any(|m| m.contains("oops: something went wrong")),
        "{:?}",
        invoked.to_export().meta.messages
    );
}

#[test]
fn shell_macro_completes_command_output() {
    // `$bash(...)` runs the command with the positional arguments appended
    let yaml = "name: mycmd\ncompletion:\n  positionalany: [\"$bash(printf 'first\tfirst-description\\n')\"]\n";
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![("first".to_owned(), "first-description".to_owned())]
    );
}

#[test]
fn spec_macro_executes_a_nested_spec() {
    let dir = std::env::temp_dir().join(format!("carapace-specmacro-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let nested = dir.join("nested.yaml");
    std::fs::write(
        &nested,
        "name: inner\ncompletion:\n  positionalany: [\"nested-value\"]\n",
    )
    .unwrap();

    let yaml = format!(
        r#"
name: mycmd
completion:
  positionalany: ["$spec({})"]
"#,
        nested.to_string_lossy()
    );
    let model = carapace_rs_spec::from_yaml(&yaml).unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    let values: Vec<String> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| v.value.clone())
        .collect();
    assert!(
        values.contains(&"nested-value".to_owned()),
        "{values:?} messages={:?}",
        invoked.to_export().meta.messages
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn external_macro_dispatches_to_binary() {
    // `$binary.macro(...)` executes `<binary> _carapace macro <macro>
    // <args...> <value>` and imports the export JSON (Go `ActionMacro`)
    let yaml = r#"
name: mycmd
completion:
  positionalany: ["$fake.bin.tools.echo(msg)"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let mock = carapace_rs::mock::Mock {
        dir: std::env::temp_dir(),
        replies: std::collections::HashMap::from([(
            carapace_rs::mock::Mock::key(
                "fake",
                &[
                    "_carapace".to_owned(),
                    "macro".to_owned(),
                    "bin.tools.echo(msg)".to_owned(),
                    String::new(),
                ],
            ),
            r#"{"version":"v0.7.0","messages":[],"noprefix":"","nospace":"","usage":"","values":[{"value":"remote-value","display":"remote-value","description":"from remote"}]}"#
                .to_owned(),
        )]),
    };
    let completion =
        carapace_rs::traverse_seeded(&model, vec!["".to_owned()], Context::default().mock(mock));
    let invoked = completion.action.invoke(&completion.context);
    let described: Vec<(String, String)> = invoked
        .to_export()
        .values
        .iter()
        .map(|v| (v.value.clone(), v.description.clone()))
        .collect();
    assert_eq!(
        described,
        vec![("remote-value".to_owned(), "from remote".to_owned())],
        "messages={:?}",
        invoked.to_export().meta.messages
    );
}

#[test]
fn deprecated_underscore_macro_is_a_message() {
    let yaml = r#"
name: mycmd
completion:
  positionalany: ["$_files"]
"#;
    let model = carapace_rs_spec::from_yaml(yaml).unwrap();
    let completion = carapace_rs::traverse(&model, vec!["".to_owned()]);
    let invoked = completion.action.invoke(&completion.context);
    assert!(
        invoked
            .to_export()
            .meta
            .messages
            .iter()
            .any(|m| m.contains("deprecated") && m.contains("$carapace.files")),
        "{:?}",
        invoked.to_export().meta.messages
    );
}

#[test]
fn yaml_dump_uses_go_flag_notation() {
    // Go `FlagSet.MarshalYAML`: a plain description string unless the flag
    // carries a non-default nargs; the flag shape lives in the key only
    let model = carapace_rs_spec::from_yaml(EXAMPLE).unwrap();
    let dumped = carapace_rs_spec::to_yaml(&model);
    assert!(dumped.contains("  -v, --verbose: be verbose\n"), "{dumped}");
    assert!(dumped.contains("  -f, --file=: input file\n"), "{dumped}");
    assert!(
        dumped.contains("  -o, --output?: optional output\n"),
        "{dumped}"
    );
    assert!(
        dumped.contains("  --two=:\n    description: two args\n    nargs: 2\n"),
        "{dumped}"
    );
    assert!(!dumped.contains("longhand"), "{dumped}");

    // and it round-trips
    let reparsed = carapace_rs_spec::from_yaml(&dumped).unwrap();
    assert!(reparsed.lookup_long("verbose").unwrap().takes_value == false);
    assert!(matches!(
        reparsed.lookup_long("two").unwrap().nargs,
        carapace_rs::Nargs::Exact(2)
    ));
}

#[test]
fn yaml_dump_key_suffix_order_matches_pflagfork() {
    use carapace_rs::model::{Flag, Nargs};
    let flag = Flag {
        id: "x".to_owned(),
        name: "x".to_owned(),
        shorthand: Some('x'),
        takes_value: true,
        nargs: Nargs::Exact(1),
        repeatable: true,
        required: true,
        hidden: true,
        ..Default::default()
    };
    let mut model = carapace_rs::model::Command::new("cmd");
    model.flags.push(flag);
    // Go pflagfork `Definition`: hidden, required, repeatable, then value
    let dumped = carapace_rs_spec::to_yaml(&model);
    let line = dumped.lines().find(|line| line.contains("--x")).unwrap();
    assert_eq!(line, format!("  -x, --x{}{}{}{}: ''", '&', '!', '*', '='));
}

#[test]
fn yaml_dump_shorthand_only_flag() {
    use carapace_rs::model::{Flag, Nargs};
    let flag = Flag {
        id: "toggle".to_owned(),
        name: "toggle".to_owned(),
        shorthand: Some('t'),
        shorthand_only: true,
        nargs: Nargs::Exact(0),
        ..Default::default()
    };
    let mut model = carapace_rs::model::Command::new("cmd");
    model.flags.push(flag);
    // Go pflagfork `ShorthandOnly`: only the shorthand is dumped, the
    // model's `name` is the framework arg id
    assert_eq!(
        carapace_rs_spec::to_yaml(&model),
        "# yaml-language-server: $schema=https://carapace.sh/schemas/command.json\n\
         name: cmd\nflags:\n  -t: ''\n"
    );
}

#[test]
fn spec_flag_ids_match_the_yaml_key_without_normalization() {
    let model = carapace_rs_spec::from_yaml(
        "name: ids\nflags:\n  --work-tree=: work tree\n  -n: shorthand only\n",
    )
    .expect("spec");

    // Go spec parity: a Go spec's flags have one name space, the YAML
    // key's bare name; no spelling normalization may appear (e.g.
    // kebab-to-snake) or callbacks keying on the spec spelling stop
    // seeing the flag
    let flag = model.lookup_long("work-tree").expect("long");
    assert_eq!(flag.id, "work-tree");
    assert_eq!(flag.name, "work-tree");

    // shorthand-only flags key on their shorthand string
    assert_eq!(model.lookup_id("n").expect("shorthand id").id, "n");
    assert!(model.lookup_long("n").is_none());
}
