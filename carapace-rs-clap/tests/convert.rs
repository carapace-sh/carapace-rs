use carapace_rs::{Action, Context, Nargs};
use carapace_rs_clap::{Carapace, to_model};
use clap::builder::{ArgAction, ValueHint};
use clap::{Arg, Command};

/// Serializes tests that touch CARAPACE_* environment variables.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn converts_flags() {
    let mut cmd = Command::new("app")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("verbose output"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .action(ArgAction::Set)
                .required(true),
        )
        .arg(
            Arg::new("define")
                .long("define")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .require_equals(true),
        )
        .arg(Arg::new("count").long("count").action(ArgAction::Count))
        .arg(
            Arg::new("hidden")
                .long("hidden")
                .hide(true)
                .action(ArgAction::SetTrue),
        );

    let model = to_model(&mut cmd);
    assert_eq!(model.name, "app");

    let verbose = model.lookup_long("verbose").unwrap();
    assert_eq!(verbose.shorthand, Some('v'));
    assert!(!verbose.takes_value);
    assert_eq!(verbose.description, "verbose output");

    let output = model.lookup_long("output").unwrap();
    assert!(output.takes_value);
    assert!(output.required);
    assert_eq!(output.nargs, Nargs::Exact(1));
    assert_eq!(output.shorthand, Some('o'));

    let define = model.lookup_long("define").unwrap();
    assert!(define.takes_value);
    assert!(define.optional);
    assert!(!define.argument_style.accepts_next);
    assert!(define.argument_style.accepts_delimited);

    let count = model.lookup_long("count").unwrap();
    assert!(!count.takes_value);
    assert!(count.repeatable);

    let hidden = model.lookup_long("hidden").unwrap();
    assert!(hidden.hidden);
}

#[test]
fn converts_variadic_flags() {
    let mut cmd = Command::new("app").arg(
        Arg::new("include")
            .long("include")
            .action(ArgAction::Append)
            .num_args(1..),
    );
    let model = to_model(&mut cmd);
    let include = model.lookup_long("include").unwrap();
    assert_eq!(include.nargs, Nargs::AtLeastOne);
    assert!(include.repeatable);
}

#[test]
fn converts_positionals() {
    let mut cmd = Command::new("app")
        .arg(Arg::new("first").help("the first").index(1))
        .arg(
            Arg::new("rest")
                .num_args(1..)
                .index(2)
                .trailing_var_arg(true),
        );

    let model = to_model(&mut cmd);
    assert_eq!(model.positionals.len(), 2);
    assert_eq!(model.positionals[0].name, "first");
    assert_eq!(model.positionals[0].description, "the first");
    assert!(model.positionals[1].repeatable);
    assert!(!model.interspersed);
}

#[test]
fn converts_subcommands_and_aliases() {
    let mut cmd = Command::new("app")
        .alias("ap")
        .subcommand(Command::new("build").visible_alias("b"));

    let model = to_model(&mut cmd);
    assert!(model.aliases.contains(&"ap".to_string()));
    let build = model.lookup_subcommand("b").unwrap();
    assert_eq!(build.name, "build");
    assert!(model.lookup_subcommand("build").is_some());
}

#[test]
fn possible_values_become_fallback_completion() {
    let mut cmd = Command::new("app").arg(
        Arg::new("color")
            .long("color")
            .action(ArgAction::Set)
            .value_parser(clap::builder::PossibleValuesParser::new(["red", "green"])),
    );
    let model = to_model(&mut cmd);
    let color = model.lookup_long("color").unwrap();
    let action = color.completion.as_ref().unwrap();
    let invoked = action.invoke(&Context::default());
    let values: Vec<String> = invoked.values.iter().map(|v| v.value.clone()).collect();
    assert_eq!(values, vec!["red", "green"]);
}

#[test]
fn value_hint_maps_to_file_completion() {
    let mut cmd = Command::new("app")
        .arg(
            Arg::new("file")
                .long("file")
                .value_hint(ValueHint::FilePath)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("dir")
                .long("dir")
                .value_hint(ValueHint::DirPath)
                .action(ArgAction::Set),
        );

    let model = to_model(&mut cmd);
    assert!(model.lookup_long("file").unwrap().completion.is_some());
    assert!(model.lookup_long("dir").unwrap().completion.is_some());
}

