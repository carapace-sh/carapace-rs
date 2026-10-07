use carapace_core::{Action, Context};

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
    let model = carapace_spec::from_yaml(EXAMPLE).unwrap();
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
    assert_eq!(two.nargs, carapace_core::Nargs::Exact(2));

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
    let model = carapace_spec::from_yaml(EXAMPLE).unwrap();
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
    let model = carapace_spec::from_yaml(
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

    let model = carapace_spec::from_yaml("name: cmd\nparsing: non-interspersed\n").unwrap();
    assert!(!model.disable_flag_parsing);
    assert!(!model.interspersed);
}

#[test]
fn dash_completion() {
    let model = carapace_spec::from_yaml(
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
    let completion = carapace_core::traverse(&model, vec!["--".into(), "".into()]);
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
    assert!(carapace_spec::from_yaml("name: [unclosed").is_err());
}

#[test]
fn unknown_macro_is_a_message() {
    let action = Action::values(["a"]);
    let _ = action;
    let model = carapace_spec::from_yaml(
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
    let model = carapace_spec::from_yaml(EXAMPLE).unwrap();
    let yaml = carapace_spec::to_yaml(&model);
    assert!(yaml.starts_with("# yaml-language-server:"));

    let reparsed = carapace_spec::from_yaml(&yaml).unwrap();
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

    let two = reparsed
        .flags
        .iter()
        .find(|flag| flag.name == "two")
        .unwrap();
    assert!(matches!(two.nargs, carapace_core::model::Nargs::Exact(2)));
}