#[test]
fn carapace_attaches_completions() {
    let cmd = Command::new("app")
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .arg(Arg::new("target").index(1))
        .arg(Arg::new("rest").num_args(0..).index(2));

    let cmd = cmd
        .flag_completion([("profile", Action::values(["dev", "prod"]))])
        .positional_completion([Action::values(["x86", "arm"])])
        .positional_any_completion(Action::values(["anything"]));

    let model = to_model(&mut cmd.clone());
    let profile = model.lookup_long("profile").unwrap();
    let invoked = profile
        .completion
        .as_ref()
        .unwrap()
        .invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "dev");

    let target = model.positionals[0].completion.as_ref().unwrap();
    assert_eq!(target.invoke(&Context::default()).values[0].value, "x86");

    assert!(model.positional_any.is_some());
}

#[test]
fn positional_completion_maps_by_argument_index() {
    // Go completes the Nth argument: a variadic positional spans all
    // remaining indices, so each provided action completes one argument
    let cmd = Command::new("app")
        .arg(Arg::new("target").index(1))
        .arg(Arg::new("rest").num_args(0..).index(2));

    let cmd = cmd.positional_completion([
        Action::values(["first"]),
        Action::values(["second"]),
        Action::values(["third"]),
    ]);

    let model = to_model(&mut cmd.clone());
    let value_at = |index: usize| {
        model.positionals[index]
            .completion
            .as_ref()
            .unwrap()
            .invoke(&Context::default())
            .values[0]
            .value
            .clone()
    };
    assert_eq!(value_at(0), "first");
    assert_eq!(value_at(1), "second");
    assert_eq!(value_at(2), "third");
    // beyond the provided actions the variadic tail leaves the slot to
    // positional_any
    let tail = model.positionals.last().unwrap();
    assert!(tail.repeatable);
    assert!(tail.completion.is_none());
}

#[test]
fn carapace_attaches_to_subcommands() {
    let cmd = Command::new("app").subcommand(
        Command::new("build").arg(Arg::new("jobs").long("jobs").action(ArgAction::Set)),
    );

    let configured = cmd.mut_subcommand("build", |build| {
        build.flag_completion([("jobs", Action::values(["1", "2"]))])
    });

    let model = to_model(&mut configured.clone());
    let build = model.lookup_subcommand("build").unwrap();
    let jobs = build.lookup_long("jobs").unwrap();
    assert!(jobs.completion.is_some());
}

#[test]
fn hidden_subcommands_are_excluded() {
    let _guard = env_lock();
    let mut cmd = Command::new("app")
        .subcommand(Command::new("visible").about("shown"))
        .subcommand(Command::new("secret").hide(true).about("hidden"));

    let model = to_model(&mut cmd);
    let secret = model.lookup_subcommand("secret").unwrap();
    assert!(secret.hidden);

    let values: Vec<String> =
        carapace_rs::traverse::action_commands(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default())
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
    assert_eq!(values, vec!["visible", "help"]);

    unsafe { std::env::set_var("CARAPACE_HIDDEN", "2") };
    let values: Vec<String> =
        carapace_rs::traverse::action_commands(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default())
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
    unsafe { std::env::remove_var("CARAPACE_HIDDEN") };
    assert!(values.contains(&"secret".to_string()));
}

#[test]
fn hidden_flags_respect_hidden_env() {
    let _guard = env_lock();
    let mut cmd = Command::new("app").arg(
        Arg::new("secret")
            .long("secret")
            .hide(true)
            .action(ArgAction::SetTrue),
    );
    let model = to_model(&mut cmd);

    unsafe { std::env::remove_var("CARAPACE_HIDDEN") };
    let values: Vec<String> =
        carapace_rs::traverse::action_flags(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default().value("--"))
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
    // shorthand candidates do not carry the `--` prefix (Go wraps the
    // flag batch in `MultiParts(".")`)
    assert_eq!(
        values,
        vec!["--help"],
        "hidden flag must be skipped: {values:?}"
    );

    unsafe { std::env::set_var("CARAPACE_HIDDEN", "2") };
    let values: Vec<String> =
        carapace_rs::traverse::action_flags(&model, std::slice::from_ref(&model.name))
            .invoke(&Context::default().value("--"))
            .values
            .into_iter()
            .map(|v| v.value)
            .collect();
    unsafe { std::env::remove_var("CARAPACE_HIDDEN") };
    assert!(values.contains(&"--secret".to_string()));
}

#[test]
fn help_subcommand_completes_commands() {
    let mut cmd = Command::new("app")
        .subcommand(Command::new("build").about("build things"))
        .subcommand(Command::new("test").about("run tests"));
    let model = to_model(&mut cmd);
    let help = model.lookup_subcommand("help").unwrap();
    let action = help
        .positional_any
        .as_ref()
        .expect("help completes commands");
    let values: Vec<String> = action
        .invoke(&Context::default())
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["build", "test"]);
}

#[test]
fn dash_any_completion() {
    let mut cmd = Command::new("app")
        .arg(Arg::new("target").index(1))
        .arg(Arg::new("rest").num_args(0..).index(2))
        .dash_any_completion(Action::values(["after-dash"]));

    let model = to_model(&mut cmd);
    assert!(model.dash_any.is_some());
    let completion = carapace_rs::traverse(&model, vec!["t".to_string(), "--".into(), "".into()]);
    let values: Vec<String> = completion
        .action
        .invoke(&completion.context)
        .values
        .into_iter()
        .map(|v| v.value)
        .collect();
    assert_eq!(values, vec!["after-dash"]);
}

#[test]
fn carapace_attaches_dash_completions() {
    let cmd = Command::new("app")
        .dash_completion([Action::values(["dash1"])])
        .dash_any_completion(Action::values(["dash-rest"]));

    let model = to_model(&mut cmd.clone());
    assert_eq!(model.dash_positionals.len(), 1);
    let invoked = model.dash_positionals[0]
        .completion
        .as_ref()
        .unwrap()
        .invoke(&Context::default());
    assert_eq!(invoked.values[0].value, "dash1");
    assert!(model.dash_any.is_some());
}

#[test]
fn dash_completion_replaces_list_and_maps_in_order() {
    // Go assignment semantics: a later call replaces the whole list; the
    // Nth action completes the Nth dash argument.
    let cmd = Command::new("app")
        .dash_completion([Action::values(["first"]), Action::values(["stale"])])
        .dash_completion([Action::values(["a"]), Action::values(["b"])]);

    let model = to_model(&mut cmd.clone());
    let values: Vec<String> = model
        .dash_positionals
        .iter()
        .map(|positional| {
            positional
                .completion
                .as_ref()
                .unwrap()
                .invoke(&Context::default())
                .values[0]
                .value
                .clone()
        })
        .collect();
    assert_eq!(values, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn positional_completion_fills_unset_positionals_in_order() {
    // The Nth action completes the Nth positional; a positional with its
    // own completion slot (e.g. from the macros) keeps it.
    let cmd = Command::new("app")
        .arg(Arg::new("first").index(1))
        .arg(Arg::new("second").index(2))
        .mut_arg("second", |arg| {
            arg.add(carapace_rs_clap::ArgCompleter::new(Action::values(["own"])))
        })
        .positional_completion([Action::values(["first-action"]), Action::values(["unused"])]);

    let model = to_model(&mut cmd.clone());
    let first = model.positionals[0]
        .completion
        .as_ref()
        .unwrap()
        .invoke(&Context::default());
    assert_eq!(first.values[0].value, "first-action");
    let second = model.positionals[1]
        .completion
        .as_ref()
        .unwrap()
        .invoke(&Context::default());
    assert_eq!(second.values[0].value, "own");
}

#[test]
fn flag_completion_merges_per_key_and_matches_names() {
    // Go ActionMap semantics: repeated calls merge per key (a repeated key
    // keeps its last action); keys may be long names, aliases or shorthands.
    let cmd = Command::new("app")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .alias("loud")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("profile").long("profile").action(ArgAction::Set))
        .flag_completion([("profile", Action::values(["dev"]))])
        .flag_completion([
            ("v", Action::values(["verbose-shorthand"])),
            ("loud", Action::values(["verbose-alias"])),
            ("profile", Action::values(["dev", "prod"])),
        ]);

    let model = to_model(&mut cmd.clone());
    let values = |flag: &carapace_rs::model::Flag| {
        flag.completion
            .as_ref()
            .unwrap()
            .invoke(&Context::default())
            .values
            .iter()
            .map(|value| value.value.clone())
            .collect::<Vec<_>>()
    };
    // shorthand and alias keys resolve to the same flag; the repeated
    // profile key keeps its last action
    assert_eq!(
        values(model.lookup_long("profile").unwrap()),
        vec!["dev", "prod"]
    );
    assert_eq!(
        values(model.lookup_short('v').unwrap()),
        vec!["verbose-alias"]
    );
}
